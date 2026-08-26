use crate::config::Config;
use crate::error::CrawlerError;
use crate::search::models::{SearchRequest, SearchResult, SearchResultSource};
use async_trait::async_trait;
use dashmap::DashMap;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde_json::Value;
use std::sync::Arc;
use url::Url;

#[async_trait]
pub trait SearchProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn search(&self, request: &SearchRequest) -> Result<Vec<SearchResult>, CrawlerError>;
}

pub struct MockSearchProvider {
    canned_results: Arc<DashMap<String, Vec<SearchResult>>>,
}

impl Default for MockSearchProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MockSearchProvider {
    pub fn new() -> Self {
        let store = Arc::new(DashMap::new());

        // Default mock results for tests
        store.insert(
            "rust async web crawler".to_string(),
            vec![
                SearchResult {
                    url: Url::parse("https://github.com/rust-lang/crawler").unwrap(),
                    title: Some("Rust Async Web Crawler Framework".to_string()),
                    description: Some(
                        "High performance asynchronous web crawler and scraper written in Rust."
                            .to_string(),
                    ),
                    score: Some(0.95),
                    published_at: None,
                    source: SearchResultSource::Web,
                    content: None,
                },
                SearchResult {
                    url: Url::parse("https://docs.rs/rustcrawl").unwrap(),
                    title: Some("rustcrawl - Documentation".to_string()),
                    description: Some(
                        "Universal web scraping and structured extraction engine for Rust."
                            .to_string(),
                    ),
                    score: Some(0.89),
                    published_at: None,
                    source: SearchResultSource::Web,
                    content: None,
                },
            ],
        );

        Self {
            canned_results: store,
        }
    }

    pub fn set_results(&self, query: impl Into<String>, results: Vec<SearchResult>) {
        self.canned_results
            .insert(query.into().to_lowercase(), results);
    }
}

#[async_trait]
impl SearchProvider for MockSearchProvider {
    fn name(&self) -> &str {
        "mock"
    }

    async fn search(&self, request: &SearchRequest) -> Result<Vec<SearchResult>, CrawlerError> {
        let q = request.query.trim().to_lowercase();
        if let Some(res) = self.canned_results.get(&q) {
            let mut results = res.value().clone();
            if results.len() > request.limit {
                results.truncate(request.limit);
            }
            return Ok(results);
        }

        // Generic mock search response if not explicitly canned
        let mut fallback_url = Url::parse("https://example.com/search").unwrap();
        fallback_url.query_pairs_mut().append_pair("q", &q);

        Ok(vec![SearchResult {
            url: fallback_url,
            title: Some(format!("Search results for '{}'", request.query)),
            description: Some(format!(
                "Simulated search results matching query terms: {}",
                request.query
            )),
            score: Some(0.80),
            published_at: None,
            source: SearchResultSource::Web,
            content: None,
        }])
    }
}

pub struct ExternalSearchProvider {
    name: String,
    base_url: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl ExternalSearchProvider {
    pub fn new(
        name: impl Into<String>,
        base_url: String,
        api_key: Option<String>,
        timeout: std::time::Duration,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();

        Self {
            name: name.into(),
            base_url,
            api_key,
            client,
        }
    }
}

#[async_trait]
impl SearchProvider for ExternalSearchProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn search(&self, request: &SearchRequest) -> Result<Vec<SearchResult>, CrawlerError> {
        let mut endpoint = Url::parse(&self.base_url).map_err(|e| {
            CrawlerError::SearchProviderError(format!("Invalid search base URL: {e}"))
        })?;

        endpoint
            .query_pairs_mut()
            .append_pair("q", &request.query)
            .append_pair("format", "json")
            .append_pair("limit", &request.limit.to_string());

        if let Some(ref d) = request.domain {
            endpoint.query_pairs_mut().append_pair("site", d);
        }

        let mut headers = HeaderMap::new();
        if let Some(ref key) = self.api_key {
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {key}")) {
                headers.insert(AUTHORIZATION, val);
            }
        }

        let response = self
            .client
            .get(endpoint)
            .headers(headers)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    CrawlerError::SearchTimeout
                } else {
                    CrawlerError::SearchProviderError(format!("Request failed: {e}"))
                }
            })?;

        if !response.status().is_success() {
            return Err(CrawlerError::SearchProviderError(format!(
                "Search provider returned status: {}",
                response.status()
            )));
        }

        let bytes = response.bytes().await.map_err(|e| {
            CrawlerError::SearchProviderError(format!("Failed to read search response body: {e}"))
        })?;

        let json_val: Value = serde_json::from_slice(&bytes).map_err(|e| {
            CrawlerError::SearchProviderError(format!("Failed to parse search JSON response: {e}"))
        })?;

        let mut results = Vec::new();

        // Support standard SearXNG / JSON format: { "results": [ { "url": "...", "title": "...", "content": "..." } ] }
        if let Some(arr) = json_val.get("results").and_then(|r| r.as_array()) {
            for item in arr {
                if let Some(url_str) = item.get("url").and_then(|u| u.as_str()) {
                    if let Ok(parsed_url) = Url::parse(url_str) {
                        let title = item
                            .get("title")
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string());
                        let desc = item
                            .get("content")
                            .or_else(|| item.get("description"))
                            .and_then(|c| c.as_str())
                            .map(|s| s.to_string());
                        let score = item.get("score").and_then(|s| s.as_f64()).map(|s| s as f32);

                        results.push(SearchResult {
                            url: parsed_url,
                            title,
                            description: desc,
                            score,
                            published_at: None,
                            source: SearchResultSource::Web,
                            content: None,
                        });
                    }
                }
            }
        }

        Ok(results)
    }
}

pub struct SearchProviderRegistry {
    providers: DashMap<String, Arc<dyn SearchProvider>>,
    default_provider: String,
}

impl SearchProviderRegistry {
    pub fn new(config: &Config) -> Self {
        let registry = Self {
            providers: DashMap::new(),
            default_provider: config.search_provider.clone(),
        };

        // Always register mock provider
        registry.register(Arc::new(MockSearchProvider::new()));

        // Register external provider if configured
        if let Some(ref base_url) = config.search_base_url {
            let ext = ExternalSearchProvider::new(
                "external",
                base_url.clone(),
                config.search_api_key.clone(),
                config.search_timeout(),
            );
            registry.register(Arc::new(ext));
        }

        registry
    }

    pub fn register(&self, provider: Arc<dyn SearchProvider>) {
        self.providers.insert(provider.name().to_string(), provider);
    }

    pub fn get(&self, name: Option<&str>) -> Result<Arc<dyn SearchProvider>, CrawlerError> {
        let provider_name = name.unwrap_or(&self.default_provider);
        self.providers
            .get(provider_name)
            .map(|p| p.value().clone())
            .ok_or_else(|| CrawlerError::SearchProviderNotConfigured(provider_name.to_string()))
    }
}
