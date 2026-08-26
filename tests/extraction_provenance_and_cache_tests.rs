use rustcrawl::extraction::cache::ExtractionCache;
use rustcrawl::extraction::models::{
    ChunkType, ContentChunk, ExtractionMetadata, ExtractionMode, ExtractionResult,
};
use rustcrawl::extraction::provenance::ProvenanceTracker;
use serde_json::json;
use url::Url;

#[test]
fn test_provenance_tracker_generates_excerpts() {
    let target_url = Url::parse("https://news.example.com/ai").unwrap();
    let chunk = ContentChunk {
        id: "sec_1".to_string(),
        source_url: target_url.clone(),
        heading_path: vec!["AI Revolution".to_string()],
        content:
            "Anthropic and Google announce groundbreaking frontier AI model architecture in Paris."
                .to_string(),
        content_type: ChunkType::Section,
        score: 1.0,
    };

    let extracted = json!({
        "location": "Paris",
        "topic": "AI"
    });

    let provenance = ProvenanceTracker::build_provenance(&extracted, &[chunk], None);

    assert!(provenance.contains_key("location"));
    let loc_prov = provenance.get("location").unwrap();
    assert_eq!(loc_prov.source_url, target_url.to_string());
    assert!(loc_prov.excerpt.is_some());
    assert!(loc_prov.excerpt.as_ref().unwrap().contains("Paris"));
}

#[test]
fn test_extraction_cache_operations() {
    let cache = ExtractionCache::new(true, 50);

    let key = ExtractionCache::compute_key(
        "content_hash_abc",
        Some(&json!({"type": "object"})),
        Some("prompt_1"),
        Some("gpt-4o-mini"),
    );

    let result = ExtractionResult {
        success: true,
        data: Some(json!({"val": 123})),
        metadata: ExtractionMetadata {
            mode: ExtractionMode::Auto,
            extractor: "deterministic".to_string(),
            validated: true,
            sources_used: 1,
            chunks_used: 0,
            llm_calls: 0,
            cost_estimate_usd: None,
            duration_ms: 5,
            cache_hit: false,
        },
        provenance: None,
        warnings: Vec::new(),
        error: None,
    };

    cache.insert(key.clone(), result);

    let cached = cache.get(&key);
    assert!(cached.is_some());
    let hit = cached.unwrap();
    assert!(hit.metadata.cache_hit);
    assert_eq!(hit.data.unwrap()["val"], 123);
}
