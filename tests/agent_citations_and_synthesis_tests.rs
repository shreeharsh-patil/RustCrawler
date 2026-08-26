use chrono::Utc;
use rustcrawl::agent::citations::CitationValidator;
use rustcrawl::agent::evidence::{Evidence, EvidenceStore};
use rustcrawl::agent::models::{
    AgentMode, AgentRequest, Claim, ClaimConfidence, ResearchPlan, SourceType,
};
use rustcrawl::agent::synthesis::ResearchSynthesizer;
use rustcrawl::config::Config;
use std::sync::Arc;

#[test]
fn test_citation_extraction_and_validation() {
    let store = EvidenceStore::new();

    store.insert(Evidence {
        id: "E1".to_string(),
        source_url: "https://example.com/rust-concurrency".to_string(),
        source_title: Some("Rust Concurrency Guide".to_string()),
        source_type: SourceType::Documentation,
        content_hash: "hash_rust".to_string(),
        chunk_id: None,
        excerpt: "Rust provides fearless concurrency with Send and Sync traits.".to_string(),
        extracted_fact: None,
        relevance: 0.9,
        published_at: None,
        fetched_at: Utc::now(),
    });

    let synthesis_text = "Rust ensures thread safety through type system guarantees [E1]. Some other claim [E999] was also made.";
    let extracted_ids = CitationValidator::extract_citation_ids(synthesis_text);
    assert_eq!(extracted_ids, vec!["E1", "E999"]);

    let mut claims = vec![Claim {
        id: "claim_1".to_string(),
        text: "Rust ensures thread safety [E1] and [E999]".to_string(),
        evidence_ids: vec!["E1".to_string(), "E999".to_string()],
        confidence: ClaimConfidence::High,
    }];

    let (validated_citations, _warnings) =
        CitationValidator::validate_and_build_citations(synthesis_text, &mut claims, &store);
    assert_eq!(
        validated_citations.len(),
        1,
        "Only existing evidence E1 should be validated"
    );
    assert_eq!(validated_citations[0].source_id, "E1");
    assert_eq!(
        validated_citations[0].url,
        "https://example.com/rust-concurrency"
    );
}

#[tokio::test]
async fn test_deterministic_fallback_synthesis() {
    let config = Arc::new(Config::default());
    let synthesizer = ResearchSynthesizer::new(config, None);
    let store = EvidenceStore::new();

    store.insert(Evidence {
        id: "E1".to_string(),
        source_url: "https://docs.rs/tokio".to_string(),
        source_title: Some("Tokio Async Runtime".to_string()),
        source_type: SourceType::Documentation,
        content_hash: "hash_tokio".to_string(),
        chunk_id: None,
        excerpt: "Tokio is an asynchronous runtime for Rust.".to_string(),
        extracted_fact: None,
        relevance: 0.9,
        published_at: None,
        fetched_at: Utc::now(),
    });

    let req = AgentRequest {
        task: "Explain Tokio runtime architecture".to_string(),
        mode: AgentMode::Research,
        output_schema: None,
        response_format: "markdown".to_string(),
        freshness_days: None,
        limits: None,
        include_trace: true,
        initial_urls: Vec::new(),
        tenant_id: None,
    };

    let plan = ResearchPlan {
        objective: req.task.clone(),
        subgoals: Vec::new(),
        expected_output: None,
    };

    let result = synthesizer
        .synthesize(&req, &plan, &store)
        .await
        .expect("Synthesis failed");
    let markdown = result.markdown_answer.as_ref().unwrap();
    assert!(!markdown.is_empty());
    assert!(markdown.contains("Tokio Async Runtime"));
    assert!(markdown.contains("[E1]"));
}
