use crate::config::Config;
use crate::crawl::models::CrawlOptions;
use crate::crawl::CrawlerService;
use crate::error::CrawlerError;
use crate::extraction::cache::ExtractionCache;
use crate::extraction::candidate::CandidatePool;
use crate::extraction::context::ContextSelector;
use crate::extraction::deterministic::DeterministicExtractor;
use crate::extraction::llm::{build_extraction_prompt, LlmProviderRegistry};
use crate::extraction::models::{
    BatchExtractionRequest, BatchExtractionResponse, BatchItemResult, ExtractionJobInfo,
    ExtractionJobStatus, ExtractionMetadata, ExtractionMode, ExtractionRequest, ExtractionResult,
    LlmExtractionRequest,
};
use crate::extraction::provenance::ProvenanceTracker;
use crate::extraction::repair::ExtractionRepairPipeline;
use crate::extraction::schema::SchemaValidator;
use crate::extraction::validation::JsonValidator;
use crate::models::{
    HttpMetadata, OutputFormat, ScrapeOptions, ScrapeRequest, ScrapeResult, ScrapeWarning,
    WarningCode,
};
use crate::service::ScraperService;
use dashmap::DashMap;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use url::Url;

pub struct ExtractionService {
    config: Arc<Config>,
    scraper: Arc<ScraperService>,
    crawler: Arc<CrawlerService>,
    providers: Arc<LlmProviderRegistry>,
    cache: ExtractionCache,
    jobs: Arc<DashMap<String, ExtractionJobInfo>>,
    cancellation_tokens: Arc<DashMap<String, CancellationToken>>,
}

impl ExtractionService {
    pub fn new(config: Config, scraper: Arc<ScraperService>, crawler: Arc<CrawlerService>) -> Self {
        let config_arc = Arc::new(config.clone());
        let providers = Arc::new(LlmProviderRegistry::new(&config));
        let cache = ExtractionCache::new(
            config.extraction_cache_enabled,
            config.extraction_cache_max_items,
        );

        Self {
            config: config_arc,
            scraper,
            crawler,
            providers,
            cache,
            jobs: Arc::new(DashMap::new()),
            cancellation_tokens: Arc::new(DashMap::new()),
        }
    }

    pub fn providers(&self) -> Arc<LlmProviderRegistry> {
        self.providers.clone()
    }

