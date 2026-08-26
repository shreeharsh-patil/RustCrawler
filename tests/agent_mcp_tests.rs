use rustcrawl::agent::mcp::McpToolServer;
use rustcrawl::agent::service::AgentService;
use rustcrawl::config::Config;
use rustcrawl::crawl::CrawlerService;
use rustcrawl::discovery::service::MapService;
use rustcrawl::extraction::service::ExtractionService;
use rustcrawl::knowledge::KnowledgeEngine;
use rustcrawl::search::service::SearchService;
use rustcrawl::service::ScraperService;
use std::sync::Arc;

#[test]
fn test_mcp_tool_definitions() {
    let tools = McpToolServer::list_tools();
    assert_eq!(tools.len(), 7, "Should list exactly 7 MCP tools");

    let tool_names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
    assert!(tool_names.contains(&"scrape".to_string()));
    assert!(tool_names.contains(&"search".to_string()));
    assert!(tool_names.contains(&"map".to_string()));
    assert!(tool_names.contains(&"crawl".to_string()));
    assert!(tool_names.contains(&"extract".to_string()));
    assert!(tool_names.contains(&"retrieve".to_string()));
    assert!(tool_names.contains(&"research".to_string()));

    for tool in &tools {
        assert!(!tool.description.is_empty());
        assert!(tool.input_schema.is_object());
    }
}

#[tokio::test]
async fn test_mcp_tool_execution_search() {
    let config = Arc::new(Config::default());
    let scraper = Arc::new(ScraperService::new((*config).clone()).unwrap());
    let crawler = Arc::new(CrawlerService::new(scraper.clone()));
    let extraction = Arc::new(ExtractionService::new(
        (*config).clone(),
        scraper.clone(),
        crawler.clone(),
    ));
    let map_service = Arc::new(MapService::new(config.clone(), scraper.clone(), None));
    let search_service = Arc::new(SearchService::new(
        config.clone(),
        scraper.clone(),
        map_service.clone(),
        None,
        None,
    ));
    let knowledge_engine = Arc::new(KnowledgeEngine::new(config.clone(), None));

    let agent_service = Arc::new(AgentService::new(
        config,
        Some(search_service),
        Some(map_service),
        scraper,
        crawler,
        extraction,
        Some(knowledge_engine),
        None,
        None,
    ));

    let server = McpToolServer::new(agent_service);

    let result = server
        .call_tool(
            "search",
            serde_json::json!({
                "query": "Rust programming language",
                "limit": 3
            }),
        )
        .await;

    assert!(result.is_ok(), "MCP search tool call should succeed");
}
