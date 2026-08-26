use rustcrawl::agent::models::{AgentLimits, AgentMode, AgentRequest, AgentStatus};
use rustcrawl::agent::service::AgentService;
use rustcrawl::config::Config;
use rustcrawl::crawl::CrawlerService;
use rustcrawl::discovery::service::MapService;
use rustcrawl::extraction::service::ExtractionService;
use rustcrawl::knowledge::KnowledgeEngine;
use rustcrawl::search::service::SearchService;
use rustcrawl::service::ScraperService;
use std::sync::Arc;

#[tokio::test]
async fn test_agent_end_to_end_research_job() {
    let config_val = Config {
        agent_enabled: true,
        ..Default::default()
    };
    let config = Arc::new(config_val);

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

    let agent_service = AgentService::new(
        config.clone(),
        Some(search_service),
        Some(map_service),
        scraper,
        crawler,
        extraction,
        Some(knowledge_engine),
        None,
        None,
    );

    let req = AgentRequest {
        task: "Research latest updates on Rust web frameworks".to_string(),
        mode: AgentMode::Research,
        output_schema: None,
        response_format: "markdown".to_string(),
        freshness_days: Some(30),
        limits: Some(AgentLimits {
            max_steps: Some(5),
            max_pages: Some(3),
            max_search_queries: Some(2),
            ..Default::default()
        }),
        include_trace: true,
        initial_urls: Vec::new(),
        tenant_id: None,
    };

    let result = agent_service
        .submit_and_wait(req)
        .await
        .expect("Agent execution failed");
    assert_eq!(result.status, AgentStatus::Completed);
    assert!(!result.job_id.is_empty());
    assert!(result.research.steps > 0);
    assert!(result.confidence.as_ref().unwrap().score > 0.0);
    assert!(
        !result.trace.unwrap_or_default().is_empty(),
        "Trace events must be recorded"
    );
}

#[tokio::test]
async fn test_agent_job_lifecycle_pause_resume_cancel() {
    let config_val = Config {
        agent_enabled: true,
        ..Default::default()
    };
    let config = Arc::new(config_val);

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

    let agent_service = AgentService::new(
        config.clone(),
        Some(search_service),
        Some(map_service),
        scraper,
        crawler,
        extraction,
        Some(knowledge_engine),
        None,
        None,
    );

    let req = AgentRequest {
        task: "Long running research task".to_string(),
        mode: AgentMode::Research,
        output_schema: None,
        response_format: "markdown".to_string(),
        freshness_days: None,
        limits: Some(AgentLimits {
            max_steps: Some(20),
            max_pages: Some(10),
            ..Default::default()
        }),
        include_trace: true,
        initial_urls: Vec::new(),
        tenant_id: None,
    };

    let job_id = agent_service
        .submit_job(req)
        .await
        .expect("Job submission failed");
    assert!(!job_id.is_empty());

    // Test pause, resume, cancel methods
    let _ = agent_service.pause_job(&job_id);
    let _ = agent_service.resume_job(&job_id);
    let _ = agent_service.cancel_job(&job_id);
}