    /// Performs single-page, multi-page, or crawl-based structured data extraction.
    pub async fn extract(
        &self,
        request: ExtractionRequest,
        cancel: Option<CancellationToken>,
    ) -> Result<ExtractionResult, CrawlerError> {
        let start_time = Instant::now();
        let cancel_token = cancel.unwrap_or_default();

        if cancel_token.is_cancelled() {
            return Err(CrawlerError::ExtractionCancelled);
        }

        // 1. Validate Schema upfront if provided
        if let Some(ref schema) = request.schema {
            SchemaValidator::validate_schema(
                schema,
                self.config.max_schema_depth,
                self.config.max_schema_properties,
            )?;
        }

        let mode = request.mode.unwrap_or_default();
        let missing_behavior = request.missing_field_behavior.unwrap_or_default();
        let include_provenance = request.include_provenance.unwrap_or(false);

        // 2. Fetch pages (via Single scrape, Multi-URL scrape, or Crawl)
        let (scrape_results, primary_url) = self
            .fetch_extraction_targets(&request, &cancel_token)
            .await?;

        if scrape_results.is_empty() {
            return Err(CrawlerError::NoExtractableContent);
        }

        if cancel_token.is_cancelled() {
            return Err(CrawlerError::ExtractionCancelled);
        }

        // Compute combined content hash for cache check
        let combined_content_hashes: String = scrape_results
            .iter()
            .filter_map(|r| r.content_hash.clone())
            .collect::<Vec<_>>()
            .join(":");

        let cache_key = ExtractionCache::compute_key(
            &combined_content_hashes,
            request.schema.as_ref(),
            request.prompt.as_deref(),
            request.model.as_deref(),
        );

        if let Some(cached) = self.cache.get(&cache_key) {
            debug!("Extraction cache hit for key: {cache_key}");
            return Ok(cached);
        }

        let mut warnings = Vec::new();
        let sources_used = scrape_results.len();

        // 3. Step A: Deterministic Extraction attempt
        let mut candidate_pool = CandidatePool::new();
        for result in &scrape_results {
            let pool = CandidatePool::from_scrape_result(result);
            candidate_pool.candidates.extend(pool.candidates);
        }

        if mode == ExtractionMode::Deterministic || mode == ExtractionMode::Auto {
            let (det_val, det_provenance, is_confident) = DeterministicExtractor::extract(
                &candidate_pool,
                request.schema.as_ref(),
                request.prompt.as_deref(),
                &primary_url,
                missing_behavior,
            );

            if let Some(data) = det_val {
                if mode == ExtractionMode::Deterministic || is_confident {
                    // Validated if schema present
                    let validated = if let Some(ref schema) = request.schema {
                        JsonValidator::validate(&data, schema).is_ok()
                    } else {
                        false
                    };

                    let res = ExtractionResult {
                        success: true,
                        data: Some(data),
                        metadata: ExtractionMetadata {
                            mode,
                            extractor: "deterministic".to_string(),
                            validated,
                            sources_used,
                            chunks_used: 0,
                            llm_calls: 0,
                            cost_estimate_usd: None,
                            duration_ms: start_time.elapsed().as_millis() as u64,
                            cache_hit: false,
                        },
                        provenance: if include_provenance {
                            Some(det_provenance)
                        } else {
                            None
                        },
                        warnings,
                        error: None,
                    };

                    self.cache.insert(cache_key, res.clone());
                    return Ok(res);
                } else {
                    warnings.push(ScrapeWarning::new(
                        WarningCode::DeterministicExtractionIncomplete,
                        "Deterministic extraction could not resolve all required schema fields; falling back to LLM",
                    ));
                }
            }
        }

        if mode == ExtractionMode::Deterministic {
            return Err(CrawlerError::NoExtractableContent);
        }

        // 4. Step B: Context Selection for LLM
        let (selected_chunks, chunk_warnings, _total_chars) = ContextSelector::select_context(
            &scrape_results,
            request.schema.as_ref(),
            request.prompt.as_deref(),
            self.config.max_extraction_context_chars,
            self.config.max_extraction_chunks,
            self.config.max_chunk_chars,
        );
        warnings.extend(chunk_warnings);

        if selected_chunks.is_empty() {
            return Err(CrawlerError::NoExtractableContent);
        }

        if cancel_token.is_cancelled() {
            return Err(CrawlerError::ExtractionCancelled);
        }

        // 5. Step C: LLM Generation
        let provider = match self.providers.get_provider(request.provider.as_deref()) {
            Ok(p) => p,
            Err(e) => {
                if !self.config.llm_enabled && provider_is_disabled(request.provider.as_deref()) {
                    return Err(CrawlerError::LlmDisabled);
                }
                return Err(e);
            }
        };

        let semaphore = self.providers.semaphore();
        let _permit = semaphore.acquire().await.map_err(|_| {
            CrawlerError::InternalError("LLM concurrency semaphore closed".to_string())
        })?;

        let (system_prompt, user_prompt) = build_extraction_prompt(
            request.schema.as_ref(),
            request.prompt.as_deref(),
            &selected_chunks,
        );

        let llm_req = LlmExtractionRequest {
            system_prompt,
            user_prompt,
            schema: request.schema.clone(),
            model: request.model.clone(),
            temperature: Some(0.0),
            max_tokens: None,
        };

        warnings.push(ScrapeWarning::new(
            WarningCode::LlmFallbackUsed,
            format!(
                "Structured data extracted via LLM provider '{}'",
                provider.name()
            ),
        ));

        let llm_res = provider.structured_generate(llm_req).await?;
        let raw_json_val = llm_res.parsed_json.ok_or_else(|| {
            CrawlerError::LlmInvalidResponse("LLM returned non-JSON content".to_string())
        })?;

        // 6. Step D: Validation & Repair Pipeline
        let (final_data, repaired_via_llm) = if let Some(ref schema) = request.schema {
            let (repaired, used_repair_call) = ExtractionRepairPipeline::repair_and_validate(
                raw_json_val,
                schema,
                missing_behavior,
                Some(provider.clone()),
                self.config.max_extraction_repair_attempts,
            )
            .await?;

            if used_repair_call {
                warnings.push(ScrapeWarning::new(
                    WarningCode::LlmRepairUsed,
                    "LLM repair call was executed to fix schema validation mismatches",
                ));
            }

            (repaired, used_repair_call)
        } else {
            (raw_json_val, false)
        };

        // Deduplicate array elements if requested
        let final_data = if !request.dedupe_by.is_empty() {
            deduplicate_array_data(final_data, &request.dedupe_by, &mut warnings)
        } else {
            final_data
        };

        // 7. Step E: Provenance citations
        let provenance = if include_provenance {
            Some(ProvenanceTracker::build_provenance(
                &final_data,
                &selected_chunks,
                None,
            ))
        } else {
            None
        };

        let total_llm_calls = 1 + (if repaired_via_llm { 1 } else { 0 });

        let res = ExtractionResult {
            success: true,
            data: Some(final_data),
            metadata: ExtractionMetadata {
                mode,
                extractor: format!("llm ({})", provider.name()),
                validated: request.schema.is_some(),
                sources_used,
                chunks_used: selected_chunks.len(),
                llm_calls: total_llm_calls,
                cost_estimate_usd: llm_res.usage.estimated_cost_usd,
                duration_ms: start_time.elapsed().as_millis() as u64,
                cache_hit: false,
            },
            provenance,
            warnings,
            error: None,
        };

        self.cache.insert(cache_key, res.clone());
        Ok(res)
    }

