use crate::detect::models::DocumentType;
use crate::models::{OpenGraphMetadata, PageImage, PageLink, ScrapeWarning, TwitterMetadata};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

/// Represents a single page within a multi-page document (such as a PDF).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentPage {
    pub page_number: usize,
    pub text: String,
}

/// Represents a structured tabular dataset extracted from CSV, TSV, HTML, or DOCX.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentTable {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// Generalized metadata for all web and document resources.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charset: Option<String>,
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favicon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub robots: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_graph: Option<OpenGraphMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitter: Option<TwitterMetadata>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub json_ld: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub custom: HashMap<String, String>,
}

/// Unified document representation returned by all modular parsers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedDocument {
    pub source_url: Url,
    pub final_url: Url,
    pub document_type: DocumentType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clean_html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_data: Option<serde_json::Value>,
    pub metadata: DocumentMetadata,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub links: Vec<PageLink>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub images: Vec<PageImage>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tables: Vec<DocumentTable>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub pages: Vec<DocumentPage>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub warnings: Vec<ScrapeWarning>,
}

/// Calculates word count on normalized text strings.
pub fn calculate_word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Computes BLAKE3 hex digest for binary or text content.
pub fn compute_blake3_hash(bytes: &[u8]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(bytes);
    hasher.finalize().to_hex().to_string()
}
