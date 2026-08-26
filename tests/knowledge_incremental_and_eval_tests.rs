use rustcrawl::config::Config;
use rustcrawl::knowledge::models::SearchMode;
use rustcrawl::knowledge::KnowledgeEngine;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

#[tokio::test]
async fn test_incremental_indexing_chunk_reuse() {
    let config = Config {
        semantic_indexing_enabled: true,
        embeddings_enabled: true,
        embedding_provider: "mock".to_string(),
        ..Default::default()
    };

    let engine = KnowledgeEngine::new(Arc::new(config), None);
    let index = engine
        .create_index("test_inc", None, None, None)
        .expect("Index creation should succeed");

    let doc_v1 = r#"# Section 1: Getting Started

RustCrawler is easy to install and run via cargo or docker.

# Section 2: Architecture

The pipeline consists of fetchers, renderers, parsers, and extractors.
"#;

    let doc1 = engine
        .index_markdown(
            &index.id,
            doc_v1,
            Some("Manual"),
            Some("https://example.com/manual"),
            None,
        )
        .await
        .unwrap();

    let initial_embedded = engine.metrics.chunks_embedded.load(Ordering::Relaxed);
    assert!(initial_embedded >= 2);

    // Update Section 2 only; Section 1 remains identical
    let doc_v2 = r#"# Section 1: Getting Started

RustCrawler is easy to install and run via cargo or docker.

# Section 2: Architecture Updated

The pipeline consists of fetchers, renderers, parsers, extractors, and distributed workers.
"#;

    // Ingest v2 under the same document_id
    let chunks_v2 =
        rustcrawl::knowledge::chunking::structural::StructuralChunker::with_default_tokenizer(
            rustcrawl::knowledge::chunking::structural::ChunkingConfig::default(),
        )
        .chunk_markdown(
            &doc1.document_id,
            Some("https://example.com/manual"),
            doc_v2,
        );

    let doc_v2_struct = rustcrawl::knowledge::models::IndexedDocument {
        document_id: doc1.document_id.clone(),
        source_url: Some("https://example.com/manual".to_string()),
        content_hash: rustcrawl::knowledge::chunking::dedupe::compute_chunk_hash(doc_v2),
        title: Some("Manual".to_string()),
        content_type: "markdown".to_string(),
        metadata: Default::default(),
        chunks: chunks_v2,
    };

    engine
        .index_document(&index.id, doc_v2_struct)
        .await
        .unwrap();

    let reused = engine
        .metrics
        .incremental_chunks_reused
        .load(Ordering::Relaxed);
    assert!(
        reused >= 1,
        "Unchanged Section 1 chunk should be reused without re-embedding"
    );
}

#[tokio::test]
async fn test_retrieval_quality_evaluation_benchmark() {
    let config = Config {
        semantic_indexing_enabled: true,
        embeddings_enabled: true,
        embedding_provider: "mock".to_string(),
        ..Default::default()
    };

    let engine = KnowledgeEngine::new(Arc::new(config), None);
    let index = engine
        .create_index("eval_index", None, None, None)
        .expect("Index creation should succeed");

    // Index a synthetic domain knowledge corpus
    let corpus = vec![
        ("doc_1", "https://example.com/pgvector", "PostgreSQL Vector Database", "Postgres pgvector extension enables efficient HNSW vector cosine search and distance metrics."),
        ("doc_2", "https://example.com/bm25", "BM25 Lexical Ranking Algorithm", "BM25 scoring weights term frequency and inverse document frequency with k1 and b parameters."),
        ("doc_3", "https://example.com/rag", "Retrieval Augmented Generation", "RAG systems retrieve relevant source chunks and provide grounded context to LLMs."),
        ("doc_4", "https://example.com/rrf", "Reciprocal Rank Fusion", "RRF combines rankings from vector similarity and BM25 lexical search using 1 / (60 + rank)."),
        ("doc_5", "https://example.com/crawler", "Distributed Web Crawler", "A distributed crawler uses Redis task queues and PostgreSQL metadata stores to scale worker processes."),
    ];

    for (_id, url, title, body) in corpus {
        let md = format!("# {title}\n\n{body}");
        engine
            .index_markdown(&index.id, &md, Some(title), Some(url), None)
            .await
            .unwrap();
    }

    // Benchmark Queries and Expected Relevant Targets
    let queries = vec![
        ("HNSW cosine similarity Postgres", "pgvector"),
        ("k1 and b parameters term frequency", "bm25"),
        ("grounded context LLMs", "rag"),
        ("rankings vector lexical search", "rrf"),
        ("distributed task queues Redis", "crawler"),
    ];

    let mut lexical_top1_hits = 0;
    let mut hybrid_top5_hits = 0;
    let total_queries = queries.len();

    let start = Instant::now();
    for (q, expected_slug) in &queries {
        let lex_res = engine
            .search(&index.id, q, 5, SearchMode::Lexical, None, None)
            .await
            .unwrap();
        if !lex_res.is_empty()
            && lex_res[0]
                .source_url
                .as_deref()
                .unwrap_or("")
                .contains(expected_slug)
        {
            lexical_top1_hits += 1;
        }

        let hybrid_res = engine
            .search(&index.id, q, 5, SearchMode::Hybrid, None, None)
            .await
            .unwrap();
        if hybrid_res.iter().any(|r| {
            r.source_url
                .as_deref()
                .unwrap_or("")
                .contains(expected_slug)
        }) {
            hybrid_top5_hits += 1;
        }
    }
    let duration = start.elapsed();

    let lexical_recall_at_1 = lexical_top1_hits as f64 / total_queries as f64;
    let hybrid_recall_at_5 = hybrid_top5_hits as f64 / total_queries as f64;

    assert!(
        lexical_recall_at_1 >= 0.8,
        "Lexical search Recall@1 should be >= 80%, got {lexical_recall_at_1}"
    );
    assert!(
        hybrid_recall_at_5 >= 0.8,
        "Hybrid search Recall@5 should be >= 80%, got {hybrid_recall_at_5}"
    );

    println!(
        "Evaluation results: Total queries: {}, Lexical Recall@1: {:.2}%, Hybrid Recall@5: {:.2}%, Duration: {:?}",
        total_queries,
        lexical_recall_at_1 * 100.0,
        hybrid_recall_at_5 * 100.0,
        duration
    );
}
