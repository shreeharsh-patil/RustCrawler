use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Key for embedding cache lookup
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EmbeddingCacheKey {
    pub content_hash: String,
    pub provider: String,
    pub model: String,
    pub dimensions: usize,
}

impl EmbeddingCacheKey {
    pub fn new(
        content_hash: impl Into<String>,
        provider: impl Into<String>,
        model: impl Into<String>,
        dimensions: usize,
    ) -> Self {
        Self {
            content_hash: content_hash.into(),
            provider: provider.into(),
            model: model.into(),
            dimensions,
        }
    }
}

/// Statistics for embedding cache performance
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmbeddingCacheStats {
    pub hits: usize,
    pub misses: usize,
    pub entry_count: usize,
    pub hit_rate: f64,
}

/// Persistent/in-memory embedding cache to avoid regenerating vectors for identical chunks
#[derive(Clone)]
pub struct EmbeddingCache {
    store: Arc<DashMap<EmbeddingCacheKey, Vec<f32>>>,
    hits: Arc<AtomicUsize>,
    misses: Arc<AtomicUsize>,
    max_entries: usize,
}

impl EmbeddingCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            store: Arc::new(DashMap::new()),
            hits: Arc::new(AtomicUsize::new(0)),
            misses: Arc::new(AtomicUsize::new(0)),
            max_entries,
        }
    }

    pub fn get(&self, key: &EmbeddingCacheKey) -> Option<Vec<f32>> {
        if let Some(entry) = self.store.get(key) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            Some(entry.value().clone())
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    pub fn insert(&self, key: EmbeddingCacheKey, vector: Vec<f32>) {
        if self.store.len() < self.max_entries {
            self.store.insert(key, vector);
        }
    }

    pub fn stats(&self) -> EmbeddingCacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        let hit_rate = if total > 0 {
            hits as f64 / total as f64
        } else {
            0.0
        };

        EmbeddingCacheStats {
            hits,
            misses,
            entry_count: self.store.len(),
            hit_rate,
        }
    }

    pub fn clear(&self) {
        self.store.clear();
    }
}

impl Default for EmbeddingCache {
    fn default() -> Self {
        Self::new(100_000)
    }
}
