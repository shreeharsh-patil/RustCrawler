use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Model representing a distinct search index
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchIndex {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_dimensions: Option<usize>,
    pub document_count: u64,
    pub chunk_count: u64,
}

impl SearchIndex {
    pub fn new(
        name: impl Into<String>,
        embedding_model: Option<String>,
        embedding_dimensions: Option<usize>,
        tenant_id: Option<String>,
    ) -> Self {
        let name_str = name.into();
        let id = format!("idx_{}", uuid::Uuid::new_v4().simple());
        let now = Utc::now();
        Self {
            id,
            name: name_str,
            tenant_id,
            created_at: now,
            updated_at: now,
            embedding_model,
            embedding_dimensions,
            document_count: 0,
            chunk_count: 0,
        }
    }
}

/// Metadata associated with an entire indexed document
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct IndexMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crawl_job_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub custom: HashMap<String, serde_json::Value>,
}

/// Unified document representing a parsed and indexed web page or file
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexedDocument {
    pub document_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    pub content_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub content_type: String,
    pub metadata: IndexMetadata,
    pub chunks: Vec<IndexedChunk>,
}

/// Metadata specific to an individual semantic chunk
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ChunkMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char_start: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char_end: Option<usize>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub custom: HashMap<String, serde_json::Value>,
}

/// A structural semantic chunk of a document ready for indexing and retrieval
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexedChunk {
    pub chunk_id: String,
    pub document_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heading_path: Vec<String>,
    pub text: String,
    pub token_count: usize,
    pub content_hash: String,
    pub position: u32,
    pub metadata: ChunkMetadata,
}

/// Search operational modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    Lexical,
    Semantic,
    #[default]
    Hybrid,
}

/// Strongly typed filter applied before/during retrieval
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct IndexFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crawl_job_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_from: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_to: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

/// Match returned from hybrid search fusion
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HybridMatch {
    pub chunk_id: String,
    pub document_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heading_path: Vec<String>,
    pub text: String,
    pub score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lexical_rank: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector_rank: Option<usize>,
    pub rrf_score: f64,
    pub metadata: ChunkMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_metadata: Option<IndexMetadata>,
}

/// Result returned from the `/v1/retrieve` API endpoint
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetrievalResult {
    pub query: String,
    pub chunks: Vec<HybridMatch>,
    pub total_chunks: usize,
    pub total_tokens: usize,
}

/// Citation source grounded in an indexed chunk
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Citation {
    pub chunk_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heading_path: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

/// Result returned from the grounded `/v1/answer` RAG API endpoint
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RagAnswer {
    pub question: String,
    pub answer: String,
    pub citations: Vec<Citation>,
    pub retrieved_chunks: Vec<HybridMatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
}

/// Metrics and stats for a search index
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexStats {
    pub index: SearchIndex,
    pub document_count: u64,
    pub chunk_count: u64,
    pub total_tokens: u64,
    pub average_chunk_tokens: f64,
}
