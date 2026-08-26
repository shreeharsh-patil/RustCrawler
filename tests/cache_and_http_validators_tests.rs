use reqwest::header::{HeaderMap, HeaderValue, IF_MODIFIED_SINCE, IF_NONE_MATCH};
use reqwest::StatusCode;
use rustcrawl::cache::http_cache::HttpCacheUtils;
use rustcrawl::cache::store::{CacheEntry, CacheStore, InMemoryCacheStore};

#[tokio::test]
async fn test_in_memory_cache_store_ttl() {
    let store = InMemoryCacheStore::new(true, 100);

    // Entry with 1 second TTL
    let entry = CacheEntry::new(
        b"cached payload".to_vec(),
        Some("text/plain".to_string()),
        200,
        Some("\"etag-123\"".to_string()),
        Some("Wed, 25 Aug 2026 12:00:00 GMT".to_string()),
        Some("hash-abc".to_string()),
        1,
    );

    store.put("key_1".to_string(), entry).await.unwrap();

    let fetched = store.get("key_1").await.unwrap();
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().data, b"cached payload");

    // Sleep 1.2s to trigger TTL expiration
    tokio::time::sleep(tokio::time::Duration::from_millis(1200)).await;

    let expired = store.get("key_1").await.unwrap();
    assert!(expired.is_none(), "Expired cache item should return None");
}

#[test]
fn test_http_cache_conditional_headers() {
    let entry = CacheEntry::new(
        vec![],
        None,
        200,
        Some("\"w/12345\"".to_string()),
        Some("Wed, 21 Oct 2025 07:28:00 GMT".to_string()),
        None,
        3600,
    );

    let mut headers = HeaderMap::new();
    HttpCacheUtils::apply_conditional_headers(&mut headers, &entry);

    assert_eq!(
        headers.get(IF_NONE_MATCH),
        Some(&HeaderValue::from_static("\"w/12345\""))
    );
    assert_eq!(
        headers.get(IF_MODIFIED_SINCE),
        Some(&HeaderValue::from_static("Wed, 21 Oct 2025 07:28:00 GMT"))
    );
}

#[test]
fn test_http_cache_304_detection() {
    assert!(HttpCacheUtils::is_not_modified(StatusCode::NOT_MODIFIED));
    assert!(!HttpCacheUtils::is_not_modified(StatusCode::OK));
}
