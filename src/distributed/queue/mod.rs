pub mod memory;
pub mod redis_queue;

pub use memory::MemoryTaskQueue;
pub use redis_queue::RedisTaskQueue;

use crate::distributed::models::{
    QueueStats, QueuedTask, TaskFailure, TaskLease, TaskRecord, WorkerType,
};
use async_trait::async_trait;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum QueueError {
    #[error("Queue operation failed: {0}")]
    OperationFailed(String),

    #[error("Lease not found or expired: {0}")]
    LeaseExpired(String),

    #[error("Queue is full, cannot accept more tasks")]
    QueueFull,

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Redis connection error: {0}")]
    RedisError(String),
}

#[async_trait]
pub trait TaskQueue: Send + Sync {
    async fn enqueue(&self, task: QueuedTask) -> Result<(), QueueError>;
    async fn reserve(
        &self,
        worker_id: &str,
        worker_type: WorkerType,
        lease_duration: Duration,
    ) -> Result<Option<TaskLease>, QueueError>;
    async fn heartbeat(&self, lease_id: &str, extension: Duration) -> Result<bool, QueueError>;
    async fn ack(&self, lease: TaskLease) -> Result<(), QueueError>;
    async fn retry(&self, lease: TaskLease, delay: Duration) -> Result<(), QueueError>;
    async fn fail(&self, lease: TaskLease, error: TaskFailure) -> Result<(), QueueError>;
    async fn reclaim_stale_leases(&self) -> Result<usize, QueueError>;
    async fn stats(&self) -> Result<QueueStats, QueueError>;
    async fn dlq_list(&self, limit: usize) -> Result<Vec<TaskRecord>, QueueError>;
    async fn dlq_retry(&self, task_id: &str) -> Result<bool, QueueError>;
}
