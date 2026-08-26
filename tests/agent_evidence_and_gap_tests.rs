use chrono::Utc;
use rustcrawl::agent::evaluator::EvidenceGapEvaluator;
use rustcrawl::agent::evidence::{Evidence, EvidenceStore};
use rustcrawl::agent::models::{
    AgentMode, AgentRequest, ResearchPlan, ResearchSubgoal, SourceType, SubgoalStatus,
};
use rustcrawl::config::Config;
use std::sync::Arc;

#[test]
fn test_evidence_store_deduplication_and_ranking() {
    let store = EvidenceStore::new();

    let e1 = Evidence {
        id: "E1".to_string(),
        source_url: "https://example.com/page1".to_string(),
        source_title: Some("Community Post".to_string()),
        source_type: SourceType::Community,
        content_hash: "hash_1".to_string(),
        chunk_id: None,
        excerpt: "Some community text content".to_string(),
        extracted_fact: None,
        relevance: 0.8,
        published_at: None,
        fetched_at: Utc::now(),
    };

    let e2 = Evidence {
        id: "E2".to_string(),
        source_url: "https://example.gov/official".to_string(),
        source_title: Some("Government Official Release".to_string()),
        source_type: SourceType::Government,
        content_hash: "hash_2".to_string(),
        chunk_id: None,
        excerpt: "Official policy guidelines".to_string(),
        extracted_fact: None,
        relevance: 0.8,
        published_at: None,
        fetched_at: Utc::now(),
    };

    let inserted1 = store.insert(e1);
    let inserted2 = store.insert(e2);
    assert!(inserted1.is_some());
    assert!(inserted2.is_some());
    assert_eq!(store.all().len(), 2);

    // Re-inserting existing content hash should return None
    let e_dup = Evidence {
        id: "E3".to_string(),
        source_url: "https://mirror.example.com/page1".to_string(),
        source_title: Some("Mirror".to_string()),
        source_type: SourceType::Community,
        content_hash: "hash_1".to_string(),
        chunk_id: None,
        excerpt: "Some community text content".to_string(),
        extracted_fact: None,
        relevance: 0.8,
        published_at: None,
        fetched_at: Utc::now(),
    };
    let inserted_dup = store.insert(e_dup);
    assert!(
        inserted_dup.is_none(),
        "Duplicate content hash must be rejected"
    );

    let ranked = store.top_relevant(10);
    assert_eq!(
        ranked[0].id, "E2",
        "Higher quality source should rank first"
    );
}

#[tokio::test]
async fn test_evidence_gap_evaluation_missing_schema_properties() {
    let config = Arc::new(Config::default());
    let evaluator = EvidenceGapEvaluator::new(config);
    let store = EvidenceStore::new();

    let req = AgentRequest {
        task: "Extract quarterly revenue and CEO name for Acme Corp".to_string(),
        mode: AgentMode::StructuredResearch,
        output_schema: Some(serde_json::json!({
            "type": "object",
            "properties": {
                "ceo_name": { "type": "string" },
                "quarterly_revenue": { "type": "number" },
                "headquarters": { "type": "string" }
            },
            "required": ["ceo_name", "quarterly_revenue"]
        })),
        response_format: "json".to_string(),
        freshness_days: None,
        limits: None,
        include_trace: true,
        initial_urls: Vec::new(),
        tenant_id: None,
    };

    let plan = ResearchPlan {
        objective: req.task.clone(),
        subgoals: vec![ResearchSubgoal {
            id: "subgoal_1".to_string(),
            description: "Find Acme Corp financials".to_string(),
            status: SubgoalStatus::Completed,
            priority: 1,
            target_urls: Vec::new(),
            search_queries: vec!["Acme Corp quarterly revenue".to_string()],
            extracted_facts: vec!["ceo_name: John Doe".to_string()],
        }],
        expected_output: req.output_schema.clone(),
    };

    // Store only contains CEO evidence
    store.insert(Evidence {
        id: "E1".to_string(),
        source_url: "https://example.com/news".to_string(),
        source_title: Some("Acme CEO Announcement".to_string()),
        source_type: SourceType::News,
        content_hash: "hash_ceo".to_string(),
        chunk_id: None,
        excerpt: "Acme Corp announced John Doe as new CEO today.".to_string(),
        extracted_fact: Some(serde_json::json!({ "ceo_name": "John Doe" })),
        relevance: 0.9,
        published_at: None,
        fetched_at: Utc::now(),
    });

    let eval_res = evaluator.evaluate(&req, &plan, &store, 0);
    assert!(
        !eval_res.is_sufficient,
        "Evaluation should report gap for missing quarterly revenue"
    );
    assert!(!eval_res.missing_fields.is_empty());
    assert!(
        !eval_res.followup_calls.is_empty(),
        "Should generate follow-up calls"
    );
}
