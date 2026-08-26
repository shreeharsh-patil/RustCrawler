use rustcrawl::discovery::lightweight::LightweightExtractor;
use rustcrawl::discovery::models::DiscoveredLink;
use rustcrawl::discovery::ranking::{tokenize, RelevanceRanker};
use rustcrawl::DiscoverySource;
use url::Url;

#[test]
fn test_tokenization_and_stopwords() {
    let tokens = tokenize("The ultimate Guide to Rust Async Web Crawling & APIs");
    assert!(tokens.contains(&"ultimate".to_string()));
    assert!(tokens.contains(&"guide".to_string()));
    assert!(tokens.contains(&"rust".to_string()));
    assert!(tokens.contains(&"async".to_string()));
    assert!(tokens.contains(&"crawling".to_string()));
    assert!(tokens.contains(&"apis".to_string()));

    // Verify stopwords were filtered
    assert!(!tokens.contains(&"the".to_string()));
    assert!(!tokens.contains(&"to".to_string()));
}

#[test]
fn test_relevance_ranker_ordering() {
    let mut links = vec![
        DiscoveredLink {
            url: "https://example.com/about".to_string(),
            title: Some("About Acme Corp".to_string()),
            description: Some("Our team and history".to_string()),
            source: DiscoverySource::HtmlLink,
            score: None,
        },
        DiscoveredLink {
            url: "https://example.com/docs/api-authentication".to_string(),
            title: Some("API Authentication & Tokens".to_string()),
            description: Some("How to authenticate with JWT and bearer tokens".to_string()),
            source: DiscoverySource::Sitemap,
            score: None,
        },
        DiscoveredLink {
            url: "https://example.com/pricing".to_string(),
            title: Some("Pricing Plans".to_string()),
            description: Some("Monthly and annual subscriptions".to_string()),
            source: DiscoverySource::HtmlLink,
            score: None,
        },
    ];

    RelevanceRanker::rank_links(&mut links, "API Authentication");

    // The API documentation link must be top ranked
    assert_eq!(links[0].url, "https://example.com/docs/api-authentication");
    assert!(links[0].score.unwrap() > links[1].score.unwrap_or(0.0));
}

#[test]
fn test_lightweight_html_extractor() {
    let html = r##"<!DOCTYPE html>
    <html>
    <head>
        <title>Fast Discovery Page</title>
        <meta name="description" content="Lightweight URL mapper test page">
    </head>
    <body>
        <h1>Navigation</h1>
        <a href="/docs/intro">Documentation</a>
        <a href="https://example.com/contact">Contact Us</a>
        <a href="#section">Ignored Anchor</a>
        <a href="javascript:void(0)">Ignored JS</a>
    </body>
    </html>"##;

    let base_url = Url::parse("https://example.com/home").unwrap();
    let summary = LightweightExtractor::extract(html, &base_url, 100, 128);

    assert_eq!(summary.title.as_deref(), Some("Fast Discovery Page"));
    assert_eq!(
        summary.description.as_deref(),
        Some("Lightweight URL mapper test page")
    );
    assert_eq!(summary.links.len(), 2);

    let urls: Vec<String> = summary.links.iter().map(|(u, _)| u.to_string()).collect();
    assert!(urls.contains(&"https://example.com/docs/intro".to_string()));
    assert!(urls.contains(&"https://example.com/contact".to_string()));
}
