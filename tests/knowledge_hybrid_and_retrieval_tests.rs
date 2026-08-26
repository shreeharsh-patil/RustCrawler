use rustcrawl::config::Config;
use rustcrawl::knowledge::hybrid::reciprocal_rank_fusion;
use rustcrawl::knowledge::lexical::{tokenize_text, LexicalIndex};
use rustcrawl::knowledge::models::{ChunkMetadata, IndexedChunk, SearchMode};
use rustcrawl::knowledge::retrieval::RetrievalService;
use rustcrawl::knowledge::vector::{MemoryVectorStore, VectorMatch};
use std::sync::Arc;

#[test]
fn test_lexical_technical_tokenization() {
    let text = "Set MAX_CONCURRENT_FETCHES to 50 in `crawler-config.yaml` using async_worker_fn.";
    let tokens = tokenize_text(text);

    assert!(tokens.contains(&"max_concurrent_fetches".to_string()));
    assert!(tokens.contains(&"crawler-config.yaml".to_string()));
    assert!(tokens.contains(&"async_worker_fn".to_string()));
}

#[test]
fn test_lexical_bm25_index_exact_technical_match() {
    let index = LexicalIndex::new();
    let index_id = "idx_lexical";

    let chunk1 = IndexedChunk {
        chunk_id: "chk_1".to_string(),
        document_id: "doc_1".to_string(),
        source_url: None,
        heading_path: vec!["Configuration".to_string()],
        text: "The environment variable MAX_CONCURRENT_FETCHES controls crawler parallelism."
            .to_string(),
        token_count: 10,
        content_hash: "h1".to_string(),
        position: 0,
        metadata: ChunkMetadata::default(),
    };

    let chunk2 = IndexedChunk {
        chunk_id: "chk_2".to_string(),
        document_id: "doc_2".to_string(),
        source_url: None,
        heading_path: vec!["General".to_string()],
        text: "You can adjust fetch concurrency settings in the configuration file.".to_string(),
        token_count: 10,
        content_hash: "h2".to_string(),
        position: 0,
        metadata: ChunkMetadata::default(),
    };

    index.insert_chunk(index_id, &chunk1, Some("Config Docs"), None, None);
    index.insert_chunk(index_id, &chunk2, Some("General Docs"), None, None);

    let matches = index.search(index_id, "MAX_CONCURRENT_FETCHES", 10, None, None);
    assert!(!matches.is_empty());
    assert_eq!(matches[0].chunk_id, "chk_1");
}

#[test]
fn test_reciprocal_rank_fusion_and_diversification() {
    let vector_matches = vec![
        VectorMatch {
            chunk_id: "chk_docA_1".to_string(),
            document_id: "docA".to_string(),
            source_url: Some("https://example.com/a".to_string()),
            title: Some("Doc A".to_string()),
            heading_path: vec!["Section 1".to_string()],
            text: "Doc A chunk 1".to_string(),
            score: 0.95,
            metadata: ChunkMetadata::default(),
            document_metadata: None,
        },
        VectorMatch {
            chunk_id: "chk_docA_2".to_string(),
            document_id: "docA".to_string(),
            source_url: Some("https://example.com/a".to_string()),
            title: Some("Doc A".to_string()),
            heading_path: vec!["Section 2".to_string()],
            text: "Doc A chunk 2".to_string(),
            score: 0.90,
            metadata: ChunkMetadata::default(),
            document_metadata: None,
        },
        VectorMatch {
            chunk_id: "chk_docA_3".to_string(),
            document_id: "docA".to_string(),
            source_url: Some("https://example.com/a".to_string()),
            title: Some("Doc A".to_string()),
            heading_path: vec!["Section 3".to_string()],
            text: "Doc A chunk 3".to_string(),
            score: 0.85,
            metadata: ChunkMetadata::default(),
            document_metadata: None,
        },
        VectorMatch {
            chunk_id: "chk_docB_1".to_string(),
            document_id: "docB".to_string(),
            source_url: Some("https://example.com/b".to_string()),
            title: Some("Doc B".to_string()),
            heading_path: vec!["Section 1".to_string()],
            text: "Doc B chunk 1".to_string(),
            score: 0.80,
            metadata: ChunkMetadata::default(),
            document_metadata: None,
        },
    ];

    let lexical_matches = vec![
        rustcrawl::knowledge::lexical::LexicalMatch {
            chunk_id: "chk_docB_1".to_string(),
            document_id: "docB".to_string(),
            source_url: Some("https://example.com/b".to_string()),
            title: Some("Doc B".to_string()),
            heading_path: vec!["Section 1".to_string()],
            text: "Doc B chunk 1".to_string(),
            score: 4.5,
            metadata: ChunkMetadata::default(),
            document_metadata: None,
        },
        rustcrawl::knowledge::lexical::LexicalMatch {
            chunk_id: "chk_docA_1".to_string(),
            document_id: "docA".to_string(),
            source_url: Some("https://example.com/a".to_string()),
            title: Some("Doc A".to_string()),
            heading_path: vec!["Section 1".to_string()],
            text: "Doc A chunk 1".to_string(),
            score: 3.0,
            metadata: ChunkMetadata::default(),
            document_metadata: None,
        },
    ];

    // Max chunks per document = 2
    let fused = reciprocal_rank_fusion(&vector_matches, &lexical_matches, 60.0, 2, 10);
    assert!(!fused.is_empty());

    let doc_a_count = fused.iter().filter(|m| m.document_id == "docA").count();
    assert!(doc_a_count <= 2, "Must enforce max 2 chunks per document");

    // Both docA and docB appear
    assert!(fused.iter().any(|m| m.document_id == "docA"));
    assert!(fused.iter().any(|m| m.document_id == "docB"));
}

#[tokio::test]
async fn test_retrieval_service_context_budget_packing() {
    let config = Config {
        rag_max_context_tokens: 50,
        rag_max_chunks: 10,
        ..Default::default()
    };
    let arc_config = Arc::new(config);
    let vector_store = Arc::new(MemoryVectorStore::new());
    let lexical_index = Arc::new(LexicalIndex::new());
    let retrieval = RetrievalService::new(
        arc_config,
        vector_store,
        lexical_index.clone(),
        None, // Lexical only
    );

    let index_id = "idx_budget";
    for i in 1..=5 {
        let chunk = IndexedChunk {
            chunk_id: format!("chk_{i}"),
            document_id: format!("doc_{i}"),
            source_url: None,
            heading_path: vec![],
            text: format!("Section {i} detailing comprehensive web scraping and distributed indexing protocols with multiple paragraphs."),
            token_count: 20,
            content_hash: format!("hash_{i}"),
            position: i as u32,
            metadata: ChunkMetadata::default(),
        };
        lexical_index.insert_chunk(index_id, &chunk, None, None, None);
    }

    let result = retrieval
        .retrieve(
            index_id,
            "scraping indexing",
            10,
            SearchMode::Lexical,
            None,
            None,
            false,
        )
        .await
        .unwrap();

    // With 50 token budget and ~20 tokens per chunk, packed chunks should be <= 3 chunks
    assert!(result.chunks.len() <= 3);
    assert!(result.total_tokens <= 50 || result.chunks.len() == 1);
}
