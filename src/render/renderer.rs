use crate::error::CrawlerError;
use crate::render::models::{RenderRequest, RenderResult};
use async_trait::async_trait;

/// Abstract interface for page rendering independent from Chromium specifics.
#[async_trait]
pub trait PageRenderer: Send + Sync {
    async fn render(&self, request: RenderRequest) -> Result<RenderResult, CrawlerError>;
}
