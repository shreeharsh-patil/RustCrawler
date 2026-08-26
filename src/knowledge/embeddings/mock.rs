use super::provider::{EmbeddingBatch, EmbeddingError, EmbeddingProvider};
use async_trait::async_trait;
use blake3::Hasher;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Deterministic mock embedding provider for tests and local development
#[derive(Clone)]
pub struct MockEmbeddingProvider {
    model: String,
    dimensions: usize,
    call_count: Arc<AtomicUsize>,
    total_tokens: Arc<AtomicUsize>,
    simulate_rate_limit: bool,
    simulate_server_error: bool,
    simulate_delay_ms: u64,
}

impl MockEmbeddingProvider {
    pub fn new(dimensions: usize) -> Self {
        Self {
            model: "mock-text-embedding-3".to_string(),
            dimensions,
            call_count: Arc::new(AtomicUsize::new(0)),
            total_tokens: Arc::new(AtomicUsize::new(0)),
            simulate_rate_limit: false,
            simulate_server_error: false,
            simulate_delay_ms: 0,
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn with_simulated_delay(mut self, delay_ms: u64) -> Self {
        self.simulate_delay_ms = delay_ms;
        self
    }

    pub fn with_simulated_rate_limit(mut self, rate_limit: bool) -> Self {
        self.simulate_rate_limit = rate_limit;
        self
    }

    pub fn with_simulated_server_error(mut self, server_error: bool) -> Self {
        self.simulate_server_error = server_error;
        self
    }

    pub fn calls(&self) -> usize {
        self.call_count.load(Ordering::Relaxed)
    }

    pub fn tokens_used(&self) -> usize {
        self.total_tokens.load(Ordering::Relaxed)
    }
}

impl Default for MockEmbeddingProvider {
    fn default() -> Self {
        Self::new(1536)
    }
}

#[async_trait]
impl EmbeddingProvider for MockEmbeddingProvider {
    async fn embed(&self, texts: &[String]) -> Result<EmbeddingBatch, EmbeddingError> {
        self.call_count.fetch_add(1, Ordering::Relaxed);

        if self.simulate_delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.simulate_delay_ms)).await;
        }

        if self.simulate_rate_limit {
            return Err(EmbeddingError::RateLimited(
                "Mock 429 Too Many Requests".to_string(),
            ));
        }

        if self.simulate_server_error {
            return Err(EmbeddingError::ServerError {
                status: 500,
                message: "Mock 500 Internal Server Error".to_string(),
            });
        }

        let mut embeddings = Vec::with_capacity(texts.len());
        let mut batch_tokens = 0;

        for text in texts {
            let tokens = text.len().div_ceil(4);
            batch_tokens += tokens;

            // Generate deterministic unit-normalized pseudo-embedding vector
            let vec = generate_deterministic_vector(text, self.dimensions);
            embeddings.push(vec);
        }

        self.total_tokens.fetch_add(batch_tokens, Ordering::Relaxed);

        Ok(EmbeddingBatch {
            embeddings,
            model: self.model.clone(),
            prompt_tokens: batch_tokens,
        })
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn provider_name(&self) -> &str {
        "mock"
    }
}

/// Generates a deterministic unit-normalized vector of given dimension from text
pub fn generate_deterministic_vector(text: &str, dimensions: usize) -> Vec<f32> {
    let mut vec = Vec::with_capacity(dimensions);
    let words: Vec<&str> = text.split_whitespace().collect();

    // 1. Fill dimensions with pseudo-random floats seeded by word hashes + character positions
    for i in 0..dimensions {
        let mut hasher = Hasher::new();
        hasher.update(text.as_bytes());
        hasher.update(&(i as u32).to_le_bytes());
        let hash = hasher.finalize();
        let bytes = hash.as_bytes();
        let val_u32 = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let raw_f = (val_u32 as f32 / u32::MAX as f32) * 2.0 - 1.0; // [-1.0, 1.0]

        // Add bag-of-words component so shared terms increase dot product
        let mut word_bonus = 0.0f32;
        for w in &words {
            let mut w_hasher = Hasher::new();
            w_hasher.update(w.to_lowercase().as_bytes());
            w_hasher.update(&(i as u32).to_le_bytes());
            let w_hash = w_hasher.finalize();
            let w_val = (w_hash.as_bytes()[0] as f32 / 255.0) * 0.4 - 0.2;
            word_bonus += w_val;
        }

        vec.push(raw_f + word_bonus);
    }

    // 2. Normalize to unit length (L2 norm = 1.0)
    let norm_sq: f32 = vec.iter().map(|v| v * v).sum();
    let norm = norm_sq.sqrt();
    if norm > 0.0 {
        for v in vec.iter_mut() {
            *v /= norm;
        }
    }

    vec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_provider_deterministic_unit_vectors() {
        let provider = MockEmbeddingProvider::new(128);
        let batch = provider
            .embed(&[
                "hello world".to_string(),
                "hello world".to_string(),
                "completely different text".to_string(),
            ])
            .await
            .unwrap();

        assert_eq!(batch.embeddings.len(), 3);
        assert_eq!(
            batch.embeddings[0], batch.embeddings[1],
            "Identical text must produce identical embeddings"
        );

        // Verify unit norm
        let norm: f32 = batch.embeddings[0].iter().map(|v| v * v).sum();
        assert!((norm - 1.0).abs() < 1e-4, "L2 norm should be 1.0");
    }
}
