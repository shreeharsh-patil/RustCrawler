use rustcrawl::knowledge::chunking::dedupe::{compute_chunk_hash, SimHash};
use rustcrawl::knowledge::chunking::structural::{ChunkingConfig, StructuralChunker};
use rustcrawl::knowledge::chunking::tokenizer::{
    ApproximateTokenizer, TokenCounter, WordTokenizer,
};

#[test]
fn test_approximate_and_word_tokenizers() {
    let app_tok = ApproximateTokenizer;
    let word_tok = WordTokenizer;

    let text = "RustCrawler is a production-grade universal web scraper and search engine in Rust.";
    let c1 = app_tok.count(text);
    let c2 = word_tok.count(text);

    assert!(c1 > 5 && c1 < 30);
    assert!(c2 > 5 && c2 < 30);
    assert_eq!(app_tok.count(""), 0);
    assert_eq!(word_tok.count(""), 0);
}

#[test]
fn test_structural_chunker_heading_hierarchy_preservation() {
    let markdown = r#"# Chapter 1: Introduction

Welcome to the documentation.

## 1.1 Architecture

The engine uses an asynchronous pipeline.

### 1.1.1 Chunker Subsystem

The chunker breaks documents down into semantic units.

```rust
pub struct IndexedChunk {
    pub chunk_id: String,
}
```

## 1.2 Configuration

Here is how to configure parameters.
"#;

    let chunker = StructuralChunker::with_default_tokenizer(ChunkingConfig {
        target_tokens: 50,
        max_tokens: 150,
        min_tokens: 5,
        overlap_tokens: 0,
    });

    let chunks = chunker.chunk_markdown("doc_intro", Some("https://example.com/intro"), markdown);
    assert!(!chunks.is_empty(), "Should generate chunks");

    // Verify Chapter 1 > 1.1 Architecture > 1.1.1 Chunker Subsystem
    let sub_chunk = chunks
        .iter()
        .find(|c| c.text.contains("Chunker Subsystem") || c.text.contains("IndexedChunk"))
        .expect("Subsystem chunk must exist");

    assert_eq!(
        sub_chunk.heading_path,
        vec![
            "Chapter 1: Introduction",
            "1.1 Architecture",
            "1.1.1 Chunker Subsystem"
        ]
    );

    // Verify 1.2 Configuration pops 1.1 hierarchy
    let config_chunk = chunks
        .iter()
        .find(|c| c.text.contains("Configuration") || c.text.contains("configure parameters"))
        .expect("Config chunk must exist");

    assert_eq!(
        config_chunk.heading_path,
        vec!["Chapter 1: Introduction", "1.2 Configuration"]
    );
}

#[test]
fn test_structural_chunker_table_and_code_preservation() {
    let markdown = r#"# Performance Benchmarks

| Component | p50 (ms) | p95 (ms) | Throughput (req/s) |
|---|---|---|---|
| Chunker | 0.8 | 1.4 | 12,000 |
| Inverted Index | 0.2 | 0.5 | 45,000 |
| Vector Cosine | 1.1 | 2.0 | 8,500 |

```json
{
  "status": "ready",
  "version": "0.1.0"
}
```
"#;

    let chunker = StructuralChunker::with_default_tokenizer(ChunkingConfig {
        target_tokens: 200,
        max_tokens: 500,
        min_tokens: 10,
        overlap_tokens: 0,
    });

    let chunks = chunker.chunk_markdown("doc_bench", None, markdown);
    assert!(!chunks.is_empty());

    let table_chunk = chunks.iter().find(|c| c.text.contains("p50 (ms)")).unwrap();
    assert!(table_chunk.text.contains("| Inverted Index |"));

    let code_chunk = chunks.iter().find(|c| c.text.contains("```json")).unwrap();
    assert_eq!(code_chunk.metadata.code_lang, Some("json".to_string()));
}

#[test]
fn test_structural_chunker_json_path_preservation() {
    let json_data = serde_json::json!({
        "service": "RustCrawler Knowledge Engine",
        "endpoints": [
            { "path": "/v1/search/hybrid", "method": "POST" },
            { "path": "/v1/retrieve", "method": "POST" },
            { "path": "/v1/answer", "method": "POST" }
        ],
        "limits": {
            "max_tokens": 16000,
            "max_chunks": 20
        }
    });

    let chunker = StructuralChunker::with_default_tokenizer(ChunkingConfig {
        target_tokens: 50,
        max_tokens: 100,
        min_tokens: 5,
        overlap_tokens: 0,
    });

    let chunks = chunker.chunk_json(
        "doc_api_spec",
        Some("https://api.example.com/spec.json"),
        &json_data,
    );
    assert!(!chunks.is_empty());

    for chunk in &chunks {
        assert_eq!(chunk.document_id, "doc_api_spec");
        assert!(!chunk.content_hash.is_empty());
        assert!(!chunk.chunk_id.is_empty());
    }
}

#[test]
fn test_structural_chunker_pdf_page_metadata() {
    let pages = vec![
        (1, "# Introduction\nThis is page one content.".to_string()),
        (
            2,
            "# Details\nThis is page two content with further specifications.".to_string(),
        ),
    ];

    let chunker = StructuralChunker::with_default_tokenizer(ChunkingConfig::default());
    let chunks =
        chunker.chunk_pdf_pages("doc_manual", Some("https://example.com/manual.pdf"), &pages);

    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].metadata.page_number, Some(1));
    assert_eq!(chunks[1].metadata.page_number, Some(2));
}

#[test]
fn test_chunk_hash_and_simhash_near_duplicates() {
    let text_a = "RustCrawler provides distributed crawling, semantic chunking, and grounded RAG.";
    let text_b = "RustCrawler provides distributed crawling, intelligent semantic chunking, and grounded RAG.";
    let text_c =
        "Quantum computing harnesses superposition and entanglement to perform complex operations.";

    let hash_a = compute_chunk_hash(text_a);
    let hash_b = compute_chunk_hash(text_b);
    assert_ne!(
        hash_a, hash_b,
        "Different texts produce distinct BLAKE3 hashes"
    );

    let sim_a = SimHash::compute(text_a);
    let sim_b = SimHash::compute(text_b);
    let sim_c = SimHash::compute(text_c);

    let dist_similar = sim_a.hamming_distance(&sim_b);
    let dist_unrelated = sim_a.hamming_distance(&sim_c);

    assert!(
        dist_similar < dist_unrelated,
        "Similar texts ({dist_similar}) should have lower Hamming distance than unrelated texts ({dist_unrelated})"
    );
}
