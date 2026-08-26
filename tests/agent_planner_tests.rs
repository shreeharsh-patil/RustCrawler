use rustcrawl::agent::models::{AgentMode, AgentRequest};
use rustcrawl::agent::planner::AgentPlanner;
use rustcrawl::config::Config;
use std::sync::Arc;

#[tokio::test]
async fn test_deterministic_planner_direct_urls() {
    let config = Arc::new(Config::default());
    let planner = AgentPlanner::new(config, None);

    let req = AgentRequest {
        task: "Analyze the official documentation at https://example.com/docs and https://example.com/api".to_string(),
        mode: AgentMode::Research,
        output_schema: None,
        response_format: "markdown".to_string(),
        freshness_days: None,
        limits: None,
        include_trace: true,
        initial_urls: vec!["https://example.com/docs".to_string()],
        tenant_id: None,
    };

    let plan = planner.create_plan(&req).await.expect("Planning failed");
    assert!(!plan.subgoals.is_empty(), "Plan should generate subgoals");
    assert_eq!(
        plan.subgoals[0].target_urls,
        vec!["https://example.com/docs"]
    );
    assert_eq!(plan.subgoals[0].priority, 1);
}

#[tokio::test]
async fn test_deterministic_planner_comparison_mode() {
    let config = Arc::new(Config::default());
    let planner = AgentPlanner::new(config, None);

    let req = AgentRequest {
        task: "Compare Rust vs Go performance and concurrency model".to_string(),
        mode: AgentMode::Comparison,
        output_schema: None,
        response_format: "markdown".to_string(),
        freshness_days: None,
        limits: None,
        include_trace: true,
        initial_urls: Vec::new(),
        tenant_id: None,
    };

    let plan = planner.create_plan(&req).await.expect("Planning failed");
    assert!(
        plan.subgoals.len() >= 2,
        "Comparison plan should generate at least 2 entity subgoals"
    );

    let descriptions: Vec<String> = plan
        .subgoals
        .iter()
        .map(|s| s.description.to_lowercase())
        .collect();
    assert!(descriptions.iter().any(|d| d.contains("rust")));
    assert!(descriptions.iter().any(|d| d.contains("go")));
}

#[tokio::test]
async fn test_deterministic_planner_site_analysis() {
    let config = Arc::new(Config::default());
    let planner = AgentPlanner::new(config, None);

    let req = AgentRequest {
        task: "Analyze site structure and features of https://news.ycombinator.com".to_string(),
        mode: AgentMode::SiteAnalysis,
        output_schema: None,
        response_format: "markdown".to_string(),
        freshness_days: None,
        limits: None,
        include_trace: true,
        initial_urls: vec!["https://news.ycombinator.com".to_string()],
        tenant_id: None,
    };

    let plan = planner.create_plan(&req).await.expect("Planning failed");
    assert!(!plan.subgoals.is_empty());
    assert_eq!(
        plan.subgoals[0].target_urls,
        vec!["https://news.ycombinator.com"]
    );
}
