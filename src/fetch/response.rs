use crate::models::ScrapeWarning;
use url::Url;

#[derive(Debug, Clone)]
pub struct FetchedDocument {
    pub initial_url: Url,
    pub final_url: Url,
    pub status: u16,
    pub content_type: String,
    pub content_length: usize,
    pub body: Vec<u8>,
    pub html: String,
    pub fetch_time_ms: u64,
    pub warnings: Vec<ScrapeWarning>,
}
