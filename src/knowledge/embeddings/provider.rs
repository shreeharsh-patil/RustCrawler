use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Result from an embedding provider call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingBatch {
    pub embeddings: Vec<Vec<f32>>,
    pub model: String,
    pub prompt_tokens: usize,
}

/// Errors occurring during embedding generation
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("Embedding provider is not configured: {0}")]
    NotConfigured(String),

    #[error("Embedding HTTP request failed: {0}")]
    HttpError(String),

    #[error("Embedding provider rate limited (429): {0}")]
    RateLimited(String),

    #[error("Embedding provider server error ({status}): {message}")]
    ServerError { status: u16, message: String },

    #[error("Embedding request timed out")]
    Timeout,

    #[error("Embedding response parsing failed: {0}")]
    InvalidResponse(String),

    #[error("Embedding dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Batch token limit exceeded: {0}")]
    BatchLimitExceeded(String),
}

/// Trait defining the asynchronous embedding generation interface
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generates embeddings for a batch of text strings
    async fn embed(&self, texts: &[String]) -> Result<EmbeddingBatch, EmbeddingError>;

    /// Returns the embedding model identifier
    fn model(&self) -> &str;

    /// Returns the dimension of generated embedding vectors
    fn dimensions(&self) -> usize;

    /// Returns the provider name
    fn provider_name(&self) -> &str;
}
