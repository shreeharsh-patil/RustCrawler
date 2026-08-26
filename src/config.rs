use std::net::IpAddr;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub server_host: IpAddr,
    pub server_port: u16,
    pub request_timeout_seconds: u64,
    pub max_response_size_mb: usize,
    pub max_redirects: usize,
    pub max_concurrent_scrapes: usize,
    pub user_agent: String,
    pub rust_log: String,

    // Phase 2 Crawl settings
    pub max_concurrent_fetches: usize,
    pub max_concurrent_fetches_per_host: usize,
    pub default_crawl_limit: usize,
    pub default_max_depth: u32,
    pub max_frontier_size: usize,
    pub default_max_retries: usize,
    pub default_request_delay_ms: u64,
    pub max_stored_jobs: usize,
    pub job_retention_seconds: u64,
    pub max_crawl_duration_seconds: u64,
    pub allow_private_networks: bool,

    // Phase 3 Browser & Rendering settings
    pub browser_enabled: bool,
    pub browser_executable_path: Option<String>,
    pub browser_headless: bool,
    pub max_browser_pages: usize,
    pub max_browser_queue_size: usize,
    pub browser_acquire_timeout_seconds: u64,
    pub browser_navigation_timeout_seconds: u64,
    pub browser_total_timeout_seconds: u64,
    pub browser_idle_timeout_seconds: u64,
    pub render_auto_threshold: i32,
    pub network_idle_quiet_ms: u64,
    pub auto_scroll_max_steps: usize,
    pub auto_scroll_delay_ms: u64,
    pub max_browser_actions: usize,
    pub max_action_wait_seconds: u64,
    pub max_network_responses: usize,
    pub max_network_response_bytes: usize,
    pub max_total_network_capture_bytes: usize,

    // Phase 4 Universal Document Extraction settings
    pub max_document_size_mb: usize,
    pub max_pdf_size_mb: usize,
    pub max_docx_size_mb: usize,
    pub max_json_size_mb: usize,
    pub max_xml_size_mb: usize,
    pub max_csv_size_mb: usize,
    pub max_text_size_mb: usize,
    pub max_json_depth: usize,
    pub max_json_array_items: usize,
    pub max_table_rows: usize,
    pub max_table_columns: usize,
    pub max_pdf_pages: usize,
    pub max_pdf_extracted_text_mb: usize,
    pub max_archive_entries: usize,
    pub max_docx_uncompressed_mb: usize,
    pub max_archive_compression_ratio: f64,
    pub max_concurrent_document_parsers: usize,
    pub document_parse_timeout_seconds: u64,
    pub follow_feed_links: bool,
    pub follow_document_links: bool,
    pub deduplicate_content: bool,

    // Phase 5 Structured Extraction & LLM settings
    pub max_schema_depth: usize,
    pub max_schema_properties: usize,
    pub max_extraction_context_chars: usize,
    pub max_extraction_chunks: usize,
    pub max_chunk_chars: usize,
    pub llm_enabled: bool,
    pub llm_provider: String,
    pub llm_base_url: Option<String>,
    pub llm_api_key: Option<String>,
    pub llm_model: Option<String>,
    pub llm_timeout_seconds: u64,
    pub llm_max_retries: usize,
    pub max_concurrent_llm_requests: usize,
    pub max_extraction_repair_attempts: usize,
    pub max_llm_calls_per_extraction: usize,
    pub max_llm_calls_per_job: usize,
    pub extraction_cache_enabled: bool,
    pub extraction_cache_max_items: usize,

    // Phase 6 Sitemap, Discovery, Search & Cache settings
    pub max_sitemap_files: usize,
    pub max_sitemap_urls: usize,
    pub max_sitemap_depth: usize,
    pub max_sitemap_file_size_mb: usize,
    pub max_concurrent_discovery_fetches: usize,
    pub max_map_urls: usize,
    pub max_anchor_text_length: usize,
    pub max_page_links_discovered: usize,
    pub search_enabled: bool,
    pub search_provider: String,
    pub search_base_url: Option<String>,
    pub search_api_key: Option<String>,
    pub max_search_results: usize,
    pub max_search_scrape_results: usize,
    pub max_concurrent_search_scrapes: usize,
    pub search_timeout_seconds: u64,
    pub cache_enabled: bool,
    pub cache_max_entries: usize,
    pub cache_default_ttl_seconds: u64,
    pub robots_cache_ttl_seconds: u64,
    pub sitemap_cache_ttl_seconds: u64,
    pub map_cache_ttl_seconds: u64,

    // Phase 7 Distributed, Queue, Worker, Webhook & Storage settings
    pub execution_mode: String,
    pub database_url: Option<String>,
    pub redis_url: Option<String>,
    pub queue_backend: String,
    pub result_store: String,
    pub s3_endpoint: Option<String>,
    pub s3_bucket: Option<String>,
    pub s3_region: Option<String>,
    pub s3_access_key: Option<String>,
    pub s3_secret_key: Option<String>,
    pub task_lease_seconds: u64,
    pub task_heartbeat_seconds: u64,
    pub task_max_attempts: usize,
    pub task_retry_base_ms: u64,
    pub task_retry_max_ms: u64,
    pub worker_heartbeat_seconds: u64,
    pub worker_dead_after_seconds: u64,
    pub webhook_max_attempts: usize,
    pub webhook_timeout_seconds: u64,
    pub webhook_signing_secret: Option<String>,
    pub shutdown_grace_seconds: u64,
    pub database_max_connections: u32,
    pub database_min_connections: u32,
    pub database_acquire_timeout_seconds: u64,
    pub job_event_retention_seconds: u64,
    pub max_events_per_job: usize,
    pub job_retention_days: u64,
    pub result_retention_days: u64,
    pub snapshot_retention_days: u64,
    pub api_key_auth_enabled: bool,

    // Phase 8 Semantic Knowledge Engine, Vector Store, Embeddings & RAG
    pub semantic_indexing_enabled: bool,
    pub vector_backend: String,
    pub lexical_backend: String,
    pub embeddings_enabled: bool,
    pub embedding_provider: String,
    pub embedding_base_url: Option<String>,
    pub embedding_api_key: Option<String>,
    pub embedding_model: Option<String>,
    pub embedding_dimensions: Option<usize>,
    pub embedding_batch_size: usize,
    pub max_concurrent_embedding_requests: usize,
    pub embedding_timeout_seconds: u64,
    pub target_chunk_tokens: usize,
    pub max_chunk_tokens: usize,
    pub min_chunk_tokens: usize,
    pub chunk_overlap_tokens: usize,
    pub rag_max_context_tokens: usize,
    pub rag_max_chunks: usize,
    pub rag_min_score: Option<f64>,
    pub max_chunks_per_index_job: usize,
    pub max_embedding_batch_tokens: usize,
    pub max_embedding_calls_per_job: usize,
    pub max_query_expansions: usize,
    pub max_chunks_per_document: usize,

    // Phase 9 Autonomous Web Research Agent & MCP settings
    pub agent_enabled: bool,
    pub agent_max_steps: u32,
    pub agent_max_pages: u32,
    pub agent_max_search_queries: u32,
    pub agent_max_query_variants_per_subgoal: u32,
    pub agent_max_research_rounds: u32,
    pub agent_max_browser_pages: u32,
    pub agent_max_browser_actions_total: u32,
    pub agent_max_llm_calls: u32,
    pub agent_max_duration_seconds: u64,
    pub agent_min_source_diversity: usize,
    pub agent_planner_model: Option<String>,
    pub agent_evaluator_model: Option<String>,
    pub agent_synthesis_model: Option<String>,
    pub agent_result_retention_days: u32,
    pub agent_evidence_retention_days: u32,
    pub mcp_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_host: [0, 0, 0, 0].into(),
            server_port: 3000,
            request_timeout_seconds: 15,
            max_response_size_mb: 10,
            max_redirects: 10,
            max_concurrent_scrapes: 100,
            user_agent: "RustCrawler/0.1".to_string(),
            rust_log: "info".to_string(),

            // Crawl defaults
            max_concurrent_fetches: 50,
            max_concurrent_fetches_per_host: 5,
            default_crawl_limit: 100,
            default_max_depth: 3,
            max_frontier_size: 10000,
            default_max_retries: 2,
            default_request_delay_ms: 100,
            max_stored_jobs: 100,
            job_retention_seconds: 3600,
            max_crawl_duration_seconds: 300,
            allow_private_networks: false,

            // Browser defaults
            browser_enabled: true,
            browser_executable_path: None,
            browser_headless: true,
            max_browser_pages: 4,
            max_browser_queue_size: 100,
            browser_acquire_timeout_seconds: 10,
            browser_navigation_timeout_seconds: 20,
            browser_total_timeout_seconds: 30,
            browser_idle_timeout_seconds: 300,
            render_auto_threshold: 5,
            network_idle_quiet_ms: 500,
            auto_scroll_max_steps: 10,
            auto_scroll_delay_ms: 300,
            max_browser_actions: 20,
            max_action_wait_seconds: 10,
            max_network_responses: 100,
            max_network_response_bytes: 1024 * 1024, // 1MB
            max_total_network_capture_bytes: 10 * 1024 * 1024, // 10MB

            // Document extraction defaults
            max_document_size_mb: 25,
            max_pdf_size_mb: 25,
            max_docx_size_mb: 25,
            max_json_size_mb: 10,
            max_xml_size_mb: 10,
            max_csv_size_mb: 20,
            max_text_size_mb: 10,
            max_json_depth: 64,
            max_json_array_items: 10000,
            max_table_rows: 100000,
            max_table_columns: 1000,
            max_pdf_pages: 2000,
            max_pdf_extracted_text_mb: 20,
            max_archive_entries: 10000,
            max_docx_uncompressed_mb: 100,
            max_archive_compression_ratio: 100.0,
            max_concurrent_document_parsers: 4,
            document_parse_timeout_seconds: 30,
            follow_feed_links: false,
            follow_document_links: false,
            deduplicate_content: false,

            // Phase 5 defaults
            max_schema_depth: 20,
            max_schema_properties: 500,
            max_extraction_context_chars: 100000,
            max_extraction_chunks: 50,
            max_chunk_chars: 12000,
            llm_enabled: false,
            llm_provider: "openai-compatible".to_string(),
            llm_base_url: None,
            llm_api_key: None,
            llm_model: None,
            llm_timeout_seconds: 60,
            llm_max_retries: 2,
            max_concurrent_llm_requests: 5,
            max_extraction_repair_attempts: 1,
            max_llm_calls_per_extraction: 5,
            max_llm_calls_per_job: 50,
            extraction_cache_enabled: true,
            extraction_cache_max_items: 1000,

            // Phase 6 defaults
            max_sitemap_files: 1000,
            max_sitemap_urls: 1000000,
            max_sitemap_depth: 10,
            max_sitemap_file_size_mb: 50,
            max_concurrent_discovery_fetches: 100,
            max_map_urls: 100000,
            max_anchor_text_length: 512,
            max_page_links_discovered: 10000,
            search_enabled: true,
            search_provider: "mock".to_string(),
            search_base_url: None,
            search_api_key: None,
            max_search_results: 100,
            max_search_scrape_results: 25,
            max_concurrent_search_scrapes: 10,
            search_timeout_seconds: 30,
            cache_enabled: true,
            cache_max_entries: 10000,
            cache_default_ttl_seconds: 3600,
            robots_cache_ttl_seconds: 86400,
            sitemap_cache_ttl_seconds: 3600,
            map_cache_ttl_seconds: 1800,

            // Phase 7 defaults
            execution_mode: "standalone".to_string(),
            database_url: None,
            redis_url: None,
            queue_backend: "local".to_string(),
            result_store: "local".to_string(),
            s3_endpoint: None,
            s3_bucket: None,
            s3_region: None,
            s3_access_key: None,
            s3_secret_key: None,
            task_lease_seconds: 60,
            task_heartbeat_seconds: 20,
            task_max_attempts: 5,
            task_retry_base_ms: 1000,
            task_retry_max_ms: 60000,
            worker_heartbeat_seconds: 10,
            worker_dead_after_seconds: 45,
            webhook_max_attempts: 8,
            webhook_timeout_seconds: 10,
            webhook_signing_secret: None,
            shutdown_grace_seconds: 30,
            database_max_connections: 20,
            database_min_connections: 2,
            database_acquire_timeout_seconds: 5,
            job_event_retention_seconds: 86400,
            max_events_per_job: 10000,
            job_retention_days: 7,
            result_retention_days: 7,
            snapshot_retention_days: 30,
            api_key_auth_enabled: false,

            // Phase 8 Semantic Knowledge Engine defaults
            semantic_indexing_enabled: false,
            vector_backend: "memory".to_string(),
            lexical_backend: "memory".to_string(),
            embeddings_enabled: false,
            embedding_provider: "openai".to_string(),
            embedding_base_url: None,
            embedding_api_key: None,
            embedding_model: Some("text-embedding-3-small".to_string()),
            embedding_dimensions: Some(1536),
            embedding_batch_size: 32,
            max_concurrent_embedding_requests: 4,
            embedding_timeout_seconds: 60,
            target_chunk_tokens: 500,
            max_chunk_tokens: 800,
            min_chunk_tokens: 100,
            chunk_overlap_tokens: 80,
            rag_max_context_tokens: 16000,
            rag_max_chunks: 20,
            rag_min_score: None,
            max_chunks_per_index_job: 1000000,
            max_embedding_batch_tokens: 20000,
            max_embedding_calls_per_job: 10000,
            max_query_expansions: 3,
            max_chunks_per_document: 3,

            // Phase 9 Autonomous Web Research Agent defaults
            agent_enabled: false,
            agent_max_steps: 30,
            agent_max_pages: 200,
            agent_max_search_queries: 10,
            agent_max_query_variants_per_subgoal: 3,
            agent_max_research_rounds: 3,
            agent_max_browser_pages: 20,
            agent_max_browser_actions_total: 50,
            agent_max_llm_calls: 20,
            agent_max_duration_seconds: 300,
            agent_min_source_diversity: 2,
            agent_planner_model: None,
            agent_evaluator_model: None,
            agent_synthesis_model: None,
            agent_result_retention_days: 7,
            agent_evidence_retention_days: 7,
            mcp_enabled: false,
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        let default = Self::default();

        let server_host = std::env::var("SERVER_HOST")
            .ok()
            .and_then(|s| s.parse::<IpAddr>().ok())
            .unwrap_or(default.server_host);

        let server_port = std::env::var("SERVER_PORT")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(default.server_port);

        let request_timeout_seconds = std::env::var("REQUEST_TIMEOUT_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.request_timeout_seconds);

        let max_response_size_mb = std::env::var("MAX_RESPONSE_SIZE_MB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_response_size_mb);

        let max_redirects = std::env::var("MAX_REDIRECTS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_redirects);

        let max_concurrent_scrapes = std::env::var("MAX_CONCURRENT_SCRAPES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_concurrent_scrapes);

        let user_agent = std::env::var("USER_AGENT").unwrap_or(default.user_agent);
        let rust_log = std::env::var("RUST_LOG").unwrap_or(default.rust_log);

        let max_concurrent_fetches = std::env::var("MAX_CONCURRENT_FETCHES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_concurrent_fetches);

        let max_concurrent_fetches_per_host = std::env::var("MAX_CONCURRENT_FETCHES_PER_HOST")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_concurrent_fetches_per_host);

        let default_crawl_limit = std::env::var("DEFAULT_CRAWL_LIMIT")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.default_crawl_limit);

        let default_max_depth = std::env::var("DEFAULT_MAX_DEPTH")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(default.default_max_depth);

        let max_frontier_size = std::env::var("MAX_FRONTIER_SIZE")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_frontier_size);

        let default_max_retries = std::env::var("DEFAULT_MAX_RETRIES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.default_max_retries);

        let default_request_delay_ms = std::env::var("DEFAULT_REQUEST_DELAY_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.default_request_delay_ms);

        let max_stored_jobs = std::env::var("MAX_STORED_JOBS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_stored_jobs);

        let job_retention_seconds = std::env::var("JOB_RETENTION_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.job_retention_seconds);

        let max_crawl_duration_seconds = std::env::var("MAX_CRAWL_DURATION_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.max_crawl_duration_seconds);

        let allow_private_networks = std::env::var("ALLOW_PRIVATE_NETWORKS")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(default.allow_private_networks);

        let browser_enabled = std::env::var("BROWSER_ENABLED")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(default.browser_enabled);

        let browser_executable_path = std::env::var("BROWSER_EXECUTABLE_PATH")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let browser_headless = std::env::var("BROWSER_HEADLESS")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(default.browser_headless);

        let max_browser_pages = std::env::var("MAX_BROWSER_PAGES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_browser_pages);

        let max_browser_queue_size = std::env::var("MAX_BROWSER_QUEUE_SIZE")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_browser_queue_size);

        let browser_acquire_timeout_seconds = std::env::var("BROWSER_ACQUIRE_TIMEOUT_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.browser_acquire_timeout_seconds);

        let browser_navigation_timeout_seconds =
            std::env::var("BROWSER_NAVIGATION_TIMEOUT_SECONDS")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(default.browser_navigation_timeout_seconds);

        let browser_total_timeout_seconds = std::env::var("BROWSER_TOTAL_TIMEOUT_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.browser_total_timeout_seconds);

        let browser_idle_timeout_seconds = std::env::var("BROWSER_IDLE_TIMEOUT_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.browser_idle_timeout_seconds);

        let render_auto_threshold = std::env::var("RENDER_AUTO_THRESHOLD")
            .ok()
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(default.render_auto_threshold);

        let network_idle_quiet_ms = std::env::var("NETWORK_IDLE_QUIET_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.network_idle_quiet_ms);

        let auto_scroll_max_steps = std::env::var("AUTO_SCROLL_MAX_STEPS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.auto_scroll_max_steps);

        let auto_scroll_delay_ms = std::env::var("AUTO_SCROLL_DELAY_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.auto_scroll_delay_ms);

        let max_browser_actions = std::env::var("MAX_BROWSER_ACTIONS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_browser_actions);

        let max_action_wait_seconds = std::env::var("MAX_ACTION_WAIT_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.max_action_wait_seconds);

        let max_network_responses = std::env::var("MAX_NETWORK_RESPONSES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_network_responses);

        let max_network_response_bytes = std::env::var("MAX_NETWORK_RESPONSE_BYTES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_network_response_bytes);

        let max_total_network_capture_bytes = std::env::var("MAX_TOTAL_NETWORK_CAPTURE_BYTES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_total_network_capture_bytes);

        let max_document_size_mb = std::env::var("MAX_DOCUMENT_SIZE_MB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_document_size_mb);

        let max_pdf_size_mb = std::env::var("MAX_PDF_SIZE_MB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_pdf_size_mb);

        let max_docx_size_mb = std::env::var("MAX_DOCX_SIZE_MB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_docx_size_mb);

        let max_json_size_mb = std::env::var("MAX_JSON_SIZE_MB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_json_size_mb);

        let max_xml_size_mb = std::env::var("MAX_XML_SIZE_MB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_xml_size_mb);

        let max_csv_size_mb = std::env::var("MAX_CSV_SIZE_MB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_csv_size_mb);

        let max_text_size_mb = std::env::var("MAX_TEXT_SIZE_MB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_text_size_mb);

        let max_json_depth = std::env::var("MAX_JSON_DEPTH")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_json_depth);

        let max_json_array_items = std::env::var("MAX_JSON_ARRAY_ITEMS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_json_array_items);

        let max_table_rows = std::env::var("MAX_TABLE_ROWS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_table_rows);

        let max_table_columns = std::env::var("MAX_TABLE_COLUMNS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_table_columns);

        let max_pdf_pages = std::env::var("MAX_PDF_PAGES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_pdf_pages);

        let max_pdf_extracted_text_mb = std::env::var("MAX_PDF_EXTRACTED_TEXT_MB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_pdf_extracted_text_mb);

        let max_archive_entries = std::env::var("MAX_ARCHIVE_ENTRIES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_archive_entries);

        let max_docx_uncompressed_mb = std::env::var("MAX_DOCX_UNCOMPRESSED_MB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_docx_uncompressed_mb);

        let max_archive_compression_ratio = std::env::var("MAX_ARCHIVE_COMPRESSION_RATIO")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(default.max_archive_compression_ratio);

        let max_concurrent_document_parsers = std::env::var("MAX_CONCURRENT_DOCUMENT_PARSERS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_concurrent_document_parsers);

        let document_parse_timeout_seconds = std::env::var("DOCUMENT_PARSE_TIMEOUT_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.document_parse_timeout_seconds);

        let follow_feed_links = std::env::var("FOLLOW_FEED_LINKS")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(default.follow_feed_links);

        let follow_document_links = std::env::var("FOLLOW_DOCUMENT_LINKS")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(default.follow_document_links);

        let deduplicate_content = std::env::var("DEDUPLICATE_CONTENT")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(default.deduplicate_content);

        // Phase 5 env vars
        let max_schema_depth = std::env::var("MAX_SCHEMA_DEPTH")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_schema_depth);

        let max_schema_properties = std::env::var("MAX_SCHEMA_PROPERTIES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_schema_properties);

        let max_extraction_context_chars = std::env::var("MAX_EXTRACTION_CONTEXT_CHARS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_extraction_context_chars);

        let max_extraction_chunks = std::env::var("MAX_EXTRACTION_CHUNKS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_extraction_chunks);

        let max_chunk_chars = std::env::var("MAX_CHUNK_CHARS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_chunk_chars);

        let llm_enabled = std::env::var("LLM_ENABLED")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(default.llm_enabled);

        let llm_provider = std::env::var("LLM_PROVIDER").unwrap_or(default.llm_provider);

        let llm_base_url = std::env::var("LLM_BASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let llm_api_key = std::env::var("LLM_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let llm_model = std::env::var("LLM_MODEL")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let llm_timeout_seconds = std::env::var("LLM_TIMEOUT_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.llm_timeout_seconds);

        let llm_max_retries = std::env::var("LLM_MAX_RETRIES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.llm_max_retries);

        let max_concurrent_llm_requests = std::env::var("MAX_CONCURRENT_LLM_REQUESTS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_concurrent_llm_requests);

        let max_extraction_repair_attempts = std::env::var("MAX_EXTRACTION_REPAIR_ATTEMPTS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_extraction_repair_attempts);

        let max_llm_calls_per_extraction = std::env::var("MAX_LLM_CALLS_PER_EXTRACTION")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_llm_calls_per_extraction);

        let max_llm_calls_per_job = std::env::var("MAX_LLM_CALLS_PER_JOB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_llm_calls_per_job);

        let extraction_cache_enabled = std::env::var("EXTRACTION_CACHE_ENABLED")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(default.extraction_cache_enabled);

        let extraction_cache_max_items = std::env::var("EXTRACTION_CACHE_MAX_ITEMS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.extraction_cache_max_items);

        // Phase 6 env parsing
        let max_sitemap_files = std::env::var("MAX_SITEMAP_FILES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_sitemap_files);

        let max_sitemap_urls = std::env::var("MAX_SITEMAP_URLS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_sitemap_urls);

        let max_sitemap_depth = std::env::var("MAX_SITEMAP_DEPTH")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_sitemap_depth);

        let max_sitemap_file_size_mb = std::env::var("MAX_SITEMAP_FILE_SIZE_MB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_sitemap_file_size_mb);

        let max_concurrent_discovery_fetches = std::env::var("MAX_CONCURRENT_DISCOVERY_FETCHES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_concurrent_discovery_fetches);

        let max_map_urls = std::env::var("MAX_MAP_URLS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_map_urls);

        let max_anchor_text_length = std::env::var("MAX_ANCHOR_TEXT_LENGTH")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_anchor_text_length);

        let max_page_links_discovered = std::env::var("MAX_PAGE_LINKS_DISCOVERED")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_page_links_discovered);

        let search_enabled = std::env::var("SEARCH_ENABLED")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(default.search_enabled);

        let search_provider = std::env::var("SEARCH_PROVIDER").unwrap_or(default.search_provider);

        let search_base_url = std::env::var("SEARCH_BASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let search_api_key = std::env::var("SEARCH_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let max_search_results = std::env::var("MAX_SEARCH_RESULTS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_search_results);

        let max_search_scrape_results = std::env::var("MAX_SEARCH_SCRAPE_RESULTS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_search_scrape_results);

        let max_concurrent_search_scrapes = std::env::var("MAX_CONCURRENT_SEARCH_SCRAPES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_concurrent_search_scrapes);

        let search_timeout_seconds = std::env::var("SEARCH_TIMEOUT_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.search_timeout_seconds);

        let cache_enabled = std::env::var("CACHE_ENABLED")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(default.cache_enabled);

        let cache_max_entries = std::env::var("CACHE_MAX_ENTRIES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.cache_max_entries);

        let cache_default_ttl_seconds = std::env::var("CACHE_DEFAULT_TTL_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.cache_default_ttl_seconds);

        let robots_cache_ttl_seconds = std::env::var("ROBOTS_CACHE_TTL_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.robots_cache_ttl_seconds);

        let sitemap_cache_ttl_seconds = std::env::var("SITEMAP_CACHE_TTL_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.sitemap_cache_ttl_seconds);

        let map_cache_ttl_seconds = std::env::var("MAP_CACHE_TTL_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.map_cache_ttl_seconds);

        // Phase 7 env vars
        let execution_mode = std::env::var("EXECUTION_MODE").unwrap_or(default.execution_mode);
        let database_url = std::env::var("DATABASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let redis_url = std::env::var("REDIS_URL")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let queue_backend = std::env::var("QUEUE_BACKEND").unwrap_or(default.queue_backend);
        let result_store = std::env::var("RESULT_STORE").unwrap_or(default.result_store);
        let s3_endpoint = std::env::var("S3_ENDPOINT")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let s3_bucket = std::env::var("S3_BUCKET")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let s3_region = std::env::var("S3_REGION")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let s3_access_key = std::env::var("S3_ACCESS_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let s3_secret_key = std::env::var("S3_SECRET_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let task_lease_seconds = std::env::var("TASK_LEASE_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.task_lease_seconds);

        let task_heartbeat_seconds = std::env::var("TASK_HEARTBEAT_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.task_heartbeat_seconds);

        let task_max_attempts = std::env::var("TASK_MAX_ATTEMPTS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.task_max_attempts);

        let task_retry_base_ms = std::env::var("TASK_RETRY_BASE_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.task_retry_base_ms);

        let task_retry_max_ms = std::env::var("TASK_RETRY_MAX_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.task_retry_max_ms);

        let worker_heartbeat_seconds = std::env::var("WORKER_HEARTBEAT_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.worker_heartbeat_seconds);

        let worker_dead_after_seconds = std::env::var("WORKER_DEAD_AFTER_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.worker_dead_after_seconds);

        let webhook_max_attempts = std::env::var("WEBHOOK_MAX_ATTEMPTS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.webhook_max_attempts);

        let webhook_timeout_seconds = std::env::var("WEBHOOK_TIMEOUT_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.webhook_timeout_seconds);

        let webhook_signing_secret = std::env::var("WEBHOOK_SIGNING_SECRET")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let shutdown_grace_seconds = std::env::var("SHUTDOWN_GRACE_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.shutdown_grace_seconds);

        let database_max_connections = std::env::var("DATABASE_MAX_CONNECTIONS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(default.database_max_connections);

        let database_min_connections = std::env::var("DATABASE_MIN_CONNECTIONS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(default.database_min_connections);

        let database_acquire_timeout_seconds = std::env::var("DATABASE_ACQUIRE_TIMEOUT_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.database_acquire_timeout_seconds);

        let job_event_retention_seconds = std::env::var("JOB_EVENT_RETENTION_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.job_event_retention_seconds);

        let max_events_per_job = std::env::var("MAX_EVENTS_PER_JOB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_events_per_job);

        let job_retention_days = std::env::var("JOB_RETENTION_DAYS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.job_retention_days);

        let result_retention_days = std::env::var("RESULT_RETENTION_DAYS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.result_retention_days);

        let snapshot_retention_days = std::env::var("SNAPSHOT_RETENTION_DAYS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.snapshot_retention_days);

        let api_key_auth_enabled = std::env::var("API_KEY_AUTH_ENABLED")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(default.api_key_auth_enabled);

        // Phase 8 Semantic Knowledge Engine env vars
        let semantic_indexing_enabled = std::env::var("SEMANTIC_INDEXING_ENABLED")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(default.semantic_indexing_enabled);

        let vector_backend = std::env::var("VECTOR_BACKEND").unwrap_or(default.vector_backend);
        let lexical_backend = std::env::var("LEXICAL_BACKEND").unwrap_or(default.lexical_backend);

        let embeddings_enabled = std::env::var("EMBEDDINGS_ENABLED")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(default.embeddings_enabled);

        let embedding_provider =
            std::env::var("EMBEDDING_PROVIDER").unwrap_or(default.embedding_provider);
        let embedding_base_url = std::env::var("EMBEDDING_BASE_URL").ok();
        let embedding_api_key = std::env::var("EMBEDDING_API_KEY").ok();
        let embedding_model = std::env::var("EMBEDDING_MODEL")
            .ok()
            .or(default.embedding_model);

        let embedding_dimensions = std::env::var("EMBEDDING_DIMENSIONS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .or(default.embedding_dimensions);

        let embedding_batch_size = std::env::var("EMBEDDING_BATCH_SIZE")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.embedding_batch_size);

        let max_concurrent_embedding_requests = std::env::var("MAX_CONCURRENT_EMBEDDING_REQUESTS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_concurrent_embedding_requests);

        let embedding_timeout_seconds = std::env::var("EMBEDDING_TIMEOUT_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.embedding_timeout_seconds);

        let target_chunk_tokens = std::env::var("TARGET_CHUNK_TOKENS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.target_chunk_tokens);

        let max_chunk_tokens = std::env::var("MAX_CHUNK_TOKENS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_chunk_tokens);

        let min_chunk_tokens = std::env::var("MIN_CHUNK_TOKENS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.min_chunk_tokens);

        let chunk_overlap_tokens = std::env::var("CHUNK_OVERLAP_TOKENS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.chunk_overlap_tokens);

        let rag_max_context_tokens = std::env::var("RAG_MAX_CONTEXT_TOKENS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.rag_max_context_tokens);

        let rag_max_chunks = std::env::var("RAG_MAX_CHUNKS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.rag_max_chunks);

        let rag_min_score = std::env::var("RAG_MIN_SCORE")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .or(default.rag_min_score);

        let max_chunks_per_index_job = std::env::var("MAX_CHUNKS_PER_INDEX_JOB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_chunks_per_index_job);

        let max_embedding_batch_tokens = std::env::var("MAX_EMBEDDING_BATCH_TOKENS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_embedding_batch_tokens);

        let max_embedding_calls_per_job = std::env::var("MAX_EMBEDDING_CALLS_PER_JOB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_embedding_calls_per_job);

        let max_query_expansions = std::env::var("MAX_QUERY_EXPANSIONS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_query_expansions);

        let max_chunks_per_document = std::env::var("MAX_CHUNKS_PER_DOCUMENT")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.max_chunks_per_document);

        // Phase 9 Autonomous Web Research Agent environment variables
        let agent_enabled = std::env::var("AGENT_ENABLED")
            .map(|s| s.to_lowercase() == "true" || s == "1")
            .unwrap_or(default.agent_enabled);

        let agent_max_steps = std::env::var("AGENT_MAX_STEPS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(default.agent_max_steps);

        let agent_max_pages = std::env::var("AGENT_MAX_PAGES")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(default.agent_max_pages);

        let agent_max_search_queries = std::env::var("AGENT_MAX_SEARCH_QUERIES")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(default.agent_max_search_queries);

        let agent_max_query_variants_per_subgoal =
            std::env::var("AGENT_MAX_QUERY_VARIANTS_PER_SUBGOAL")
                .ok()
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(default.agent_max_query_variants_per_subgoal);

        let agent_max_research_rounds = std::env::var("AGENT_MAX_RESEARCH_ROUNDS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(default.agent_max_research_rounds);

        let agent_max_browser_pages = std::env::var("AGENT_MAX_BROWSER_PAGES")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(default.agent_max_browser_pages);

        let agent_max_browser_actions_total = std::env::var("AGENT_MAX_BROWSER_ACTIONS_TOTAL")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(default.agent_max_browser_actions_total);

        let agent_max_llm_calls = std::env::var("AGENT_MAX_LLM_CALLS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(default.agent_max_llm_calls);

        let agent_max_duration_seconds = std::env::var("AGENT_MAX_DURATION_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(default.agent_max_duration_seconds);

        let agent_min_source_diversity = std::env::var("AGENT_MIN_SOURCE_DIVERSITY")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default.agent_min_source_diversity);

        let agent_planner_model = std::env::var("AGENT_PLANNER_MODEL")
            .ok()
            .or(default.agent_planner_model);
        let agent_evaluator_model = std::env::var("AGENT_EVALUATOR_MODEL")
            .ok()
            .or(default.agent_evaluator_model);
        let agent_synthesis_model = std::env::var("AGENT_SYNTHESIS_MODEL")
            .ok()
            .or(default.agent_synthesis_model);

        let agent_result_retention_days = std::env::var("AGENT_RESULT_RETENTION_DAYS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(default.agent_result_retention_days);

        let agent_evidence_retention_days = std::env::var("AGENT_EVIDENCE_RETENTION_DAYS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(default.agent_evidence_retention_days);

        let mcp_enabled = std::env::var("MCP_ENABLED")
            .map(|s| s.to_lowercase() == "true" || s == "1")
            .unwrap_or(default.mcp_enabled);

        Self {
            server_host,
            server_port,
            request_timeout_seconds,
            max_response_size_mb,
            max_redirects,
            max_concurrent_scrapes,
            user_agent,
            rust_log,
            max_concurrent_fetches,
            max_concurrent_fetches_per_host,
            default_crawl_limit,
            default_max_depth,
            max_frontier_size,
            default_max_retries,
            default_request_delay_ms,
            max_stored_jobs,
            job_retention_seconds,
            max_crawl_duration_seconds,
            allow_private_networks,
            browser_enabled,
            browser_executable_path,
            browser_headless,
            max_browser_pages,
            max_browser_queue_size,
            browser_acquire_timeout_seconds,
            browser_navigation_timeout_seconds,
            browser_total_timeout_seconds,
            browser_idle_timeout_seconds,
            render_auto_threshold,
            network_idle_quiet_ms,
            auto_scroll_max_steps,
            auto_scroll_delay_ms,
            max_browser_actions,
            max_action_wait_seconds,
            max_network_responses,
            max_network_response_bytes,
            max_total_network_capture_bytes,
            max_document_size_mb,
            max_pdf_size_mb,
            max_docx_size_mb,
            max_json_size_mb,
            max_xml_size_mb,
            max_csv_size_mb,
            max_text_size_mb,
            max_json_depth,
            max_json_array_items,
            max_table_rows,
            max_table_columns,
            max_pdf_pages,
            max_pdf_extracted_text_mb,
            max_archive_entries,
            max_docx_uncompressed_mb,
            max_archive_compression_ratio,
            max_concurrent_document_parsers,
            document_parse_timeout_seconds,
            follow_feed_links,
            follow_document_links,
            deduplicate_content,
            max_schema_depth,
            max_schema_properties,
            max_extraction_context_chars,
            max_extraction_chunks,
            max_chunk_chars,
            llm_enabled,
            llm_provider,
            llm_base_url,
            llm_api_key,
            llm_model,
            llm_timeout_seconds,
            llm_max_retries,
            max_concurrent_llm_requests,
            max_extraction_repair_attempts,
            max_llm_calls_per_extraction,
            max_llm_calls_per_job,
            extraction_cache_enabled,
            extraction_cache_max_items,
            max_sitemap_files,
            max_sitemap_urls,
            max_sitemap_depth,
            max_sitemap_file_size_mb,
            max_concurrent_discovery_fetches,
            max_map_urls,
            max_anchor_text_length,
            max_page_links_discovered,
            search_enabled,
            search_provider,
            search_base_url,
            search_api_key,
            max_search_results,
            max_search_scrape_results,
            max_concurrent_search_scrapes,
            search_timeout_seconds,
            cache_enabled,
            cache_max_entries,
            cache_default_ttl_seconds,
            robots_cache_ttl_seconds,
            sitemap_cache_ttl_seconds,
            map_cache_ttl_seconds,
            execution_mode,
            database_url,
            redis_url,
            queue_backend,
            result_store,
            s3_endpoint,
            s3_bucket,
            s3_region,
            s3_access_key,
            s3_secret_key,
            task_lease_seconds,
            task_heartbeat_seconds,
            task_max_attempts,
            task_retry_base_ms,
            task_retry_max_ms,
            worker_heartbeat_seconds,
            worker_dead_after_seconds,
            webhook_max_attempts,
            webhook_timeout_seconds,
            webhook_signing_secret,
            shutdown_grace_seconds,
            database_max_connections,
            database_min_connections,
            database_acquire_timeout_seconds,
            job_event_retention_seconds,
            max_events_per_job,
            job_retention_days,
            result_retention_days,
            snapshot_retention_days,
            api_key_auth_enabled,
            semantic_indexing_enabled,
            vector_backend,
            lexical_backend,
            embeddings_enabled,
            embedding_provider,
            embedding_base_url,
            embedding_api_key,
            embedding_model,
            embedding_dimensions,
            embedding_batch_size,
            max_concurrent_embedding_requests,
            embedding_timeout_seconds,
            target_chunk_tokens,
            max_chunk_tokens,
            min_chunk_tokens,
            chunk_overlap_tokens,
            rag_max_context_tokens,
            rag_max_chunks,
            rag_min_score,
            max_chunks_per_index_job,
            max_embedding_batch_tokens,
            max_embedding_calls_per_job,
            max_query_expansions,
            max_chunks_per_document,
            agent_enabled,
            agent_max_steps,
            agent_max_pages,
            agent_max_search_queries,
            agent_max_query_variants_per_subgoal,
            agent_max_research_rounds,
            agent_max_browser_pages,
            agent_max_browser_actions_total,
            agent_max_llm_calls,
            agent_max_duration_seconds,
            agent_min_source_diversity,
            agent_planner_model,
            agent_evaluator_model,
            agent_synthesis_model,
            agent_result_retention_days,
            agent_evidence_retention_days,
            mcp_enabled,
        }
    }

    pub fn max_response_size_bytes(&self) -> usize {
        self.max_response_size_mb * 1024 * 1024
    }

    pub fn max_sitemap_file_size_bytes(&self) -> usize {
        self.max_sitemap_file_size_mb * 1024 * 1024
    }

    pub fn max_pdf_extracted_text_bytes(&self) -> usize {
        self.max_pdf_extracted_text_mb * 1024 * 1024
    }

    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.request_timeout_seconds)
    }

    pub fn search_timeout(&self) -> Duration {
        Duration::from_secs(self.search_timeout_seconds)
    }

    pub fn browser_acquire_timeout(&self) -> Duration {
        Duration::from_secs(self.browser_acquire_timeout_seconds)
    }

    pub fn browser_navigation_timeout(&self) -> Duration {
        Duration::from_secs(self.browser_navigation_timeout_seconds)
    }

    pub fn browser_total_timeout(&self) -> Duration {
        Duration::from_secs(self.browser_total_timeout_seconds)
    }

    pub fn document_parse_timeout(&self) -> Duration {
        Duration::from_secs(self.document_parse_timeout_seconds)
    }

    pub fn llm_timeout(&self) -> Duration {
        Duration::from_secs(self.llm_timeout_seconds)
    }

    pub fn agent_timeout(&self) -> Duration {
        Duration::from_secs(self.agent_max_duration_seconds)
    }

    pub fn server_addr(&self) -> std::net::SocketAddr {
        std::net::SocketAddr::new(self.server_host, self.server_port)
    }
}
