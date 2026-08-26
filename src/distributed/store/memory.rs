use crate::distributed::models::{
    JobRecord, JobStatus, JobUpdate, SystemEventRecord, TaskLease, TaskRecord, TaskStatus,
    WorkerInfo,
};
use crate::distributed::store::{MetadataStore, StoreError};
use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Default)]
pub struct MemoryMetadataStore {
    jobs: Arc<DashMap<String, JobRecord>>,
    tasks: Arc<DashMap<String, TaskRecord>>,
    workers: Arc<DashMap<String, WorkerInfo>>,
    events: Arc<DashMap<String, Vec<SystemEventRecord>>>,
}

impl MemoryMetadataStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl MetadataStore for MemoryMetadataStore {
    async fn create_job(&self, job: JobRecord) -> Result<(), StoreError> {
        self.jobs.insert(job.id.clone(), job);
        Ok(())
    }

    async fn update_job(&self, job_id: &str, update: JobUpdate) -> Result<(), StoreError> {
        if let Some(mut job) = self.jobs.get_mut(job_id) {
            if let Some(status) = update.status {
                job.status = status;
            }
            if let Some(started_at) = update.started_at {
                job.started_at = Some(started_at);
            }
            if let Some(completed_at) = update.completed_at {
                job.completed_at = Some(completed_at);
            }
            if let Some(progress) = update.progress {
                job.progress = progress;
            }
            if let Some(error_summary) = update.error_summary {
                job.error_summary = Some(error_summary);
            }
            if let Some(result_location) = update.result_location {
                job.result_location = Some(result_location);
            }
            Ok(())
        } else {
            Err(StoreError::JobNotFound(job_id.to_string()))
        }
    }

    async fn get_job(&self, id: &str) -> Result<Option<JobRecord>, StoreError> {
        Ok(self.jobs.get(id).map(|r| r.clone()))
    }

    async fn list_jobs(
        &self,
        status: Option<JobStatus>,
        limit: usize,
    ) -> Result<Vec<JobRecord>, StoreError> {
        let mut list: Vec<JobRecord> = self
            .jobs
            .iter()
            .filter(|entry| status.is_none_or(|s| entry.value().status == s))
            .map(|entry| entry.value().clone())
            .collect();

        list.sort_by_key(|a| std::cmp::Reverse(a.created_at));
        if list.len() > limit {
            list.truncate(limit);
        }
        Ok(list)
    }

    async fn store_task(&self, task: TaskRecord) -> Result<(), StoreError> {
        self.tasks.insert(task.task_id.clone(), task);
        Ok(())
    }

    async fn update_task_status(
        &self,
        task_id: &str,
        status: TaskStatus,
        lease: Option<&TaskLease>,
        error: Option<String>,
    ) -> Result<(), StoreError> {
        if let Some(mut task) = self.tasks.get_mut(task_id) {
            task.status = status;
            if let Some(l) = lease {
                task.lease_owner = Some(l.worker_id.clone());
                task.lease_expiry = Some(l.lease_expiry);
                task.attempt = l.attempt;
            } else if status == TaskStatus::Completed
                || status == TaskStatus::Failed
                || status == TaskStatus::DeadLetter
            {
                task.lease_owner = None;
                task.lease_expiry = None;
            }
            if let Some(err) = error {
                task.error = Some(err);
            }
            Ok(())
        } else {
            Err(StoreError::TaskNotFound(task_id.to_string()))
        }
    }

    async fn get_task(&self, task_id: &str) -> Result<Option<TaskRecord>, StoreError> {
        Ok(self.tasks.get(task_id).map(|r| r.clone()))
    }

    async fn list_job_tasks(
        &self,
        job_id: &str,
        limit: usize,
    ) -> Result<Vec<TaskRecord>, StoreError> {
        let mut list: Vec<TaskRecord> = self
            .tasks
            .iter()
            .filter(|entry| entry.value().job_id == job_id)
            .map(|entry| entry.value().clone())
            .collect();

        list.sort_by_key(|a| a.created_at);
        if list.len() > limit {
            list.truncate(limit);
        }
        Ok(list)
    }

    async fn register_worker(&self, worker: WorkerInfo) -> Result<(), StoreError> {
        self.workers.insert(worker.worker_id.clone(), worker);
        Ok(())
    }

    async fn heartbeat_worker(
        &self,
        worker_id: &str,
        active_tasks: usize,
    ) -> Result<bool, StoreError> {
        let now = Utc::now().timestamp() as u64;
        if let Some(mut worker) = self.workers.get_mut(worker_id) {
            worker.last_heartbeat = now;
            worker.active_tasks = active_tasks;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn list_active_workers(&self, max_age: Duration) -> Result<Vec<WorkerInfo>, StoreError> {
        let now = Utc::now().timestamp() as u64;
        let threshold = now.saturating_sub(max_age.as_secs());

        let mut list: Vec<WorkerInfo> = self
            .workers
            .iter()
            .filter(|entry| entry.value().last_heartbeat >= threshold)
            .map(|entry| entry.value().clone())
            .collect();

        list.sort_by_key(|a| a.started_at);
        Ok(list)
    }

    async fn store_event(&self, event: SystemEventRecord) -> Result<(), StoreError> {
        let job_id = event.job_id.clone();
        let mut entry = self.events.entry(job_id).or_default();
        entry.push(event);
        Ok(())
    }

    async fn get_job_events(
        &self,
        job_id: &str,
        after_timestamp: Option<u64>,
        limit: usize,
    ) -> Result<Vec<SystemEventRecord>, StoreError> {
        if let Some(events) = self.events.get(job_id) {
            let mut filtered: Vec<SystemEventRecord> = events
                .iter()
                .filter(|e| after_timestamp.is_none_or(|ts| e.timestamp > ts))
                .cloned()
                .collect();
            if filtered.len() > limit {
                filtered.truncate(limit);
            }
            Ok(filtered)
        } else {
            Ok(Vec::new())
        }
    }
}
