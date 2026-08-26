pub mod cache;
pub mod candidate;
pub mod context;
pub mod deterministic;
pub mod llm;
pub mod models;
pub mod provenance;
pub mod repair;
pub mod schema;
pub mod service;
pub mod validation;

pub use candidate::CandidatePool;
pub use context::ContextSelector;
pub use deterministic::DeterministicExtractor;
pub use llm::{LlmProvider, LlmProviderRegistry, MockLlmProvider, OpenAiCompatibleProvider};
pub use models::{
    BatchExtractionItem, BatchExtractionRequest, BatchExtractionResponse, BatchItemResult,
    ChunkType, ContentChunk, ExtractedField, ExtractionCrawlOptions, ExtractionJobInfo,
    ExtractionJobStatus, ExtractionMetadata, ExtractionMode, ExtractionRequest, ExtractionResponse,
    ExtractionResult, ExtractionSource, FieldProvenance, LlmExtractionRequest,
    LlmExtractionResponse, LlmUsage, MissingFieldBehavior,
};
pub use provenance::ProvenanceTracker;
pub use repair::ExtractionRepairPipeline;
pub use schema::{canonicalize_schema, compute_schema_hash, SchemaValidator};
pub use service::ExtractionService;
pub use validation::JsonValidator;
