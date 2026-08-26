use chrono::Utc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageSnapshot {
    pub url: String,
    pub final_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    pub status_code: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlSnapshot {
    pub id: String,
    pub seed_url: String,
    pub created_at: String,
    pub pages: HashMap<String, PageSnapshot>,
}

impl CrawlSnapshot {
    pub fn new(id: String, seed_url: String) -> Self {
        Self {
            id,
            seed_url,
            created_at: Utc::now().to_rfc3339(),
            pages: HashMap::new(),
        }
    }

    pub fn insert_page(&mut self, page: PageSnapshot) {
        self.pages.insert(page.url.clone(), page);
    }
}

pub fn compute_normalized_content_hash(text: &str) -> String {
    let normalized = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    blake3::hash(normalized.as_bytes()).to_hex().to_string()
}

#[derive(Clone, Default)]
pub struct SnapshotStore {
    snapshots: Arc<DashMap<String, CrawlSnapshot>>,
    job_to_snapshot: Arc<DashMap<String, String>>,
}

impl SnapshotStore {
    pub fn new() -> Self {
        Self {
            snapshots: Arc::new(DashMap::new()),
            job_to_snapshot: Arc::new(DashMap::new()),
        }
    }

    pub fn save_snapshot(&self, snapshot: CrawlSnapshot) {
        self.snapshots.insert(snapshot.id.clone(), snapshot);
    }

    pub fn link_job(&self, job_id: &str, snapshot_id: &str) {
        self.job_to_snapshot
            .insert(job_id.to_string(), snapshot_id.to_string());
    }

    pub fn get_snapshot(&self, id_or_job_id: &str) -> Option<CrawlSnapshot> {
        if let Some(snap) = self.snapshots.get(id_or_job_id) {
            return Some(snap.clone());
        }

        if let Some(snap_id) = self.job_to_snapshot.get(id_or_job_id) {
            return self.snapshots.get(snap_id.as_str()).map(|s| s.clone());
        }

        None
    }
}
