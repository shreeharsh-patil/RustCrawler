use crate::crawl::dedupe::UrlDeduplicator;
use crate::crawl::filters::{evaluate_url_for_crawl, FilterDecision};
use crate::crawl::frontier::CrawlFrontier;
use crate::crawl::models::{CrawlOptions, CrawlResult, CrawlStatus, CrawlTarget, DiscoverySource};
use crate::crawl::progress::CrawlProgress;
use crate::crawl::robots::RobotsManager;
use crate::crawl::scheduler::CrawlScheduler;
use crate::crawl::sitemap::discover_sitemap_urls;
use crate::crawl::worker::process_crawl_target;
use crate::error::CrawlerError;
use crate::service::ScraperService;
use crate::utils::urls::validate_url_syntax;
use chrono::Utc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use uuid::Uuid;

/// Executes a complete multi-page crawl starting from a seed URL.
pub async fn execute_crawl(
    seed_raw_url: &str,
    options: CrawlOptions,
    scraper: Arc<ScraperService>,
    cancel: CancellationToken,
    progress: Arc<CrawlProgress>,
) -> Result<CrawlResult, CrawlerError> {
    let job_id = Uuid::new_v4().to_string();
    let seed_url = validate_url_syntax(seed_raw_url)?;

    info!(
        job_id = %job_id,
        seed = %seed_url,
        limit = options.limit,
        max_depth = options.max_depth,
        "Starting crawl execution"
    );

    let started_at = chrono_or_now();

    // 1. Initialize components
    let deduplicator = Arc::new(UrlDeduplicator::new());
    let frontier = Arc::new(CrawlFrontier::new(options.max_frontier_size));
    let robots = Arc::new(RobotsManager::new());
    let scheduler = Arc::new(CrawlScheduler::new(
        scraper.config().max_concurrent_fetches,
        scraper.config().max_concurrent_fetches_per_host,
        options.request_delay_ms,
    ));

    // 2. Enqueue seed target
    match evaluate_url_for_crawl(
        &seed_url,
        0,
        &seed_url,
        &options,
        &deduplicator,
        scraper.config().allow_private_networks,
    ) {
        FilterDecision::Allowed { normalized_url } => {
            frontier.try_push(CrawlTarget {
                url: seed_url.clone(),
                normalized_url,
                depth: 0,
                parent_url: None,
                discovered_from: DiscoverySource::Seed,
            })?;
            progress.inc_queued();
            progress.inc_discovered(1);
        }
        FilterDecision::Disallowed(reason) => {
            return Err(CrawlerError::InvalidUrl(format!(
                "Seed URL rejected by filter: {reason:?}"
            )));
        }
    }

    // 3. Optional sitemap discovery in background or pre-crawl
    if options.use_sitemap {
        let seed_clone = seed_url.clone();
        let robots_clone = robots.clone();
        let fetcher_clone = scraper.fetcher().clone();
        let options_clone = options.clone();
        let dedupe_clone = deduplicator.clone();
        let frontier_clone = frontier.clone();
        let progress_clone = progress.clone();
        let cancel_clone = cancel.clone();
        let allow_private = scraper.config().allow_private_networks;

        tokio::spawn(async move {
            if cancel_clone.is_cancelled() {
                return;
            }
            let parsed_robots = robots_clone.get_or_fetch(&seed_clone, &fetcher_clone).await;
            let sitemaps = parsed_robots.sitemaps.clone();

            let sitemap_urls = discover_sitemap_urls(
                &seed_clone,
                &sitemaps,
                &fetcher_clone,
                options_clone.limit.min(1000),
            )
            .await;

            for sm_url in sitemap_urls {
                if cancel_clone.is_cancelled() {
                    break;
                }
                if let FilterDecision::Allowed { normalized_url } = evaluate_url_for_crawl(
                    &sm_url,
                    1,
                    &seed_clone,
                    &options_clone,
                    &dedupe_clone,
                    allow_private,
                ) {
                    let target = CrawlTarget {
                        url: sm_url,
                        normalized_url,
                        depth: 1,
                        parent_url: Some(seed_clone.clone()),
                        discovered_from: DiscoverySource::Sitemap,
                    };
                    if frontier_clone.try_push(target).is_ok() {
                        progress_clone.inc_queued();
                        progress_clone.inc_discovered(1);
                    }
                }
            }
        });
    }

    // 4. Concurrency coordinator and worker tasks
    let pages = Arc::new(Mutex::new(Vec::new()));
    let active_workers = Arc::new(AtomicUsize::new(0));
    let crawl_finished = Arc::new(AtomicBool::new(false));

    let max_workers = scraper
        .config()
        .max_concurrent_fetches
        .min(options.limit)
        .max(1);

    let mut worker_handles = Vec::with_capacity(max_workers);

    for _ in 0..max_workers {
        let frontier = frontier.clone();
        let scraper = scraper.clone();
        let scheduler = scheduler.clone();
        let deduplicator = deduplicator.clone();
        let robots = robots.clone();
        let progress = progress.clone();
        let cancel = cancel.clone();
        let seed_url = seed_url.clone();
        let options = options.clone();
        let pages = pages.clone();
        let active_workers = active_workers.clone();
        let crawl_finished = crawl_finished.clone();

        let handle = tokio::spawn(async move {
            loop {
                if cancel.is_cancelled() || crawl_finished.load(Ordering::Relaxed) {
                    break;
                }

                // Check limit reached
                let current_snapshot = progress.snapshot();
                if current_snapshot.pages_crawled >= options.limit {
                    crawl_finished.store(true, Ordering::Relaxed);
                    break;
                }

                // Try popping next item
                let maybe_target = frontier.try_pop();
                match maybe_target {
                    Some(target) => {
                        active_workers.fetch_add(1, Ordering::SeqCst);
                        let crawled = process_crawl_target(
                            target,
                            &seed_url,
                            &options,
                            &scraper,
                            &scheduler,
                            &frontier,
                            &deduplicator,
                            &robots,
                            &progress,
                            &cancel,
                        )
                        .await;

                        if let Some(page) = crawled {
                            let mut list = pages.lock().await;
                            list.push(page);
                        }

                        active_workers.fetch_sub(1, Ordering::SeqCst);
                    }
                    None => {
                        // If queue is empty and no workers are currently fetching/parsing, we are done
                        if active_workers.load(Ordering::SeqCst) == 0 {
                            // Give a small grace period for any pending sitemap or link discovery
                            tokio::time::sleep(Duration::from_millis(50)).await;
                            if frontier.is_empty() && active_workers.load(Ordering::SeqCst) == 0 {
                                crawl_finished.store(true, Ordering::Relaxed);
                                break;
                            }
                        } else {
                            // Wait for active workers to push discovered links
                            tokio::time::sleep(Duration::from_millis(15)).await;
                        }
                    }
                }
            }
        });

        worker_handles.push(handle);
    }

    // 5. Monitor timeout and cancellation
    let max_duration = Duration::from_secs(options.max_duration_seconds);
    let start_instant = tokio::time::Instant::now();

    loop {
        if cancel.is_cancelled() {
            info!(job_id = %job_id, "Crawl was cancelled by user");
            break;
        }

        if start_instant.elapsed() >= max_duration {
            warn!(job_id = %job_id, "Crawl reached maximum duration limit");
            break;
        }

        if crawl_finished.load(Ordering::Relaxed) {
            break;
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    crawl_finished.store(true, Ordering::Relaxed);

    // 6. Join worker tasks
    for handle in worker_handles {
        let _ = handle.await;
    }

    let final_status = if cancel.is_cancelled() {
        CrawlStatus::Cancelled
    } else {
        CrawlStatus::Completed
    };

    let completed_at = Some(chrono_or_now());
    let collected_pages = pages.lock().await.clone();
    let stats = progress.snapshot();

    info!(
        job_id = %job_id,
        crawled = stats.pages_crawled,
        succeeded = stats.pages_succeeded,
        failed = stats.pages_failed,
        elapsed_ms = stats.elapsed_ms,
        "Crawl execution finished"
    );

    Ok(CrawlResult {
        job_id,
        status: final_status,
        seed_url: seed_url.to_string(),
        started_at,
        completed_at,
        stats,
        pages: collected_pages,
    })
}

fn chrono_or_now() -> String {
    Utc::now().to_rfc3339()
}
