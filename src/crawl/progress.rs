use crate::crawl::models::CrawlStats;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

/// Thread-safe lock-free crawl progress tracker.
#[derive(Debug)]
pub struct CrawlProgress {
    pages_discovered: AtomicUsize,
    pages_queued: AtomicUsize,
    pages_crawled: AtomicUsize,
    pages_succeeded: AtomicUsize,
    pages_failed: AtomicUsize,
    pages_skipped: AtomicUsize,
    pages_blocked_by_robots: AtomicUsize,
    dedupe_hits: AtomicUsize,
    retries_total: AtomicUsize,
    bytes_downloaded: AtomicUsize,
    current_depth: AtomicU32,
    http_pages: AtomicUsize,
    browser_pages: AtomicUsize,
    browser_fallback_count: AtomicUsize,
    browser_render_time_ms: AtomicU64,
    documents_processed: AtomicUsize,
    duplicate_content_hits: AtomicUsize,
    start_time: Instant,
}

impl Default for CrawlProgress {
    fn default() -> Self {
        Self::new()
    }
}

impl CrawlProgress {
    pub fn new() -> Self {
        Self {
            pages_discovered: AtomicUsize::new(0),
            pages_queued: AtomicUsize::new(0),
            pages_crawled: AtomicUsize::new(0),
            pages_succeeded: AtomicUsize::new(0),
            pages_failed: AtomicUsize::new(0),
            pages_skipped: AtomicUsize::new(0),
            pages_blocked_by_robots: AtomicUsize::new(0),
            dedupe_hits: AtomicUsize::new(0),
            retries_total: AtomicUsize::new(0),
            bytes_downloaded: AtomicUsize::new(0),
            current_depth: AtomicU32::new(0),
            http_pages: AtomicUsize::new(0),
            browser_pages: AtomicUsize::new(0),
            browser_fallback_count: AtomicUsize::new(0),
            browser_render_time_ms: AtomicU64::new(0),
            documents_processed: AtomicUsize::new(0),
            duplicate_content_hits: AtomicUsize::new(0),
            start_time: Instant::now(),
        }
    }

    pub fn inc_discovered(&self, count: usize) {
        self.pages_discovered.fetch_add(count, Ordering::Relaxed);
    }

    pub fn inc_queued(&self) {
        self.pages_queued.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_crawled(&self) {
        self.pages_crawled.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_succeeded(&self) {
        self.pages_succeeded.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_failed(&self) {
        self.pages_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_skipped(&self) {
        self.pages_skipped.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_blocked_by_robots(&self) {
        self.pages_blocked_by_robots.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_dedupe_hits(&self) {
        self.dedupe_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_retries(&self) {
        self.retries_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_bytes(&self, bytes: usize) {
        self.bytes_downloaded.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn update_depth(&self, depth: u32) {
        self.current_depth.fetch_max(depth, Ordering::Relaxed);
    }

    pub fn inc_http_page(&self) {
        self.http_pages.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_browser_page(&self, render_time_ms: u64) {
        self.browser_pages.fetch_add(1, Ordering::Relaxed);
        self.browser_render_time_ms
            .fetch_add(render_time_ms, Ordering::Relaxed);
    }

    pub fn inc_browser_fallback(&self) {
        self.browser_fallback_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_document_processed(&self) {
        self.documents_processed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_duplicate_content_hit(&self) {
        self.duplicate_content_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> CrawlStats {
        CrawlStats {
            pages_discovered: self.pages_discovered.load(Ordering::Relaxed),
            pages_queued: self.pages_queued.load(Ordering::Relaxed),
            pages_crawled: self.pages_crawled.load(Ordering::Relaxed),
            pages_succeeded: self.pages_succeeded.load(Ordering::Relaxed),
            pages_failed: self.pages_failed.load(Ordering::Relaxed),
            pages_skipped: self.pages_skipped.load(Ordering::Relaxed),
            pages_blocked_by_robots: self.pages_blocked_by_robots.load(Ordering::Relaxed),
            dedupe_hits: self.dedupe_hits.load(Ordering::Relaxed),
            retries_total: self.retries_total.load(Ordering::Relaxed),
            bytes_downloaded: self.bytes_downloaded.load(Ordering::Relaxed),
            current_depth: self.current_depth.load(Ordering::Relaxed),
            elapsed_ms: self.start_time.elapsed().as_millis() as u64,
            http_pages: self.http_pages.load(Ordering::Relaxed),
            browser_pages: self.browser_pages.load(Ordering::Relaxed),
            browser_fallback_count: self.browser_fallback_count.load(Ordering::Relaxed),
            browser_render_time_ms: self.browser_render_time_ms.load(Ordering::Relaxed),
            documents_processed: self.documents_processed.load(Ordering::Relaxed),
            duplicate_content_hits: self.duplicate_content_hits.load(Ordering::Relaxed),
        }
    }
}
