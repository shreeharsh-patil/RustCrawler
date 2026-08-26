use rustcrawl::search::local_index::{IndexedDocument, LocalSearchIndex};
use rustcrawl::search::models::{SearchMode, SearchRequest};
use rustcrawl::search::provider::{MockSearchProvider, SearchProvider};
use url::Url;

#[tokio::test]
async fn test_mock_search_provider() {
    let mock = MockSearchProvider::new();
    let req = SearchRequest {
        query: "rust async web crawler".to_string(),
        limit: 5,
        scrape: false,
        formats: Vec::new(),
        mode: Some(SearchMode::Web),
        domain: None,
        exclude_domains: Vec::new(),
        provider: None,
    };

    let results = mock.search(&req).await.unwrap();
    assert!(!results.is_empty());
    assert_eq!(
        results[0].title.as_deref(),
        Some("Rust Async Web Crawler Framework")
    );
}

#[test]
fn test_local_search_index_snippet_generation() {
    let index = LocalSearchIndex::new(500);

    let doc = IndexedDocument {
        url: Url::parse("https://rust-lang.org/learn").unwrap(),
        title: Some("Learn Rust".to_string()),
        description: Some("Getting started with the Rust programming language.".to_string()),
        text: "Rust is a systems programming language that is extremely fast and memory-efficient. Concurrency is safe and asynchronous I/O is powered by Tokio runtime.".to_string(),
        content_hash: Some("mock_learn_hash".to_string()),
        last_seen: 100,
    };

    index.insert_document(doc);

    let results = index.search("Tokio concurrency", 5);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url.as_str(), "https://rust-lang.org/learn");
    assert!(results[0].description.is_some());
    let desc = results[0].description.as_ref().unwrap();
    assert!(desc.contains("Tokio") || desc.contains("Concurrency"));
}
