use rustcrawl::distributed::coordinator::frontier::{DistributedFrontier, UrlStatus};
use rustcrawl::distributed::limiter::DistributedHostLimiter;
use std::time::{Duration, Instant};

#[tokio::test]
async fn test_distributed_host_limiter_min_delay() {
    let limiter = DistributedHostLimiter::new();
    let host = "example.com";
    let min_delay = Duration::from_millis(50);

    let start = Instant::now();

    // Acquire permit 1
    let permit1 = limiter.acquire(host, 5, min_delay).await;
    drop(permit1);

    // Acquire permit 2 immediately
    let permit2 = limiter.acquire(host, 5, min_delay).await;
    drop(permit2);

    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(45),
        "Limiter should have delayed the second request by ~50ms (elapsed: {elapsed:?})"
    );
}

#[test]
fn test_distributed_frontier_deduplication() {
    let frontier = DistributedFrontier::new();
    let job_id = "job_test_1";

    let res1 = frontier.insert_discovered(job_id, "https://example.com/docs/", 0, true);
    assert!(res1.is_some(), "First URL discovery must succeed");

    // Trailing slash variation
    let res2 = frontier.insert_discovered(job_id, "https://example.com/docs", 1, true);
    assert!(
        res2.is_none(),
        "Equivalent normalized URL must be deduplicated"
    );

    // Update status
    let entry = res1.unwrap();
    assert!(frontier.update_status(job_id, &entry.url_hash, UrlStatus::Completed));

    let (total, completed, failed) = frontier.get_job_stats(job_id);
    assert_eq!(total, 1);
    assert_eq!(completed, 1);
    assert_eq!(failed, 0);
}