    /// Fetches scrape targets for single-page, multi-URL, or crawl requests.
    async fn fetch_extraction_targets(
        &self,
        request: &ExtractionRequest,
        cancel: &CancellationToken,
    ) -> Result<(Vec<ScrapeResult>, Url), CrawlerError> {
        let scrape_options = ScrapeOptions {
            formats: vec![
                OutputFormat::Markdown,
                OutputFormat::Metadata,
                OutputFormat::Tables,
                OutputFormat::Json,
                OutputFormat::CleanHtml,
            ],
            render_mode: request.render_mode.unwrap_or_default(),
            ..Default::default()
        };

        if let Some(ref crawl_opts) = request.crawl {
            // Crawl mode
            let seed_url_str = request
                .url
                .as_deref()
                .or_else(|| request.urls.first().map(|s| s.as_str()))
                .ok_or_else(|| {
                    CrawlerError::InvalidExtractionRequest(
                        "Crawl extraction requires a seed URL".to_string(),
                    )
                })?;

            let seed_url = Url::parse(seed_url_str)
                .map_err(|_| CrawlerError::InvalidUrl(seed_url_str.to_string()))?;

            let crawl_options = CrawlOptions {
                limit: crawl_opts.limit,
                max_depth: crawl_opts.max_depth,
                include_paths: crawl_opts.include_paths.clone(),
                exclude_paths: crawl_opts.exclude_paths.clone(),
                allow_subdomains: crawl_opts.allow_subdomains,
                formats: Some(scrape_options.formats.clone()),
                render_mode: scrape_options.render_mode,
                ..Default::default()
            };

            let crawl_res = self
                .crawler
                .crawl(seed_url_str, crawl_options, cancel.clone())
                .await?;

            let mut results = Vec::new();
            for page in crawl_res.pages {
                results.push(ScrapeResult {
                    url: page.url,
                    final_url: page.final_url,
                    content_type: page.content_type.clone(),
                    content_hash: page.content_hash,
                    markdown: page.markdown,
                    text: page.text,
                    html: page.html,
                    clean_html: page.clean_html,
                    json: page.json,
                    metadata: page.metadata,
                    headings: page.headings,
                    links: page.links,
                    images: page.images,
                    tables: page.tables,
                    pages: page.pages,
                    http: HttpMetadata {
                        status: page.status_code,
                        content_type: page.content_type.unwrap_or_default(),
                        content_length: 0,
                        fetch_time_ms: page.timings.fetch_ms,
                        parse_time_ms: page.timings.parse_ms,
                        extract_time_ms: page.timings.extract_ms,
                        total_time_ms: page.timings.total_ms,
                    },
                    renderer: page.renderer,
                    render_reason: page.render_reason,
                    render_diagnostics: None,
                    network_responses: page.network_responses,
                    warnings: page.warnings,
                });
            }

            Ok((results, seed_url))
        } else if !request.urls.is_empty() {
            // Multi-URL mode
            let primary_url_str = &request.urls[0];
            let primary_url = Url::parse(primary_url_str)
                .map_err(|_| CrawlerError::InvalidUrl(primary_url_str.to_string()))?;

            let mut results = Vec::new();
            for url_str in &request.urls {
                if cancel.is_cancelled() {
                    return Err(CrawlerError::ExtractionCancelled);
                }
                let scrape_req = ScrapeRequest {
                    url: url_str.clone(),
                    formats: Some(scrape_options.formats.clone()),
                    only_main_content: Some(scrape_options.only_main_content),
                    render_mode: Some(scrape_options.render_mode),
                    wait_for: scrape_options.wait_for.clone(),
                    auto_scroll: Some(scrape_options.auto_scroll),
                    capture_network: Some(scrape_options.capture_network),
                    block_trackers: Some(scrape_options.block_trackers),
                    block_resources: Some(scrape_options.block_resources.clone()),
                    actions: Some(scrape_options.actions.clone()),
                    timeout_ms: scrape_options.timeout_ms,
                    json_path: scrape_options.json_path.clone(),
                    follow_feed_links: scrape_options.follow_feed_links,
                    follow_document_links: scrape_options.follow_document_links,
                };
                if let Ok(scrape_res) = self.scraper.scrape(scrape_req).await {
                    results.push(scrape_res);
                }
            }

            Ok((results, primary_url))
        } else if let Some(ref url_str) = request.url {
            // Single URL mode
            let target_url =
                Url::parse(url_str).map_err(|_| CrawlerError::InvalidUrl(url_str.to_string()))?;

            let scrape_req = ScrapeRequest {
                url: url_str.clone(),
                formats: Some(scrape_options.formats.clone()),
                only_main_content: Some(scrape_options.only_main_content),
                render_mode: Some(scrape_options.render_mode),
                wait_for: scrape_options.wait_for.clone(),
                auto_scroll: Some(scrape_options.auto_scroll),
                capture_network: Some(scrape_options.capture_network),
                block_trackers: Some(scrape_options.block_trackers),
                block_resources: Some(scrape_options.block_resources.clone()),
                actions: Some(scrape_options.actions.clone()),
                timeout_ms: scrape_options.timeout_ms,
                json_path: scrape_options.json_path.clone(),
                follow_feed_links: scrape_options.follow_feed_links,
                follow_document_links: scrape_options.follow_document_links,
            };
            let scrape_res = self.scraper.scrape(scrape_req).await?;
            Ok((vec![scrape_res], target_url))
        } else {
            Err(CrawlerError::InvalidExtractionRequest(
                "Extraction request must provide either 'url', 'urls', or 'crawl'".to_string(),
            ))
        }
    }

