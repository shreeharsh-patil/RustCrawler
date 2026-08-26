use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::agent::models::AgentRequest;
use crate::agent::service::AgentService;
use crate::agent::tools::{
    AgentToolCall, CrawlToolInput, ExtractToolInput, MapToolInput, RetrieveToolInput,
    ScrapeToolInput, SearchToolInput,
};
use crate::error::CrawlerError;

/// MCP Tool definition schema compatible with Model Context Protocol standards
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Model Context Protocol (MCP) server integration layer exposing RustCrawler tools to AI agents
pub struct McpToolServer {
    agent_service: Arc<AgentService>,
}

impl McpToolServer {
    pub fn new(agent_service: Arc<AgentService>) -> Self {
        Self { agent_service }
    }

    /// Lists all tools exposed by the MCP layer with strict JSON schemas
    pub fn list_tools() -> Vec<McpToolDefinition> {
        vec![
            McpToolDefinition {
                name: "scrape".to_string(),
                description: "Scrapes a URL and extracts clean Markdown, plain text, and metadata using HTTP-first parsing with automatic Chromium fallback.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "url": { "type": "string", "description": "The target webpage HTTP/HTTPS URL" },
                        "only_main_content": { "type": "boolean", "description": "Whether to strip navigational chrome and headers (default: true)" },
                        "use_browser": { "type": "boolean", "description": "Force headless browser rendering (default: false)" }
                    },
                    "required": ["url"]
                }),
            },
            McpToolDefinition {
                name: "search".to_string(),
                description: "Searches the web for relevant URLs matching a query.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Web search query terms" },
                        "limit": { "type": "integer", "description": "Maximum number of search results (default: 10)" },
                        "freshness_days": { "type": "integer", "description": "Maximum age of indexed content in days" }
                    },
                    "required": ["query"]
                }),
            },
            McpToolDefinition {
                name: "map".to_string(),
                description: "Maps and discovers all internal links and URLs under a domain hierarchy.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "url": { "type": "string", "description": "Domain or seed URL to map" },
                        "search_query": { "type": "string", "description": "Optional pattern or term to filter mapped links" },
                        "max_urls": { "type": "integer", "description": "Maximum number of links to discover" }
                    },
                    "required": ["url"]
                }),
            },
            McpToolDefinition {
                name: "crawl".to_string(),
                description: "Crawls multiple pages starting from a seed URL with bounded concurrency and depth.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "seed_url": { "type": "string", "description": "Starting seed URL for the crawl" },
                        "max_pages": { "type": "integer", "description": "Maximum pages to crawl (default: 10)" },
                        "max_depth": { "type": "integer", "description": "Maximum link depth (default: 2)" }
                    },
                    "required": ["seed_url"]
                }),
            },
            McpToolDefinition {
                name: "extract".to_string(),
                description: "Extracts structured JSON schema data from a URL or text using deterministic preprocessing and optional LLM.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "url": { "type": "string", "description": "Source URL to extract from" },
                        "text": { "type": "string", "description": "Direct text/markdown content to extract from" },
                        "schema": { "type": "object", "description": "JSON schema definition for extraction" },
                        "prompt": { "type": "string", "description": "Optional natural language extraction instructions" }
                    }
                }),
            },
            McpToolDefinition {
                name: "retrieve".to_string(),
                description: "Performs hybrid vector and BM25 lexical search over indexed knowledge bases.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "index_id": { "type": "string", "description": "Target knowledge index ID" },
                        "query": { "type": "string", "description": "Search query text" },
                        "top_k": { "type": "integer", "description": "Number of top chunks to retrieve (default: 5)" }
                    },
                    "required": ["index_id", "query"]
                }),
            },
            McpToolDefinition {
                name: "research".to_string(),
                description: "Initiates an autonomous multi-step web research job, synthesizing verified findings with complete provenance and citations.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "task": { "type": "string", "description": "High-level research objective or comparison question" },
                        "mode": { "type": "string", "enum": ["research", "structured_research", "site_analysis", "comparison", "monitoring_analysis"], "description": "Agent operational mode" },
                        "output_schema": { "type": "object", "description": "Optional target JSON schema" },
                        "freshness_days": { "type": "integer", "description": "Maximum source age in days" }
                    },
                    "required": ["task"]
                }),
            },
        ]
    }

    /// Invokes a named MCP tool with JSON arguments
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<Value, CrawlerError> {
        match name {
            "scrape" => {
                let url = arguments["url"].as_str().ok_or_else(|| {
                    CrawlerError::InvalidAgentRequest("Missing 'url' argument".to_string())
                })?;
                let only_main_content =
                    arguments.get("only_main_content").and_then(|v| v.as_bool());
                let use_browser = arguments.get("use_browser").and_then(|v| v.as_bool());

                let call = AgentToolCall::Scrape(ScrapeToolInput {
                    url: url.to_string(),
                    formats: None,
                    only_main_content,
                    use_browser,
                });

                let res = self.agent_service.tool_executor().execute(call).await?;
                Ok(serde_json::to_value(res).unwrap_or_default())
            }
            "search" => {
                let query = arguments["query"].as_str().ok_or_else(|| {
                    CrawlerError::InvalidAgentRequest("Missing 'query' argument".to_string())
                })?;
                let limit = arguments
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|u| u as usize);
                let freshness_days = arguments
                    .get("freshness_days")
                    .and_then(|v| v.as_u64())
                    .map(|u| u as u32);

                let call = AgentToolCall::Search(SearchToolInput {
                    query: query.to_string(),
                    limit,
                    freshness_days,
                });

                let res = self.agent_service.tool_executor().execute(call).await?;
                Ok(serde_json::to_value(res).unwrap_or_default())
            }
            "map" => {
                let url = arguments["url"].as_str().ok_or_else(|| {
                    CrawlerError::InvalidAgentRequest("Missing 'url' argument".to_string())
                })?;
                let search_query = arguments
                    .get("search_query")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let max_urls = arguments
                    .get("max_urls")
                    .and_then(|v| v.as_u64())
                    .map(|u| u as usize);

                let call = AgentToolCall::Map(MapToolInput {
                    url: url.to_string(),
                    search_query,
                    max_urls,
                });

                let res = self.agent_service.tool_executor().execute(call).await?;
                Ok(serde_json::to_value(res).unwrap_or_default())
            }
            "crawl" => {
                let seed_url = arguments["seed_url"].as_str().ok_or_else(|| {
                    CrawlerError::InvalidAgentRequest("Missing 'seed_url' argument".to_string())
                })?;
                let max_pages = arguments
                    .get("max_pages")
                    .and_then(|v| v.as_u64())
                    .map(|u| u as usize);
                let max_depth = arguments
                    .get("max_depth")
                    .and_then(|v| v.as_u64())
                    .map(|u| u as usize);

                let call = AgentToolCall::Crawl(CrawlToolInput {
                    seed_url: seed_url.to_string(),
                    max_pages,
                    max_depth,
                });

                let res = self.agent_service.tool_executor().execute(call).await?;
                Ok(serde_json::to_value(res).unwrap_or_default())
            }
            "extract" => {
                let url = arguments
                    .get("url")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let text = arguments
                    .get("text")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let schema = arguments.get("schema").cloned();
                let prompt = arguments
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let call = AgentToolCall::Extract(ExtractToolInput {
                    url,
                    text,
                    schema,
                    prompt,
                });

                let res = self.agent_service.tool_executor().execute(call).await?;
                Ok(serde_json::to_value(res).unwrap_or_default())
            }
            "retrieve" => {
                let index_id = arguments["index_id"].as_str().ok_or_else(|| {
                    CrawlerError::InvalidAgentRequest("Missing 'index_id' argument".to_string())
                })?;
                let query = arguments["query"].as_str().ok_or_else(|| {
                    CrawlerError::InvalidAgentRequest("Missing 'query' argument".to_string())
                })?;
                let top_k = arguments
                    .get("top_k")
                    .and_then(|v| v.as_u64())
                    .map(|u| u as usize);

                let call = AgentToolCall::Retrieve(RetrieveToolInput {
                    index_id: index_id.to_string(),
                    query: query.to_string(),
                    top_k,
                    mode: None,
                });

                let res = self.agent_service.tool_executor().execute(call).await?;
                Ok(serde_json::to_value(res).unwrap_or_default())
            }
            "research" => {
                let req: AgentRequest = serde_json::from_value(arguments).map_err(|e| {
                    CrawlerError::InvalidAgentRequest(format!(
                        "Failed to parse research parameters: {e}"
                    ))
                })?;

                let result = self.agent_service.submit_and_wait(req).await?;
                Ok(serde_json::to_value(result).unwrap_or_default())
            }
            other => Err(CrawlerError::AgentToolInvalid(format!(
                "Unknown tool: {other}"
            ))),
        }
    }
}
