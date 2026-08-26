use super::store::{VectorMatch, VectorQuery, VectorRecord, VectorStore, VectorStoreError};
use crate::knowledge::models::IndexFilter;
use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;

/// In-memory vector store for fast development, testing, and single-node indexing
#[derive(Clone, Default)]
pub struct MemoryVectorStore {
    // index_id -> Vec<VectorRecord>
    indexes: Arc<DashMap<String, Vec<VectorRecord>>>,
}

impl MemoryVectorStore {
    pub fn new() -> Self {
        Self {
            indexes: Arc::new(DashMap::new()),
        }
    }
}

#[async_trait]
impl VectorStore for MemoryVectorStore {
    async fn upsert(
        &self,
        index_id: &str,
        records: Vec<VectorRecord>,
    ) -> Result<(), VectorStoreError> {
        let mut entry = self.indexes.entry(index_id.to_string()).or_default();
        for record in records {
            // Replace if chunk_id already exists, otherwise push
            if let Some(pos) = entry.iter().position(|r| r.chunk_id == record.chunk_id) {
                entry[pos] = record;
            } else {
                entry.push(record);
            }
        }
        Ok(())
    }

    async fn search(
        &self,
        index_id: &str,
        query: VectorQuery,
    ) -> Result<Vec<VectorMatch>, VectorStoreError> {
        let entry = match self.indexes.get(index_id) {
            Some(e) => e,
            None => return Ok(Vec::new()),
        };

        if query.vector.is_empty() {
            return Err(VectorStoreError::InvalidQuery(
                "Query vector cannot be empty".to_string(),
            ));
        }

        let query_dim = query.vector.len();
        let query_norm_sq: f32 = query.vector.iter().map(|v| v * v).sum();
        let query_norm = query_norm_sq.sqrt();

        let mut scored_matches: Vec<VectorMatch> = Vec::new();

        for record in entry.value() {
            // Tenant isolation check
            if query.tenant_id.is_some() && record.tenant_id != query.tenant_id {
                continue;
            }

            // Apply metadata filters
            if let Some(ref filter) = query.filter {
                if !matches_filter(record, filter) {
                    continue;
                }
            }

            if record.vector.len() != query_dim {
                continue;
            }

            // Cosine similarity
            let dot_product: f32 = record
                .vector
                .iter()
                .zip(&query.vector)
                .map(|(a, b)| a * b)
                .sum();

            let doc_norm_sq: f32 = record.vector.iter().map(|v| v * v).sum();
            let doc_norm = doc_norm_sq.sqrt();

            let sim = if query_norm > 0.0 && doc_norm > 0.0 {
                (dot_product / (query_norm * doc_norm)) as f64
            } else {
                0.0
            };

            // Normalize cosine similarity to [0.0, 1.0] range
            let normalized_score = ((sim + 1.0) / 2.0).clamp(0.0, 1.0);

            if let Some(min_score) = query.min_score {
                if normalized_score < min_score {
                    continue;
                }
            }

            scored_matches.push(VectorMatch {
                chunk_id: record.chunk_id.clone(),
                document_id: record.document_id.clone(),
                source_url: record.source_url.clone(),
                title: record.title.clone(),
                heading_path: record.heading_path.clone(),
                text: record.text.clone(),
                score: normalized_score,
                metadata: record.metadata.clone(),
                document_metadata: record.document_metadata.clone(),
            });
        }

        // Sort descending by similarity score
        scored_matches.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if scored_matches.len() > query.limit {
            scored_matches.truncate(query.limit);
        }

        Ok(scored_matches)
    }

    async fn delete_document(
        &self,
        index_id: &str,
        document_id: &str,
    ) -> Result<usize, VectorStoreError> {
        if let Some(mut entry) = self.indexes.get_mut(index_id) {
            let initial_len = entry.len();
            entry.retain(|r| r.document_id != document_id);
            Ok(initial_len - entry.len())
        } else {
            Ok(0)
        }
    }

    async fn delete_index(&self, index_id: &str) -> Result<(), VectorStoreError> {
        self.indexes.remove(index_id);
        Ok(())
    }

    async fn count(&self, index_id: &str) -> Result<usize, VectorStoreError> {
        Ok(self.indexes.get(index_id).map(|e| e.len()).unwrap_or(0))
    }
}

fn matches_filter(record: &VectorRecord, filter: &IndexFilter) -> bool {
    let doc_meta = record.document_metadata.as_ref();

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
        if let Some(url) = &record.source_url {
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
        if &record.document_id != doc_id {
            return false;
        }
    }

    if let Some(ref req_tags) = filter.tags {
        if let Some(meta) = doc_meta {
            for t in req_tags {
                if !meta.tags.contains(t) {
                    return false;
                }
            }
        } else {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::models::ChunkMetadata;

    #[tokio::test]
    async fn test_memory_vector_store_upsert_and_cosine_search() {
        let store = MemoryVectorStore::new();

        let rec1 = VectorRecord {
            chunk_id: "chk_1".to_string(),
            document_id: "doc_1".to_string(),
            index_id: "idx_test".to_string(),
            tenant_id: None,
            source_url: Some("https://example.com/page1".to_string()),
            title: Some("Page 1".to_string()),
            heading_path: vec!["Header".to_string()],
            text: "Content 1".to_string(),
            vector: vec![1.0, 0.0, 0.0],
            metadata: ChunkMetadata::default(),
            document_metadata: None,
        };

        let rec2 = VectorRecord {
            chunk_id: "chk_2".to_string(),
            document_id: "doc_2".to_string(),
            index_id: "idx_test".to_string(),
            tenant_id: None,
            source_url: Some("https://example.com/page2".to_string()),
            title: Some("Page 2".to_string()),
            heading_path: vec!["Header".to_string()],
            text: "Content 2".to_string(),
            vector: vec![0.0, 1.0, 0.0],
            metadata: ChunkMetadata::default(),
            document_metadata: None,
        };

        store.upsert("idx_test", vec![rec1, rec2]).await.unwrap();

        // Search closest to [1.0, 0.1, 0.0]
        let query = VectorQuery {
            vector: vec![1.0, 0.1, 0.0],
            limit: 2,
            filter: None,
            tenant_id: None,
            min_score: None,
        };

        let matches = store.search("idx_test", query).await.unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].chunk_id, "chk_1", "chk_1 should be top match");
        assert!(matches[0].score > matches[1].score);
    }
}