    /// Executes batch structured data extractions with bounded concurrency.
    pub async fn extract_batch(
        &self,
        request: BatchExtractionRequest,
        cancel: Option<CancellationToken>,
    ) -> BatchExtractionResponse {
        let start = Instant::now();
        let cancel_token = cancel.unwrap_or_default();
        let max_concurrency = request.max_concurrency.clamp(1, 20);
        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrency));

        let mut tasks = Vec::new();

        for item in request.items {
            let sem = semaphore.clone();
            let c_token = cancel_token.clone();
            let default_schema = request.default_schema.clone();
            let default_prompt = request.default_prompt.clone();
            let default_mode = request.default_mode;
            let default_prov = request.default_include_provenance;

            // Clone service Arc for spawned workers
            let scraper = self.scraper.clone();
            let crawler = self.crawler.clone();
            let config = (*self.config).clone();
            let providers = self.providers.clone();

            tasks.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.ok();
                if c_token.is_cancelled() {
                    return BatchItemResult {
                        url: item.url,
                        success: false,
                        data: None,
                        metadata: None,
                        provenance: None,
                        error: Some("Extraction cancelled".to_string()),
                        warnings: Vec::new(),
                    };
                }

                let ext_req = ExtractionRequest {
                    url: Some(item.url.clone()),
                    schema: item.schema.or(default_schema),
                    prompt: item.prompt.or(default_prompt),
                    mode: item.mode.or(default_mode),
                    include_provenance: item.include_provenance.or(default_prov),
                    ..Default::default()
                };

                let local_service = ExtractionService {
                    config: Arc::new(config),
                    scraper,
                    crawler,
                    providers,
                    cache: ExtractionCache::new(false, 100),
                    jobs: Arc::new(DashMap::new()),
                    cancellation_tokens: Arc::new(DashMap::new()),
                };

                match local_service.extract(ext_req, Some(c_token)).await {
                    Ok(res) => BatchItemResult {
                        url: item.url,
                        success: res.success,
                        data: res.data,
                        metadata: Some(res.metadata),
                        provenance: res.provenance,
                        error: None,
                        warnings: res.warnings,
                    },
                    Err(e) => BatchItemResult {
                        url: item.url,
                        success: false,
                        data: None,
                        metadata: None,
                        provenance: None,
                        error: Some(e.to_string()),
                        warnings: Vec::new(),
                    },
                }
            }));
        }

        let mut results = Vec::new();
        let mut successful = 0;
        let mut failed = 0;

        for task in tasks {
            if let Ok(res) = task.await {
                if res.success {
                    successful += 1;
                } else {
                    failed += 1;
                }
                results.push(res);
            } else {
                failed += 1;
            }
        }

        BatchExtractionResponse {
            total: results.len(),
            successful,
            failed,
            results,
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Background job creation
    pub fn create_job(&self, _request: ExtractionRequest) -> (String, CancellationToken) {
        let job_id = uuid::Uuid::new_v4().to_string();
        let token = CancellationToken::new();

        let job_info = ExtractionJobInfo {
            job_id: job_id.clone(),
            status: ExtractionJobStatus::Queued,
            created_at: chrono::Utc::now().to_rfc3339(),
            finished_at: None,
            result: None,
            error: None,
        };

        self.jobs.insert(job_id.clone(), job_info);
        self.cancellation_tokens
            .insert(job_id.clone(), token.clone());

        (job_id, token)
    }

    pub fn get_job(&self, job_id: &str) -> Option<ExtractionJobInfo> {
        self.jobs.get(job_id).map(|r| r.clone())
    }

    pub fn update_job_status(&self, job_id: &str, status: ExtractionJobStatus) {
        if let Some(mut job) = self.jobs.get_mut(job_id) {
            job.status = status;
        }
    }

    pub fn complete_job(&self, job_id: &str, result: ExtractionResult) {
        if let Some(mut job) = self.jobs.get_mut(job_id) {
            job.status = ExtractionJobStatus::Completed;
            job.finished_at = Some(chrono::Utc::now().to_rfc3339());
            job.result = Some(result);
        }
    }

    pub fn fail_job(&self, job_id: &str, error_msg: String) {
        if let Some(mut job) = self.jobs.get_mut(job_id) {
            job.status = ExtractionJobStatus::Failed;
            job.finished_at = Some(chrono::Utc::now().to_rfc3339());
            job.error = Some(error_msg);
        }
    }

    pub fn cancel_job(&self, job_id: &str) -> bool {
        if let Some(token) = self.cancellation_tokens.get(job_id) {
            token.cancel();
            if let Some(mut job) = self.jobs.get_mut(job_id) {
                job.status = ExtractionJobStatus::Cancelled;
                job.finished_at = Some(chrono::Utc::now().to_rfc3339());
            }
            true
        } else {
            false
        }
    }

    pub fn delete_job(&self, job_id: &str) -> bool {
        self.cancel_job(job_id);
        self.jobs.remove(job_id).is_some()
    }
}

