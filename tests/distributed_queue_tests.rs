use rustcrawl::distributed::models::{
    QueuedTask, TaskFailure, TaskPriority, TaskStatus, TaskType, WorkerType,
};
use rustcrawl::distributed::queue::memory::MemoryTaskQueue;
use rustcrawl::distributed::queue::TaskQueue;
use std::time::Duration;

#[tokio::test]
async fn test_priority_scheduling_and_reservation() {
    let queue = MemoryTaskQueue::new(100);

    // 1. Enqueue Low, High, Normal tasks
    let low_task = QueuedTask::new(
        "job_1",
        TaskType::HttpFetch,
        serde_json::json!({"url": "http://example.com/low"}),
        TaskPriority::Low,
        3,
    );
    let high_task = QueuedTask::new(
        "job_1",
        TaskType::HttpFetch,
        serde_json::json!({"url": "http://example.com/high"}),
        TaskPriority::High,
        3,
    );
    let normal_task = QueuedTask::new(
        "job_1",
        TaskType::HttpFetch,
        serde_json::json!({"url": "http://example.com/normal"}),
        TaskPriority::Normal,
        3,
    );

    queue.enqueue(low_task).await.unwrap();
    queue.enqueue(high_task).await.unwrap();
    queue.enqueue(normal_task).await.unwrap();

    // 2. High priority task must be reserved first
    let lease1 = queue
        .reserve("worker_1", WorkerType::Http, Duration::from_secs(30))
        .await
        .unwrap()
        .expect("Should reserve task");
    assert_eq!(lease1.payload["url"], "http://example.com/high");

    // 3. Normal priority task must be reserved second
    let lease2 = queue
        .reserve("worker_1", WorkerType::Http, Duration::from_secs(30))
        .await
        .unwrap()
        .expect("Should reserve task");
    assert_eq!(lease2.payload["url"], "http://example.com/normal");

    // 4. Low priority task must be reserved last
    let lease3 = queue
        .reserve("worker_1", WorkerType::Http, Duration::from_secs(30))
        .await
        .unwrap()
        .expect("Should reserve task");
    assert_eq!(lease3.payload["url"], "http://example.com/low");
}

#[tokio::test]
async fn test_lease_expiration_and_reclaim() {
    let queue = MemoryTaskQueue::new(100);

    let task = QueuedTask::new(
        "job_1",
        TaskType::HttpFetch,
        serde_json::json!({"url": "http://example.com/crash"}),
        TaskPriority::Normal,
        3,
    );
    queue.enqueue(task).await.unwrap();

    // Reserve task with 1-second lease
    let lease = queue
        .reserve("worker_crashed", WorkerType::Http, Duration::from_secs(1))
        .await
        .unwrap()
        .expect("Should reserve");

    // Immediately before expiry, queue has 0 available tasks
    let empty_reserve = queue
        .reserve("worker_2", WorkerType::Http, Duration::from_secs(30))
        .await
        .unwrap();
    assert!(empty_reserve.is_none());

    // Sleep 1.2s to expire lease
    tokio::time::sleep(Duration::from_millis(1200)).await;

    // Reclaim stale leases
    let reclaimed = queue.reclaim_stale_leases().await.unwrap();
    assert_eq!(reclaimed, 1);

    // Another worker should now successfully reserve the reclaimed task
    let lease2 = queue
        .reserve("worker_2", WorkerType::Http, Duration::from_secs(30))
        .await
        .unwrap()
        .expect("Should re-reserve task");
    assert_eq!(lease2.task_id, lease.task_id);
}

#[tokio::test]
async fn test_max_attempts_and_dead_letter_queue() {
    let queue = MemoryTaskQueue::new(100);

    let task = QueuedTask::new(
        "job_1",
        TaskType::HttpFetch,
        serde_json::json!({"url": "http://example.com/failing"}),
        TaskPriority::Normal,
        1,
    );
    queue.enqueue(task).await.unwrap();

    let lease = queue
        .reserve("worker_1", WorkerType::Http, Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();

    // Permanent failure -> move to DLQ
    let failure = TaskFailure {
        error_message: "Fatal parsing error".to_string(),
        is_retryable: false,
    };
    queue.fail(lease, failure).await.unwrap();

    let dlq_items = queue.dlq_list(10).await.unwrap();
    assert_eq!(dlq_items.len(), 1);
    assert_eq!(dlq_items[0].status, TaskStatus::DeadLetter);
}
