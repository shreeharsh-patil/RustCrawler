use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidUrl,
    UnsupportedScheme,
    PrivateNetworkBlocked,
    DnsResolutionFailed,
    ConnectionFailed,
    RequestTimeout,
    TooManyRedirects,
    ResponseTooLarge,
    UnsupportedContentType,
    InvalidHtml,
    ServerOverloaded,
    InternalError,

    // Phase 2 Crawl error codes
    InvalidCrawlConfig,
    CrawlLimitReached,
    CrawlTimeout,
    CrawlCancelled,
    FrontierFull,
    RobotsFetchFailed,
    SitemapParseFailed,
    JobNotFound,
    JobStoreFull,

    // Phase 3 Browser error codes
    BrowserDisabled,
    BrowserNotFound,
    BrowserLaunchFailed,
    BrowserUnavailable,
    BrowserPoolExhausted,
    BrowserNavigationFailed,
    BrowserTimeout,
    BrowserCrashed,
    SelectorNotFound,
    ActionFailed,
    NetworkCaptureLimitReached,
    RenderFailed,

    // Phase 4 Document Parser error codes
    ContentTypeDetectionFailed,
    DocumentTooLarge,
    JsonParseFailed,
    JsonTooDeep,
    XmlParseFailed,
    XmlTooDeep,
    CsvParseFailed,
    CsvLimitExceeded,
    PdfParseFailed,
    PdfEncrypted,
    PdfPageLimitExceeded,
    ScannedPdf,
    DocxParseFailed,
    DocxArchiveInvalid,
    ArchiveBombDetected,
    UnsupportedDocumentType,
    TextEncodingFailed,

    // Phase 5 Extraction error codes
    InvalidExtractionRequest,
    InvalidExtractionSchema,
    SchemaTooDeep,
    SchemaTooLarge,
    NoExtractableContent,
    ContextSelectionFailed,
    ExtractionContextTooLarge,
    LlmDisabled,
    LlmProviderNotConfigured,
    LlmProviderError,
    LlmTimeout,
    LlmRateLimited,
    LlmInvalidResponse,
    LlmContextLengthExceeded,
    SchemaValidationFailed,
    ExtractionRepairFailed,
    ExtractionCancelled,
    ExtractionLimitReached,

    // Phase 6 Error codes
    MapLimitExceeded,
    MapDiscoveryFailed,
    SitemapLimitExceeded,
    SearchProviderNotConfigured,
    SearchProviderError,
    SearchTimeout,
    SearchLimitExceeded,
    CacheError,
    SnapshotNotFound,
    InvalidBaseline,
    IncrementalCrawlFailed,
    LocalIndexUnavailable,

    // Phase 8 Semantic Knowledge Engine error codes
    IndexNotFound,
    IndexAlreadyExists,
    IndexLimitExceeded,
    IndexingFailed,
    ChunkingFailed,
    EmbeddingsDisabled,
    EmbeddingProviderNotConfigured,
    EmbeddingProviderError,
    EmbeddingDimensionMismatch,
    VectorStoreError,
    LexicalIndexError,
    SearchFailed,
    InvalidSearchFilter,
    RetrievalFailed,
    InsufficientContext,
    AnswerProviderDisabled,
    AnswerGenerationFailed,
    InvalidCitation,
    ReindexFailed,

    // Phase 9 Autonomous Web Research Agent error codes
    AgentDisabled,
    InvalidAgentRequest,
    AgentPlanFailed,
    AgentStepLimitReached,
    AgentPageLimitReached,
    AgentTimeLimitReached,
    AgentLlmLimitReached,
    AgentResearchFailed,
    AgentToolInvalid,
    AgentToolDenied,
    AgentEvidenceInsufficient,
    AgentSynthesisFailed,
    AgentCitationInvalid,
    AgentCancelled,
    AgentPaused,
    AgentModelUnavailable,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidUrl => "INVALID_URL",
            Self::UnsupportedScheme => "UNSUPPORTED_SCHEME",
            Self::PrivateNetworkBlocked => "PRIVATE_NETWORK_BLOCKED",
            Self::DnsResolutionFailed => "DNS_RESOLUTION_FAILED",
            Self::ConnectionFailed => "CONNECTION_FAILED",
            Self::RequestTimeout => "REQUEST_TIMEOUT",
            Self::TooManyRedirects => "TOO_MANY_REDIRECTS",
            Self::ResponseTooLarge => "RESPONSE_TOO_LARGE",
            Self::UnsupportedContentType => "UNSUPPORTED_CONTENT_TYPE",
            Self::InvalidHtml => "INVALID_HTML",
            Self::ServerOverloaded => "SERVER_OVERLOADED",
            Self::InternalError => "INTERNAL_ERROR",
            Self::InvalidCrawlConfig => "INVALID_CRAWL_CONFIG",
            Self::CrawlLimitReached => "CRAWL_LIMIT_REACHED",
            Self::CrawlTimeout => "CRAWL_TIMEOUT",
            Self::CrawlCancelled => "CRAWL_CANCELLED",
            Self::FrontierFull => "FRONTIER_FULL",
            Self::RobotsFetchFailed => "ROBOTS_FETCH_FAILED",
            Self::SitemapParseFailed => "SITEMAP_PARSE_FAILED",
            Self::JobNotFound => "JOB_NOT_FOUND",
            Self::JobStoreFull => "JOB_STORE_FULL",
            Self::BrowserDisabled => "BROWSER_DISABLED",
            Self::BrowserNotFound => "BROWSER_NOT_FOUND",
            Self::BrowserLaunchFailed => "BROWSER_LAUNCH_FAILED",
            Self::BrowserUnavailable => "BROWSER_UNAVAILABLE",
            Self::BrowserPoolExhausted => "BROWSER_POOL_EXHAUSTED",
            Self::BrowserNavigationFailed => "BROWSER_NAVIGATION_FAILED",
            Self::BrowserTimeout => "BROWSER_TIMEOUT",
            Self::BrowserCrashed => "BROWSER_CRASHED",
            Self::SelectorNotFound => "SELECTOR_NOT_FOUND",
            Self::ActionFailed => "ACTION_FAILED",
            Self::NetworkCaptureLimitReached => "NETWORK_CAPTURE_LIMIT_REACHED",
            Self::RenderFailed => "RENDER_FAILED",
            Self::ContentTypeDetectionFailed => "CONTENT_TYPE_DETECTION_FAILED",
            Self::DocumentTooLarge => "DOCUMENT_TOO_LARGE",
            Self::JsonParseFailed => "JSON_PARSE_FAILED",
            Self::JsonTooDeep => "JSON_TOO_DEEP",
            Self::XmlParseFailed => "XML_PARSE_FAILED",
            Self::XmlTooDeep => "XML_TOO_DEEP",
            Self::CsvParseFailed => "CSV_PARSE_FAILED",
            Self::CsvLimitExceeded => "CSV_LIMIT_EXCEEDED",
            Self::PdfParseFailed => "PDF_PARSE_FAILED",
            Self::PdfEncrypted => "PDF_ENCRYPTED",
            Self::PdfPageLimitExceeded => "PDF_PAGE_LIMIT_EXCEEDED",
            Self::ScannedPdf => "SCANNED_PDF",
            Self::DocxParseFailed => "DOCX_PARSE_FAILED",
            Self::DocxArchiveInvalid => "DOCX_ARCHIVE_INVALID",
            Self::ArchiveBombDetected => "ARCHIVE_BOMB_DETECTED",
            Self::UnsupportedDocumentType => "UNSUPPORTED_DOCUMENT_TYPE",
            Self::TextEncodingFailed => "TEXT_ENCODING_FAILED",
            Self::InvalidExtractionRequest => "INVALID_EXTRACTION_REQUEST",
            Self::InvalidExtractionSchema => "INVALID_EXTRACTION_SCHEMA",
            Self::SchemaTooDeep => "SCHEMA_TOO_DEEP",
            Self::SchemaTooLarge => "SCHEMA_TOO_LARGE",
            Self::NoExtractableContent => "NO_EXTRACTABLE_CONTENT",
            Self::ContextSelectionFailed => "CONTEXT_SELECTION_FAILED",
            Self::ExtractionContextTooLarge => "EXTRACTION_CONTEXT_TOO_LARGE",
            Self::LlmDisabled => "LLM_DISABLED",
            Self::LlmProviderNotConfigured => "LLM_PROVIDER_NOT_CONFIGURED",
            Self::LlmProviderError => "LLM_PROVIDER_ERROR",
            Self::LlmTimeout => "LLM_TIMEOUT",
            Self::LlmRateLimited => "LLM_RATE_LIMITED",
            Self::LlmInvalidResponse => "LLM_INVALID_RESPONSE",
            Self::LlmContextLengthExceeded => "LLM_CONTEXT_LENGTH_EXCEEDED",
            Self::SchemaValidationFailed => "SCHEMA_VALIDATION_FAILED",
            Self::ExtractionRepairFailed => "EXTRACTION_REPAIR_FAILED",
            Self::ExtractionCancelled => "EXTRACTION_CANCELLED",
            Self::ExtractionLimitReached => "EXTRACTION_LIMIT_REACHED",
            Self::MapLimitExceeded => "MAP_LIMIT_EXCEEDED",
            Self::MapDiscoveryFailed => "MAP_DISCOVERY_FAILED",
            Self::SitemapLimitExceeded => "SITEMAP_LIMIT_EXCEEDED",
            Self::SearchProviderNotConfigured => "SEARCH_PROVIDER_NOT_CONFIGURED",
            Self::SearchProviderError => "SEARCH_PROVIDER_ERROR",
            Self::SearchTimeout => "SEARCH_TIMEOUT",
            Self::SearchLimitExceeded => "SEARCH_LIMIT_EXCEEDED",
            Self::CacheError => "CACHE_ERROR",
            Self::SnapshotNotFound => "SNAPSHOT_NOT_FOUND",
            Self::InvalidBaseline => "INVALID_BASELINE",
            Self::IncrementalCrawlFailed => "INCREMENTAL_CRAWL_FAILED",
            Self::LocalIndexUnavailable => "LOCAL_INDEX_UNAVAILABLE",
            Self::IndexNotFound => "INDEX_NOT_FOUND",
            Self::IndexAlreadyExists => "INDEX_ALREADY_EXISTS",
            Self::IndexLimitExceeded => "INDEX_LIMIT_EXCEEDED",
            Self::IndexingFailed => "INDEXING_FAILED",
            Self::ChunkingFailed => "CHUNKING_FAILED",
            Self::EmbeddingsDisabled => "EMBEDDINGS_DISABLED",
            Self::EmbeddingProviderNotConfigured => "EMBEDDING_PROVIDER_NOT_CONFIGURED",
            Self::EmbeddingProviderError => "EMBEDDING_PROVIDER_ERROR",
            Self::EmbeddingDimensionMismatch => "EMBEDDING_DIMENSION_MISMATCH",
            Self::VectorStoreError => "VECTOR_STORE_ERROR",
            Self::LexicalIndexError => "LEXICAL_INDEX_ERROR",
            Self::SearchFailed => "SEARCH_FAILED",
            Self::InvalidSearchFilter => "INVALID_SEARCH_FILTER",
            Self::RetrievalFailed => "RETRIEVAL_FAILED",
            Self::InsufficientContext => "INSUFFICIENT_CONTEXT",
            Self::AnswerProviderDisabled => "ANSWER_PROVIDER_DISABLED",
            Self::AnswerGenerationFailed => "ANSWER_GENERATION_FAILED",
            Self::InvalidCitation => "INVALID_CITATION",
            Self::ReindexFailed => "REINDEX_FAILED",
            Self::AgentDisabled => "AGENT_DISABLED",
            Self::InvalidAgentRequest => "INVALID_AGENT_REQUEST",
            Self::AgentPlanFailed => "AGENT_PLAN_FAILED",
            Self::AgentStepLimitReached => "AGENT_STEP_LIMIT_REACHED",
            Self::AgentPageLimitReached => "AGENT_PAGE_LIMIT_REACHED",
            Self::AgentTimeLimitReached => "AGENT_TIME_LIMIT_REACHED",
            Self::AgentLlmLimitReached => "AGENT_LLM_LIMIT_REACHED",
            Self::AgentResearchFailed => "AGENT_RESEARCH_FAILED",
            Self::AgentToolInvalid => "AGENT_TOOL_INVALID",
            Self::AgentToolDenied => "AGENT_TOOL_DENIED",
            Self::AgentEvidenceInsufficient => "AGENT_EVIDENCE_INSUFFICIENT",
            Self::AgentSynthesisFailed => "AGENT_SYNTHESIS_FAILED",
            Self::AgentCitationInvalid => "AGENT_CITATION_INVALID",
            Self::AgentCancelled => "AGENT_CANCELLED",
            Self::AgentPaused => "AGENT_PAUSED",
            Self::AgentModelUnavailable => "AGENT_MODEL_UNAVAILABLE",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CrawlerError {
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Unsupported URL scheme: {0}. Only http:// and https:// are supported.")]
    UnsupportedScheme(String),

    #[error("Access to private/local network address is blocked: {0}")]
    PrivateNetworkBlocked(String),

    #[error("DNS resolution failed for host: {0}")]
    DnsResolutionFailed(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("The target website did not respond within the timeout: {0}")]
    RequestTimeout(String),

    #[error("Too many redirects (exceeded limit of {0})")]
    TooManyRedirects(usize),

    #[error("Response body exceeded maximum allowed size of {0} bytes")]
    ResponseTooLarge(usize),

    #[error("Unsupported content type: '{0}'.")]
    UnsupportedContentType(String),

    #[error("Invalid HTML content: {0}")]
    InvalidHtml(String),

    #[error("Server is currently overloaded. Max concurrent operations exceeded.")]
    ServerOverloaded,

    #[error("Internal processing error: {0}")]
    InternalError(String),

    // Phase 2 errors
    #[error("Invalid crawl configuration: {0}")]
    InvalidCrawlConfig(String),

    #[error("Crawl limit reached: {0}")]
    CrawlLimitReached(String),

    #[error("Crawl operation timed out: {0}")]
    CrawlTimeout(String),

    #[error("Crawl job was cancelled")]
    CrawlCancelled,

    #[error("Crawl frontier queue is full (max size {0} reached)")]
    FrontierFull(usize),

    #[error("Failed to fetch or parse robots.txt: {0}")]
    RobotsFetchFailed(String),

    #[error("Failed to parse sitemap: {0}")]
    SitemapParseFailed(String),

    #[error("Crawl job '{0}' was not found")]
    JobNotFound(String),

    #[error("Job store is full (maximum capacity {0} reached)")]
    JobStoreFull(usize),

    // Phase 3 Browser & Render errors
    #[error("Browser rendering is disabled by configuration")]
    BrowserDisabled,

    #[error("Chromium/Chrome executable was not found: {0}")]
    BrowserNotFound(String),

    #[error("Failed to launch browser process: {0}")]
    BrowserLaunchFailed(String),

    #[error("Browser service is currently unavailable: {0}")]
    BrowserUnavailable(String),

    #[error("Browser page pool is exhausted (maximum {0} active pages reached)")]
    BrowserPoolExhausted(usize),

    #[error("Browser failed to navigate to '{0}': {1}")]
    BrowserNavigationFailed(String, String),

    #[error("Browser rendering operation timed out: {0}")]
    BrowserTimeout(String),

    #[error("Browser process or page crashed: {0}")]
    BrowserCrashed(String),

    #[error("Selector '{0}' was not found on page")]
    SelectorNotFound(String),

    #[error("Browser action '{0}' failed: {1}")]
    ActionFailed(String, String),

    #[error("Network capture limit reached (exceeded {0} bytes)")]
    NetworkCaptureLimitReached(usize),

    #[error("Page rendering failed: {0}")]
    RenderFailed(String),

    // Phase 4 Document Parser errors
    #[error("Content-type detection failed: {0}")]
    ContentTypeDetectionFailed(String),

    #[error("Document size exceeded maximum allowed limit of {0} bytes")]
    DocumentTooLarge(usize),

    #[error("JSON parsing failed: {0}")]
    JsonParseFailed(String),

    #[error("JSON structure exceeded maximum nesting depth of {0}")]
    JsonTooDeep(usize),

    #[error("XML parsing failed: {0}")]
    XmlParseFailed(String),

    #[error("XML structure exceeded maximum nesting depth of {0}")]
    XmlTooDeep(usize),

    #[error("CSV parsing failed: {0}")]
    CsvParseFailed(String),

    #[error("CSV table exceeded row limit ({0}) or column limit ({1})")]
    CsvLimitExceeded(usize, usize),

    #[error("PDF parsing failed: {0}")]
    PdfParseFailed(String),

    #[error("PDF is encrypted and password-protected")]
    PdfEncrypted,

    #[error("PDF page count exceeded maximum limit of {0} pages")]
    PdfPageLimitExceeded(usize),

    #[error("PDF appears to be scanned or image-only without extractable text layer")]
    ScannedPdf,

    #[error("DOCX parsing failed: {0}")]
    DocxParseFailed(String),

    #[error("DOCX archive is invalid or corrupted: {0}")]
    DocxArchiveInvalid(String),

    #[error("ZIP archive bomb or path traversal detected: {0}")]
    ArchiveBombDetected(String),

    #[error("Unsupported document type: '{0}'")]
    UnsupportedDocumentType(String),

    #[error("Text encoding conversion failed: {0}")]
    TextEncodingFailed(String),

    // Phase 5 Extraction errors
    #[error("Invalid extraction request: {0}")]
    InvalidExtractionRequest(String),

    #[error("Invalid extraction schema: {0}")]
    InvalidExtractionSchema(String),

    #[error("Schema exceeded maximum nesting depth ({depth} > {max_depth})")]
    SchemaTooDeep { depth: usize, max_depth: usize },

    #[error("Schema exceeded maximum properties limit ({properties} > {max_properties})")]
    SchemaTooLarge {
        properties: usize,
        max_properties: usize,
    },

    #[error("No extractable content found for the request")]
    NoExtractableContent,

    #[error("Context selection failed: {0}")]
    ContextSelectionFailed(String),

    #[error("Extraction context too large: {0}")]
    ExtractionContextTooLarge(String),

    #[error("LLM extraction is disabled in server configuration")]
    LlmDisabled,

    #[error("LLM provider '{0}' is not configured")]
    LlmProviderNotConfigured(String),

    #[error("LLM provider error: {0}")]
    LlmProviderError(String),

    #[error("LLM operation timed out: {0}")]
    LlmTimeout(String),

    #[error("LLM rate limit reached (429): {0}")]
    LlmRateLimited(String),

    #[error("LLM returned invalid response: {0}")]
    LlmInvalidResponse(String),

    #[error("Schema validation failed: {0}")]
    SchemaValidationFailed(String),

    #[error("Extraction repair failed: {0}")]
    ExtractionRepairFailed(String),

    #[error("Extraction operation was cancelled")]
    ExtractionCancelled,

    #[error("Extraction limit reached: {0}")]
    ExtractionLimitReached(String),

    // Phase 6 Map, Search, Cache & Incremental errors
    #[error("Map discovery limit reached: {0}")]
    MapLimitExceeded(String),

    #[error("Map discovery failed: {0}")]
    MapDiscoveryFailed(String),

    #[error("Sitemap limit reached: {0}")]
    SitemapLimitExceeded(String),

    #[error("Search provider '{0}' is not configured")]
    SearchProviderNotConfigured(String),

    #[error("Search provider error: {0}")]
    SearchProviderError(String),

    #[error("Search operation timed out")]
    SearchTimeout,

    #[error("Search results limit exceeded: {0}")]
    SearchLimitExceeded(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Crawl snapshot '{0}' was not found")]
    SnapshotNotFound(String),

    #[error("Invalid baseline crawl snapshot for incremental crawl: {0}")]
    InvalidBaseline(String),

    #[error("Incremental crawl failed: {0}")]
    IncrementalCrawlFailed(String),

    #[error("Local search index is unavailable or not initialized")]
    LocalIndexUnavailable,

    #[error("Internal server error: {0}")]
    Internal(String),

    // Phase 8 Semantic Knowledge Engine errors
    #[error("Search index '{0}' was not found")]
    IndexNotFound(String),

    #[error("Search index '{0}' already exists")]
    IndexAlreadyExists(String),

    #[error("Search index limit exceeded: {0}")]
    IndexLimitExceeded(String),

    #[error("Indexing operation failed: {0}")]
    IndexingFailed(String),

    #[error("Document chunking failed: {0}")]
    ChunkingFailed(String),

    #[error("Embeddings are disabled in server configuration")]
    EmbeddingsDisabled,

    #[error("Embedding provider '{0}' is not configured")]
    EmbeddingProviderNotConfigured(String),

    #[error("Embedding provider error: {0}")]
    EmbeddingProviderError(String),

    #[error("Embedding dimension mismatch: expected {0}, got {1}")]
    EmbeddingDimensionMismatch(usize, usize),

    #[error("Vector store error: {0}")]
    VectorStoreError(String),

    #[error("Lexical index error: {0}")]
    LexicalIndexError(String),

    #[error("Search failed: {0}")]
    SearchFailed(String),

    #[error("Invalid search filter: {0}")]
    InvalidSearchFilter(String),

    #[error("Retrieval failed: {0}")]
    RetrievalFailed(String),

    #[error("Insufficient context for answering question")]
    InsufficientContext,

    #[error("Answer generation requires LLM provider which is disabled")]
    AnswerProviderDisabled,

    #[error("Answer generation failed: {0}")]
    AnswerGenerationFailed(String),

    #[error("Invalid citation: {0}")]
    InvalidCitation(String),

    #[error("Reindexing failed: {0}")]
    ReindexFailed(String),

    // Phase 9 Autonomous Web Research Agent error variants
    #[error("Agent research is disabled in server configuration")]
    AgentDisabled,

    #[error("Invalid agent request: {0}")]
    InvalidAgentRequest(String),

    #[error("Agent planning failed: {0}")]
    AgentPlanFailed(String),

    #[error("Agent reached maximum step budget limit ({0})")]
    AgentStepLimitReached(u32),

    #[error("Agent reached maximum page budget limit ({0})")]
    AgentPageLimitReached(u32),

    #[error("Agent reached maximum execution time limit ({0}s)")]
    AgentTimeLimitReached(u64),

    #[error("Agent reached maximum LLM call budget limit ({0})")]
    AgentLlmLimitReached(u32),

    #[error("Agent research failed: {0}")]
    AgentResearchFailed(String),

    #[error("Agent tool execution error: {0}")]
    AgentToolInvalid(String),

    #[error("Agent tool execution denied by security policy: {0}")]
    AgentToolDenied(String),

    #[error("Insufficient evidence gathered to answer research task: {0}")]
    AgentEvidenceInsufficient(String),

    #[error("Agent research synthesis failed: {0}")]
    AgentSynthesisFailed(String),

    #[error("Agent citation validation failed: {0}")]
    AgentCitationInvalid(String),

    #[error("Agent research job '{0}' was cancelled")]
    AgentCancelled(String),

    #[error("Agent research job '{0}' is paused")]
    AgentPaused(String),

    #[error("Agent model provider unavailable: {0}")]
    AgentModelUnavailable(String),
}

impl CrawlerError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidUrl(_) => ErrorCode::InvalidUrl,
            Self::UnsupportedScheme(_) => ErrorCode::UnsupportedScheme,
            Self::PrivateNetworkBlocked(_) => ErrorCode::PrivateNetworkBlocked,
            Self::DnsResolutionFailed(_) => ErrorCode::DnsResolutionFailed,
            Self::ConnectionFailed(_) => ErrorCode::ConnectionFailed,
            Self::RequestTimeout(_) => ErrorCode::RequestTimeout,
            Self::TooManyRedirects(_) => ErrorCode::TooManyRedirects,
            Self::ResponseTooLarge(_) => ErrorCode::ResponseTooLarge,
            Self::UnsupportedContentType(_) => ErrorCode::UnsupportedContentType,
            Self::InvalidHtml(_) => ErrorCode::InvalidHtml,
            Self::ServerOverloaded => ErrorCode::ServerOverloaded,
            Self::InternalError(_) => ErrorCode::InternalError,
            Self::InvalidCrawlConfig(_) => ErrorCode::InvalidCrawlConfig,
            Self::CrawlLimitReached(_) => ErrorCode::CrawlLimitReached,
            Self::CrawlTimeout(_) => ErrorCode::CrawlTimeout,
            Self::CrawlCancelled => ErrorCode::CrawlCancelled,
            Self::FrontierFull(_) => ErrorCode::FrontierFull,
            Self::RobotsFetchFailed(_) => ErrorCode::RobotsFetchFailed,
            Self::SitemapParseFailed(_) => ErrorCode::SitemapParseFailed,
            Self::JobNotFound(_) => ErrorCode::JobNotFound,
            Self::JobStoreFull(_) => ErrorCode::JobStoreFull,
            Self::BrowserDisabled => ErrorCode::BrowserDisabled,
            Self::BrowserNotFound(_) => ErrorCode::BrowserNotFound,
            Self::BrowserLaunchFailed(_) => ErrorCode::BrowserLaunchFailed,
            Self::BrowserUnavailable(_) => ErrorCode::BrowserUnavailable,
            Self::BrowserPoolExhausted(_) => ErrorCode::BrowserPoolExhausted,
            Self::BrowserNavigationFailed(_, _) => ErrorCode::BrowserNavigationFailed,
            Self::BrowserTimeout(_) => ErrorCode::BrowserTimeout,
            Self::BrowserCrashed(_) => ErrorCode::BrowserCrashed,
            Self::SelectorNotFound(_) => ErrorCode::SelectorNotFound,
            Self::ActionFailed(_, _) => ErrorCode::ActionFailed,
            Self::NetworkCaptureLimitReached(_) => ErrorCode::NetworkCaptureLimitReached,
            Self::RenderFailed(_) => ErrorCode::RenderFailed,
            Self::ContentTypeDetectionFailed(_) => ErrorCode::ContentTypeDetectionFailed,
            Self::DocumentTooLarge(_) => ErrorCode::DocumentTooLarge,
            Self::JsonParseFailed(_) => ErrorCode::JsonParseFailed,
            Self::JsonTooDeep(_) => ErrorCode::JsonTooDeep,
            Self::XmlParseFailed(_) => ErrorCode::XmlParseFailed,
            Self::XmlTooDeep(_) => ErrorCode::XmlTooDeep,
            Self::CsvParseFailed(_) => ErrorCode::CsvParseFailed,
            Self::CsvLimitExceeded(_, _) => ErrorCode::CsvLimitExceeded,
            Self::PdfParseFailed(_) => ErrorCode::PdfParseFailed,
            Self::PdfEncrypted => ErrorCode::PdfEncrypted,
            Self::PdfPageLimitExceeded(_) => ErrorCode::PdfPageLimitExceeded,
            Self::ScannedPdf => ErrorCode::ScannedPdf,
            Self::DocxParseFailed(_) => ErrorCode::DocxParseFailed,
            Self::DocxArchiveInvalid(_) => ErrorCode::DocxArchiveInvalid,
            Self::ArchiveBombDetected(_) => ErrorCode::ArchiveBombDetected,
            Self::UnsupportedDocumentType(_) => ErrorCode::UnsupportedDocumentType,
            Self::TextEncodingFailed(_) => ErrorCode::TextEncodingFailed,
            Self::InvalidExtractionRequest(_) => ErrorCode::InvalidExtractionRequest,
            Self::InvalidExtractionSchema(_) => ErrorCode::InvalidExtractionSchema,
            Self::SchemaTooDeep { .. } => ErrorCode::SchemaTooDeep,
            Self::SchemaTooLarge { .. } => ErrorCode::SchemaTooLarge,
            Self::NoExtractableContent => ErrorCode::NoExtractableContent,
            Self::ContextSelectionFailed(_) => ErrorCode::ContextSelectionFailed,
            Self::ExtractionContextTooLarge(_) => ErrorCode::ExtractionContextTooLarge,
            Self::LlmDisabled => ErrorCode::LlmDisabled,
            Self::LlmProviderNotConfigured(_) => ErrorCode::LlmProviderNotConfigured,
            Self::LlmProviderError(_) => ErrorCode::LlmProviderError,
            Self::LlmTimeout(_) => ErrorCode::LlmTimeout,
            Self::LlmRateLimited(_) => ErrorCode::LlmRateLimited,
            Self::LlmInvalidResponse(_) => ErrorCode::LlmInvalidResponse,
            Self::SchemaValidationFailed(_) => ErrorCode::SchemaValidationFailed,
            Self::ExtractionRepairFailed(_) => ErrorCode::ExtractionRepairFailed,
            Self::ExtractionCancelled => ErrorCode::ExtractionCancelled,
            Self::ExtractionLimitReached(_) => ErrorCode::ExtractionLimitReached,
            Self::MapLimitExceeded(_) => ErrorCode::MapLimitExceeded,
            Self::MapDiscoveryFailed(_) => ErrorCode::MapDiscoveryFailed,
            Self::SitemapLimitExceeded(_) => ErrorCode::SitemapLimitExceeded,
            Self::SearchProviderNotConfigured(_) => ErrorCode::SearchProviderNotConfigured,
            Self::SearchProviderError(_) => ErrorCode::SearchProviderError,
            Self::SearchTimeout => ErrorCode::SearchTimeout,
            Self::SearchLimitExceeded(_) => ErrorCode::SearchLimitExceeded,
            Self::CacheError(_) => ErrorCode::CacheError,
            Self::SnapshotNotFound(_) => ErrorCode::SnapshotNotFound,
            Self::InvalidBaseline(_) => ErrorCode::InvalidBaseline,
            Self::IncrementalCrawlFailed(_) => ErrorCode::IncrementalCrawlFailed,
            Self::LocalIndexUnavailable => ErrorCode::LocalIndexUnavailable,
            Self::Internal(_) => ErrorCode::InternalError,
            Self::IndexNotFound(_) => ErrorCode::IndexNotFound,
            Self::IndexAlreadyExists(_) => ErrorCode::IndexAlreadyExists,
            Self::IndexLimitExceeded(_) => ErrorCode::IndexLimitExceeded,
            Self::IndexingFailed(_) => ErrorCode::IndexingFailed,
            Self::ChunkingFailed(_) => ErrorCode::ChunkingFailed,
            Self::EmbeddingsDisabled => ErrorCode::EmbeddingsDisabled,
            Self::EmbeddingProviderNotConfigured(_) => ErrorCode::EmbeddingProviderNotConfigured,
            Self::EmbeddingProviderError(_) => ErrorCode::EmbeddingProviderError,
            Self::EmbeddingDimensionMismatch(_, _) => ErrorCode::EmbeddingDimensionMismatch,
            Self::VectorStoreError(_) => ErrorCode::VectorStoreError,
            Self::LexicalIndexError(_) => ErrorCode::LexicalIndexError,
            Self::SearchFailed(_) => ErrorCode::SearchFailed,
            Self::InvalidSearchFilter(_) => ErrorCode::InvalidSearchFilter,
            Self::RetrievalFailed(_) => ErrorCode::RetrievalFailed,
            Self::InsufficientContext => ErrorCode::InsufficientContext,
            Self::AnswerProviderDisabled => ErrorCode::AnswerProviderDisabled,
            Self::AnswerGenerationFailed(_) => ErrorCode::AnswerGenerationFailed,
            Self::InvalidCitation(_) => ErrorCode::InvalidCitation,
            Self::ReindexFailed(_) => ErrorCode::ReindexFailed,
            Self::AgentDisabled => ErrorCode::AgentDisabled,
            Self::InvalidAgentRequest(_) => ErrorCode::InvalidAgentRequest,
            Self::AgentPlanFailed(_) => ErrorCode::AgentPlanFailed,
            Self::AgentStepLimitReached(_) => ErrorCode::AgentStepLimitReached,
            Self::AgentPageLimitReached(_) => ErrorCode::AgentPageLimitReached,
            Self::AgentTimeLimitReached(_) => ErrorCode::AgentTimeLimitReached,
            Self::AgentLlmLimitReached(_) => ErrorCode::AgentLlmLimitReached,
            Self::AgentResearchFailed(_) => ErrorCode::AgentResearchFailed,
            Self::AgentToolInvalid(_) => ErrorCode::AgentToolInvalid,
            Self::AgentToolDenied(_) => ErrorCode::AgentToolDenied,
            Self::AgentEvidenceInsufficient(_) => ErrorCode::AgentEvidenceInsufficient,
            Self::AgentSynthesisFailed(_) => ErrorCode::AgentSynthesisFailed,
            Self::AgentCitationInvalid(_) => ErrorCode::AgentCitationInvalid,
            Self::AgentCancelled(_) => ErrorCode::AgentCancelled,
            Self::AgentPaused(_) => ErrorCode::AgentPaused,
            Self::AgentModelUnavailable(_) => ErrorCode::AgentModelUnavailable,
        }
    }

    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidUrl(_) => StatusCode::BAD_REQUEST,
            Self::UnsupportedScheme(_) => StatusCode::BAD_REQUEST,
            Self::PrivateNetworkBlocked(_) => StatusCode::FORBIDDEN,
            Self::DnsResolutionFailed(_) => StatusCode::BAD_GATEWAY,
            Self::ConnectionFailed(_) => StatusCode::BAD_GATEWAY,
            Self::RequestTimeout(_) => StatusCode::GATEWAY_TIMEOUT,
            Self::TooManyRedirects(_) => StatusCode::BAD_GATEWAY,
            Self::ResponseTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::UnsupportedContentType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::InvalidHtml(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::ServerOverloaded => StatusCode::TOO_MANY_REQUESTS,
            Self::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InvalidCrawlConfig(_) => StatusCode::BAD_REQUEST,
            Self::CrawlLimitReached(_) => StatusCode::OK,
            Self::CrawlTimeout(_) => StatusCode::GATEWAY_TIMEOUT,
            Self::CrawlCancelled => StatusCode::OK,
            Self::FrontierFull(_) => StatusCode::INSUFFICIENT_STORAGE,
            Self::RobotsFetchFailed(_) => StatusCode::BAD_GATEWAY,
            Self::SitemapParseFailed(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::JobNotFound(_) => StatusCode::NOT_FOUND,
            Self::JobStoreFull(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::BrowserDisabled => StatusCode::FORBIDDEN,
            Self::BrowserNotFound(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::BrowserLaunchFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::BrowserUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::BrowserPoolExhausted(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::BrowserNavigationFailed(_, _) => StatusCode::BAD_GATEWAY,
            Self::BrowserTimeout(_) => StatusCode::GATEWAY_TIMEOUT,
            Self::BrowserCrashed(_) => StatusCode::BAD_GATEWAY,
            Self::SelectorNotFound(_) => StatusCode::NOT_FOUND,
            Self::ActionFailed(_, _) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::NetworkCaptureLimitReached(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::RenderFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ContentTypeDetectionFailed(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::DocumentTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::JsonParseFailed(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::JsonTooDeep(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::XmlParseFailed(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::XmlTooDeep(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::CsvParseFailed(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::CsvLimitExceeded(_, _) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::PdfParseFailed(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::PdfEncrypted => StatusCode::FORBIDDEN,
            Self::PdfPageLimitExceeded(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::ScannedPdf => StatusCode::UNPROCESSABLE_ENTITY,
            Self::DocxParseFailed(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::DocxArchiveInvalid(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::ArchiveBombDetected(_) => StatusCode::BAD_REQUEST,
            Self::UnsupportedDocumentType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::TextEncodingFailed(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::InvalidExtractionRequest(_) => StatusCode::BAD_REQUEST,
            Self::InvalidExtractionSchema(_) => StatusCode::BAD_REQUEST,
            Self::SchemaTooDeep { .. } => StatusCode::BAD_REQUEST,
            Self::SchemaTooLarge { .. } => StatusCode::BAD_REQUEST,
            Self::NoExtractableContent => StatusCode::UNPROCESSABLE_ENTITY,
            Self::ContextSelectionFailed(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::ExtractionContextTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::LlmDisabled => StatusCode::SERVICE_UNAVAILABLE,
            Self::LlmProviderNotConfigured(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::LlmProviderError(_) => StatusCode::BAD_GATEWAY,
            Self::LlmTimeout(_) => StatusCode::GATEWAY_TIMEOUT,
            Self::LlmRateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::LlmInvalidResponse(_) => StatusCode::BAD_GATEWAY,
            Self::SchemaValidationFailed(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::ExtractionRepairFailed(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::ExtractionCancelled => StatusCode::OK,
            Self::ExtractionLimitReached(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::MapLimitExceeded(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::MapDiscoveryFailed(_) => StatusCode::BAD_GATEWAY,
            Self::SitemapLimitExceeded(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::SearchProviderNotConfigured(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::SearchProviderError(_) => StatusCode::BAD_GATEWAY,
            Self::SearchTimeout => StatusCode::GATEWAY_TIMEOUT,
            Self::SearchLimitExceeded(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::CacheError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::SnapshotNotFound(_) => StatusCode::NOT_FOUND,
            Self::InvalidBaseline(_) => StatusCode::BAD_REQUEST,
            Self::IncrementalCrawlFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::LocalIndexUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::IndexNotFound(_) => StatusCode::NOT_FOUND,
            Self::IndexAlreadyExists(_) => StatusCode::CONFLICT,
            Self::IndexLimitExceeded(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::IndexingFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ChunkingFailed(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::EmbeddingsDisabled => StatusCode::SERVICE_UNAVAILABLE,
            Self::EmbeddingProviderNotConfigured(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::EmbeddingProviderError(_) => StatusCode::BAD_GATEWAY,
            Self::EmbeddingDimensionMismatch(_, _) => StatusCode::BAD_REQUEST,
            Self::VectorStoreError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::LexicalIndexError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::SearchFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InvalidSearchFilter(_) => StatusCode::BAD_REQUEST,
            Self::RetrievalFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InsufficientContext => StatusCode::UNPROCESSABLE_ENTITY,
            Self::AnswerProviderDisabled => StatusCode::SERVICE_UNAVAILABLE,
            Self::AnswerGenerationFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InvalidCitation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::ReindexFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::AgentDisabled => StatusCode::FORBIDDEN,
            Self::InvalidAgentRequest(_) => StatusCode::BAD_REQUEST,
            Self::AgentPlanFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::AgentStepLimitReached(_) => StatusCode::OK,
            Self::AgentPageLimitReached(_) => StatusCode::OK,
            Self::AgentTimeLimitReached(_) => StatusCode::GATEWAY_TIMEOUT,
            Self::AgentLlmLimitReached(_) => StatusCode::OK,
            Self::AgentResearchFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::AgentToolInvalid(_) => StatusCode::BAD_REQUEST,
            Self::AgentToolDenied(_) => StatusCode::FORBIDDEN,
            Self::AgentEvidenceInsufficient(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::AgentSynthesisFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::AgentCitationInvalid(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::AgentCancelled(_) => StatusCode::OK,
            Self::AgentPaused(_) => StatusCode::OK,
            Self::AgentModelUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    pub fn user_message(&self) -> String {
        self.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorPayload {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    pub success: bool,
    pub error: ApiErrorPayload,
}

impl From<&CrawlerError> for ApiErrorResponse {
    fn from(err: &CrawlerError) -> Self {
        Self {
            success: false,
            error: ApiErrorPayload {
                code: err.code().as_str().to_string(),
                message: err.user_message(),
            },
        }
    }
}

impl IntoResponse for CrawlerError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let payload = ApiErrorResponse::from(&self);
        (status, Json(payload)).into_response()
    }
}
