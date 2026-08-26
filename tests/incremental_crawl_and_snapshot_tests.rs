use rustcrawl::crawl::snapshot::{
    compute_normalized_content_hash, CrawlSnapshot, PageSnapshot, SnapshotStore,
};

#[test]
fn test_normalized_content_hash_whitespace_insensitivity() {
    let text1 = "Title\n\nSome paragraph text.  \n   More text.\n";
    let text2 = "Title\nSome paragraph text.\nMore text.";

    let hash1 = compute_normalized_content_hash(text1);
    let hash2 = compute_normalized_content_hash(text2);

    assert_eq!(
        hash1, hash2,
        "Normalized hashes should ignore trivial whitespace differences"
    );

    let text3 = "Title\nSome modified paragraph text.\nMore text.";
    let hash3 = compute_normalized_content_hash(text3);
    assert_ne!(
        hash1, hash3,
        "Content alterations must produce distinct hashes"
    );
}

#[test]
fn test_crawl_snapshot_store() {
    let store = SnapshotStore::new();
    let mut snapshot =
        CrawlSnapshot::new("snap_123".to_string(), "https://example.com".to_string());

    snapshot.insert_page(PageSnapshot {
        url: "https://example.com/pricing".to_string(),
        final_url: "https://example.com/pricing".to_string(),
        content_hash: Some("hash_pricing_v1".to_string()),
        raw_content_hash: Some("hash_pricing_v1".to_string()),
        normalized_content_hash: Some("norm_pricing_v1".to_string()),
        metadata_hash: None,
        etag: Some("\"etag_v1\"".to_string()),
        last_modified: None,
        status_code: 200,
        title: Some("Pricing Plans".to_string()),
    });

    store.save_snapshot(snapshot);
    store.link_job("job_abc", "snap_123");

    let retrieved_by_id = store.get_snapshot("snap_123");
    assert!(retrieved_by_id.is_some());
    assert_eq!(retrieved_by_id.unwrap().pages.len(), 1);

    let retrieved_by_job = store.get_snapshot("job_abc");
    assert!(retrieved_by_job.is_some());
    assert_eq!(retrieved_by_job.unwrap().seed_url, "https://example.com");
}
