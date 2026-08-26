pub mod dedupe;
pub mod engine;
pub mod filters;
pub mod frontier;
pub mod incremental;
pub mod job;
pub mod job_store;
pub mod models;
pub mod progress;
pub mod retry;
pub mod robots;
pub mod scheduler;
pub mod sitemap;
pub mod snapshot;
pub mod worker;

pub use engine::execute_crawl;
pub use incremental::{
    ChangeSummary, ChangeType, IncrementalCrawlRequest, IncrementalCrawlResponse,
    IncrementalCrawler,
};
pub use job::CrawlJob;
pub use job_store::JobStore;
pub use models::{
    CrawlJobInitResponse, CrawlJobStatusResponse, CrawlOptions, CrawlRequest, CrawlResult,
    CrawlStats, CrawlStatus, CrawlTarget, CrawledPage, DiscoverySource, PageTimings,
};
pub use progress::CrawlProgress;
pub use snapshot::{CrawlSnapshot, PageSnapshot, SnapshotStore};

use crate::error::CrawlerError;
use crate::service::ScraperService;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// High-level crawler service providing synchronous crawl execution and background job lifecycle management.
#[derive(Clone)]
pub struct CrawlerService {
    scraper: Arc<ScraperService>,
    job_store: Arc<JobStore>,
    snapshot_store: Arc<SnapshotStore>,
}

impl CrawlerService {
    pub fn new(scraper: Arc<ScraperService>) -> Self {
        let config = scraper.config();
        let job_store = Arc::new(JobStore::new(
            config.max_stored_jobs,
            config.job_retention_seconds,
        ));
        let snapshot_store = Arc::new(SnapshotStore::new());
        Self {
            scraper,
            job_store,
            snapshot_store,
        }
    }

    /// Executes a crawl synchronously (used by CLI and direct service calls).
    pub async fn crawl(
        &self,
        seed_url: &str,
        options: CrawlOptions,
        cancel: CancellationToken,
    ) -> Result<CrawlResult, CrawlerError> {
        let progress = Arc::new(CrawlProgress::new());
        execute_crawl(seed_url, options, self.scraper.clone(), cancel, progress).await
    }

    /// Starts a background crawl job (used by POST /v1/crawl).
    pub fn start_job(
        &self,
        seed_url: String,
        options: CrawlOptions,
    ) -> Result<Arc<CrawlJob>, CrawlerError> {
        self.job_store
            .create_and_spawn_job(seed_url, options, self.scraper.clone())
    }

    /// Retrieves an existing crawl job by ID (used by GET /v1/crawl/{job_id}).
    pub fn get_job(&self, job_id: &str) -> Result<Arc<CrawlJob>, CrawlerError> {
        self.job_store.get_job(job_id)
    }

    /// Cancels an existing crawl job by ID (used by DELETE /v1/crawl/{job_id}).
    pub fn cancel_job(&self, job_id: &str) -> Result<Arc<CrawlJob>, CrawlerError> {
        self.job_store.cancel_job(job_id)
    }

    /// Deletes an existing crawl job from storage.
    pub fn delete_job(&self, job_id: &str) -> Result<(), CrawlerError> {
        self.job_store.delete_job(job_id)
    }

    pub fn scraper(&self) -> &Arc<ScraperService> {
        &self.scraper
    }

    pub fn job_store(&self) -> &Arc<JobStore> {
        &self.job_store
    }

    pub fn snapshot_store(&self) -> &Arc<SnapshotStore> {
        &self.snapshot_store
    }
}