fn provider_is_disabled(provider_name: Option<&str>) -> bool {
    let name = provider_name.unwrap_or("openai-compatible");
    name != "mock"
}

fn deduplicate_array_data(
    data: Value,
    dedupe_keys: &[String],
    warnings: &mut Vec<ScrapeWarning>,
) -> Value {
    if let Value::Array(items) = data {
        let mut seen = HashSet::new();
        let mut deduplicated = Vec::new();
        let initial_len = items.len();

        for item in items {
            let key = if let Some(obj) = item.as_object() {
                let mut composite = Vec::new();
                for k in dedupe_keys {
                    if let Some(v) = obj.get(k) {
                        composite.push(v.to_string());
                    }
                }
                if composite.is_empty() {
                    item.to_string()
                } else {
                    composite.join(":")
                }
            } else {
                item.to_string()
            };

            if seen.insert(key) {
                deduplicated.push(item);
            }
        }

        if deduplicated.len() < initial_len {
            warnings.push(ScrapeWarning::new(
                WarningCode::DuplicateEntityRemoved,
                format!(
                    "Removed {} duplicate entities based on keys {:?}",
                    initial_len - deduplicated.len(),
                    dedupe_keys
                ),
            ));
        }

        Value::Array(deduplicated)
    } else {
        data
    }
}
