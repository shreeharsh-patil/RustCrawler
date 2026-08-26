use crate::knowledge::models::{ChunkMetadata, IndexFilter, IndexMetadata, IndexedChunk};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Match returned from lexical full-text inverted index search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LexicalMatch {
    pub chunk_id: String,
    pub document_id: String,
    pub source_url: Option<String>,
    pub title: Option<String>,
    pub heading_path: Vec<String>,
    pub text: String,
    pub score: f64, // BM25-derived score
    pub metadata: ChunkMetadata,
    pub document_metadata: Option<IndexMetadata>,
}

#[derive(Clone)]
struct DocumentPosting {
    chunk_id: String,
    document_id: String,
    tenant_id: Option<String>,
    source_url: Option<String>,
    title: Option<String>,
    heading_path: Vec<String>,
    text: String,
    field_term_freqs: HashMap<String, f64>, // term -> weighted frequency
    total_terms: usize,
    metadata: ChunkMetadata,
    document_metadata: Option<IndexMetadata>,
}

/// In-memory BM25-style inverted index with multi-field weighting and technical term preservation
#[derive(Clone, Default)]
pub struct LexicalIndex {
    // index_id -> (term -> Vec<chunk_id>)
    inverted_index: Arc<DashMap<String, HashMap<String, HashSet<String>>>>,
    // index_id -> (chunk_id -> DocumentPosting)
    postings: Arc<DashMap<String, HashMap<String, DocumentPosting>>>,
}

impl LexicalIndex {
    pub fn new() -> Self {
        Self {
            inverted_index: Arc::new(DashMap::new()),
            postings: Arc::new(DashMap::new()),
        }
    }

    /// Indexes a single chunk into the lexical inverted index
    pub fn insert_chunk(
        &self,
        index_id: &str,
        chunk: &IndexedChunk,
        title: Option<&str>,
        tenant_id: Option<&str>,
        doc_metadata: Option<&IndexMetadata>,
    ) {
        let mut term_freqs: HashMap<String, f64> = HashMap::new();
        let mut total_term_count = 0;

        // 1. Title tokens (weight: 3.0)
        if let Some(t) = title {
            for token in tokenize_text(t) {
                *term_freqs.entry(token).or_default() += 3.0;
                total_term_count += 1;
            }
        }

        // 2. Heading path tokens (weight: 2.0)
        for h in &chunk.heading_path {
            for token in tokenize_text(h) {
                *term_freqs.entry(token).or_default() += 2.0;
                total_term_count += 1;
            }
        }

        // 3. Source URL tokens (weight: 1.5)
        if let Some(ref u) = chunk.source_url {
            for token in tokenize_text(u) {
                *term_freqs.entry(token).or_default() += 1.5;
                total_term_count += 1;
            }
        }

        // 4. Body text tokens (weight: 1.0)
        for token in tokenize_text(&chunk.text) {
            *term_freqs.entry(token).or_default() += 1.0;
            total_term_count += 1;
        }

        let posting = DocumentPosting {
            chunk_id: chunk.chunk_id.clone(),
            document_id: chunk.document_id.clone(),
            tenant_id: tenant_id.map(ToString::to_string),
            source_url: chunk.source_url.clone(),
            title: title.map(ToString::to_string),
            heading_path: chunk.heading_path.clone(),
            text: chunk.text.clone(),
            field_term_freqs: term_freqs.clone(),
            total_terms: total_term_count.max(1),
            metadata: chunk.metadata.clone(),
            document_metadata: doc_metadata.cloned(),
        };

        // Insert into postings store
        let mut index_postings = self.postings.entry(index_id.to_string()).or_default();
        index_postings.insert(chunk.chunk_id.clone(), posting);

        // Update inverted index
        let mut index_inv = self.inverted_index.entry(index_id.to_string()).or_default();
        for term in term_freqs.keys() {
            index_inv
                .entry(term.clone())
                .or_default()
                .insert(chunk.chunk_id.clone());
        }
    }

