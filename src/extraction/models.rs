use crate::models::ScrapeWarning;
use crate::render::models::RenderMode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionMode {
    Deterministic,
    Llm,
    #[default]
    Auto,
}

impl std::str::FromStr for ExtractionMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "deterministic" | "rules" => Ok(ExtractionMode::Deterministic),
            "llm" | "ai" => Ok(ExtractionMode::Llm),
            "auto" | "hybrid" => Ok(ExtractionMode::Auto),
            _ => Err(format!("Unknown extraction mode: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MissingFieldBehavior {
    #[default]
    Null,
    Omit,
    Error,
}

impl std::str::FromStr for MissingFieldBehavior {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "null" => Ok(MissingFieldBehavior::Null),
            "omit" => Ok(MissingFieldBehavior::Omit),
            "error" => Ok(MissingFieldBehavior::Error),
            _ => Err(format!("Unknown missing field behavior: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionSource {
    JsonLd,
    EmbeddedJson,
    NetworkJson,
    MetaTag,
    HtmlTable,
    HtmlText,
    PdfText,
    DocxText,
    Llm,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtractedField<T> {
    pub value: T,
    pub confidence: f32,
    pub source: ExtractionSource,
    pub excerpt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldProvenance {
    pub field_path: String,
    pub source_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_id: Option<String>,
    pub source: ExtractionSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkType {
    Paragraph,
    Section,
    Table,
    Json,
    Metadata,
    FeedEntry,
    PdfPage,
    DocxSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentChunk {
    pub id: String,
    pub source_url: Url,
    pub heading_path: Vec<String>,
    pub content: String,
    pub content_type: ChunkType,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractionCrawlOptions {
    #[serde(default = "default_crawl_limit")]
    pub limit: usize,
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    #[serde(default)]
    pub include_paths: Vec<String>,
    #[serde(default)]
    pub exclude_paths: Vec<String>,
    #[serde(default)]
    pub allow_subdomains: bool,
}

fn default_crawl_limit() -> usize {
    20
}

fn default_max_depth() -> u32 {
    2
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractionRequest {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub urls: Vec<String>,
    #[serde(default)]
    pub crawl: Option<ExtractionCrawlOptions>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub schema: Option<serde_json::Value>,
    #[serde(default)]
    pub mode: Option<ExtractionMode>,
    #[serde(default)]
    pub include_provenance: Option<bool>,
    #[serde(default)]
    pub missing_field_behavior: Option<MissingFieldBehavior>,
    #[serde(default)]
    pub render_mode: Option<RenderMode>,
    #[serde(default)]
    pub dedupe_by: Vec<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionMetadata {
    pub mode: ExtractionMode,
    pub extractor: String,
    pub validated: bool,
    pub sources_used: usize,
    pub chunks_used: usize,
    pub llm_calls: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_estimate_usd: Option<f64>,
    pub duration_ms: u64,
    pub cache_hit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    pub metadata: ExtractionMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<HashMap<String, FieldProvenance>>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub warnings: Vec<ScrapeWarning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    pub metadata: ExtractionMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<HashMap<String, FieldProvenance>>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub warnings: Vec<ScrapeWarning>,
}

impl From<ExtractionResult> for ExtractionResponse {
    fn from(res: ExtractionResult) -> Self {
        Self {
            success: res.success,
            data: res.data,
            metadata: res.metadata,
            provenance: res.provenance,
            warnings: res.warnings,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchExtractionItem {
    pub url: String,
    #[serde(default)]
    pub schema: Option<serde_json::Value>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub mode: Option<ExtractionMode>,
    #[serde(default)]
    pub include_provenance: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchExtractionRequest {
    pub items: Vec<BatchExtractionItem>,
    #[serde(default = "default_batch_concurrency")]
    pub max_concurrency: usize,
    #[serde(default)]
    pub default_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub default_prompt: Option<String>,
    #[serde(default)]
    pub default_mode: Option<ExtractionMode>,
    #[serde(default)]
    pub default_include_provenance: Option<bool>,
}

fn default_batch_concurrency() -> usize {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchItemResult {
    pub url: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ExtractionMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<HashMap<String, FieldProvenance>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub warnings: Vec<ScrapeWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchExtractionResponse {
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub results: Vec<BatchItemResult>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionJobStatus {
    Queued,
    Scraping,
    SelectingContext,
    Extracting,
    Validating,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionJobInfo {
    pub job_id: String,
    pub status: ExtractionJobStatus,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<ExtractionResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmExtractionRequest {
    pub system_prompt: String,
    pub user_prompt: String,
    pub schema: Option<serde_json::Value>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmUsage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
    pub estimated_cost_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmExtractionResponse {
    pub raw_content: String,
    pub parsed_json: Option<serde_json::Value>,
    pub usage: LlmUsage,
    pub model: String,
    pub duration_ms: u64,
}
