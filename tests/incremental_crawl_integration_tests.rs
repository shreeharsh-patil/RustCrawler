use rustcrawl::crawl::snapshot::{
    compute_normalized_content_hash, CrawlSnapshot, PageSnapshot, SnapshotStore,
};
use std::collections::HashSet;

#[test]
fn test_fixture_site_snapshot_v1_and_v2_change_detection() {
    let store = SnapshotStore::new();

    // 1. Build Snapshot v1 (Baseline)
    let mut snap_v1 = CrawlSnapshot::new("snap_v1".to_string(), "https://example.com".to_string());

    let pages_v1 = vec![
        ("https://example.com/", "Home Page v1", 200, "\"etag-1\""),
        (
            "https://example.com/docs",
            "Documentation v1",
            200,
            "\"etag-2\"",
        ),
        (
            "https://example.com/docs/api",
            "API Reference v1",
            200,
            "\"etag-3\"",
        ),
        (
            "https://example.com/blog",
            "Old Blog Post",
            200,
            "\"etag-4\"",
        ),
        (
            "https://example.com/pricing",
            "Pricing Table $10/mo",
            200,
            "\"etag-5\"",
        ),
    ];

    for (url, content, status, etag) in pages_v1 {
        let norm_hash = compute_normalized_content_hash(content);
        let raw_hash = blake3::hash(content.as_bytes()).to_hex().to_string();
        snap_v1.insert_page(PageSnapshot {
            url: url.to_string(),
            final_url: url.to_string(),
            content_hash: Some(raw_hash.clone()),
            raw_content_hash: Some(raw_hash),
            normalized_content_hash: Some(norm_hash),
            metadata_hash: None,
            etag: Some(etag.to_string()),
            last_modified: None,
            status_code: status,
            title: Some(content.to_string()),
        });
    }

    store.save_snapshot(snap_v1);

    // 2. Build Snapshot v2 (Recrawl)
    // - / is unchanged (304)
    // - /docs is unchanged (same content hash)
    // - /docs/api is changed (different content hash)
    // - /blog is deleted (404)
    // - /pricing is unchanged (whitespace variation -> normalized hash equal)
    // - /new-feature is new
    let pages_v2 = vec![
        ("https://example.com/", "Home Page v1", 304, "\"etag-1\""),
        (
            "https://example.com/docs",
            "Documentation v1",
            200,
            "\"etag-2\"",
        ),
        (
            "https://example.com/docs/api",
            "API Reference v2 (Updated with JWT)",
            200,
            "\"etag-3-new\"",
        ),
        ("https://example.com/blog", "404 Not Found", 404, ""),
        (
            "https://example.com/pricing",
            "Pricing Table $10/mo  \n\n",
            200,
            "\"etag-5\"",
        ),
        (
            "https://example.com/new-feature",
            "Brand New Feature Announcement",
            200,
            "\"etag-6\"",
        ),
    ];

    let baseline = store.get_snapshot("snap_v1").unwrap();
    let mut new_urls = Vec::new();
    let mut changed_urls = Vec::new();
    let mut deleted_urls = Vec::new();
    let mut unchanged_count = 0;
    let mut seen_v2 = HashSet::new();

    for (url, content, status, _etag) in pages_v2 {
        seen_v2.insert(url.to_string());
        let norm_hash = compute_normalized_content_hash(content);

        if let Some(baseline_page) = baseline.pages.get(url) {
            if status == 304 {
                unchanged_count += 1;
            } else if status == 404 || status == 410 {
                deleted_urls.push(url.to_string());
            } else if norm_hash
                == baseline_page
                    .normalized_content_hash
                    .clone()
                    .unwrap_or_default()
            {
                unchanged_count += 1;
            } else {
                changed_urls.push(url.to_string());
            }
        } else {
            new_urls.push(url.to_string());
        }
    }

    // Verify classification results
    assert_eq!(
        new_urls,
        vec!["https://example.com/new-feature".to_string()]
    );
    assert_eq!(
        changed_urls,
        vec!["https://example.com/docs/api".to_string()]
    );
    assert_eq!(deleted_urls, vec!["https://example.com/blog".to_string()]);
    assert_eq!(unchanged_count, 3); // /, /docs, and /pricing (via whitespace-insensitive normalization)
}
