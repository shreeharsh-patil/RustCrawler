use rustcrawl::distributed::coordinator::frontier::DistributedFrontier;
use rustcrawl::distributed::coordinator::scheduler::JobScheduler;
use rustcrawl::distributed::models::{JobStatus, JobType, SystemEventType};
use rustcrawl::distributed::queue::memory::MemoryTaskQueue;
use rustcrawl::distributed::store::memory::MemoryMetadataStore;
use rustcrawl::distributed::store::MetadataStore;
use std::sync::Arc;

#[tokio::test]
async fn test_job_submission_and_lifecycle() {
    let store = Arc::new(MemoryMetadataStore::new());
    let queue = Arc::new(MemoryTaskQueue::new(100));
    let frontier = Arc::new(DistributedFrontier::new());

    let scheduler = JobScheduler::new(store.clone(), queue.clone(), frontier);
    let mut rx = scheduler.subscribe_events();

    // 1. Submit Crawl Job
    let config = serde_json::json!({
        "url": "https://example.com",
        "limit": 100
    });

    let job_id = scheduler
        .submit_job(JobType::Crawl, config, Some("tenant_123".to_string()))
        .await
        .expect("Job submission should succeed");

    // Receive JobStarted event
    let start_event = rx.recv().await.unwrap();
    assert_eq!(start_event.job_id, job_id);
    assert!(matches!(
        start_event.event_type,
        SystemEventType::JobStarted
    ));

    // 2. Pause Job
    scheduler.pause_job(&job_id).await.unwrap();
    let job_paused = store.get_job(&job_id).await.unwrap().unwrap();
    assert_eq!(job_paused.status, JobStatus::Paused);

    // 3. Resume Job
    scheduler.resume_job(&job_id).await.unwrap();
    let job_resumed = store.get_job(&job_id).await.unwrap().unwrap();
    assert_eq!(job_resumed.status, JobStatus::Running);

    // 4. Cancel Job
    scheduler.cancel_job(&job_id).await.unwrap();
    let job_cancelled = store.get_job(&job_id).await.unwrap().unwrap();
    assert_eq!(job_cancelled.status, JobStatus::Cancelled);
}
