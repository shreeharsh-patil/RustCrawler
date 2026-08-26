use crate::distributed::models::{
    QueueStats, QueuedTask, TaskFailure, TaskLease, TaskPriority, TaskRecord, TaskStatus, TaskType,
    WorkerType,
};
use crate::distributed::queue::{QueueError, TaskQueue};
use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone)]
#[allow(clippy::type_complexity)]
pub struct MemoryTaskQueue {
    // Queues partitioned by (TaskType, Priority 0..3)
    queues: Arc<Mutex<HashMap<(TaskType, TaskPriority), VecDeque<QueuedTask>>>>,
    active_leases: Arc<DashMap<String, (TaskLease, QueuedTask, usize)>>, // lease_id -> (Lease, OriginalTask, attempt)
    dlq: Arc<DashMap<String, TaskRecord>>,
    max_capacity: usize,
    completed_count: Arc<std::sync::atomic::AtomicUsize>,
    failed_count: Arc<std::sync::atomic::AtomicUsize>,
}

impl MemoryTaskQueue {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            queues: Arc::new(Mutex::new(HashMap::new())),
            active_leases: Arc::new(DashMap::new()),
            dlq: Arc::new(DashMap::new()),
            max_capacity,
            completed_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            failed_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }
}

impl Default for MemoryTaskQueue {
    fn default() -> Self {
        Self::new(100_000)
    }
}

#[async_trait]
impl TaskQueue for MemoryTaskQueue {
    async fn enqueue(&self, task: QueuedTask) -> Result<(), QueueError> {
        let mut map = self.queues.lock().await;
        let total_queued: usize = map.values().map(|q| q.len()).sum();
        if total_queued >= self.max_capacity {
            return Err(QueueError::QueueFull);
        }

        let key = (task.task_type, task.priority);
        let queue = map.entry(key).or_insert_with(VecDeque::new);
        queue.push_back(task);
        Ok(())
    }

    async fn reserve(
        &self,
        worker_id: &str,
        worker_type: WorkerType,
        lease_duration: Duration,
    ) -> Result<Option<TaskLease>, QueueError> {
        let mut map = self.queues.lock().await;
        let now = Utc::now().timestamp() as u64;

        let priorities = [
            TaskPriority::Critical,
            TaskPriority::High,
            TaskPriority::Normal,
            TaskPriority::Low,
        ];

        let task_types = [
            TaskType::HttpFetch,
            TaskType::BrowserRender,
            TaskType::DocumentParse,
            TaskType::Extract,
            TaskType::SitemapParse,
            TaskType::Search,
        ];

        for &prio in &priorities {
            for &tt in &task_types {
                if !worker_type.can_handle(tt) {
                    continue;
                }

                let key = (tt, prio);
                if let Some(queue) = map.get_mut(&key) {
                    // Find first available task
                    let mut found_idx = None;
                    for (idx, task) in queue.iter().enumerate() {
                        if task.available_at <= now {
                            found_idx = Some(idx);
                            break;
                        }
                    }

                    if let Some(idx) = found_idx {
                        let task = queue.remove(idx).unwrap();
                        let lease_id = Uuid::new_v4().to_string();
                        let now_ms = Utc::now().timestamp_millis() as u64;
                        let lease_expiry = now_ms + (lease_duration.as_millis() as u64);

                        let lease = TaskLease {
                            lease_id: lease_id.clone(),
                            task_id: task.task_id.clone(),
                            job_id: task.job_id.clone(),
                            worker_id: worker_id.to_string(),
                            task_type: task.task_type,
                            payload: task.payload.clone(),
                            attempt: 1,
                            max_attempts: task.max_attempts,
                            lease_expiry,
                        };

                        self.active_leases
                            .insert(lease_id, (lease.clone(), task, 1));
                        return Ok(Some(lease));
                    }
                }
            }
        }

        Ok(None)
    }

