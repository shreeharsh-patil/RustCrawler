use super::provider::{EmbeddingBatch, EmbeddingError, EmbeddingProvider};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, RETRY_AFTER};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone)]
pub struct OpenAiCompatibleEmbeddingProvider {
    base_url: String,
    api_key: Option<String>,
    model: String,
    dimensions: usize,
    client: reqwest::Client,
    timeout: Duration,
    max_retries: usize,
}

impl OpenAiCompatibleEmbeddingProvider {
    pub fn new(
        base_url: impl Into<String>,
        api_key: Option<String>,
        model: impl Into<String>,
        dimensions: usize,
        timeout: Duration,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();

        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key,
            model: model.into(),
            dimensions,
            client,
            timeout,
            max_retries: 3,
        }
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    model: &'a str,
    input: &'a [String],
}

#[derive(Deserialize)]
struct EmbeddingDataItem {
    embedding: Vec<f32>,
    #[serde(default)]
    index: usize,
}

#[derive(Deserialize)]
struct EmbeddingUsage {
    #[serde(default)]
    prompt_tokens: usize,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingDataItem>,
    #[serde(default)]
    usage: Option<EmbeddingUsage>,
}

#[async_trait]
impl EmbeddingProvider for OpenAiCompatibleEmbeddingProvider {
    async fn embed(&self, texts: &[String]) -> Result<EmbeddingBatch, EmbeddingError> {
        if texts.is_empty() {
            return Ok(EmbeddingBatch {
                embeddings: Vec::new(),
                model: self.model.clone(),
                prompt_tokens: 0,
            });
        }

        let endpoint = format!("{}/v1/embeddings", self.base_url);
        let req_body = EmbeddingRequest {
            model: &self.model,
            input: texts,
        };

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(ref key) = self.api_key {
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {key}")) {
                headers.insert(AUTHORIZATION, val);
            }
        }

        let mut attempt = 0;
        let mut backoff = Duration::from_millis(500);

        loop {
            attempt += 1;
            let res = self
                .client
                .post(&endpoint)
                .headers(headers.clone())
                .json(&req_body)
                .send()
                .await;

            match res {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        let parsed: EmbeddingResponse = response.json().await.map_err(|e| {
                            EmbeddingError::InvalidResponse(format!(
                                "Failed to parse JSON response: {e}"
                            ))
                        })?;

                        let mut items = parsed.data;
                        items.sort_by_key(|item| item.index);
                        let embeddings: Vec<Vec<f32>> =
                            items.into_iter().map(|item| item.embedding).collect();

                        if embeddings.len() != texts.len() {
                            return Err(EmbeddingError::InvalidResponse(format!(
                                "Expected {} embeddings, got {}",
                                texts.len(),
                                embeddings.len()
                            )));
                        }

                        let prompt_tokens = parsed.usage.map(|u| u.prompt_tokens).unwrap_or(0);
                        return Ok(EmbeddingBatch {
                            embeddings,
                            model: self.model.clone(),
                            prompt_tokens,
                        });
                    }

                    if status.as_u16() == 429 {
                        let retry_after_sec = response
                            .headers()
                            .get(RETRY_AFTER)
                            .and_then(|h| h.to_str().ok())
                            .and_then(|s| s.parse::<u64>().ok());

                        if attempt <= self.max_retries {
                            let sleep_dur =
                                retry_after_sec.map(Duration::from_secs).unwrap_or(backoff);
                            tokio::time::sleep(sleep_dur).await;
                            backoff = (backoff * 2).min(Duration::from_secs(10));
                            continue;
                        }

                        let err_text = response.text().await.unwrap_or_default();
                        return Err(EmbeddingError::RateLimited(err_text));
                    }

                    if status.is_server_error() {
                        if attempt <= self.max_retries {
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(Duration::from_secs(10));
                            continue;
                        }
                        let err_text = response.text().await.unwrap_or_default();
                        return Err(EmbeddingError::ServerError {
                            status: status.as_u16(),
                            message: err_text,
                        });
                    }

                    let err_text = response.text().await.unwrap_or_default();
                    return Err(EmbeddingError::HttpError(format!(
                        "HTTP {} - {}",
                        status.as_u16(),
                        err_text
                    )));
                }
                Err(e) => {
                    if e.is_timeout() {
                        if attempt <= self.max_retries {
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(Duration::from_secs(10));
                            continue;
                        }
                        return Err(EmbeddingError::Timeout);
                    }
                    if attempt <= self.max_retries {
                        tokio::time::sleep(backoff).await;
                        backoff = (backoff * 2).min(Duration::from_secs(10));
                        continue;
                    }
                    return Err(EmbeddingError::HttpError(e.to_string()));
                }
            }
        }
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn provider_name(&self) -> &str {
        "openai"
    }
}
