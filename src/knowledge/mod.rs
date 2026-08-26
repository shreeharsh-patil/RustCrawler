pub mod chunking;
pub mod embeddings;
pub mod hybrid;
pub mod lexical;
pub mod models;
pub mod rag;
pub mod retrieval;
pub mod service;
pub mod vector;

pub use chunking::{
    compute_chunk_hash, ApproximateTokenizer, ChunkingConfig, SimHash, StructuralChunker,
    TokenCounter, WordTokenizer,
};
pub use embeddings::{
    generate_deterministic_vector, BatcherConfig, EmbeddingBatch, EmbeddingBatcher, EmbeddingCache,
    EmbeddingCacheKey, EmbeddingCacheStats, EmbeddingError, EmbeddingProvider,
    MockEmbeddingProvider, OpenAiCompatibleEmbeddingProvider,
};
pub use hybrid::reciprocal_rank_fusion;
pub use lexical::{tokenize_text, LexicalIndex, LexicalMatch};
pub use models::{
    ChunkMetadata, Citation, HybridMatch, IndexFilter, IndexMetadata, IndexStats, IndexedChunk,
    IndexedDocument, RagAnswer, RetrievalResult, SearchIndex, SearchMode,
};
pub use rag::RagAnswerService;
pub use retrieval::RetrievalService;
pub use service::{KnowledgeEngine, KnowledgeMetrics};
pub use vector::{
    DistanceMetric, MemoryVectorStore, PgVectorMigrations, VectorMatch, VectorQuery, VectorRecord,
    VectorStore, VectorStoreError,
};
