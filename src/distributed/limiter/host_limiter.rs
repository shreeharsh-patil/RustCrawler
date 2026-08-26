use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub struct DistributedHostLimiter {
    host_semaphores: Arc<DashMap<String, Arc<Semaphore>>>,
    host_last_access: Arc<DashMap<String, Instant>>,
}

impl DistributedHostLimiter {
    pub fn new() -> Self {
        Self {
            host_semaphores: Arc::new(DashMap::new()),
            host_last_access: Arc::new(DashMap::new()),
        }
    }

    pub async fn acquire(
        &self,
        host: &str,
        max_concurrency: usize,
        min_delay: Duration,
    ) -> HostPermit {
        let semaphore = self
            .host_semaphores
            .entry(host.to_string())
            .or_insert_with(|| Arc::new(Semaphore::new(max_concurrency.max(1))))
            .clone();

        // 1. Acquire host concurrency permit
        let permit = semaphore
            .acquire_owned()
            .await
            .expect("Semaphore not closed");

        // 2. Enforce minimum request delay
        let now = Instant::now();
        if let Some(last_time) = self.host_last_access.get(host) {
            let elapsed = now.duration_since(*last_time);
            if elapsed < min_delay {
                tokio::time::sleep(min_delay - elapsed).await;
            }
        }
        self.host_last_access
            .insert(host.to_string(), Instant::now());

        HostPermit {
            _permit: permit,
            host: host.to_string(),
        }
    }
}

impl Default for DistributedHostLimiter {
    fn default() -> Self {
        Self::new()
    }
}

pub struct HostPermit {
    _permit: OwnedSemaphorePermit,
    pub host: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_distributed_host_limiter_min_delay() {
        let limiter = DistributedHostLimiter::new();
        let host = "example.com";
        let min_delay = Duration::from_millis(50);

        let start = Instant::now();

        let permit1 = limiter.acquire(host, 5, min_delay).await;
        drop(permit1);

        let permit2 = limiter.acquire(host, 5, min_delay).await;
        drop(permit2);

        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(45),
            "Limiter should have delayed second request by ~50ms (elapsed: {elapsed:?})"
        );
    }
}
