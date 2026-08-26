use crate::browser::BrowserManager;
use crate::config::Config;
use crate::detect::models::{DetectedContentType, DocumentType};
use crate::detect::ContentDetector;
use crate::document::models::compute_blake3_hash;
use crate::document::parser::{ParseInput, ParserRegistry};
use crate::document::pool::DocumentParserPool;
use crate::error::CrawlerError;
use crate::extract::{clean_html, extract_main_content, html_to_markdown};
use crate::fetch::HttpFetcher;
use crate::models::{
    Heading, HttpMetadata, OutputFormat, PageImage, PageLink, PageMetadata, ScrapeOptions,
    ScrapeRequest, ScrapeResult, ScrapeWarning, WarningCode,
};
use crate::parser::{
    extract_headings, extract_images, extract_json_ld, extract_links, extract_metadata,
    ParsedDocument,
};
use crate::render::decision::{HttpScrapeAnalysis, SmartDecisionEngine};
use crate::render::models::{RenderMode, RenderRequest, SelectedRenderer};
use crate::render::{PageRenderer, RenderDecisionEngine};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};
use url::Url;
use uuid::Uuid;

/// Extracted page elements produced by the deterministic DOM extraction pipeline.
struct ExtractedFeatures {
    pub markdown: Option<String>,
    pub html: Option<String>,
    pub clean_html: Option<String>,
    pub metadata: Option<PageMetadata>,
    pub headings: Option<Vec<Heading>>,
    pub links: Option<Vec<PageLink>>,
    pub images: Option<Vec<PageImage>>,
    pub main_content_text: String,
}

/// Core scraper service orchestrating HTTP fetching, smart JS detection, browser fallback, and universal document extraction.
#[derive(Clone)]
pub struct ScraperService {
    fetcher: HttpFetcher,
    browser_manager: Arc<BrowserManager>,
    decision_engine: SmartDecisionEngine,
    parser_registry: Arc<ParserRegistry>,
    parser_pool: Arc<DocumentParserPool>,
    semaphore: Arc<Semaphore>,
    config: Arc<Config>,
}

impl ScraperService {
    pub fn new(config: Config) -> Result<Self, CrawlerError> {
        let config_arc = Arc::new(config.clone());
        let fetcher = HttpFetcher::new(config_arc.clone())?;
        let browser_manager = Arc::new(BrowserManager::new(config.clone()));
        let decision_engine = SmartDecisionEngine::new(config.render_auto_threshold);
        let parser_registry = Arc::new(ParserRegistry::new_default());
        let parser_pool = Arc::new(DocumentParserPool::new(
            config.max_concurrent_document_parsers,
        ));
        let semaphore = Arc::new(Semaphore::new(config_arc.max_concurrent_scrapes));

        Ok(Self {
            fetcher,
            browser_manager,
            decision_engine,
            parser_registry,
            parser_pool,
            semaphore,
            config: config_arc,
        })
    }

    pub fn config(&self) -> &Arc<Config> {
        &self.config
    }

    pub fn fetcher(&self) -> &HttpFetcher {
        &self.fetcher
    }

    pub fn browser_manager(&self) -> &Arc<BrowserManager> {
        &self.browser_manager
    }

    pub fn parser_registry(&self) -> &Arc<ParserRegistry> {
        &self.parser_registry
    }

    pub fn parser_pool(&self) -> &Arc<DocumentParserPool> {
        &self.parser_pool
    }

    /// Helper to extract requested formats and metadata from any HTML string (HTTP or Browser rendered).
    fn extract_features(
        &self,
        html: &str,
        final_url: &Url,
        options: &ScrapeOptions,
        warnings: &mut Vec<ScrapeWarning>,
    ) -> ExtractedFeatures {
        let parsed_doc = ParsedDocument::new(html, final_url.clone());

        let mut result_markdown = None;
        let mut result_html = None;
        let mut result_clean_html = None;
        let mut result_metadata = None;
        let mut result_headings = None;
        let mut result_links = None;
        let mut result_images = None;

        let needs_markdown = options.formats.contains(&OutputFormat::Markdown);
        let needs_clean_html = options.formats.contains(&OutputFormat::CleanHtml);
        let needs_raw_html = options.formats.contains(&OutputFormat::Html);
        let needs_metadata = options.formats.contains(&OutputFormat::Metadata);
        let needs_links = options.formats.contains(&OutputFormat::Links);
        let needs_images = options.formats.contains(&OutputFormat::Images);

        if needs_raw_html {
            result_html = Some(html.to_string());
        }

        if needs_metadata {
            let mut meta = extract_metadata(&parsed_doc, warnings);
            meta.json_ld = extract_json_ld(&parsed_doc, warnings);
            result_metadata = Some(meta);
            result_headings = Some(extract_headings(&parsed_doc));
        }

        if needs_links {
            result_links = Some(extract_links(&parsed_doc, warnings));
        }

        if needs_images {
            result_images = Some(extract_images(&parsed_doc));
        }

        let main_content_html = extract_main_content(&parsed_doc.html, warnings);

        if needs_markdown || needs_clean_html {
            let intermediate_html = if options.only_main_content {
                main_content_html.clone()
            } else {
                html.to_string()
            };

            let cleaned = clean_html(&intermediate_html);

            if needs_clean_html {
                result_clean_html = Some(cleaned.clone());
            }

            if needs_markdown {
                let markdown = html_to_markdown(&cleaned, final_url);
                result_markdown = Some(markdown);
            }
        }

        ExtractedFeatures {
            markdown: result_markdown,
            html: result_html,
            clean_html: result_clean_html,
            metadata: result_metadata,
            headings: result_headings,
            links: result_links,
            images: result_images,
            main_content_text: main_content_html,
        }
    }