    /// Searches the lexical index using BM25 scoring with exact term bonus
    pub fn search(
        &self,
        index_id: &str,
        query: &str,
        limit: usize,
        filter: Option<&IndexFilter>,
        tenant_id: Option<&str>,
    ) -> Vec<LexicalMatch> {
        let index_postings = match self.postings.get(index_id) {
            Some(p) => p,
            None => return Vec::new(),
        };

        let index_inv = match self.inverted_index.get(index_id) {
            Some(i) => i,
            None => return Vec::new(),
        };

        let query_tokens = tokenize_text(query);
        if query_tokens.is_empty() {
            return Vec::new();
        }

        let total_docs = index_postings.len() as f64;
        if total_docs == 0.0 {
            return Vec::new();
        }

        // Calculate average document length
        let total_doc_len: usize = index_postings.values().map(|p| p.total_terms).sum();
        let avg_dl = (total_doc_len as f64 / total_docs).max(1.0);

        // BM25 parameters
        let k1 = 1.2f64;
        let b = 0.75f64;

        // Candidate chunk accumulation
        let mut candidate_ids: HashSet<String> = HashSet::new();
        for q_token in &query_tokens {
            if let Some(chunk_set) = index_inv.get(q_token) {
                for cid in chunk_set {
                    candidate_ids.insert(cid.clone());
                }
            }
        }

        let mut scored_matches: Vec<LexicalMatch> = Vec::new();

        for cid in candidate_ids {
            let posting = match index_postings.get(&cid) {
                Some(p) => p,
                None => continue,
            };

            // Tenant isolation
            if tenant_id.is_some() && posting.tenant_id.as_deref() != tenant_id {
                continue;
            }

            // Filters
            if let Some(f) = filter {
                if !matches_lexical_filter(posting, f) {
                    continue;
                }
            }

            let doc_len = posting.total_terms as f64;
            let mut score = 0.0f64;

            for q_token in &query_tokens {
                if let Some(&tf) = posting.field_term_freqs.get(q_token) {
                    // Document frequency for term
                    let df = index_inv.get(q_token).map(|s| s.len()).unwrap_or(1) as f64;
                    let idf = ((total_docs - df + 0.5) / (df + 0.5) + 1.0).ln().max(0.1);

                    // BM25 term weight
                    let tf_component =
                        (tf * (k1 + 1.0)) / (tf + k1 * (1.0 - b + b * (doc_len / avg_dl)));
                    score += idf * tf_component;
                }
            }

            // Bonus for exact multi-token substring match
            if posting.text.to_lowercase().contains(&query.to_lowercase()) {
                score *= 1.5;
            }

            scored_matches.push(LexicalMatch {
                chunk_id: posting.chunk_id.clone(),
                document_id: posting.document_id.clone(),
                source_url: posting.source_url.clone(),
                title: posting.title.clone(),
                heading_path: posting.heading_path.clone(),
                text: posting.text.clone(),
                score,
                metadata: posting.metadata.clone(),
                document_metadata: posting.document_metadata.clone(),
            });
        }

        // Sort descending by score
        scored_matches.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if scored_matches.len() > limit {
            scored_matches.truncate(limit);
        }

        scored_matches
    }

    pub fn delete_document(&self, index_id: &str, document_id: &str) {
        if let Some(mut postings_map) = self.postings.get_mut(index_id) {
            let removed_chunk_ids: Vec<String> = postings_map
                .values()
                .filter(|p| p.document_id == document_id)
                .map(|p| p.chunk_id.clone())
                .collect();

            for cid in &removed_chunk_ids {
                postings_map.remove(cid);
            }

            if let Some(mut inv_map) = self.inverted_index.get_mut(index_id) {
                for set in inv_map.values_mut() {
                    for cid in &removed_chunk_ids {
                        set.remove(cid);
                    }
                }
            }
        }
    }

