use super::cache::{EmbeddingCache, EmbeddingCacheKey};
use super::provider::{EmbeddingError, EmbeddingProvider};
use crate::knowledge::models::IndexedChunk;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Configuration for embedding batching pipeline
#[derive(Debug, Clone)]
pub struct BatcherConfig {
    pub max_batch_size: usize,
    pub max_batch_tokens: usize,
    pub max_concurrent_requests: usize,
}

impl Default for BatcherConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 32,
            max_batch_tokens: 20000,
            max_concurrent_requests: 4,
        }
    }
}

/// Pipeline manager for batching, caching, and generating embeddings
pub struct EmbeddingBatcher {
    provider: Arc<dyn EmbeddingProvider>,
    cache: Arc<EmbeddingCache>,
    config: BatcherConfig,
    semaphore: Arc<Semaphore>,
}

impl EmbeddingBatcher {
    pub fn new(
        provider: Arc<dyn EmbeddingProvider>,
        cache: Arc<EmbeddingCache>,
        config: BatcherConfig,
    ) -> Self {
        let max_concurrent = config.max_concurrent_requests.clamp(1, 64);
        Self {
            provider,
            cache,
            config,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    /// Generates or retrieves embeddings for a list of indexed chunks in order
    pub async fn embed_chunks(
        &self,
        chunks: &[IndexedChunk],
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        let provider_name = self.provider.provider_name();
        let model = self.provider.model();
        let dimensions = self.provider.dimensions();

        let mut results: Vec<Option<Vec<f32>>> = vec![None; chunks.len()];
        let mut uncached_indices = Vec::new();
        let mut uncached_texts = Vec::new();

        // 1. Query cache first
        for (i, chunk) in chunks.iter().enumerate() {
            let key = EmbeddingCacheKey::new(&chunk.content_hash, provider_name, model, dimensions);
            if let Some(cached_vec) = self.cache.get(&key) {
                results[i] = Some(cached_vec);
            } else {
                uncached_indices.push(i);
                uncached_texts.push((
                    i,
                    &chunk.content_hash,
                    chunk.text.clone(),
                    chunk.token_count,
                ));
            }
        }

        // If everything was cached, return immediately
        if uncached_texts.is_empty() {
            return Ok(results.into_iter().flatten().collect());
        }

        // 2. Partition uncached texts into bounded batches
        let mut batches: Vec<Vec<(usize, String, String)>> = Vec::new(); // (original_index, hash, text)
        let mut current_batch = Vec::new();
        let mut current_tokens = 0;

        for (idx, hash, text, tokens) in uncached_texts {
            if (current_batch.len() >= self.config.max_batch_size
                || current_tokens + tokens > self.config.max_batch_tokens)
                && !current_batch.is_empty()
            {
                batches.push(current_batch);
                current_batch = Vec::new();
                current_tokens = 0;
            }
            current_tokens += tokens;
            current_batch.push((idx, hash.to_string(), text));
        }
        if !current_batch.is_empty() {
            batches.push(current_batch);
        }

        // 3. Process batches concurrently up to max_concurrent_requests
        let mut tasks = Vec::new();

        for batch in batches {
            let provider = self.provider.clone();
            let cache = self.cache.clone();
            let permit = self
                .semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| EmbeddingError::HttpError(format!("Semaphore error: {e}")))?;
            let provider_name = provider_name.to_string();
            let model = model.to_string();

            tasks.push(tokio::spawn(async move {
                let _permit = permit;
                let texts: Vec<String> = batch.iter().map(|(_, _, t)| t.clone()).collect();
                let batch_res = provider.embed(&texts).await?;

                let mut batch_outputs = Vec::with_capacity(batch.len());
                for (k, (orig_idx, hash, _)) in batch.into_iter().enumerate() {
                    let vec = batch_res.embeddings[k].clone();
                    let key = EmbeddingCacheKey::new(&hash, &provider_name, &model, dimensions);
                    cache.insert(key, vec.clone());
                    batch_outputs.push((orig_idx, vec));
                }

                Ok::<Vec<(usize, Vec<f32>)>, EmbeddingError>(batch_outputs)
            }));
        }

        // 4. Collect results and populate output array in original order
        for task in tasks {
            let task_result = task
                .await
                .map_err(|e| EmbeddingError::HttpError(format!("Join error: {e}")))?;
            let batch_items = task_result?;
            for (idx, vec) in batch_items {
                results[idx] = Some(vec);
            }
        }

        let final_embeddings: Vec<Vec<f32>> = results.into_iter().flatten().collect();
        if final_embeddings.len() != chunks.len() {
            return Err(EmbeddingError::InvalidResponse(format!(
                "Expected {} embeddings, produced {}",
                chunks.len(),
                final_embeddings.len()
            )));
        }

        Ok(final_embeddings)
    }

    /// Embeds a single query string (e.g. for semantic search)
    pub async fn embed_query(&self, query: &str) -> Result<Vec<f32>, EmbeddingError> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| EmbeddingError::HttpError(format!("Semaphore error: {e}")))?;
        let batch = self.provider.embed(&[query.to_string()]).await?;
        batch.embeddings.into_iter().next().ok_or_else(|| {
            EmbeddingError::InvalidResponse("Empty embedding response for query".to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::chunking::dedupe::compute_chunk_hash;
    use crate::knowledge::embeddings::mock::MockEmbeddingProvider;
    use crate::knowledge::models::ChunkMetadata;

    #[tokio::test]
    async fn test_embedding_batcher_with_cache_hits() {
        let provider = Arc::new(MockEmbeddingProvider::new(64));
        let cache = Arc::new(EmbeddingCache::default());
        let batcher =
            EmbeddingBatcher::new(provider.clone(), cache.clone(), BatcherConfig::default());

        let chunks = vec![
            IndexedChunk {
                chunk_id: "chk_1".to_string(),
                document_id: "doc_1".to_string(),
                source_url: None,
                heading_path: Vec::new(),
                text: "First test chunk".to_string(),
                token_count: 5,
                content_hash: compute_chunk_hash("First test chunk"),
                position: 0,
                metadata: ChunkMetadata::default(),
            },
            IndexedChunk {
                chunk_id: "chk_2".to_string(),
                document_id: "doc_1".to_string(),
                source_url: None,
                heading_path: Vec::new(),
                text: "Second test chunk".to_string(),
                token_count: 5,
                content_hash: compute_chunk_hash("Second test chunk"),
                position: 1,
                metadata: ChunkMetadata::default(),
            },
        ];

        // 1. Initial embed call -> cache misses
        let vectors1 = batcher.embed_chunks(&chunks).await.unwrap();
        assert_eq!(vectors1.len(), 2);
        assert_eq!(provider.calls(), 1);

        // 2. Second embed call with same chunks -> cache hits, 0 extra provider calls!
        let vectors2 = batcher.embed_chunks(&chunks).await.unwrap();
        assert_eq!(vectors2.len(), 2);
        assert_eq!(
            provider.calls(),
            1,
            "Cache hit must avoid calling provider again"
        );
        assert_eq!(vectors1, vectors2);
    }
}
