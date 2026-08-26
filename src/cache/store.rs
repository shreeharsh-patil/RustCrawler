use crate::error::CrawlerError;
use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub data: Vec<u8>,
    pub content_type: Option<String>,
    pub status_code: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    pub created_at: u64,
    pub ttl_seconds: u64,
}

impl CacheEntry {
    pub fn new(
        data: Vec<u8>,
        content_type: Option<String>,
        status_code: u16,
        etag: Option<String>,
        last_modified: Option<String>,
        content_hash: Option<String>,
        ttl_seconds: u64,
    ) -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            data,
            content_type,
            status_code,
            etag,
            last_modified,
            content_hash,
            created_at,
            ttl_seconds,
        }
    }

    pub fn is_expired(&self) -> bool {
        if self.ttl_seconds == 0 {
            return false;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now >= self.created_at.saturating_add(self.ttl_seconds)
    }
}

#[async_trait]
pub trait CacheStore: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<CacheEntry>, CrawlerError>;
    async fn put(&self, key: String, entry: CacheEntry) -> Result<(), CrawlerError>;
    async fn remove(&self, key: &str) -> Result<(), CrawlerError>;
    async fn clear(&self) -> Result<(), CrawlerError>;
}

#[derive(Clone)]
pub struct InMemoryCacheStore {
    entries: Arc<DashMap<String, CacheEntry>>,
    max_entries: usize,
    enabled: bool,
}

impl InMemoryCacheStore {
    pub fn new(enabled: bool, max_entries: usize) -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            max_entries,
            enabled,
        }
    }
}

#[async_trait]
impl CacheStore for InMemoryCacheStore {
    async fn get(&self, key: &str) -> Result<Option<CacheEntry>, CrawlerError> {
        if !self.enabled {
            return Ok(None);
        }

        if let Some(entry_ref) = self.entries.get(key) {
            if entry_ref.is_expired() {
                drop(entry_ref);
                self.entries.remove(key);
                return Ok(None);
            }
            return Ok(Some(entry_ref.clone()));
        }

        Ok(None)
    }

    async fn put(&self, key: String, entry: CacheEntry) -> Result<(), CrawlerError> {
        if !self.enabled {
            return Ok(());
        }

        // Evict if capacity exceeded
        if self.entries.len() >= self.max_entries {
            // First sweep expired entries
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let mut expired_keys = Vec::new();
            for item in self.entries.iter() {
                if item.value().ttl_seconds > 0
                    && now
                        >= item
                            .value()
                            .created_at
                            .saturating_add(item.value().ttl_seconds)
                {
                    expired_keys.push(item.key().clone());
                    if expired_keys.len() >= 20 {
                        break;
                    }
                }
            }

            for k in expired_keys {
                self.entries.remove(&k);
            }

            // If still full, evict arbitrary key
            if self.entries.len() >= self.max_entries {
                if let Some(first_key) = self.entries.iter().next().map(|i| i.key().clone()) {
                    self.entries.remove(&first_key);
                }
            }
        }

        self.entries.insert(key, entry);
        Ok(())
    }

    async fn remove(&self, key: &str) -> Result<(), CrawlerError> {
        self.entries.remove(key);
        Ok(())
    }

    async fn clear(&self) -> Result<(), CrawlerError> {
        self.entries.clear();
        Ok(())
    }
}
