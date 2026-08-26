use crate::crawl::dedupe::UrlDeduplicator;
use crate::crawl::filters::{evaluate_url_for_crawl, FilterDecision};
use crate::crawl::frontier::CrawlFrontier;
use crate::crawl::models::{CrawlOptions, CrawlTarget, CrawledPage, DiscoverySource, PageTimings};
use crate::crawl::progress::CrawlProgress;
use crate::crawl::retry::{calculate_retry_delay, is_retryable_status};
use crate::crawl::robots::RobotsManager;
use crate::crawl::scheduler::CrawlScheduler;
use crate::models::{OutputFormat, ScrapeRequest, ScrapeWarning, WarningCode};
use crate::service::ScraperService;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};
use url::Url;

/// Processes a single crawl target from the frontier: fetches, parses, updates stats, and enqueues newly discovered URLs.
#[allow(clippy::too_many_arguments)]
pub async fn process_crawl_target(
    target: CrawlTarget,
    seed_url: &Url,
    options: &CrawlOptions,
    scraper: &Arc<ScraperService>,
    _scheduler: &Arc<CrawlScheduler>,
    frontier: &Arc<CrawlFrontier>,
    deduplicator: &Arc<UrlDeduplicator>,
    robots: &Arc<RobotsManager>,
    progress: &Arc<CrawlProgress>,
    cancel: &CancellationToken,
) -> Option<CrawledPage> {
    if cancel.is_cancelled() {
        return None;
    }

    progress.update_depth(target.depth);

    // 1. Robots.txt check if enabled
    if options.respect_robots_txt {
        let robots_rules = robots.get_or_fetch(&target.url, scraper.fetcher()).await;
        let is_allowed =
            robots_rules.is_path_allowed(target.url.path(), &scraper.config().user_agent);

        if !is_allowed {
            debug!("URL disallowed by robots.txt: {}", target.url);
            progress.inc_blocked_by_robots();
            progress.inc_crawled();
            return Some(CrawledPage {
                url: target.url.to_string(),
                final_url: target.url.to_string(),
                depth: target.depth,
                status_code: 403,
                content_type: None,
                content_hash: None,
                markdown: None,
                text: None,
                html: None,
                clean_html: None,
                json: None,
                metadata: None,
                headings: None,
                links: None,
                images: None,
                tables: None,
                pages: None,
                timings: PageTimings {
                    fetch_ms: 0,
                    parse_ms: 0,
                    extract_ms: 0,
                    total_ms: 0,
                },
                renderer: None,
                render_reason: None,
                network_responses: None,
                warnings: vec![ScrapeWarning::new(
                    WarningCode::MissingMainContent,
                    "Disallowed by robots.txt",
                )],
                error: Some("Disallowed by robots.txt".to_string()),
            });
        }
    }

    // 2. Request delay (politeness per host)
    if options.request_delay_ms > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(options.request_delay_ms)).await;
    }

    if cancel.is_cancelled() {
        return None;
    }

    // 3. Build ScrapeRequest
    let mut requested_formats = options
        .formats
        .clone()
        .unwrap_or_else(|| vec![OutputFormat::Markdown, OutputFormat::Metadata]);

    // Always include Links format if we need to discover child pages
    if target.depth < options.max_depth && !requested_formats.contains(&OutputFormat::Links) {
        requested_formats.push(OutputFormat::Links);
    }

    let scrape_req = ScrapeRequest {
        url: target.url.to_string(),
        formats: Some(requested_formats.clone()),
        only_main_content: Some(options.only_main_content.unwrap_or(true)),
        render_mode: Some(options.render_mode),
        wait_for: options.wait_for.clone(),
        auto_scroll: Some(options.auto_scroll),
        capture_network: Some(options.capture_network),
        block_trackers: Some(options.block_trackers),
        block_resources: None,
        actions: None,
        timeout_ms: None,
        json_path: None,
        follow_feed_links: Some(options.follow_feed_links),
        follow_document_links: Some(options.follow_document_links),
    };

    // 4. Execute fetch & scrape with bounded retries
    let start_instant = Instant::now();
    let mut attempts = 0;
    let max_retries = options.max_retries;

    let scrape_result = loop {
        attempts += 1;
        match scraper.scrape(scrape_req.clone()).await {
            Ok(result) => {
                let status = result.http.status;
                if is_retryable_status(status) && attempts <= max_retries {
                    progress.inc_retries();
                    let delay = calculate_retry_delay(attempts, None, 500);
                    debug!(
                        "Retrying {} after {}ms (status {})",
                        target.url,
                        delay.as_millis(),
                        status
                    );
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => continue,
                        _ = cancel.cancelled() => return None,
                    }
                }
                break Ok(result);
            }
            Err(err) => {
                if attempts <= max_retries {
                    progress.inc_retries();
                    let delay = calculate_retry_delay(attempts, None, 500);
                    debug!(
                        "Retrying {} after {}ms due to error: {}",
                        target.url,
                        delay.as_millis(),
                        err
                    );
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => continue,
                        _ = cancel.cancelled() => return None,
                    }
                }
                break Err(err);
            }
        }
    };

    let total_elapsed = start_instant.elapsed().as_millis() as u64;

    match scrape_result {
        Ok(result) => {
            progress.inc_crawled();
            if result.http.status >= 200 && result.http.status < 400 {
                progress.inc_succeeded();
            } else {
                progress.inc_failed();
            }
            progress.add_bytes(result.http.content_length);

            if result.renderer.as_deref() == Some("browser") {
                progress.inc_browser_page(result.http.total_time_ms);
            } else if result.renderer.as_deref() == Some("document_parser") {
                progress.inc_document_processed();
            } else {
                progress.inc_http_page();
            }

            if result
                .warnings
                .iter()
                .any(|w| w.code == crate::models::WarningCode::BrowserFallbackUsed)
            {
                progress.inc_browser_fallback();
            }

            // 5. Discover new links if depth limit not reached and link discovery is enabled for this content type
            let is_html =
                result.content_type.as_deref() == Some("html") || result.content_type.is_none();
            let is_feed = result.content_type.as_deref() == Some("rss")
                || result.content_type.as_deref() == Some("atom");
            let should_follow =
                is_html || (is_feed && options.follow_feed_links) || options.follow_document_links;

            if target.depth < options.max_depth && should_follow {
                if let Some(ref links) = result.links {
                    progress.inc_discovered(links.len());
                    for page_link in links {
                        if let Ok(candidate_url) = Url::parse(&page_link.url) {
                            match evaluate_url_for_crawl(
                                &candidate_url,
                                target.depth + 1,
                                seed_url,
                                options,
                                deduplicator,
                                scraper.config().allow_private_networks,
                            ) {
                                FilterDecision::Allowed { normalized_url } => {
                                    let source = if is_feed {
                                        DiscoverySource::Feed
                                    } else if is_html {
                                        DiscoverySource::HtmlLink
                                    } else {
                                        DiscoverySource::Document
                                    };

                                    let new_target = CrawlTarget {
                                        url: candidate_url,
                                        normalized_url,
                                        depth: target.depth + 1,
                                        parent_url: Some(target.url.clone()),
                                        discovered_from: source,
                                    };
                                    if frontier.try_push(new_target).is_ok() {
                                        progress.inc_queued();
                                    }
                                }
                                FilterDecision::Disallowed(
                                    crate::crawl::filters::FilterReason::Duplicate,
                                ) => {
                                    progress.inc_dedupe_hits();
                                }
                                FilterDecision::Disallowed(_) => {
                                    progress.inc_skipped();
                                }
                            }
                        }
                    }
                }
            }

            // 6. Handle Canonical URL deduplication hint
            if options.respect_canonical {
                if let Some(ref meta) = result.metadata {
                    if let Some(ref canonical_str) = meta.canonical_url {
                        if let Ok(canonical_url) = Url::parse(canonical_str) {
                            if canonical_url != target.url && target.depth < options.max_depth {
                                if let FilterDecision::Allowed { normalized_url } =
                                    evaluate_url_for_crawl(
                                        &canonical_url,
                                        target.depth + 1,
                                        seed_url,
                                        options,
                                        deduplicator,
                                        scraper.config().allow_private_networks,
                                    )
                                {
                                    let new_target = CrawlTarget {
                                        url: canonical_url,
                                        normalized_url,
                                        depth: target.depth + 1,
                                        parent_url: Some(target.url.clone()),
                                        discovered_from: DiscoverySource::Canonical,
                                    };
                                    if frontier.try_push(new_target).is_ok() {
                                        progress.inc_queued();
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Some(CrawledPage {
                url: result.url,
                final_url: result.final_url,
                depth: target.depth,
                status_code: result.http.status,
                content_type: result.content_type,
                content_hash: result.content_hash,
                markdown: result.markdown,
                text: result.text,
                html: result.html,
                clean_html: result.clean_html,
                json: result.json,
                metadata: result.metadata,
                headings: result.headings,
                links: result.links,
                images: result.images,
                tables: result.tables,
                pages: result.pages,
                timings: PageTimings {
                    fetch_ms: result.http.fetch_time_ms,
                    parse_ms: result.http.parse_time_ms,
                    extract_ms: result.http.extract_time_ms,
                    total_ms: total_elapsed,
                },
                renderer: result.renderer,
                render_reason: result.render_reason,
                network_responses: result.network_responses,
                warnings: result.warnings,
                error: None,
            })
        }
        Err(err) => {
            warn!("Failed to crawl {}: {}", target.url, err);
            progress.inc_crawled();
            progress.inc_failed();

            Some(CrawledPage {
                url: target.url.to_string(),
                final_url: target.url.to_string(),
                depth: target.depth,
                status_code: err.status_code().as_u16(),
                content_type: None,
                content_hash: None,
                markdown: None,
                text: None,
                html: None,
                clean_html: None,
                json: None,
                metadata: None,
                headings: None,
                links: None,
                images: None,
                tables: None,
                pages: None,
                timings: PageTimings {
                    fetch_ms: total_elapsed,
                    parse_ms: 0,
                    extract_ms: 0,
                    total_ms: total_elapsed,
                },
                renderer: None,
                render_reason: None,
                network_responses: None,
                warnings: Vec::new(),
                error: Some(err.user_message()),
            })
        }
    }
}
