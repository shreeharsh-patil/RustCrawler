use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use url::Url;

/// Host-level concurrency and rate limiting scheduler.
pub struct CrawlScheduler {
    global_semaphore: Arc<Semaphore>,
    host_semaphores: DashMap<String, Arc<Semaphore>>,
    host_last_request: DashMap<String, Arc<Mutex<Instant>>>,
    max_concurrent_per_host: usize,
    default_delay: Duration,
}

impl CrawlScheduler {
    pub fn new(
        max_global_concurrency: usize,
        max_concurrent_per_host: usize,
        default_delay_ms: u64,
    ) -> Self {
        Self {
            global_semaphore: Arc::new(Semaphore::new(max_global_concurrency.max(1))),
            host_semaphores: DashMap::new(),
            host_last_request: DashMap::new(),
            max_concurrent_per_host: max_concurrent_per_host.max(1),
            default_delay: Duration::from_millis(default_delay_ms),
        }
    }

    /// Acquires global and host-level permits and enforces per-host rate limiting.
    pub async fn acquire(
        &self,
        target_url: &Url,
        custom_delay: Option<Duration>,
    ) -> Option<SchedulerPermit> {
        let host = target_url
            .host_str()
            .unwrap_or("unknown_host")
            .to_lowercase();

        // 1. Acquire global concurrency permit
        let global_permit = self.global_semaphore.clone().acquire_owned().await.ok()?;

        // 2. Acquire per-host concurrency permit
        let host_sem = self
            .host_semaphores
            .entry(host.clone())
            .or_insert_with(|| Arc::new(Semaphore::new(self.max_concurrent_per_host)))
            .clone();

        let host_permit = host_sem.acquire_owned().await.ok()?;

        // 3. Enforce per-host rate limit
        let delay = custom_delay.unwrap_or(self.default_delay);
        if !delay.is_zero() {
            let last_req_mutex = self
                .host_last_request
                .entry(host)
                .or_insert_with(|| Arc::new(Mutex::new(Instant::now() - delay)))
                .clone();

            let mut last_req = last_req_mutex.lock().await;
            let elapsed = last_req.elapsed();
            if elapsed < delay {
                tokio::time::sleep(delay - elapsed).await;
            }
            *last_req = Instant::now();
        }

        Some(SchedulerPermit {
            _global: global_permit,
            _host: host_permit,
        })
    }
}

/// RAII guard releasing global and per-host concurrency permits when dropped.
pub struct SchedulerPermit {
    _global: OwnedSemaphorePermit,
    _host: OwnedSemaphorePermit,
}