    /// Primary entrypoint: Scrapes a web or document resource according to options and render mode.
    pub async fn scrape(&self, request: ScrapeRequest) -> Result<ScrapeResult, CrawlerError> {
        let _permit = self
            .semaphore
            .try_acquire()
            .map_err(|_| CrawlerError::ServerOverloaded)?;

        let trace_id = Uuid::new_v4();
        let total_start = Instant::now();
        let options = request.resolved_options();

        info!(
            %trace_id,
            url = %request.url,
            render_mode = ?options.render_mode,
            "Starting universal scrape operation"
        );

        match options.render_mode {
            // Explicit Browser Rendering Mode
            RenderMode::Browser => {
                self.scrape_browser(&request.url, &options, total_start, None)
                    .await
            }

            // Explicit HTTP Mode
            RenderMode::Http => {
                self.scrape_http_only(&request.url, &options, total_start)
                    .await
            }

            // Smart Auto Mode (Default: HTTP fetch first -> Content detection -> Smart Decision Engine)
            RenderMode::Auto => {
                let target_url = Url::parse(&request.url)
                    .map_err(|e| CrawlerError::InvalidUrl(format!("Invalid URL: {e}")))?;

                let fetch_res = self.fetcher.fetch(request.url.as_str()).await;

                match fetch_res {
                    Ok(fetched) => {
                        // 1. Detect content type from headers, magic bytes, and body
                        let detected = ContentDetector::detect(
                            &fetched.final_url,
                            Some(fetched.content_type.as_str()),
                            &fetched.body,
                        );

                        // 2. If non-HTML resource, route directly to the document parser!
                        if detected.document_type != DocumentType::Html {
                            info!(
                                url = %request.url,
                                doc_type = detected.document_type.as_str(),
                                "Non-HTML resource detected; dispatching to document parser"
                            );
                            return self
                                .parse_document_bytes(
                                    &target_url,
                                    &fetched.final_url,
                                    &fetched.body,
                                    &detected,
                                    &options,
                                    fetched.status,
                                    fetched.fetch_time_ms,
                                    total_start,
                                    fetched.warnings,
                                )
                                .await;
                        }

                        // 3. For HTML resources, execute Phase 1/3 intelligent JS detection
                        let html_str = fetched.html.clone();

                        let parse_start = Instant::now();
                        let mut warnings = fetched.warnings;
                        let features = self.extract_features(
                            &html_str,
                            &fetched.final_url,
                            &options,
                            &mut warnings,
                        );
                        let parse_time_ms = parse_start.elapsed().as_millis() as u64;

                        let analysis =
                            HttpScrapeAnalysis::analyze(&html_str, &features.main_content_text);
                        let decision = self.decision_engine.decide(&analysis);

                        if decision.renderer == SelectedRenderer::Http {
                            // HTTP result is sufficient -> return directly with zero browser overhead!
                            let total_time_ms = total_start.elapsed().as_millis() as u64;
                            let http_meta = HttpMetadata {
                                status: fetched.status,
                                content_type: fetched.content_type,
                                content_length: fetched.content_length,
                                fetch_time_ms: fetched.fetch_time_ms,
                                parse_time_ms,
                                extract_time_ms: 0,
                                total_time_ms,
                            };

                            let content_hash = Some(compute_blake3_hash(&fetched.body));

                            Ok(ScrapeResult {
                                url: request.url,
                                final_url: fetched.final_url.to_string(),
                                content_type: Some("html".to_string()),
                                content_hash,
                                markdown: features.markdown,
                                text: Some(clean_html(&features.main_content_text)),
                                html: features.html,
                                clean_html: features.clean_html,
                                json: features
                                    .metadata
                                    .as_ref()
                                    .map(|m| serde_json::to_value(&m.json_ld).unwrap_or_default()),
                                metadata: features.metadata,
                                headings: features.headings,
                                links: features.links,
                                images: features.images,
                                tables: None,
                                pages: None,
                                http: http_meta,
                                renderer: Some("http".to_string()),
                                render_reason: Some(
                                    decision
                                        .reasons
                                        .first()
                                        .map(|r| r.as_str().to_string())
                                        .unwrap_or_else(|| "http_content_sufficient".to_string()),
                                ),
                                render_diagnostics: Some(decision.diagnostics),
                                network_responses: None,
                                warnings,
                            })
                        } else {
                            // HTTP result is insufficient -> escalate to Browser rendering!
                            info!(
                                url = %request.url,
                                score = decision.score,
                                reasons = ?decision.reasons,
                                "Escalating to browser rendering"
                            );

                            let browser_res = self
                                .scrape_browser(
                                    &request.url,
                                    &options,
                                    total_start,
                                    Some(decision.clone()),
                                )
                                .await;

                            match browser_res {
                                Ok(mut res) => {
                                    res.warnings.push(ScrapeWarning::new(
                                        WarningCode::BrowserFallbackUsed,
                                        "Page escalated to browser rendering due to JavaScript dependency",
                                    ));
                                    Ok(res)
                                }
                                Err(err) => {
                                    warn!(
                                        url = %request.url,
                                        error = %err,
                                        "Browser fallback failed; falling back to partial HTTP content"
                                    );
                                    let total_time_ms = total_start.elapsed().as_millis() as u64;
                                    warnings.push(ScrapeWarning::new(
                                        WarningCode::BrowserFallbackFailed,
                                        format!("Browser fallback failed ({err}); returned HTTP content"),
                                    ));

                                    let http_meta = HttpMetadata {
                                        status: fetched.status,
                                        content_type: fetched.content_type,
                                        content_length: fetched.content_length,
                                        fetch_time_ms: fetched.fetch_time_ms,
                                        parse_time_ms,
                                        extract_time_ms: 0,
                                        total_time_ms,
                                    };

                                    let content_hash = Some(compute_blake3_hash(&fetched.body));

                                    Ok(ScrapeResult {
                                        url: request.url,
                                        final_url: fetched.final_url.to_string(),
                                        content_type: Some("html".to_string()),
                                        content_hash,
                                        markdown: features.markdown,
                                        text: Some(clean_html(&features.main_content_text)),
                                        html: features.html,
                                        clean_html: features.clean_html,
                                        json: features.metadata.as_ref().map(|m| {
                                            serde_json::to_value(&m.json_ld).unwrap_or_default()
                                        }),
                                        metadata: features.metadata,
                                        headings: features.headings,
                                        links: features.links,
                                        images: features.images,
                                        tables: None,
                                        pages: None,
                                        http: http_meta,
                                        renderer: Some("http".to_string()),
                                        render_reason: Some("browser_fallback_failed".to_string()),
                                        render_diagnostics: Some(decision.diagnostics),
                                        network_responses: None,
                                        warnings,
                                    })
                                }
                            }
                        }
                    }
                    Err(err) => {
                        debug!("HTTP fetch failed ({err}); attempting browser fallback");
                        self.scrape_browser(&request.url, &options, total_start, None)
                            .await
                    }
                }
            }
        }
    }

