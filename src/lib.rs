pub mod agent;
pub mod api;
pub mod browser;
pub mod cache;
pub mod config;
pub mod crawl;
pub mod detect;
pub mod discovery;
pub mod distributed;
pub mod document;
pub mod error;
pub mod extract;
pub mod extraction;
pub mod fetch;
pub mod knowledge;
pub mod models;
pub mod parser;
pub mod render;
pub mod search;
pub mod service;
pub mod utils;

pub use browser::BrowserManager;
pub use cache::{CacheEntry, CacheStore, HttpCacheUtils, InMemoryCacheStore};
pub use config::Config;
pub use crawl::{
    ChangeSummary, ChangeType, CrawlJobInitResponse, CrawlJobStatusResponse, CrawlOptions,
    CrawlProgress, CrawlRequest, CrawlResult, CrawlSnapshot, CrawlStats, CrawlStatus, CrawlTarget,
    CrawledPage, CrawlerService, DiscoverySource, IncrementalCrawlRequest,
    IncrementalCrawlResponse, IncrementalCrawler, PageSnapshot, PageTimings, SnapshotStore,
};
pub use detect::{ContentDetector, DetectedContentType, DocumentType};
pub use discovery::{
    DiscoveredLink, FetchPurpose, LightweightExtractor, LightweightPageSummary, MapMetadata,
    MapRequest, MapResponse, MapService, ParsedSitemapContent, RelevanceRanker, SitemapEntry,
    StreamingSitemapParser, UrlEdge, UrlNode,
};
pub use distributed::{
    DistributedFrontier, DistributedHostLimiter, EventBus, ExecutionMode, HostPermit,
    JobProgressSummary, JobRecord, JobScheduler, JobStatus, JobType, JobUpdate, LocalResultStore,
    MemoryMetadataStore, MemoryTaskQueue, MetadataStore, QueueStats, QueuedTask, RedisTaskQueue,
    ResultStore, S3ResultStore, SqlMetadataStore, StoreError, SystemEventRecord, SystemEventType,
    TaskEnvelope, TaskFailure, TaskLease, TaskPriority, TaskQueue, TaskRecord, TaskStatus,
    TaskType, UrlStatus, WebhookDeliveryRecord, WebhookDeliveryStatus, WebhookDispatcher,
    WebhookEndpoint, WorkerCapability, WorkerInfo, WorkerRuntime, WorkerType,
};
pub use document::{
    calculate_word_count, compute_blake3_hash, ContentParser, DocumentMetadata, DocumentPage,
    DocumentParserPool, DocumentTable, ParseInput, ParserRegistry, UnifiedDocument,
};
pub use error::{ApiErrorPayload, ApiErrorResponse, CrawlerError, ErrorCode};
pub use extraction::{
    BatchExtractionItem, BatchExtractionRequest, BatchExtractionResponse, BatchItemResult,
    CandidatePool, ChunkType, ContentChunk, DeterministicExtractor, ExtractedField,
    ExtractionCrawlOptions, ExtractionJobInfo, ExtractionJobStatus, ExtractionMetadata,
    ExtractionMode, ExtractionRequest, ExtractionResponse, ExtractionResult, ExtractionService,
    ExtractionSource, FieldProvenance, JsonValidator, LlmExtractionRequest, LlmExtractionResponse,
    LlmProvider, LlmProviderRegistry, LlmUsage, MissingFieldBehavior, MockLlmProvider,
    OpenAiCompatibleProvider, ProvenanceTracker, SchemaValidator,
};
pub use knowledge::{
    ApproximateTokenizer, ChunkMetadata, ChunkingConfig, Citation, DistanceMetric, EmbeddingBatch,
    EmbeddingBatcher, EmbeddingCache, EmbeddingCacheKey, EmbeddingCacheStats, EmbeddingError,
    EmbeddingProvider, HybridMatch, IndexFilter, IndexMetadata, IndexStats, IndexedChunk,
    KnowledgeEngine, KnowledgeMetrics, LexicalIndex, LexicalMatch, MemoryVectorStore,
    MockEmbeddingProvider, OpenAiCompatibleEmbeddingProvider, PgVectorMigrations, RagAnswer,
    RagAnswerService, RetrievalResult, RetrievalService, SearchIndex, SimHash, StructuralChunker,
    TokenCounter, VectorMatch, VectorQuery, VectorRecord, VectorStore, VectorStoreError,
    WordTokenizer,
};
pub use models::{
    Heading, HttpMetadata, OpenGraphMetadata, OutputFormat, PageImage, PageLink, PageMetadata,
    ScrapeOptions, ScrapeRequest, ScrapeResponse, ScrapeResult, ScrapeWarning, TwitterMetadata,
    WarningCode,
};
pub use render::{
    BrowserAction, HttpScrapeAnalysis, NetworkResponse, PageRenderer, RenderDecision,
    RenderDecisionEngine, RenderDiagnostics, RenderMode, RenderReason, RenderRequest, RenderResult,
    RenderTimings, SelectedRenderer, SmartDecisionEngine, WaitStrategy,
};
pub use search::{
    ExternalSearchProvider, IndexedDocument, LocalSearchIndex, MockSearchProvider, SearchMode,
    SearchProvider, SearchProviderRegistry, SearchRequest, SearchResponse, SearchResult,
    SearchResultSource, SearchService,
};
pub use service::ScraperService;
