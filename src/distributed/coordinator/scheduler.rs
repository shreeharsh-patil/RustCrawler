use crate::distributed::coordinator::frontier::DistributedFrontier;
use crate::distributed::models::{
    JobRecord, JobStatus, JobType, JobUpdate, QueuedTask, SystemEventRecord, SystemEventType,
    TaskPriority, TaskRecord, TaskStatus, TaskType,
};
use crate::distributed::queue::TaskQueue;
use crate::distributed::store::MetadataStore;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::info;
use uuid::Uuid;

pub struct JobScheduler {
    store: Arc<dyn MetadataStore>,
    queue: Arc<dyn TaskQueue>,
    frontier: Arc<DistributedFrontier>,
    event_tx: broadcast::Sender<SystemEventRecord>,
}

impl JobScheduler {
    pub fn new(
        store: Arc<dyn MetadataStore>,
        queue: Arc<dyn TaskQueue>,
        frontier: Arc<DistributedFrontier>,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(10_000);
        Self {
            store,
            queue,
            frontier,
            event_tx,
        }
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<SystemEventRecord> {
        self.event_tx.subscribe()
    }

    pub fn frontier(&self) -> &Arc<DistributedFrontier> {
        &self.frontier
    }

    pub fn emit_event(&self, event: SystemEventRecord) {
        let _ = self.event_tx.send(event);
    }

    pub async fn submit_job(
        &self,
        job_type: JobType,
        config: serde_json::Value,
        tenant_id: Option<String>,
    ) -> Result<String, String> {
        let job_id = format!("job_{}", Uuid::new_v4().simple());
        let record = JobRecord::new(job_id.clone(), job_type, config.clone(), tenant_id);

        self.store
            .create_job(record)
            .await
            .map_err(|e| format!("Failed to create job: {e}"))?;

        // 1. If Scrape/Crawl/Map/Extract, create initial seed task
        let seed_url = config.get("url").and_then(|u| u.as_str()).unwrap_or("");
        if !seed_url.is_empty() {
            let task_type = match job_type {
                JobType::Scrape => TaskType::HttpFetch,
                JobType::Crawl | JobType::IncrementalCrawl => TaskType::HttpFetch,
                JobType::Map => TaskType::SitemapParse,
                JobType::Search => TaskType::Search,
                JobType::Extract => TaskType::Extract,
                JobType::Agent => TaskType::AgentTask,
            };

            let queued_task = QueuedTask::new(
                job_id.clone(),
                task_type,
                serde_json::json!({
                    "url": seed_url,
                    "depth": 0,
                    "config": config,
                }),
                TaskPriority::Normal,
                5,
            );

            let task_record = TaskRecord {
                task_id: queued_task.task_id.clone(),
                job_id: job_id.clone(),
                task_type,
                payload: queued_task.payload.clone(),
                priority: queued_task.priority,
                attempt: 0,
                max_attempts: 5,
                status: TaskStatus::Queued,
                lease_owner: None,
                lease_expiry: None,
                created_at: Utc::now().timestamp() as u64,
                available_at: queued_task.available_at,
                error: None,
            };

            self.store
                .store_task(task_record)
                .await
                .map_err(|e| format!("Failed to store task: {e}"))?;

            self.queue
                .enqueue(queued_task)
                .await
                .map_err(|e| format!("Failed to enqueue task: {e}"))?;
        }

        // Emit JobStarted event
        let event = SystemEventRecord {
            event_id: Uuid::new_v4().to_string(),
            job_id: job_id.clone(),
            event_type: SystemEventType::JobStarted,
            timestamp: Utc::now().timestamp() as u64,
            data: serde_json::json!({ "job_type": job_type.to_string() }),
        };
        let _ = self.store.store_event(event.clone()).await;
        self.emit_event(event);

        Ok(job_id)
    }

    pub async fn pause_job(&self, job_id: &str) -> Result<bool, String> {
        let update = JobUpdate {
            status: Some(JobStatus::Paused),
            ..Default::default()
        };
        self.store
            .update_job(job_id, update)
            .await
            .map_err(|e| format!("Failed to pause job: {e}"))?;

        let event = SystemEventRecord {
            event_id: Uuid::new_v4().to_string(),
            job_id: job_id.to_string(),
            event_type: SystemEventType::JobPaused,
            timestamp: Utc::now().timestamp() as u64,
            data: serde_json::json!({}),
        };
        let _ = self.store.store_event(event.clone()).await;
        self.emit_event(event);
        Ok(true)
    }

    pub async fn resume_job(&self, job_id: &str) -> Result<bool, String> {
        let update = JobUpdate {
            status: Some(JobStatus::Running),
            ..Default::default()
        };
        self.store
            .update_job(job_id, update)
            .await
            .map_err(|e| format!("Failed to resume job: {e}"))?;

        let event = SystemEventRecord {
            event_id: Uuid::new_v4().to_string(),
            job_id: job_id.to_string(),
            event_type: SystemEventType::JobResumed,
            timestamp: Utc::now().timestamp() as u64,
            data: serde_json::json!({}),
        };
        let _ = self.store.store_event(event.clone()).await;
        self.emit_event(event);
        Ok(true)
    }

    pub async fn cancel_job(&self, job_id: &str) -> Result<bool, String> {
        let update = JobUpdate {
            status: Some(JobStatus::Cancelled),
            completed_at: Some(Utc::now().timestamp() as u64),
            ..Default::default()
        };
        self.store
            .update_job(job_id, update)
            .await
            .map_err(|e| format!("Failed to cancel job: {e}"))?;

        let event = SystemEventRecord {
            event_id: Uuid::new_v4().to_string(),
            job_id: job_id.to_string(),
            event_type: SystemEventType::JobCancelled,
            timestamp: Utc::now().timestamp() as u64,
            data: serde_json::json!({}),
        };
        let _ = self.store.store_event(event.clone()).await;
        self.emit_event(event);
        Ok(true)
    }

    pub async fn run_scheduler_loop(&self, cancel_token: CancellationToken) {
        info!("Starting JobScheduler coordinator loop");
        let mut interval = tokio::time::interval(Duration::from_millis(500));

        while !cancel_token.is_cancelled() {
            interval.tick().await;

            // 1. Reclaim stale leases
            if let Ok(reclaimed) = self.queue.reclaim_stale_leases().await {
                if reclaimed > 0 {
                    info!("Reclaimed {} stale task leases", reclaimed);
                }
            }

            // 2. Check running jobs for completion
            if let Ok(jobs) = self.store.list_jobs(Some(JobStatus::Running), 100).await {
                for job in jobs {
                    if let Ok(tasks) = self.store.list_job_tasks(&job.id, 1000).await {
                        let active_or_queued = tasks.iter().any(|t| {
                            t.status == TaskStatus::Queued
                                || t.status == TaskStatus::Leased
                                || t.status == TaskStatus::Running
                        });

                        if !active_or_queued && !tasks.is_empty() {
                            // All tasks finished for this job -> mark completed
                            let update = JobUpdate {
                                status: Some(JobStatus::Completed),
                                completed_at: Some(Utc::now().timestamp() as u64),
                                ..Default::default()
                            };
                            let _ = self.store.update_job(&job.id, update).await;

                            let event = SystemEventRecord {
                                event_id: Uuid::new_v4().to_string(),
                                job_id: job.id.clone(),
                                event_type: SystemEventType::JobCompleted,
                                timestamp: Utc::now().timestamp() as u64,
                                data: serde_json::json!({ "status": "completed" }),
                            };
                            let _ = self.store.store_event(event.clone()).await;
                            self.emit_event(event);
                        }
                    }
                }
            }
        }
        info!("JobScheduler loop stopped gracefully");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributed::queue::memory::MemoryTaskQueue;
    use crate::distributed::store::memory::MemoryMetadataStore;

    #[tokio::test]
    async fn test_job_submission_and_lifecycle() {
        let store = Arc::new(MemoryMetadataStore::new());
        let queue = Arc::new(MemoryTaskQueue::new(100));
        let frontier = Arc::new(DistributedFrontier::new());

        let scheduler = JobScheduler::new(store.clone(), queue.clone(), frontier);
        let mut rx = scheduler.subscribe_events();

        let config = serde_json::json!({
            "url": "https://example.com",
            "limit": 100
        });

        let job_id = scheduler
            .submit_job(JobType::Crawl, config, Some("tenant_123".to_string()))
            .await
            .expect("Job submission should succeed");

        let start_event = rx.recv().await.unwrap();
        assert_eq!(start_event.job_id, job_id);
        assert!(matches!(
            start_event.event_type,
            SystemEventType::JobStarted
        ));

        scheduler.pause_job(&job_id).await.unwrap();
        let job_paused = store.get_job(&job_id).await.unwrap().unwrap();
        assert_eq!(job_paused.status, JobStatus::Paused);

        scheduler.resume_job(&job_id).await.unwrap();
        let job_resumed = store.get_job(&job_id).await.unwrap().unwrap();
        assert_eq!(job_resumed.status, JobStatus::Running);

        scheduler.cancel_job(&job_id).await.unwrap();
        let job_cancelled = store.get_job(&job_id).await.unwrap().unwrap();
        assert_eq!(job_cancelled.status, JobStatus::Cancelled);
    }
}