    /// Internal HTTP-only scraping path with content detection.
    async fn scrape_http_only(
        &self,
        raw_url: &str,
        options: &ScrapeOptions,
        total_start: Instant,
    ) -> Result<ScrapeResult, CrawlerError> {
        let target_url = Url::parse(raw_url)
            .map_err(|e| CrawlerError::InvalidUrl(format!("Invalid URL: {e}")))?;

        let fetched = self.fetcher.fetch(raw_url).await?;

        let detected = ContentDetector::detect(
            &fetched.final_url,
            Some(fetched.content_type.as_str()),
            &fetched.body,
        );

        if detected.document_type != DocumentType::Html {
            return self
                .parse_document_bytes(
                    &target_url,
                    &fetched.final_url,
                    &fetched.body,
                    &detected,
                    options,
                    fetched.status,
                    fetched.fetch_time_ms,
                    total_start,
                    fetched.warnings,
                )
                .await;
        }

        let html_str = fetched.html.clone();

        let parse_start = Instant::now();
        let mut warnings = fetched.warnings;
        let features = self.extract_features(&html_str, &fetched.final_url, options, &mut warnings);
        let parse_time_ms = parse_start.elapsed().as_millis() as u64;
        let total_time_ms = total_start.elapsed().as_millis() as u64;

        let http_meta = HttpMetadata {
            status: fetched.status,
            content_type: fetched.content_type,
            content_length: fetched.content_length,
            fetch_time_ms: fetched.fetch_time_ms,
            parse_time_ms,
            extract_time_ms: 0,
            total_time_ms,
        };

        let content_hash = Some(compute_blake3_hash(&fetched.body));

        Ok(ScrapeResult {
            url: raw_url.to_string(),
            final_url: fetched.final_url.to_string(),
            content_type: Some("html".to_string()),
            content_hash,
            markdown: features.markdown,
            text: Some(clean_html(&features.main_content_text)),
            html: features.html,
            clean_html: features.clean_html,
            json: features
                .metadata
                .as_ref()
                .map(|m| serde_json::to_value(&m.json_ld).unwrap_or_default()),
            metadata: features.metadata,
            headings: features.headings,
            links: features.links,
            images: features.images,
            tables: None,
            pages: None,
            http: http_meta,
            renderer: Some("http".to_string()),
            render_reason: Some("forced_http_mode".to_string()),
            render_diagnostics: None,
            network_responses: None,
            warnings,
        })
    }