    async fn heartbeat(&self, lease_id: &str, extension: Duration) -> Result<bool, QueueError> {
        let now_ms = Utc::now().timestamp_millis() as u64;
        if let Some(mut entry) = self.active_leases.get_mut(lease_id) {
            entry.0.lease_expiry = now_ms + (extension.as_millis() as u64);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn ack(&self, lease: TaskLease) -> Result<(), QueueError> {
        if self.active_leases.remove(&lease.lease_id).is_some() {
            self.completed_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        } else {
            // Already ACKed or expired
            Ok(())
        }
    }

    async fn retry(&self, lease: TaskLease, delay: Duration) -> Result<(), QueueError> {
        if let Some((_, (_, mut original_task, attempt))) =
            self.active_leases.remove(&lease.lease_id)
        {
            let next_attempt = attempt + 1;
            if next_attempt > original_task.max_attempts {
                // Move to DLQ
                let record = TaskRecord {
                    task_id: original_task.task_id.clone(),
                    job_id: original_task.job_id.clone(),
                    task_type: original_task.task_type,
                    payload: original_task.payload.clone(),
                    priority: original_task.priority,
                    attempt: next_attempt,
                    max_attempts: original_task.max_attempts,
                    status: TaskStatus::DeadLetter,
                    lease_owner: None,
                    lease_expiry: None,
                    created_at: Utc::now().timestamp() as u64,
                    available_at: 0,
                    error: Some("Max attempts exceeded".to_string()),
                };
                self.dlq.insert(record.task_id.clone(), record);
                self.failed_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            } else {
                original_task.available_at =
                    (Utc::now().timestamp() as u64) + delay.as_secs().max(1);
                let mut map = self.queues.lock().await;
                let key = (original_task.task_type, original_task.priority);
                map.entry(key)
                    .or_insert_with(VecDeque::new)
                    .push_front(original_task);
            }
            Ok(())
        } else {
            Err(QueueError::LeaseExpired(lease.lease_id))
        }
    }

    async fn fail(&self, lease: TaskLease, error: TaskFailure) -> Result<(), QueueError> {
        let (is_retryable, attempt, max_attempts) = match self.active_leases.get(&lease.lease_id) {
            Some(entry) => (
                error.is_retryable,
                entry.value().2,
                entry.value().1.max_attempts,
            ),
            None => return Err(QueueError::LeaseExpired(lease.lease_id)),
        };

        if is_retryable && attempt < max_attempts {
            self.retry(lease, Duration::from_secs(5)).await
        } else if let Some((_, (_, original_task, attempt))) =
            self.active_leases.remove(&lease.lease_id)
        {
            let record = TaskRecord {
                task_id: original_task.task_id.clone(),
                job_id: original_task.job_id.clone(),
                task_type: original_task.task_type,
                payload: original_task.payload.clone(),
                priority: original_task.priority,
                attempt,
                max_attempts: original_task.max_attempts,
                status: TaskStatus::DeadLetter,
                lease_owner: None,
                lease_expiry: None,
                created_at: Utc::now().timestamp() as u64,
                available_at: 0,
                error: Some(error.error_message),
            };
            self.dlq.insert(record.task_id.clone(), record);
            self.failed_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        } else {
            Err(QueueError::LeaseExpired(lease.lease_id))
        }
    }

    async fn reclaim_stale_leases(&self) -> Result<usize, QueueError> {
        let now_ms = Utc::now().timestamp_millis() as u64;
        let mut expired_keys = Vec::new();

        for entry in self.active_leases.iter() {
            if entry.value().0.lease_expiry <= now_ms {
                expired_keys.push(entry.key().clone());
            }
        }

        let mut reclaimed = 0;
        for key in expired_keys {
            if let Some((_, (_, original_task, _attempt))) = self.active_leases.remove(&key) {
                let mut map = self.queues.lock().await;
                let q_key = (original_task.task_type, original_task.priority);
                map.entry(q_key)
                    .or_insert_with(VecDeque::new)
                    .push_front(original_task);
                reclaimed += 1;
            }
        }

        Ok(reclaimed)
    }

    async fn stats(&self) -> Result<QueueStats, QueueError> {
        let map = self.queues.lock().await;
        let mut queued_by_type = HashMap::new();
        let mut queued_total = 0;

        for ((tt, _), q) in map.iter() {
            let count = q.len();
            *queued_by_type.entry(*tt).or_insert(0) += count;
            queued_total += count;
        }

        Ok(QueueStats {
            queued_total,
            queued_by_type,
            leased_total: self.active_leases.len(),
            completed_total: self
                .completed_count
                .load(std::sync::atomic::Ordering::Relaxed),
            failed_total: self.failed_count.load(std::sync::atomic::Ordering::Relaxed),
            dlq_total: self.dlq.len(),
        })
    }

    async fn dlq_list(&self, limit: usize) -> Result<Vec<TaskRecord>, QueueError> {
        let mut list: Vec<TaskRecord> =
            self.dlq.iter().map(|entry| entry.value().clone()).collect();
        list.sort_by_key(|a| std::cmp::Reverse(a.created_at));
        if list.len() > limit {
            list.truncate(limit);
        }
        Ok(list)
    }

    async fn dlq_retry(&self, task_id: &str) -> Result<bool, QueueError> {
        if let Some((_, record)) = self.dlq.remove(task_id) {
            let q_task = QueuedTask {
                task_id: record.task_id,
                job_id: record.job_id,
                task_type: record.task_type,
                payload: record.payload,
                priority: record.priority,
                max_attempts: record.max_attempts,
                available_at: Utc::now().timestamp() as u64,
            };
            self.enqueue(q_task).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributed::models::{TaskPriority, TaskType, WorkerType};

    #[tokio::test]
    async fn test_fail_retryable_reenqueues_task() {
        let queue = MemoryTaskQueue::default();
        let task = QueuedTask::new(
            "job_1",
            TaskType::HttpFetch,
            serde_json::json!({"url": "https://example.com"}),
            TaskPriority::Normal,
            3,
        );

        queue.enqueue(task).await.unwrap();

        // 1. Reserve the task
        let lease = queue
            .reserve("worker_1", WorkerType::Http, Duration::from_secs(30))
            .await
            .unwrap()
            .expect("Task should be reserved");

        // 2. Fail with retryable error
        let failure = TaskFailure {
            error_message: "Connection reset by peer".to_string(),
            is_retryable: true,
        };
        queue.fail(lease, failure).await.unwrap();

        // Check stats: task should be back in queue, not in DLQ
        let stats = queue.stats().await.unwrap();
        assert_eq!(stats.dlq_total, 0, "Task should not be in DLQ yet");
        assert_eq!(stats.queued_total, 1, "Task should be re-enqueued for retry");
    }

    #[tokio::test]
    async fn test_fail_non_retryable_moves_to_dlq() {
        let queue = MemoryTaskQueue::default();
        let task = QueuedTask::new(
            "job_1",
            TaskType::HttpFetch,
            serde_json::json!({"url": "https://example.com"}),
            TaskPriority::Normal,
            3,
        );

        queue.enqueue(task).await.unwrap();

        let lease = queue
            .reserve("worker_1", WorkerType::Http, Duration::from_secs(30))
            .await
            .unwrap()
            .expect("Task should be reserved");

        let failure = TaskFailure {
            error_message: "404 Not Found".to_string(),
            is_retryable: false,
        };
        queue.fail(lease, failure).await.unwrap();

        let stats = queue.stats().await.unwrap();
        assert_eq!(stats.dlq_total, 1, "Non-retryable failure must move to DLQ");
        assert_eq!(stats.queued_total, 0);
    }
}
