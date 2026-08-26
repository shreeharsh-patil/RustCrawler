use chrono::Utc;
use rustcrawl::agent::evidence::{Evidence, EvidenceStore};
use rustcrawl::agent::models::SourceType;

#[test]
fn test_evidence_store_conflict_recording() {
    let store = EvidenceStore::new();

    let e1 = Evidence {
        id: "E1".to_string(),
        source_url: "https://source-a.com/release-date".to_string(),
        source_title: Some("Source A".to_string()),
        source_type: SourceType::News,
        content_hash: "hash_a".to_string(),
        chunk_id: None,
        excerpt: "Product X was released in March 2024".to_string(),
        extracted_fact: Some(serde_json::json!({ "release_date": "March 2024" })),
        relevance: 0.8,
        published_at: None,
        fetched_at: Utc::now(),
    };

    let e2 = Evidence {
        id: "E2".to_string(),
        source_url: "https://source-b.com/release-date".to_string(),
        source_title: Some("Source B".to_string()),
        source_type: SourceType::News,
        content_hash: "hash_b".to_string(),
        chunk_id: None,
        excerpt: "Product X was officially released in June 2024".to_string(),
        extracted_fact: Some(serde_json::json!({ "release_date": "June 2024" })),
        relevance: 0.8,
        published_at: None,
        fetched_at: Utc::now(),
    };

    store.insert(e1);
    store.insert(e2);

    store.record_conflict(
        "release_date",
        vec!["E1".to_string(), "E2".to_string()],
        "March 2024 vs June 2024 discrepancy",
        None,
    );

    let conflicts = store.conflicts();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].field, "release_date");
    assert_eq!(conflicts[0].evidence_ids, vec!["E1", "E2"]);
}
