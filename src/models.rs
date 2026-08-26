use crate::document::models::{DocumentPage, DocumentTable};
use crate::render::models::{
    BrowserAction, NetworkResponse, RenderDiagnostics, RenderMode, WaitStrategy,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Markdown,
    Html,
    CleanHtml,
    Links,
    Images,
    Metadata,
    Text,
    Json,
    Tables,
    Pages,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "markdown" | "md" => Ok(OutputFormat::Markdown),
            "html" | "raw_html" => Ok(OutputFormat::Html),
            "clean_html" | "clean-html" | "cleaned_html" => Ok(OutputFormat::CleanHtml),
            "links" | "link" => Ok(OutputFormat::Links),
            "images" | "image" | "imgs" => Ok(OutputFormat::Images),
            "metadata" | "meta" => Ok(OutputFormat::Metadata),
            "text" | "txt" | "plain_text" => Ok(OutputFormat::Text),
            "json" | "structured" => Ok(OutputFormat::Json),
            "tables" | "table" | "csv" => Ok(OutputFormat::Tables),
            "pages" | "page" => Ok(OutputFormat::Pages),
            _ => Err(format!("Unknown output format: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeOptions {
    #[serde(default = "default_formats")]
    pub formats: Vec<OutputFormat>,
    #[serde(default = "default_true")]
    pub only_main_content: bool,
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
    #[serde(default)]
    pub block_resources: Vec<String>,
    #[serde(default)]
    pub actions: Vec<BrowserAction>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,

    // Phase 4 additions
    #[serde(default)]
    pub json_path: Option<String>,
    #[serde(default)]
    pub follow_feed_links: Option<bool>,
    #[serde(default)]
    pub follow_document_links: Option<bool>,
}

fn default_formats() -> Vec<OutputFormat> {
    vec![OutputFormat::Markdown]
}

fn default_true() -> bool {
    true
}

impl Default for ScrapeOptions {
    fn default() -> Self {
        Self {
            formats: default_formats(),
            only_main_content: true,
            render_mode: RenderMode::Auto,
            wait_for: None,
            auto_scroll: false,
            capture_network: false,
            block_trackers: false,
            block_resources: Vec::new(),
            actions: Vec::new(),
            timeout_ms: None,
            json_path: None,
            follow_feed_links: None,
            follow_document_links: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScrapeRequest {
    pub url: String,
    #[serde(default)]
    pub formats: Option<Vec<OutputFormat>>,
    #[serde(default)]
    pub only_main_content: Option<bool>,
    #[serde(default)]
    pub render_mode: Option<RenderMode>,
    #[serde(default)]
    pub wait_for: Option<WaitStrategy>,
    #[serde(default)]
    pub auto_scroll: Option<bool>,
    #[serde(default)]
    pub capture_network: Option<bool>,
    #[serde(default)]
    pub block_trackers: Option<bool>,
    #[serde(default)]
    pub block_resources: Option<Vec<String>>,
    #[serde(default)]
    pub actions: Option<Vec<BrowserAction>>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,

    // Phase 4 additions
    #[serde(default)]
    pub json_path: Option<String>,
    #[serde(default)]
    pub follow_feed_links: Option<bool>,
    #[serde(default)]
    pub follow_document_links: Option<bool>,
}

impl ScrapeRequest {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            ..Default::default()
        }
    }

    pub fn resolved_options(&self) -> ScrapeOptions {
        ScrapeOptions {
            formats: self.formats.clone().unwrap_or_else(default_formats),
            only_main_content: self.only_main_content.unwrap_or(true),
            render_mode: self.render_mode.unwrap_or_default(),
            wait_for: self.wait_for.clone(),
            auto_scroll: self.auto_scroll.unwrap_or(false),
            capture_network: self.capture_network.unwrap_or(false),
            block_trackers: self.block_trackers.unwrap_or(false),
            block_resources: self.block_resources.clone().unwrap_or_default(),
            actions: self.actions.clone().unwrap_or_default(),
            timeout_ms: self.timeout_ms,
            json_path: self.json_path.clone(),
            follow_feed_links: self.follow_feed_links,
            follow_document_links: self.follow_document_links,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WarningCode {
    InvalidJsonLd,
    MissingTitle,
    MissingMainContent,
    InvalidCanonicalUrl,
    MalformedLink,
    CharsetFallback,

    // Phase 3 Browser warning codes
    BrowserFallbackUsed,
    BrowserFallbackFailed,
    NetworkResponseTruncated,
    AutoScrollLimitReached,
    ActionSkipped,
    SelectorTimeout,
    RenderTimeoutPartialContent,

    // Phase 4 Document warning codes
    ContentTypeMismatch,
    ContentTypeInferred,
    TruncatedDocument,
    TruncatedTable,
    MalformedRow,
    InvalidEncodingSequence,
    PdfMetadataMissing,
    PdfTextExtractionPartial,
    ScannedPdfDetected,
    DocxUnsupportedElement,
    DuplicateContent,

    // Phase 5 Extraction warning codes
    ExtractionContextTruncated,
    LowConfidenceField,
    MissingRequiredSourceValue,
    DeterministicExtractionIncomplete,
    LlmFallbackUsed,
    LlmRepairUsed,
    ProvenanceUnavailable,
    DuplicateEntityRemoved,

    // Phase 6 Map, Search, Cache & Incremental warning codes
    SitemapTruncated,
    MapResultsTruncated,
    DiscoveryPageLimitReached,
    SearchResultsDeduplicated,
    CacheEntryStale,
    ConditionalRequestUnsupported,
    ChangeStatusUnknown,
    LocalIndexPartial,

    // Phase 8 Semantic Knowledge Engine warning codes
    ChunkTruncated,
    DuplicateChunkSkipped,
    NearDuplicateSkipped,
    EmbeddingCacheHit,
    LexicalOnlyFallback,
    SemanticSearchUnavailable,
    LowRetrievalConfidence,
    ContextTruncated,
    AnswerInsufficientEvidence,

    // Phase 9 Autonomous Web Research Agent warning codes
    ResearchBudgetNearLimit,
    InsufficientSourceDiversity,
    LowQualitySource,
    ConflictingEvidence,
    EvidenceGapUnresolved,
    FreshSourceNotFound,
    CitationRepaired,
    PartialResearchResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScrapeWarning {
    pub code: WarningCode,
    pub message: String,
}

impl ScrapeWarning {
    pub fn new(code: WarningCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageLink {
    pub text: String,
    pub url: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub rel: Vec<String>,
    pub is_external: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageImage {
    pub src: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub srcset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_src: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loading: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenGraphMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub og_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TwitterMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favicon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub robots: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_graph: Option<OpenGraphMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitter: Option<TwitterMetadata>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub json_ld: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub custom: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpMetadata {
    pub status: u16,
    pub content_type: String,
    pub content_length: usize,
    pub fetch_time_ms: u64,
    pub parse_time_ms: u64,
    pub extract_time_ms: u64,
    pub total_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeResult {
    pub url: String,
    pub final_url: String,
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
    pub http: HttpMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renderer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render_diagnostics: Option<RenderDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_responses: Option<Vec<NetworkResponse>>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub warnings: Vec<ScrapeWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeResponse {
    pub success: bool,
    pub data: ScrapeResult,
}

impl ScrapeResponse {
    pub fn success(data: ScrapeResult) -> Self {
        Self {
            success: true,
            data,
        }
    }
}
