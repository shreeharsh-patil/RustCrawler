use crate::crawl::job::CrawlJob;
use crate::crawl::models::CrawlOptions;
use crate::error::CrawlerError;
use crate::service::ScraperService;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;

/// In-memory concurrent store for active and completed crawl jobs.
pub struct JobStore {
    jobs: DashMap<String, Arc<CrawlJob>>,
    max_stored_jobs: usize,
    job_retention: Duration,
}

impl JobStore {
    pub fn new(max_stored_jobs: usize, job_retention_seconds: u64) -> Self {
        Self {
            jobs: DashMap::new(),
            max_stored_jobs,
            job_retention: Duration::from_secs(job_retention_seconds),
        }
    }

    /// Spawns a new crawl job and stores it in the job manager.
    pub fn create_and_spawn_job(
        &self,
        seed_url: String,
        options: CrawlOptions,
        scraper: Arc<ScraperService>,
    ) -> Result<Arc<CrawlJob>, CrawlerError> {
        self.cleanup_expired();

        if self.jobs.len() >= self.max_stored_jobs {
            // Attempt to evict the oldest completed job
            if !self.evict_oldest_completed() {
                return Err(CrawlerError::JobStoreFull(self.max_stored_jobs));
            }
        }

        let job = Arc::new(CrawlJob::new(seed_url));
        self.jobs.insert(job.job_id.clone(), job.clone());

        job.spawn_execution(options, scraper);

        Ok(job)
    }

    /// Retrieves an existing crawl job by ID.
    pub fn get_job(&self, job_id: &str) -> Result<Arc<CrawlJob>, CrawlerError> {
        self.jobs
            .get(job_id)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| CrawlerError::JobNotFound(job_id.to_string()))
    }

    /// Cancels an active crawl job.
    pub fn cancel_job(&self, job_id: &str) -> Result<Arc<CrawlJob>, CrawlerError> {
        let job = self.get_job(job_id)?;
        job.cancel.cancel();
        Ok(job)
    }

    /// Deletes a crawl job from storage.
    pub fn delete_job(&self, job_id: &str) -> Result<(), CrawlerError> {
        let job = self.get_job(job_id)?;
        job.cancel.cancel();
        self.jobs.remove(job_id);
        Ok(())
    }

    /// Cleans up jobs that have exceeded the retention window.
    pub fn cleanup_expired(&self) {
        let now = std::time::Instant::now();
        self.jobs.retain(|_, job| {
            if now.duration_since(job.started_at_instant) > self.job_retention {
                // Remove expired
                false
            } else {
                true
            }
        });
    }

    fn evict_oldest_completed(&self) -> bool {
        let mut oldest_id: Option<String> = None;
        let mut oldest_age = Duration::ZERO;
        let now = std::time::Instant::now();

        for entry in self.jobs.iter() {
            let age = now.duration_since(entry.value().started_at_instant);
            if age > oldest_age {
                oldest_age = age;
                oldest_id = Some(entry.key().clone());
            }
        }

        if let Some(id) = oldest_id {
            self.jobs.remove(&id);
            true
        } else {
            false
        }
    }
}
