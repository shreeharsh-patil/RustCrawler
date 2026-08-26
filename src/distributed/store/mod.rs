pub mod memory;
pub mod migrations;
pub mod sql;

pub use memory::MemoryMetadataStore;
pub use sql::SqlMetadataStore;

use crate::distributed::models::{
    JobRecord, JobStatus, JobUpdate, SystemEventRecord, TaskLease, TaskRecord, TaskStatus,
    WorkerInfo,
};
use async_trait::async_trait;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("Job not found: {0}")]
    JobNotFound(String),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Conflict error: {0}")]
    Conflict(String),
}

#[async_trait]
pub trait MetadataStore: Send + Sync {
    async fn create_job(&self, job: JobRecord) -> Result<(), StoreError>;
    async fn update_job(&self, job_id: &str, update: JobUpdate) -> Result<(), StoreError>;
    async fn get_job(&self, id: &str) -> Result<Option<JobRecord>, StoreError>;
    async fn list_jobs(
        &self,
        status: Option<JobStatus>,
        limit: usize,
    ) -> Result<Vec<JobRecord>, StoreError>;

    async fn store_task(&self, task: TaskRecord) -> Result<(), StoreError>;
    async fn update_task_status(
        &self,
        task_id: &str,
        status: TaskStatus,
        lease: Option<&TaskLease>,
        error: Option<String>,
    ) -> Result<(), StoreError>;
    async fn get_task(&self, task_id: &str) -> Result<Option<TaskRecord>, StoreError>;
    async fn list_job_tasks(
        &self,
        job_id: &str,
        limit: usize,
    ) -> Result<Vec<TaskRecord>, StoreError>;

    async fn register_worker(&self, worker: WorkerInfo) -> Result<(), StoreError>;
    async fn heartbeat_worker(
        &self,
        worker_id: &str,
        active_tasks: usize,
    ) -> Result<bool, StoreError>;
    async fn list_active_workers(&self, max_age: Duration) -> Result<Vec<WorkerInfo>, StoreError>;

    async fn store_event(&self, event: SystemEventRecord) -> Result<(), StoreError>;
    async fn get_job_events(
        &self,
        job_id: &str,
        after_timestamp: Option<u64>,
        limit: usize,
    ) -> Result<Vec<SystemEventRecord>, StoreError>;
}
