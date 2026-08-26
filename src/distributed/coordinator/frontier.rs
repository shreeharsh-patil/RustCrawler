use crate::crawl::dedupe::normalize_crawl_url;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UrlStatus {
    Discovered,
    Queued,
    Processing,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontierEntry {
    pub url: String,
    pub normalized_url: String,
    pub url_hash: String,
    pub depth: u32,
    pub status: UrlStatus,
}

#[derive(Clone, Default)]
pub struct DistributedFrontier {
    // job_id -> (url_hash -> FrontierEntry)
    entries: Arc<DashMap<String, DashMap<String, FrontierEntry>>>,
}

impl DistributedFrontier {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn compute_hash(normalized_url: &str) -> String {
        blake3::hash(normalized_url.as_bytes()).to_hex().to_string()
    }

    pub fn insert_discovered(
        &self,
        job_id: &str,
        raw_url: &str,
        depth: u32,
        ignore_query_parameters: bool,
    ) -> Option<FrontierEntry> {
        let parsed = Url::parse(raw_url).ok()?;
        let norm_str = normalize_crawl_url(&parsed, ignore_query_parameters, true);
        let hash = Self::compute_hash(&norm_str);

        let job_map = self.entries.entry(job_id.to_string()).or_default();
        if job_map.contains_key(&hash) {
            return None; // Already known
        }

        let entry = FrontierEntry {
            url: raw_url.to_string(),
            normalized_url: norm_str,
            url_hash: hash.clone(),
            depth,
            status: UrlStatus::Discovered,
        };

        job_map.insert(hash, entry.clone());
        Some(entry)
    }

    pub fn update_status(&self, job_id: &str, url_hash: &str, status: UrlStatus) -> bool {
        if let Some(job_map) = self.entries.get(job_id) {
            if let Some(mut entry) = job_map.get_mut(url_hash) {
                entry.status = status;
                return true;
            }
        }
        false
    }

    pub fn get_job_stats(&self, job_id: &str) -> (usize, usize, usize) {
        if let Some(job_map) = self.entries.get(job_id) {
            let mut total = 0;
            let mut completed = 0;
            let mut failed = 0;
            for entry in job_map.iter() {
                total += 1;
                match entry.value().status {
                    UrlStatus::Completed => completed += 1,
                    UrlStatus::Failed => failed += 1,
                    _ => {}
                }
            }
            (total, completed, failed)
        } else {
            (0, 0, 0)
        }
    }

    pub fn clear_job(&self, job_id: &str) {
        self.entries.remove(job_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distributed_frontier_deduplication() {
        let frontier = DistributedFrontier::new();
        let job_id = "job_test_1";

        let res1 = frontier.insert_discovered(
            job_id,
            "https://example.com/docs?utm_source=twitter",
            0,
            false,
        );
        assert!(res1.is_some(), "First URL discovery must succeed");

        let res2 = frontier.insert_discovered(
            job_id,
            "https://example.com/docs?utm_source=twitter#section2",
            1,
            false,
        );
        assert!(
            res2.is_none(),
            "Equivalent normalized URL must be deduplicated"
        );

        let entry = res1.unwrap();
        assert!(frontier.update_status(job_id, &entry.url_hash, UrlStatus::Completed));

        let (total, completed, failed) = frontier.get_job_stats(job_id);
        assert_eq!(total, 1);
        assert_eq!(completed, 1);
        assert_eq!(failed, 0);
    }
}
