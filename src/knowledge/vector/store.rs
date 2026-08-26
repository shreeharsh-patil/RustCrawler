use crate::knowledge::models::{ChunkMetadata, IndexFilter, IndexMetadata};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Distance metrics supported for vector similarity comparison
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DistanceMetric {
    #[default]
    Cosine,
    DotProduct,
    L2,
}

/// A stored vector record containing chunk identity, embedding vector, and search metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorRecord {
    pub chunk_id: String,
    pub document_id: String,
    pub index_id: String,
    pub tenant_id: Option<String>,
    pub source_url: Option<String>,
    pub title: Option<String>,
    pub heading_path: Vec<String>,
    pub text: String,
    pub vector: Vec<f32>,
    pub metadata: ChunkMetadata,
    pub document_metadata: Option<IndexMetadata>,
}

/// Query parameters for vector similarity search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorQuery {
    pub vector: Vec<f32>,
    pub limit: usize,
    pub filter: Option<IndexFilter>,
    pub tenant_id: Option<String>,
    pub min_score: Option<f64>,
}

/// Match returned from a vector search query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorMatch {
    pub chunk_id: String,
    pub document_id: String,
    pub source_url: Option<String>,
    pub title: Option<String>,
    pub heading_path: Vec<String>,
    pub text: String,
    pub score: f64, // Similarity score (0.0 to 1.0 for cosine)
    pub metadata: ChunkMetadata,
    pub document_metadata: Option<IndexMetadata>,
}

/// Errors occurring during vector store operations
#[derive(Debug, thiserror::Error)]
pub enum VectorStoreError {
    #[error("Vector store database error: {0}")]
    DatabaseError(String),

    #[error("Index '{0}' not found in vector store")]
    IndexNotFound(String),

    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Invalid query: {0}")]
    InvalidQuery(String),
}

/// Asynchronous Vector Store trait abstraction
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Upserts a batch of vector records into the specified index
    async fn upsert(
        &self,
        index_id: &str,
        records: Vec<VectorRecord>,
    ) -> Result<(), VectorStoreError>;

    /// Searches for top-k similar vectors matching the query
    async fn search(
        &self,
        index_id: &str,
        query: VectorQuery,
    ) -> Result<Vec<VectorMatch>, VectorStoreError>;

    /// Deletes all chunks associated with a specific document
    async fn delete_document(
        &self,
        index_id: &str,
        document_id: &str,
    ) -> Result<usize, VectorStoreError>;

    /// Deletes an entire vector index
    async fn delete_index(&self, index_id: &str) -> Result<(), VectorStoreError>;

    /// Counts the total number of vector records in an index
    async fn count(&self, index_id: &str) -> Result<usize, VectorStoreError>;
}
