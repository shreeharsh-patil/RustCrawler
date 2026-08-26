use crate::crawl::engine::execute_crawl;
use crate::crawl::models::{CrawlOptions, CrawlResult, CrawlStatus};
use crate::crawl::progress::CrawlProgress;
use crate::service::ScraperService;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::error;
use uuid::Uuid;

pub struct CrawlJob {
    pub job_id: String,
    pub seed_url: String,
    pub status: Arc<Mutex<CrawlStatus>>,
    pub started_at_instant: Instant,
    pub started_at_timestamp: String,
    pub completed_at_timestamp: Arc<Mutex<Option<String>>>,
    pub progress: Arc<CrawlProgress>,
    pub result: Arc<Mutex<Option<CrawlResult>>>,
    pub cancel: CancellationToken,
}

impl CrawlJob {
    pub fn new(seed_url: String) -> Self {
        let job_id = Uuid::new_v4().to_string();
        let timestamp = current_epoch_seconds();

        Self {
            job_id,
            seed_url,
            status: Arc::new(Mutex::new(CrawlStatus::Queued)),
            started_at_instant: Instant::now(),
            started_at_timestamp: timestamp,
            completed_at_timestamp: Arc::new(Mutex::new(None)),
            progress: Arc::new(CrawlProgress::new()),
            result: Arc::new(Mutex::new(None)),
            cancel: CancellationToken::new(),
        }
    }

    /// Spawns the crawl execution in the background.
    pub fn spawn_execution(self: &Arc<Self>, options: CrawlOptions, scraper: Arc<ScraperService>) {
        let job_clone = self.clone();

        tokio::spawn(async move {
            {
                let mut st = job_clone.status.lock().await;
                *st = CrawlStatus::Running;
            }

            match execute_crawl(
                &job_clone.seed_url,
                options,
                scraper,
                job_clone.cancel.clone(),
                job_clone.progress.clone(),
            )
            .await
            {
                Ok(crawl_result) => {
                    let mut st = job_clone.status.lock().await;
                    *st = crawl_result.status;

                    let mut comp = job_clone.completed_at_timestamp.lock().await;
                    *comp = crawl_result.completed_at.clone();

                    let mut res = job_clone.result.lock().await;
                    *res = Some(crawl_result);
                }
                Err(err) => {
                    error!(job_id = %job_clone.job_id, "Crawl failed with fatal error: {}", err);
                    let mut st = job_clone.status.lock().await;
                    *st = CrawlStatus::Failed;

                    let mut comp = job_clone.completed_at_timestamp.lock().await;
                    *comp = Some(current_epoch_seconds());
                }
            }
        });
    }

    /// Builds a snapshot of the current job status and result.
    pub async fn to_result_snapshot(&self) -> CrawlResult {
        let status = *self.status.lock().await;
        let completed_at = self.completed_at_timestamp.lock().await.clone();

        // If completed result is available, return it
        if let Some(ref res) = *self.result.lock().await {
            return res.clone();
        }

        // Otherwise return in-progress snapshot
        CrawlResult {
            job_id: self.job_id.clone(),
            status,
            seed_url: self.seed_url.clone(),
            started_at: self.started_at_timestamp.clone(),
            completed_at,
            stats: self.progress.snapshot(),
            pages: Vec::new(),
        }
    }
}

fn current_epoch_seconds() -> String {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => format!("{}", d.as_secs()),
        Err(_) => "0".to_string(),
    }
}