    /// Internal Browser-based scraping path.
    async fn scrape_browser(
        &self,
        raw_url: &str,
        options: &ScrapeOptions,
        total_start: Instant,
        decision_hint: Option<crate::render::decision::RenderDecision>,
    ) -> Result<ScrapeResult, CrawlerError> {
        let target_url = Url::parse(raw_url)
            .map_err(|e| CrawlerError::InvalidUrl(format!("Invalid URL: {e}")))?;

        let timeout = options
            .timeout_ms
            .map(Duration::from_millis)
            .unwrap_or_else(|| self.config.browser_total_timeout());

        let render_req = RenderRequest {
            url: target_url.clone(),
            wait_strategy: options.wait_for.clone().unwrap_or_default(),
            timeout,
            actions: options.actions.clone(),
            auto_scroll: options.auto_scroll,
            capture_network: options.capture_network,
            block_trackers: options.block_trackers,
            block_resources: options.block_resources.clone(),
            user_agent: self.config.user_agent.clone(),
        };

        let render_result = self.browser_manager.render(render_req).await?;

        let mut warnings = Vec::new();
        let parse_start = Instant::now();
        let features = self.extract_features(
            &render_result.html,
            &render_result.final_url,
            options,
            &mut warnings,
        );
        let parse_time_ms = parse_start.elapsed().as_millis() as u64;
        let total_time_ms = total_start.elapsed().as_millis() as u64;

        let http_meta = HttpMetadata {
            status: render_result.status_code.unwrap_or(200),
            content_type: "text/html; charset=utf-8".to_string(),
            content_length: render_result.html.len(),
            fetch_time_ms: render_result.timings.navigation_time_ms,
            parse_time_ms,
            extract_time_ms: render_result.timings.action_time_ms,
            total_time_ms,
        };

        let (reason_str, diagnostics) = if let Some(d) = decision_hint {
            let r_str = d
                .reasons
                .first()
                .map(|r| r.as_str().to_string())
                .unwrap_or_else(|| "empty_spa_root".to_string());
            (r_str, Some(d.diagnostics))
        } else {
            ("forced_browser_mode".to_string(), None)
        };

        let content_hash = Some(compute_blake3_hash(render_result.html.as_bytes()));

        Ok(ScrapeResult {
            url: raw_url.to_string(),
            final_url: render_result.final_url.to_string(),
            content_type: Some("html".to_string()),
            content_hash,
            markdown: features.markdown,
            text: Some(clean_html(&features.main_content_text)),
            html: features.html,
            clean_html: features.clean_html,
            json: features
                .metadata
                .as_ref()
                .map(|m| serde_json::to_value(&m.json_ld).unwrap_or_default()),
            metadata: features.metadata,
            headings: features.headings,
            links: features.links,
            images: features.images,
            tables: None,
            pages: None,
            http: http_meta,
            renderer: Some("browser".to_string()),
            render_reason: Some(reason_str),
            render_diagnostics: diagnostics,
            network_responses: if options.capture_network {
                Some(render_result.network_responses)
            } else {
                None
            },
            warnings,
        })
    }