    pub fn delete_index(&self, index_id: &str) {
        self.postings.remove(index_id);
        self.inverted_index.remove(index_id);
    }
}

/// Tokenizes text into search terms, preserving technical identifiers (UPPERCASE, snake_case, kebab-case, filenames)
pub fn tokenize_text(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let words = text.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != '.');

    for word in words {
        let trimmed = word.trim_matches(|c: char| !c.is_alphanumeric());
        if trimmed.is_empty() {
            continue;
        }

        // Add exact term in lowercase
        tokens.push(trimmed.to_lowercase());

        // For technical identifiers like snake_case, kebab-case, or filenames with extensions, also add sub-tokens
        if trimmed.contains('_') || trimmed.contains('-') || trimmed.contains('.') {
            for sub in trimmed.split(['_', '-', '.']) {
                let sub_trimmed = sub.trim();
                if !sub_trimmed.is_empty() && sub_trimmed.len() > 1 {
                    tokens.push(sub_trimmed.to_lowercase());
                }
            }
        }
    }

    tokens
}

fn matches_lexical_filter(posting: &DocumentPosting, filter: &IndexFilter) -> bool {
    let doc_meta = posting.document_metadata.as_ref();

    if let Some(ref domain) = filter.domain {
        let r_domain = doc_meta.and_then(|m| m.domain.as_ref());
        if r_domain != Some(domain) {
            return false;
        }
    }

    if let Some(ref hostname) = filter.hostname {
        let r_host = doc_meta.and_then(|m| m.hostname.as_ref());
        if r_host != Some(hostname) {
            return false;
        }
    }

    if let Some(ref prefix) = filter.path_prefix {
        if let Some(url) = &posting.source_url {
            if let Ok(parsed) = url::Url::parse(url) {
                if !parsed.path().starts_with(prefix) {
                    return false;
                }
            } else if !url.starts_with(prefix) {
                return false;
            }
        } else {
            return false;
        }
    }

    if let Some(ref lang) = filter.language {
        let r_lang = doc_meta.and_then(|m| m.language.as_ref());
        if r_lang != Some(lang) {
            return false;
        }
    }

    if let Some(ref crawl_job_id) = filter.crawl_job_id {
        let r_job = doc_meta.and_then(|m| m.crawl_job_id.as_ref());
        if r_job != Some(crawl_job_id) {
            return false;
        }
    }

    if let Some(ref doc_id) = filter.document_id {
        if &posting.document_id != doc_id {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexical_index_exact_technical_term_search() {
        let index = LexicalIndex::new();

        let chunk1 = IndexedChunk {
            chunk_id: "chk_1".to_string(),
            document_id: "doc_1".to_string(),
            source_url: Some("https://example.com/config".to_string()),
            heading_path: vec!["Configuration".to_string()],
            text: "Set MAX_CONCURRENT_FETCHES to 50 for optimal throughput.".to_string(),
            token_count: 10,
            content_hash: "hash1".to_string(),
            position: 0,
            metadata: ChunkMetadata::default(),
        };

        let chunk2 = IndexedChunk {
            chunk_id: "chk_2".to_string(),
            document_id: "doc_2".to_string(),
            source_url: Some("https://example.com/about".to_string()),
            heading_path: vec!["About".to_string()],
            text: "General overview of the crawling architecture.".to_string(),
            token_count: 8,
            content_hash: "hash2".to_string(),
            position: 0,
            metadata: ChunkMetadata::default(),
        };

        index.insert_chunk("idx_docs", &chunk1, Some("Crawler Config"), None, None);
        index.insert_chunk("idx_docs", &chunk2, Some("About Us"), None, None);

        // Search for exact technical token
        let results = index.search("idx_docs", "MAX_CONCURRENT_FETCHES", 5, None, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].chunk_id, "chk_1");
        assert!(results[0].score > 0.0);
    }
}
