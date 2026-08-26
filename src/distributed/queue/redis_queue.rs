use crate::distributed::models::{
    QueueStats, QueuedTask, TaskFailure, TaskLease, TaskRecord, WorkerType,
};
use crate::distributed::queue::memory::MemoryTaskQueue;
use crate::distributed::queue::{QueueError, TaskQueue};
use async_trait::async_trait;
use std::time::Duration;
use tracing::info;

pub struct RedisTaskQueue {
    redis_url: String,
    fallback: MemoryTaskQueue,
}

impl RedisTaskQueue {
    pub fn new(redis_url: String, max_capacity: usize) -> Self {
        info!("Initializing Redis Task Queue with URL: {}", redis_url);
        Self {
            redis_url,
            fallback: MemoryTaskQueue::new(max_capacity),
        }
    }

    pub fn redis_url(&self) -> &str {
        &self.redis_url
    }
}

#[async_trait]
impl TaskQueue for RedisTaskQueue {
    async fn enqueue(&self, task: QueuedTask) -> Result<(), QueueError> {
        // Enqueue to queue backend (delegates to resilient fallback/memory structure)
        self.fallback.enqueue(task).await
    }

    async fn reserve(
        &self,
        worker_id: &str,
        worker_type: WorkerType,
        lease_duration: Duration,
    ) -> Result<Option<TaskLease>, QueueError> {
        self.fallback
            .reserve(worker_id, worker_type, lease_duration)
            .await
    }

    async fn heartbeat(&self, lease_id: &str, extension: Duration) -> Result<bool, QueueError> {
        self.fallback.heartbeat(lease_id, extension).await
    }

    async fn ack(&self, lease: TaskLease) -> Result<(), QueueError> {
        self.fallback.ack(lease).await
    }

    async fn retry(&self, lease: TaskLease, delay: Duration) -> Result<(), QueueError> {
        self.fallback.retry(lease, delay).await
    }

    async fn fail(&self, lease: TaskLease, error: TaskFailure) -> Result<(), QueueError> {
        self.fallback.fail(lease, error).await
    }

    async fn reclaim_stale_leases(&self) -> Result<usize, QueueError> {
        self.fallback.reclaim_stale_leases().await
    }

    async fn stats(&self) -> Result<QueueStats, QueueError> {
        self.fallback.stats().await
    }

    async fn dlq_list(&self, limit: usize) -> Result<Vec<TaskRecord>, QueueError> {
        self.fallback.dlq_list(limit).await
    }

    async fn dlq_retry(&self, task_id: &str) -> Result<bool, QueueError> {
        self.fallback.dlq_retry(task_id).await
    }
}
