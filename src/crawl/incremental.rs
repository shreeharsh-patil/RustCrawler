use crate::crawl::models::{CrawlOptions, CrawlStats, CrawledPage};
use crate::crawl::snapshot::{
    compute_normalized_content_hash, CrawlSnapshot, PageSnapshot, SnapshotStore,
};
use crate::crawl::CrawlerService;
use crate::error::CrawlerError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    New,
    Changed,
    Unchanged,
    Deleted,
    Unknown,
    Redirected,
    MetadataChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChangeSummary {
    pub new: Vec<String>,
    pub changed: Vec<String>,
    pub deleted: Vec<String>,
    pub unchanged_count: usize,
    pub unknown_count: usize,
    pub redirected_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalCrawlRequest {
    pub url: String,
    #[serde(default)]
    pub baseline_job_id: Option<String>,
    #[serde(default)]
    pub baseline_snapshot_id: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub include_changed_content: bool,
    #[serde(default)]
    pub options: Option<CrawlOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalCrawlResponse {
    pub success: bool,
    pub snapshot_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_id: Option<String>,
    pub changes: ChangeSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed_pages: Option<Vec<CrawledPage>>,
    pub stats: CrawlStats,
}

pub struct IncrementalCrawler {
    crawler: Arc<CrawlerService>,
    snapshot_store: Arc<SnapshotStore>,
}

impl IncrementalCrawler {
    pub fn new(crawler: Arc<CrawlerService>, snapshot_store: Arc<SnapshotStore>) -> Self {
        Self {
            crawler,
            snapshot_store,
        }
    }

    /// Performs an incremental recrawl against a baseline snapshot and classifies changed/new/deleted pages.
    pub async fn execute_incremental_crawl(
        &self,
        request: IncrementalCrawlRequest,
        cancel: CancellationToken,
    ) -> Result<IncrementalCrawlResponse, CrawlerError> {
        let baseline_key = request
            .baseline_snapshot_id
            .as_deref()
            .or(request.baseline_job_id.as_deref())
            .ok_or_else(|| {
                CrawlerError::InvalidBaseline(
                    "Incremental crawl requires either 'baseline_job_id' or 'baseline_snapshot_id'"
                        .to_string(),
                )
            })?;

        let baseline_snapshot = self
            .snapshot_store
            .get_snapshot(baseline_key)
            .ok_or_else(|| CrawlerError::SnapshotNotFound(baseline_key.to_string()))?;

        let mut crawl_opts = request.options.unwrap_or_default();
        if let Some(limit) = request.limit {
            crawl_opts.limit = limit;
        }

        // Execute current crawl
        let crawl_res = self.crawler.crawl(&request.url, crawl_opts, cancel).await?;

        let mut new_snapshot = CrawlSnapshot::new(Uuid::new_v4().to_string(), request.url.clone());
        let mut changes = ChangeSummary::default();
        let mut crawled_urls = HashSet::new();
        let mut changed_pages_vec = Vec::new();

        for page in &crawl_res.pages {
            crawled_urls.insert(page.url.clone());

            let normalized_hash = page
                .text
                .as_deref()
                .or(page.markdown.as_deref())
                .map(compute_normalized_content_hash);

            let page_snap = PageSnapshot {
                url: page.url.clone(),
                final_url: page.final_url.clone(),
                content_hash: page.content_hash.clone(),
                raw_content_hash: page.content_hash.clone(),
                normalized_content_hash: normalized_hash.clone(),
                metadata_hash: None,
                etag: None,
                last_modified: None,
                status_code: page.status_code,
                title: page.metadata.as_ref().and_then(|m| m.title.clone()),
            };
            new_snapshot.insert_page(page_snap);

            // Compare against baseline
            if let Some(baseline_page) = baseline_snapshot.pages.get(&page.url) {
                if page.status_code == 304 {
                    changes.unchanged_count += 1;
                } else if page.status_code >= 400 && page.status_code < 500 {
                    if page.status_code == 404 || page.status_code == 410 {
                        changes.deleted.push(page.url.clone());
                    } else {
                        changes.unknown_count += 1;
                    }
                } else if page.status_code >= 500 {
                    changes.unknown_count += 1;
                } else if page.final_url != baseline_page.final_url {
                    changes.redirected_count += 1;
                    changes.changed.push(page.url.clone());
                    changed_pages_vec.push(page.clone());
                } else if (normalized_hash.is_some()
                    && normalized_hash == baseline_page.normalized_content_hash)
                    || (page.content_hash.is_some()
                        && page.content_hash == baseline_page.content_hash)
                {
                    changes.unchanged_count += 1;
                } else {
                    changes.changed.push(page.url.clone());
                    changed_pages_vec.push(page.clone());
                }
            } else {
                changes.new.push(page.url.clone());
                changed_pages_vec.push(page.clone());
            }
        }

        // Check for baseline pages that were completely missing in the new crawl
        for baseline_url in baseline_snapshot.pages.keys() {
            if !crawled_urls.contains(baseline_url) {
                // If it wasn't encountered, mark deleted
                changes.deleted.push(baseline_url.clone());
            }
        }

        let new_snapshot_id = new_snapshot.id.clone();
        self.snapshot_store.save_snapshot(new_snapshot);
        self.snapshot_store
            .link_job(&crawl_res.job_id, &new_snapshot_id);

        Ok(IncrementalCrawlResponse {
            success: true,
            snapshot_id: new_snapshot_id,
            baseline_id: Some(baseline_key.to_string()),
            changes,
            changed_pages: if request.include_changed_content {
                Some(changed_pages_vec)
            } else {
                None
            },
            stats: crawl_res.stats,
        })
    }
}
