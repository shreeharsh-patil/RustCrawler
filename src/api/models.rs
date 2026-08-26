use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserHealthInfo {
    pub enabled: bool,
    pub status: String,
    pub active_pages: usize,
    pub max_pages: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub http_scraper: &'static str,
    pub document_parsers: &'static str,
    pub extraction_engine: &'static str,
    pub discovery_engine: &'static str,
    pub search_engine: &'static str,
    pub cache_store: &'static str,
    pub browser: BrowserHealthInfo,
}