    /// Dispatches non-HTML document bytes to the appropriate parser in the ParserRegistry.
    #[allow(clippy::too_many_arguments)]
    async fn parse_document_bytes(
        &self,
        url: &Url,
        final_url: &Url,
        bytes: &[u8],
        detected: &DetectedContentType,
        options: &ScrapeOptions,
        status: u16,
        fetch_time_ms: u64,
        total_start: Instant,
        mut warnings: Vec<ScrapeWarning>,
    ) -> Result<ScrapeResult, CrawlerError> {
        let parser = self.parser_registry.find_parser(detected.document_type)?;

        let parse_input = ParseInput {
            url,
            final_url,
            bytes,
            detected,
            options,
            config: &self.config,
        };

        let parse_start = Instant::now();
        let parsed_doc = parser.parse(parse_input).await?;
        let parse_time_ms = parse_start.elapsed().as_millis() as u64;
        let total_time_ms = total_start.elapsed().as_millis() as u64;

        warnings.extend(parsed_doc.warnings);

        let http_meta = HttpMetadata {
            status,
            content_type: detected.mime_type.clone(),
            content_length: bytes.len(),
            fetch_time_ms,
            parse_time_ms,
            extract_time_ms: 0,
            total_time_ms,
        };

        let page_meta = PageMetadata {
            title: parsed_doc.metadata.title.clone(),
            description: parsed_doc.metadata.description.clone(),
            language: parsed_doc.metadata.language.clone(),
            canonical_url: parsed_doc.metadata.canonical_url.clone(),
            charset: parsed_doc.metadata.charset.clone(),
            favicon: None,
            author: parsed_doc.metadata.author.clone(),
            subject: parsed_doc.metadata.subject.clone(),
            creator: parsed_doc.metadata.creator.clone(),
            robots: None,
            published_time: parsed_doc.metadata.published_time.clone(),
            modified_time: parsed_doc.metadata.modified_time.clone(),
            generator: parsed_doc.metadata.generator.clone(),
            theme_color: None,
            page_count: parsed_doc.metadata.page_count,
            row_count: parsed_doc.metadata.row_count,
            word_count: parsed_doc.metadata.word_count,
            content_hash: parsed_doc.metadata.content_hash.clone(),
            open_graph: parsed_doc.metadata.open_graph.clone(),
            twitter: parsed_doc.metadata.twitter.clone(),
            json_ld: parsed_doc.metadata.json_ld.clone(),
            custom: parsed_doc.metadata.custom.clone(),
        };

        Ok(ScrapeResult {
            url: url.to_string(),
            final_url: final_url.to_string(),
            content_type: Some(detected.document_type.as_str().to_string()),
            content_hash: parsed_doc.metadata.content_hash,
            markdown: parsed_doc.markdown,
            text: parsed_doc.text,
            html: parsed_doc.html,
            clean_html: parsed_doc.clean_html,
            json: parsed_doc.structured_data,
            metadata: Some(page_meta),
            headings: None,
            links: if options.formats.contains(&OutputFormat::Links) {
                Some(parsed_doc.links)
            } else {
                None
            },
            images: if options.formats.contains(&OutputFormat::Images) {
                Some(parsed_doc.images)
            } else {
                None
            },
            tables: if options.formats.contains(&OutputFormat::Tables) {
                Some(parsed_doc.tables)
            } else {
                None
            },
            pages: if options.formats.contains(&OutputFormat::Pages) {
                Some(parsed_doc.pages)
            } else {
                None
            },
            http: http_meta,
            renderer: Some("document_parser".to_string()),
            render_reason: Some(detected.detection_reason.clone()),
            render_diagnostics: None,
            network_responses: None,
            warnings,
        })
    }

    /// CLI-only local file parser reusing ParserRegistry without bypassing HTTP SSRF security.
    pub async fn parse_file(
        &self,
        path: &Path,
        options: &ScrapeOptions,
    ) -> Result<ScrapeResult, CrawlerError> {
        if !path.exists() {
            return Err(CrawlerError::InvalidUrl(format!(
                "File does not exist: {}",
                path.display()
            )));
        }

        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| CrawlerError::InternalError(format!("Failed to read local file: {e}")))?;

        let filename = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("document");
        let fake_url = Url::parse(&format!("http://localhost/{}", filename))
            .unwrap_or_else(|_| Url::parse("http://localhost/document").unwrap());

        let detected = ContentDetector::detect(&fake_url, None, &bytes);
        let total_start = Instant::now();

        self.parse_document_bytes(
            &fake_url,
            &fake_url,
            &bytes,
            &detected,
            options,
            200,
            0,
            total_start,
            Vec::new(),
        )
        .await
    }
}
