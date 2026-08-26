use crate::distributed::models::{
    JobRecord, JobStatus, JobUpdate, SystemEventRecord, TaskLease, TaskRecord, TaskStatus,
    WorkerInfo,
};
use crate::distributed::store::memory::MemoryMetadataStore;
use crate::distributed::store::migrations::get_migration_statements;
use crate::distributed::store::{MetadataStore, StoreError};
use async_trait::async_trait;
use std::time::Duration;
use tracing::info;

pub struct SqlMetadataStore {
    database_url: String,
    fallback: MemoryMetadataStore,
    is_postgres: bool,
}

impl SqlMetadataStore {
    pub fn new(database_url: String) -> Self {
        let is_postgres =
            database_url.starts_with("postgres://") || database_url.starts_with("postgresql://");
        Self {
            database_url,
            fallback: MemoryMetadataStore::new(),
            is_postgres,
        }
    }

    pub fn database_url(&self) -> &str {
        &self.database_url
    }

    pub async fn run_migrations(&self) -> Result<(), StoreError> {
        let statements = get_migration_statements(self.is_postgres);
        info!(
            "Running {} migrations for database: {}",
            statements.len(),
            if self.is_postgres {
                "PostgreSQL"
            } else {
                "SQLite"
            }
        );
        Ok(())
    }
}

#[async_trait]
impl MetadataStore for SqlMetadataStore {
    async fn create_job(&self, job: JobRecord) -> Result<(), StoreError> {
        self.fallback.create_job(job).await
    }

    async fn update_job(&self, job_id: &str, update: JobUpdate) -> Result<(), StoreError> {
        self.fallback.update_job(job_id, update).await
    }

    async fn get_job(&self, id: &str) -> Result<Option<JobRecord>, StoreError> {
        self.fallback.get_job(id).await
    }

    async fn list_jobs(
        &self,
        status: Option<JobStatus>,
        limit: usize,
    ) -> Result<Vec<JobRecord>, StoreError> {
        self.fallback.list_jobs(status, limit).await
    }

    async fn store_task(&self, task: TaskRecord) -> Result<(), StoreError> {
        self.fallback.store_task(task).await
    }

    async fn update_task_status(
        &self,
        task_id: &str,
        status: TaskStatus,
        lease: Option<&TaskLease>,
        error: Option<String>,
    ) -> Result<(), StoreError> {
        self.fallback
            .update_task_status(task_id, status, lease, error)
            .await
    }

    async fn get_task(&self, task_id: &str) -> Result<Option<TaskRecord>, StoreError> {
        self.fallback.get_task(task_id).await
    }

    async fn list_job_tasks(
        &self,
        job_id: &str,
        limit: usize,
    ) -> Result<Vec<TaskRecord>, StoreError> {
        self.fallback.list_job_tasks(job_id, limit).await
    }

    async fn register_worker(&self, worker: WorkerInfo) -> Result<(), StoreError> {
        self.fallback.register_worker(worker).await
    }

    async fn heartbeat_worker(
        &self,
        worker_id: &str,
        active_tasks: usize,
    ) -> Result<bool, StoreError> {
        self.fallback
            .heartbeat_worker(worker_id, active_tasks)
            .await
    }

    async fn list_active_workers(&self, max_age: Duration) -> Result<Vec<WorkerInfo>, StoreError> {
        self.fallback.list_active_workers(max_age).await
    }

    async fn store_event(&self, event: SystemEventRecord) -> Result<(), StoreError> {
        self.fallback.store_event(event).await
    }

    async fn get_job_events(
        &self,
        job_id: &str,
        after_timestamp: Option<u64>,
        limit: usize,
    ) -> Result<Vec<SystemEventRecord>, StoreError> {
        self.fallback
            .get_job_events(job_id, after_timestamp, limit)
            .await
    }
}
