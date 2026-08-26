use crate::document::models::{DocumentPage, DocumentTable};
use crate::models::{Heading, OutputFormat, PageImage, PageLink, PageMetadata, ScrapeWarning};
use crate::render::models::{NetworkResponse, RenderMode, WaitStrategy};
use serde::{Deserialize, Serialize};
use url::Url;

fn default_crawl_limit() -> usize {
    100
}

fn default_max_depth() -> u32 {
    3
}

fn default_max_duration() -> u64 {
    300
}

fn default_max_frontier_size() -> usize {
    10000
}

fn default_request_delay_ms() -> u64 {
    100
}

fn default_max_retries() -> usize {
    2
}

fn default_true() -> bool {
    true
}

fn default_document_types() -> Vec<String> {
    vec!["html".to_string()]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverySource {
    Seed,
    HtmlLink,
    Sitemap,
    Feed,
    Document,
    Redirect,
    Canonical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrawlTarget {
    pub url: Url,
    pub normalized_url: String,
    pub depth: u32,
    pub parent_url: Option<Url>,
    pub discovered_from: DiscoverySource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrawlStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlOptions {
    #[serde(default = "default_crawl_limit")]
    pub limit: usize,
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    #[serde(default = "default_max_duration")]
    pub max_duration_seconds: u64,
    #[serde(default = "default_max_frontier_size")]
    pub max_frontier_size: usize,
    #[serde(default)]
    pub max_pages_per_host: Option<usize>,
    #[serde(default)]
    pub allow_subdomains: bool,
    #[serde(default)]
    pub include_paths: Vec<String>,
    #[serde(default)]
    pub exclude_paths: Vec<String>,
    #[serde(default = "default_true")]
    pub respect_robots_txt: bool,
    #[serde(default = "default_true")]
    pub use_sitemap: bool,
    #[serde(default)]
    pub ignore_query_parameters: bool,
    #[serde(default = "default_true")]
    pub remove_tracking_parameters: bool,
    #[serde(default = "default_true")]
    pub respect_canonical: bool,
    #[serde(default = "default_request_delay_ms")]
    pub request_delay_ms: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,
    #[serde(default)]
    pub formats: Option<Vec<OutputFormat>>,
    #[serde(default)]
    pub only_main_content: Option<bool>,

    // Phase 3 Rendering options
    #[serde(default)]
    pub render_mode: RenderMode,
    #[serde(default)]
    pub wait_for: Option<WaitStrategy>,
    #[serde(default)]
    pub auto_scroll: bool,
    #[serde(default)]
    pub capture_network: bool,
    #[serde(default)]
    pub block_trackers: bool,

    // Phase 4 Document Crawling options
    #[serde(default = "default_document_types")]
    pub document_types: Vec<String>,
    #[serde(default)]
    pub follow_feed_links: bool,
    #[serde(default)]
    pub follow_document_links: bool,
    #[serde(default)]
    pub deduplicate_content: bool,
}

impl Default for CrawlOptions {
    fn default() -> Self {
        Self {
            limit: default_crawl_limit(),
            max_depth: default_max_depth(),
            max_duration_seconds: default_max_duration(),
            max_frontier_size: default_max_frontier_size(),
            max_pages_per_host: None,
            allow_subdomains: false,
            include_paths: Vec::new(),
            exclude_paths: Vec::new(),
            respect_robots_txt: true,
            use_sitemap: true,
            ignore_query_parameters: false,
            remove_tracking_parameters: true,
            respect_canonical: true,
            request_delay_ms: default_request_delay_ms(),
            max_retries: default_max_retries(),
            formats: None,
            only_main_content: None,
            render_mode: RenderMode::Auto,
            wait_for: None,
            auto_scroll: false,
            capture_network: false,
            block_trackers: false,
            document_types: default_document_types(),
            follow_feed_links: false,
            follow_document_links: false,
            deduplicate_content: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlRequest {
    pub url: String,
    #[serde(flatten)]
    pub options: CrawlOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageTimings {
    pub fetch_ms: u64,
    pub parse_ms: u64,
    pub extract_ms: u64,
    pub total_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawledPage {
    pub url: String,
    pub final_url: String,
    pub depth: u32,
    pub status_code: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clean_html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<PageMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headings: Option<Vec<Heading>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<PageLink>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<PageImage>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tables: Option<Vec<DocumentTable>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages: Option<Vec<DocumentPage>>,
    pub timings: PageTimings,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renderer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_responses: Option<Vec<NetworkResponse>>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub warnings: Vec<ScrapeWarning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CrawlStats {
    pub pages_discovered: usize,
    pub pages_queued: usize,
    pub pages_crawled: usize,
    pub pages_succeeded: usize,
    pub pages_failed: usize,
    pub pages_skipped: usize,
    pub pages_blocked_by_robots: usize,
    pub dedupe_hits: usize,
    pub retries_total: usize,
    pub bytes_downloaded: usize,
    pub current_depth: u32,
    pub elapsed_ms: u64,

    // Phase 3 Rendering stats
    pub http_pages: usize,
    pub browser_pages: usize,
    pub browser_fallback_count: usize,
    pub browser_render_time_ms: u64,

    // Phase 4 Document stats
    pub documents_processed: usize,
    pub duplicate_content_hits: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlResult {
    pub job_id: String,
    pub status: CrawlStatus,
    pub seed_url: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub stats: CrawlStats,
    pub pages: Vec<CrawledPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlJobInitResponse {
    pub success: bool,
    pub job_id: String,
    pub status: CrawlStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlJobStatusResponse {
    pub success: bool,
    pub data: CrawlResult,
}
