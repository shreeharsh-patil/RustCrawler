use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::policy::AgentPolicy;
use crate::config::Config;
use crate::crawl::CrawlerService;
use crate::discovery::models::DiscoveredLink;
use crate::discovery::service::MapService;
use crate::error::CrawlerError;
use crate::extraction::models::ExtractionMode;
use crate::extraction::service::ExtractionService;
use crate::knowledge::models::{HybridMatch, SearchMode};
use crate::knowledge::KnowledgeEngine;
use crate::models::OutputFormat;
use crate::render::RenderMode;
use crate::search::service::SearchService;
use crate::service::ScraperService;

/// Typed input parameters for internal agent tool calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchToolInput {
    pub query: String,
    pub limit: Option<usize>,
    pub freshness_days: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapToolInput {
    pub url: String,
    pub search_query: Option<String>,
    pub max_urls: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeToolInput {
    pub url: String,
    pub formats: Option<Vec<OutputFormat>>,
    pub only_main_content: Option<bool>,
    pub use_browser: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlToolInput {
    pub seed_url: String,
    pub max_pages: Option<usize>,
    pub max_depth: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractToolInput {
    pub url: Option<String>,
    pub text: Option<String>,
    pub schema: Option<Value>,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveToolInput {
    pub index_id: String,
    pub query: String,
    pub top_k: Option<usize>,
    pub mode: Option<SearchMode>,
}

/// Strongly-typed tool invocation dispatched by the agent engine
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tool", content = "parameters", rename_all = "snake_case")]
pub enum AgentToolCall {
    Search(SearchToolInput),
    Map(MapToolInput),
    Scrape(ScrapeToolInput),
    Crawl(CrawlToolInput),
    Extract(ExtractToolInput),
    Retrieve(RetrieveToolInput),
}

/// Strongly-typed tool execution outputs returned to the agent state machine
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentToolResult {
    Search {
        results: Vec<DiscoveredLink>,
        total: usize,
    },
    Map {
        links: Vec<DiscoveredLink>,
        total: usize,
    },
    Scrape {
        url: String,
        title: Option<String>,
        markdown: Option<String>,
        text: Option<String>,
        raw_html: Option<String>,
    },
    Crawl {
        pages_crawled: usize,
        pages: Vec<ScrapedPageResult>,
    },
    Extract {
        data: Value,
        confidence: f32,
    },
    Retrieve {
        matches: Vec<HybridMatch>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapedPageResult {
    pub url: String,
    pub title: Option<String>,
    pub markdown: Option<String>,
}

/// Tool executor coordinating trusted calls to existing RustCrawler subsystems
pub struct AgentToolExecutor {
    config: Arc<Config>,
    search_service: Option<Arc<SearchService>>,
    map_service: Option<Arc<MapService>>,
    scraper_service: Arc<ScraperService>,
    crawler_service: Arc<CrawlerService>,
    extraction_service: Arc<ExtractionService>,
    knowledge_engine: Option<Arc<KnowledgeEngine>>,
}

impl AgentToolExecutor {
    pub fn new(
        config: Arc<Config>,
        search_service: Option<Arc<SearchService>>,
        map_service: Option<Arc<MapService>>,
        scraper_service: Arc<ScraperService>,
        crawler_service: Arc<CrawlerService>,
        extraction_service: Arc<ExtractionService>,
        knowledge_engine: Option<Arc<KnowledgeEngine>>,
    ) -> Self {
        Self {
            config,
            search_service,
            map_service,
            scraper_service,
            crawler_service,
            extraction_service,
            knowledge_engine,
        }
    }

    /// Validates and executes an agent tool call
    pub async fn execute(&self, call: AgentToolCall) -> Result<AgentToolResult, CrawlerError> {
        match call {
            AgentToolCall::Search(input) => self.execute_search(input).await,
            AgentToolCall::Map(input) => self.execute_map(input).await,
            AgentToolCall::Scrape(input) => self.execute_scrape(input).await,
            AgentToolCall::Crawl(input) => self.execute_crawl(input).await,
            AgentToolCall::Extract(input) => self.execute_extract(input).await,
            AgentToolCall::Retrieve(input) => self.execute_retrieve(input).await,
        }
    }

    async fn execute_search(
        &self,
        input: SearchToolInput,
    ) -> Result<AgentToolResult, CrawlerError> {
        let search_svc = self.search_service.as_ref().ok_or_else(|| {
            CrawlerError::SearchProviderNotConfigured("Search service not initialized".to_string())
        })?;

        let limit = input
            .limit
            .unwrap_or(10)
            .min(self.config.max_search_results);
        let search_req = crate::search::models::SearchRequest {
            query: input.query,
            limit,
            scrape: false,
            formats: Vec::new(),
            mode: None,
            domain: None,
            exclude_domains: Vec::new(),
            provider: None,
        };

        let resp = search_svc.search(search_req).await?;
        let results: Vec<DiscoveredLink> = resp
            .data
            .into_iter()
            .map(|r| DiscoveredLink {
                url: r.url.to_string(),
                title: r.title,
                description: r.description,
                source: crate::crawl::DiscoverySource::HtmlLink,
                score: r.score,
            })
            .collect();

        Ok(AgentToolResult::Search {
            total: results.len(),
            results,
        })
    }

    async fn execute_map(&self, input: MapToolInput) -> Result<AgentToolResult, CrawlerError> {
        let _valid_url = AgentPolicy::validate_url(&input.url, self.config.allow_private_networks)?;

        let map_svc = self.map_service.as_ref().ok_or_else(|| {
            CrawlerError::MapDiscoveryFailed("Map service not initialized".to_string())
        })?;

        let max_urls = input.max_urls.unwrap_or(50).min(self.config.max_map_urls);
        let map_req = crate::discovery::models::MapRequest {
            url: input.url,
            limit: max_urls,
            search: input.search_query,
            include_subdomains: false,
            include_sitemap: true,
            include_paths: Vec::new(),
            exclude_paths: Vec::new(),
            ignore_query_parameters: true,
            render_dynamic_links: false,
            max_depth: 3,
        };

        let resp = map_svc.map(map_req).await?;

        Ok(AgentToolResult::Map {
            total: resp.links.len(),
            links: resp.links,
        })
    }

    async fn execute_scrape(
        &self,
        input: ScrapeToolInput,
    ) -> Result<AgentToolResult, CrawlerError> {
        let _valid_url = AgentPolicy::validate_url(&input.url, self.config.allow_private_networks)?;

        let render_mode = if input.use_browser.unwrap_or(false) {
            RenderMode::Browser
        } else {
            RenderMode::Auto
        };

        let req = crate::models::ScrapeRequest {
            url: input.url.clone(),
            formats: Some(input.formats.unwrap_or_else(|| {
                vec![
                    OutputFormat::Markdown,
                    OutputFormat::Text,
                    OutputFormat::Html,
                ]
            })),
            only_main_content: Some(input.only_main_content.unwrap_or(true)),
            render_mode: Some(render_mode),
            ..Default::default()
        };

        let result = self.scraper_service.scrape(req).await?;

        Ok(AgentToolResult::Scrape {
            url: input.url,
            title: result.metadata.as_ref().and_then(|m| m.title.clone()),
            markdown: result.markdown,
            text: result.text,
            raw_html: result.html,
        })
    }

    async fn execute_crawl(&self, input: CrawlToolInput) -> Result<AgentToolResult, CrawlerError> {
        let _valid_url =
            AgentPolicy::validate_url(&input.seed_url, self.config.allow_private_networks)?;

        let max_pages = input
            .max_pages
            .unwrap_or(10)
            .min(self.config.agent_max_pages as usize);
        let max_depth = input.max_depth.unwrap_or(2).min(5);

        let crawl_opts = crate::crawl::models::CrawlOptions {
            limit: max_pages,
            max_depth: max_depth as u32,
            ..Default::default()
        };

        let cancel_token = tokio_util::sync::CancellationToken::new();
        let crawl_res = self
            .crawler_service
            .crawl(&input.seed_url, crawl_opts, cancel_token)
            .await?;

        let mut pages = Vec::new();
        for page in crawl_res.pages {
            pages.push(ScrapedPageResult {
                url: page.url,
                title: page.metadata.and_then(|m| m.title),
                markdown: page.markdown,
            });
        }

        Ok(AgentToolResult::Crawl {
            pages_crawled: pages.len(),
            pages,
        })
    }

    async fn execute_extract(
        &self,
        input: ExtractToolInput,
    ) -> Result<AgentToolResult, CrawlerError> {
        let schema = input.schema.unwrap_or_else(|| {
            serde_json::json!({
                "type": "object"
            })
        });

        let mode = if self.config.llm_enabled {
            ExtractionMode::Llm
        } else {
            ExtractionMode::Deterministic
        };

        let req = crate::extraction::models::ExtractionRequest {
            url: input.url,
            urls: Vec::new(),
            crawl: None,
            prompt: input.prompt,
            schema: Some(schema),
            mode: Some(mode),
            missing_field_behavior: None,
            dedupe_by: Vec::new(),
            include_provenance: None,
            render_mode: None,
            provider: None,
            model: None,
        };

        let ext_res = self.extraction_service.extract(req, None).await?;
        let confidence = if ext_res.metadata.validated { 1.0 } else { 0.8 };

        Ok(AgentToolResult::Extract {
            data: ext_res.data.unwrap_or_default(),
            confidence,
        })
    }

    async fn execute_retrieve(
        &self,
        input: RetrieveToolInput,
    ) -> Result<AgentToolResult, CrawlerError> {
        let engine = self
            .knowledge_engine
            .as_ref()
            .ok_or(CrawlerError::LocalIndexUnavailable)?;

        let top_k = input.top_k.unwrap_or(5).min(20);
        let mode = input.mode.unwrap_or(SearchMode::Hybrid);

        let matches = engine
            .search(&input.index_id, &input.query, top_k, mode, None, None)
            .await?;

        Ok(AgentToolResult::Retrieve { matches })
    }
}
