use crate::extraction::models::ExtractionResult;
use crate::extraction::schema::compute_schema_hash;
use dashmap::DashMap;
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone)]
pub struct ExtractionCache {
    cache: Arc<DashMap<String, ExtractionResult>>,
    max_items: usize,
    enabled: bool,
}

impl ExtractionCache {
    pub fn new(enabled: bool, max_items: usize) -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
            max_items,
            enabled,
        }
    }

    pub fn compute_key(
        content_hash: &str,
        schema: Option<&Value>,
        prompt: Option<&str>,
        model: Option<&str>,
    ) -> String {
        let schema_hash = schema
            .map(compute_schema_hash)
            .unwrap_or_else(|| "no_schema".to_string());
        let prompt_str = prompt.unwrap_or("");
        let model_str = model.unwrap_or("default");

        let combined = format!("{content_hash}:{schema_hash}:{prompt_str}:{model_str}");
        blake3::hash(combined.as_bytes()).to_hex().to_string()
    }

    pub fn get(&self, key: &str) -> Option<ExtractionResult> {
        if !self.enabled {
            return None;
        }
        self.cache.get(key).map(|r| {
            let mut res = r.clone();
            res.metadata.cache_hit = true;
            res
        })
    }

    pub fn insert(&self, key: String, result: ExtractionResult) {
        if !self.enabled {
            return;
        }
        if self.cache.len() >= self.max_items {
            // Evict arbitrary entry
            if let Some(first_key) = self.cache.iter().next().map(|entry| entry.key().clone()) {
                self.cache.remove(&first_key);
            }
        }
        self.cache.insert(key, result);
    }
}
