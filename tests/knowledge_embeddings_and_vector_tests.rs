use rustcrawl::knowledge::embeddings::{
    BatcherConfig, EmbeddingBatcher, EmbeddingCache, EmbeddingProvider, MockEmbeddingProvider,
};
use rustcrawl::knowledge::models::{ChunkMetadata, IndexFilter, IndexedChunk};
use rustcrawl::knowledge::vector::{MemoryVectorStore, VectorQuery, VectorRecord, VectorStore};
use std::sync::Arc;

#[tokio::test]
async fn test_mock_embedding_provider_deterministic_vectors() {
    let provider = MockEmbeddingProvider::new(128);
    assert_eq!(provider.dimensions(), 128);
    assert_eq!(provider.provider_name(), "mock");

    let texts = vec![
        "Rust crawler architecture".to_string(),
        "Vector embeddings and similarity search".to_string(),
        "Rust crawler architecture".to_string(), // duplicate text
    ];

    let batch = provider
        .embed(&texts)
        .await
        .expect("Embedding should succeed");
    let vectors = batch.embeddings;
    assert_eq!(vectors.len(), 3);
    assert_eq!(vectors[0].len(), 128);
    assert_eq!(vectors[1].len(), 128);
    assert_eq!(vectors[2].len(), 128);

    // Identical text produces identical deterministic vector
    assert_eq!(vectors[0], vectors[2]);
    // Different text produces different vector
    assert_ne!(vectors[0], vectors[1]);
}

#[tokio::test]
async fn test_mock_embedding_provider_simulated_errors() {
    let rate_limited = MockEmbeddingProvider::new(64).with_simulated_rate_limit(true);

    let texts = vec!["Hello".to_string()];
    let err = rate_limited.embed(&texts).await.unwrap_err();
    assert!(err.to_string().contains("429"));

    let server_error = MockEmbeddingProvider::new(64).with_simulated_server_error(true);
    let err = server_error.embed(&texts).await.unwrap_err();
    assert!(err.to_string().contains("500"));
}

#[tokio::test]
async fn test_embedding_cache_hit_and_batcher() {
    let provider = Arc::new(MockEmbeddingProvider::new(64));
    let cache = Arc::new(EmbeddingCache::default());
    let batcher = EmbeddingBatcher::new(
        provider,
        cache.clone(),
        BatcherConfig {
            max_batch_size: 10,
            max_batch_tokens: 2000,
            max_concurrent_requests: 2,
        },
    );

    let chunk1 = IndexedChunk {
        chunk_id: "chk_1".to_string(),
        document_id: "doc_1".to_string(),
        source_url: None,
        heading_path: vec![],
        text: "Alpha content".to_string(),
        token_count: 5,
        content_hash: "hash_alpha".to_string(),
        position: 0,
        metadata: ChunkMetadata::default(),
    };

    let chunk2 = IndexedChunk {
        chunk_id: "chk_2".to_string(),
        document_id: "doc_1".to_string(),
        source_url: None,
        heading_path: vec![],
        text: "Beta content".to_string(),
        token_count: 5,
        content_hash: "hash_beta".to_string(),
        position: 1,
        metadata: ChunkMetadata::default(),
    };

    // First call: both miss cache
    let v1 = batcher
        .embed_chunks(&[chunk1.clone(), chunk2.clone()])
        .await
        .unwrap();
    assert_eq!(v1.len(), 2);
    let stats = cache.stats();
    assert_eq!(stats.misses, 2);
    assert_eq!(stats.hits, 0);

    // Second call: both hit cache
    let v2 = batcher.embed_chunks(&[chunk1, chunk2]).await.unwrap();
    assert_eq!(v2.len(), 2);
    assert_eq!(v1, v2);
    let stats = cache.stats();
    assert_eq!(stats.hits, 2);
}

#[tokio::test]
async fn test_memory_vector_store_crud_and_filters() {
    let store = MemoryVectorStore::new();
    let index_id = "test_index";

    let v_rust = vec![1.0, 0.0, 0.0, 0.0];
    let v_python = vec![0.0, 1.0, 0.0, 0.0];
    let v_go = vec![0.0, 0.0, 1.0, 0.0];

    let rec1 = VectorRecord {
        chunk_id: "chk_rust".to_string(),
        document_id: "doc_rust".to_string(),
        index_id: index_id.to_string(),
        tenant_id: Some("tenant_a".to_string()),
        source_url: Some("https://example.com/rust/guide".to_string()),
        title: Some("Rust Guide".to_string()),
        heading_path: vec!["Rust".to_string()],
        text: "Rust memory safety and concurrency".to_string(),
        vector: v_rust.clone(),
        metadata: ChunkMetadata::default(),
        document_metadata: Some(rustcrawl::knowledge::models::IndexMetadata {
            language: Some("en".to_string()),
            ..Default::default()
        }),
    };

    let rec2 = VectorRecord {
        chunk_id: "chk_python".to_string(),
        document_id: "doc_python".to_string(),
        index_id: index_id.to_string(),
        tenant_id: Some("tenant_a".to_string()),
        source_url: Some("https://example.com/python/guide".to_string()),
        title: Some("Python Guide".to_string()),
        heading_path: vec!["Python".to_string()],
        text: "Python dynamic scripting and simplicity".to_string(),
        vector: v_python.clone(),
        metadata: ChunkMetadata::default(),
        document_metadata: Some(rustcrawl::knowledge::models::IndexMetadata {
            language: Some("en".to_string()),
            ..Default::default()
        }),
    };

    let rec3 = VectorRecord {
        chunk_id: "chk_go".to_string(),
        document_id: "doc_go".to_string(),
        index_id: index_id.to_string(),
        tenant_id: Some("tenant_b".to_string()), // Tenant B
        source_url: Some("https://example.com/go/guide".to_string()),
        title: Some("Go Guide".to_string()),
        heading_path: vec!["Go".to_string()],
        text: "Go goroutines and channels".to_string(),
        vector: v_go.clone(),
        metadata: ChunkMetadata::default(),
        document_metadata: Some(rustcrawl::knowledge::models::IndexMetadata {
            language: Some("en".to_string()),
            ..Default::default()
        }),
    };

    store
        .upsert(index_id, vec![rec1, rec2, rec3])
        .await
        .unwrap();
    assert_eq!(store.count(index_id).await.unwrap(), 3);

    // Query for Rust (Vector [1.0, 0.0, 0.0, 0.0]) under Tenant A
    let query_a = VectorQuery {
        vector: vec![0.9, 0.1, 0.0, 0.0],
        limit: 10,
        filter: None,
        tenant_id: Some("tenant_a".to_string()),
        min_score: None,
    };

    let results_a = store.search(index_id, query_a).await.unwrap();
    assert_eq!(results_a.len(), 2, "Tenant A should see 2 documents");
    assert_eq!(results_a[0].chunk_id, "chk_rust");
    assert!(results_a[0].score > 0.9);

    // Path prefix filter test
    let query_filter = VectorQuery {
        vector: vec![1.0, 0.0, 0.0, 0.0],
        limit: 10,
        filter: Some(IndexFilter {
            path_prefix: Some("/rust".to_string()),
            ..Default::default()
        }),
        tenant_id: None,
        min_score: None,
    };

    let results_filter = store.search(index_id, query_filter).await.unwrap();
    assert_eq!(results_filter.len(), 1);
    assert_eq!(results_filter[0].chunk_id, "chk_rust");

    // Delete document
    store.delete_document(index_id, "doc_rust").await.unwrap();
    assert_eq!(store.count(index_id).await.unwrap(), 2);
}
