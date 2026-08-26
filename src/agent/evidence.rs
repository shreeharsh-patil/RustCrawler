use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::models::{EvidenceConflict, SourceType};
use crate::agent::policy::AgentPolicy;

/// An individual unit of evidence gathered during web research
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub id: String,
    pub source_url: String,
    pub source_title: Option<String>,
    pub source_type: SourceType,
    pub content_hash: String,
    pub chunk_id: Option<String>,
    pub excerpt: String,
    pub extracted_fact: Option<Value>,
    pub relevance: f32,
    pub published_at: Option<DateTime<Utc>>,
    pub fetched_at: DateTime<Utc>,
}

/// Thread-safe evidence repository tracking collected facts, source provenance, deduplication, and conflicts
pub struct EvidenceStore {
    items: RwLock<Vec<Evidence>>,
    visited_urls: RwLock<HashSet<String>>,
    content_hashes: RwLock<HashSet<String>>,
    conflicts: RwLock<Vec<EvidenceConflict>>,
    next_id: AtomicUsize,
}

impl Default for EvidenceStore {
    fn default() -> Self {
        Self::new()
    }
}

impl EvidenceStore {
    pub fn new() -> Self {
        Self {
            items: RwLock::new(Vec::new()),
            visited_urls: RwLock::new(HashSet::new()),
            content_hashes: RwLock::new(HashSet::new()),
            conflicts: RwLock::new(Vec::new()),
            next_id: AtomicUsize::new(1),
        }
    }

    /// Allocates the next sequential evidence ID (e.g. "E1", "E2")
    pub fn next_evidence_id(&self) -> String {
        let num = self.next_id.fetch_add(1, Ordering::SeqCst);
        format!("E{}", num)
    }

    /// Records a visited URL to avoid redundant scraping
    pub fn record_visited_url(&self, url: &str) {
        if let Ok(mut set) = self.visited_urls.write() {
            set.insert(url.to_string());
        }
    }

    /// Checks if a URL has already been visited
    pub fn has_visited_url(&self, url: &str) -> bool {
        self.visited_urls
            .read()
            .map(|set| set.contains(url))
            .unwrap_or(false)
    }

    /// Inserts an evidence record, deduplicating identical content hashes from the same URL
    pub fn insert(&self, mut evidence: Evidence) -> Option<String> {
        let mut hashes = self.content_hashes.write().ok()?;
        if hashes.contains(&evidence.content_hash) {
            return None;
        }
        hashes.insert(evidence.content_hash.clone());

        if evidence.id.is_empty() {
            evidence.id = self.next_evidence_id();
        }

        let assigned_id = evidence.id.clone();
        if let Ok(mut items) = self.items.write() {
            items.push(evidence);
            Some(assigned_id)
        } else {
            None
        }
    }

    /// Retrieves an evidence record by ID
    pub fn get(&self, id: &str) -> Option<Evidence> {
        self.items.read().ok()?.iter().find(|e| e.id == id).cloned()
    }

    /// Retrieves all collected evidence
    pub fn all(&self) -> Vec<Evidence> {
        self.items.read().map(|i| i.clone()).unwrap_or_default()
    }

    /// Retrieves top relevant evidence ranked by relevance * source quality weight
    pub fn top_relevant(&self, limit: usize) -> Vec<Evidence> {
        let mut all_evidence = self.all();
        all_evidence.sort_by(|a, b| {
            let score_a = a.relevance * a.source_type.quality_weight();
            let score_b = b.relevance * b.source_type.quality_weight();
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        all_evidence.truncate(limit);
        all_evidence
    }

    /// Returns the number of distinct source URLs represented in the evidence
    pub fn unique_sources_count(&self) -> usize {
        self.items
            .read()
            .map(|items| {
                let urls: HashSet<&str> = items.iter().map(|e| e.source_url.as_str()).collect();
                urls.len()
            })
            .unwrap_or(0)
    }

    /// Records an identified evidence conflict
    pub fn record_conflict(
        &self,
        field: &str,
        evidence_ids: Vec<String>,
        details: &str,
        resolved_value: Option<Value>,
    ) {
        if let Ok(mut conflicts) = self.conflicts.write() {
            conflicts.push(EvidenceConflict {
                field: field.to_string(),
                evidence_ids,
                details: details.to_string(),
                resolved_value,
            });
        }
    }

    /// Returns all recorded conflicts
    pub fn conflicts(&self) -> Vec<EvidenceConflict> {
        self.conflicts.read().map(|c| c.clone()).unwrap_or_default()
    }

    /// Formats collected evidence into a bounded XML context string for LLM synthesis
    pub fn format_for_prompt(&self, max_items: usize, max_excerpt_chars: usize) -> String {
        let top = self.top_relevant(max_items);
        if top.is_empty() {
            return "<evidence_items count=\"0\"></evidence_items>".to_string();
        }

        let mut out = format!("<evidence_items count=\"{}\">\n", top.len());
        for item in top {
            let sanitized = AgentPolicy::sanitize_observation(&item.excerpt, max_excerpt_chars);
            let title = item.source_title.as_deref().unwrap_or("Untitled");
            out.push_str(&format!(
                "  <evidence_item id=\"{}\" url=\"{}\" title=\"{}\" source_type=\"{:?}\">\n    {}\n  </evidence_item>\n",
                item.id, item.source_url, title, item.source_type, sanitized
            ));
        }
        out.push_str("</evidence_items>");
        out
    }
}
