use crate::crawl::models::CrawlTarget;
use crate::error::CrawlerError;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use tokio::sync::Notify;

/// An asynchronous, bounded, and memory-conscious crawl frontier queue.
pub struct CrawlFrontier {
    queue: Mutex<VecDeque<CrawlTarget>>,
    capacity: usize,
    notify: Notify,
    high_water_mark: AtomicUsize,
    total_enqueued: AtomicUsize,
}

impl CrawlFrontier {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: Mutex::new(VecDeque::with_capacity(capacity.min(1024))),
            capacity,
            notify: Notify::new(),
            high_water_mark: AtomicUsize::new(0),
            total_enqueued: AtomicUsize::new(0),
        }
    }

    /// Tries to enqueue a target without blocking. Returns an error if the queue is full.
    pub fn try_push(&self, target: CrawlTarget) -> Result<(), CrawlerError> {
        let mut q = self.queue.lock().unwrap();
        if q.len() >= self.capacity {
            return Err(CrawlerError::FrontierFull(self.capacity));
        }

        q.push_back(target);
        let len = q.len();
        self.high_water_mark.fetch_max(len, Ordering::Relaxed);
        self.total_enqueued.fetch_add(1, Ordering::Relaxed);

        self.notify.notify_one();
        Ok(())
    }

    /// Pops the next target from the frontier. Returns None if empty and not waiting.
    pub fn try_pop(&self) -> Option<CrawlTarget> {
        let mut q = self.queue.lock().unwrap();
        q.pop_front()
    }

    /// Waits asynchronously for a target to be enqueued.
    pub async fn pop(&self) -> Option<CrawlTarget> {
        loop {
            {
                let mut q = self.queue.lock().unwrap();
                if let Some(target) = q.pop_front() {
                    return Some(target);
                }
            }
            self.notify.notified().await;
        }
    }

    /// Returns the current number of queued targets.
    pub fn len(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    /// Returns true if the frontier queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.lock().unwrap().is_empty()
    }

    /// Returns the maximum capacity of the frontier.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the highest queue length observed during the crawl.
    pub fn high_water_mark(&self) -> usize {
        self.high_water_mark.load(Ordering::Relaxed)
    }

    /// Returns total items ever enqueued.
    pub fn total_enqueued(&self) -> usize {
        self.total_enqueued.load(Ordering::Relaxed)
    }

    /// Clears the queue and wakes up any waiting tasks.
    pub fn clear(&self) {
        let mut q = self.queue.lock().unwrap();
        q.clear();
        self.notify.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawl::models::DiscoverySource;
    use url::Url;

    #[tokio::test]
    async fn test_frontier_bounded_capacity() {
        let frontier = CrawlFrontier::new(2);
        let target1 = CrawlTarget {
            url: Url::parse("https://example.com/1").unwrap(),
            normalized_url: "https://example.com/1".to_string(),
            depth: 0,
            parent_url: None,
            discovered_from: DiscoverySource::Seed,
        };
        let target2 = CrawlTarget {
            url: Url::parse("https://example.com/2").unwrap(),
            normalized_url: "https://example.com/2".to_string(),
            depth: 1,
            parent_url: None,
            discovered_from: DiscoverySource::HtmlLink,
        };
        let target3 = CrawlTarget {
            url: Url::parse("https://example.com/3").unwrap(),
            normalized_url: "https://example.com/3".to_string(),
            depth: 1,
            parent_url: None,
            discovered_from: DiscoverySource::HtmlLink,
        };

        assert!(frontier.try_push(target1.clone()).is_ok());
        assert!(frontier.try_push(target2.clone()).is_ok());
        assert!(frontier.try_push(target3).is_err());
        assert_eq!(frontier.len(), 2);
        assert_eq!(frontier.high_water_mark(), 2);

        let popped1 = frontier.try_pop().unwrap();
        assert_eq!(popped1.url.as_str(), "https://example.com/1");
        let popped2 = frontier.try_pop().unwrap();
        assert_eq!(popped2.url.as_str(), "https://example.com/2");
        assert!(frontier.is_empty());
    }
}
