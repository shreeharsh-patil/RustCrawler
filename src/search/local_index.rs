use crate::discovery::ranking::tokenize;
use crate::search::models::{SearchResult, SearchResultSource};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedDocument {
    pub url: Url,
    pub title: Option<String>,
    pub description: Option<String>,
    pub text: String,
    pub content_hash: Option<String>,
    pub last_seen: u64,
}

#[derive(Clone)]
pub struct LocalSearchIndex {
    docs: Arc<DashMap<String, IndexedDocument>>,
    term_index: Arc<DashMap<String, Vec<String>>>, // token -> list of doc URLs
    max_docs: usize,
}

impl LocalSearchIndex {
    pub fn new(max_docs: usize) -> Self {
        Self {
            docs: Arc::new(DashMap::new()),
            term_index: Arc::new(DashMap::new()),
            max_docs,
        }
    }

    pub fn insert_document(&self, doc: IndexedDocument) {
        if self.docs.len() >= self.max_docs {
            // Evict oldest doc
            let oldest_key = self
                .docs
                .iter()
                .min_by_key(|i| i.value().last_seen)
                .map(|i| i.key().clone());
            if let Some(key) = oldest_key {
                self.docs.remove(&key);
            }
        }

        let url_key = doc.url.to_string();
        let tokens = tokenize(&format!(
            "{} {} {}",
            doc.title.as_deref().unwrap_or_default(),
            doc.description.as_deref().unwrap_or_default(),
            doc.text
        ));

        for token in tokens {
            let mut entry = self.term_index.entry(token).or_default();
            if !entry.contains(&url_key) {
                entry.push(url_key.clone());
            }
        }

        self.docs.insert(url_key, doc);
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let query_tokens = tokenize(query);
        if query_tokens.is_empty() {
            return Vec::new();
        }

        let mut doc_scores: HashMap<String, f32> = HashMap::new();

        for q_token in &query_tokens {
            if let Some(matching_urls) = self.term_index.get(q_token) {
                for url in matching_urls.value() {
                    let entry = doc_scores.entry(url.clone()).or_insert(0.0);
                    *entry += 1.0;
                }
            }
        }

        let mut ranked: Vec<(String, f32)> = doc_scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut results = Vec::new();
        for (url_str, raw_score) in ranked.into_iter().take(limit) {
            if let Some(doc_ref) = self.docs.get(&url_str) {
                let doc = doc_ref.value();
                let snippet = generate_snippet(&doc.text, &query_tokens);
                let score = (raw_score / (query_tokens.len() as f32 * 2.0)).clamp(0.1, 0.99);

                results.push(SearchResult {
                    url: doc.url.clone(),
                    title: doc.title.clone(),
                    description: if !snippet.is_empty() {
                        Some(snippet)
                    } else {
                        doc.description.clone()
                    },
                    score: Some((score * 100.0).round() / 100.0),
                    published_at: None,
                    source: SearchResultSource::LocalIndex,
                    content: None,
                });
            }
        }

        results
    }
}

fn generate_snippet(full_text: &str, query_tokens: &[String]) -> String {
    let lower = full_text.to_lowercase();
    for token in query_tokens {
        if let Some(pos) = lower.find(token) {
            let start = pos.saturating_sub(40);
            let end = (pos + token.len() + 80).min(full_text.len());
            let slice = &full_text[start..end];
            let prefix = if start > 0 { "..." } else { "" };
            let suffix = if end < full_text.len() { "..." } else { "" };
            return format!("{prefix}{}{suffix}", slice.trim().replace('\n', " "));
        }
    }

    if full_text.len() > 150 {
        format!("{}...", full_text[..150].trim().replace('\n', " "))
    } else {
        full_text.trim().replace('\n', " ")
    }
}
