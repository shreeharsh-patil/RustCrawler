use rustcrawl::extraction::candidate::CandidatePool;
use rustcrawl::extraction::deterministic::DeterministicExtractor;
use rustcrawl::extraction::models::MissingFieldBehavior;
use rustcrawl::models::{OpenGraphMetadata, PageMetadata, ScrapeResult};
use rustcrawl::DocumentTable;
use serde_json::json;
use url::Url;

fn mock_scrape_result_with_json_ld() -> ScrapeResult {
    let json_ld_product = json!({
        "@context": "https://schema.org",
        "@type": "Product",
        "name": "Sony WH-1000XM5 Wireless Headphones",
        "description": "Industry Leading Noise Canceling with 2 processors",
        "offers": {
            "@type": "Offer",
            "price": 399.99,
            "priceCurrency": "USD",
            "availability": "https://schema.org/InStock"
        },
        "brand": {
            "@type": "Brand",
            "name": "Sony"
        }
    });

    let metadata = PageMetadata {
        json_ld: vec![json_ld_product],
        title: Some("Sony WH-1000XM5 Headphones | Store".to_string()),
        description: Some("Buy the latest Sony WH-1000XM5".to_string()),
        ..Default::default()
    };

    ScrapeResult {
        url: "https://store.example.com/products/sony-wh1000xm5".to_string(),
        final_url: "https://store.example.com/products/sony-wh1000xm5".to_string(),
        content_type: Some("text/html".to_string()),
        content_hash: Some("mock_hash_123".to_string()),
        markdown: Some("# Sony WH-1000XM5\nBest headphones".to_string()),
        text: Some("Sony WH-1000XM5 Best headphones".to_string()),
        html: Some("<html><head><title>Sony WH-1000XM5</title></head><body><h1>Sony WH-1000XM5</h1></body></html>".to_string()),
        clean_html: None,
        json: None,
        metadata: Some(metadata),
        headings: None,
        links: None,
        images: None,
        tables: None,
        pages: None,
        http: rustcrawl::models::HttpMetadata {
            status: 200,
            content_type: "text/html".to_string(),
            content_length: 1000,
            fetch_time_ms: 10,
            parse_time_ms: 2,
            extract_time_ms: 1,
            total_time_ms: 13,
        },
        renderer: Some("http".to_string()),
        render_reason: None,
        render_diagnostics: None,
        network_responses: None,
        warnings: Vec::new(),
    }
}

#[test]
fn test_deterministic_extraction_from_json_ld() {
    let result = mock_scrape_result_with_json_ld();
    let pool = CandidatePool::from_scrape_result(&result);
    let target_url = Url::parse(&result.url).unwrap();

    let schema = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "price": { "type": "number" },
            "currency": { "type": "string" }
        },
        "required": ["name", "price"]
    });

    let (extracted, provenance, is_confident) = DeterministicExtractor::extract(
        &pool,
        Some(&schema),
        None,
        &target_url,
        MissingFieldBehavior::Null,
    );

    assert!(is_confident);
    assert!(extracted.is_some());
    let data = extracted.unwrap();

    assert_eq!(data["name"], "Sony WH-1000XM5 Wireless Headphones");
    assert_eq!(data["price"], 399.99);
    assert_eq!(data["currency"], "USD");

    assert!(provenance.contains_key("name"));
    assert!(provenance.contains_key("price"));
}

#[test]
fn test_deterministic_extraction_from_opengraph_and_tables() {
    let metadata = PageMetadata {
        open_graph: Some(OpenGraphMetadata {
            title: Some("Acme Corp Cloud Platform".to_string()),
            description: Some("Scalable cloud infrastructure".to_string()),
            image: Some("https://acme.com/og.png".to_string()),
            url: Some("https://acme.com".to_string()),
            site_name: Some("Acme Cloud".to_string()),
            locale: None,
            og_type: None,
        }),
        ..Default::default()
    };

    let table = DocumentTable {
        name: None,
        headers: vec![
            "Plan".to_string(),
            "Price".to_string(),
            "Storage".to_string(),
        ],
        rows: vec![
            vec![
                "Starter".to_string(),
                "$29.00".to_string(),
                "100GB".to_string(),
            ],
            vec!["Pro".to_string(), "$99.00".to_string(), "1TB".to_string()],
        ],
    };

    let scrape_res = ScrapeResult {
        url: "https://acme.com/pricing".to_string(),
        final_url: "https://acme.com/pricing".to_string(),
        content_type: Some("text/html".to_string()),
        content_hash: Some("hash_acme".to_string()),
        markdown: None,
        text: None,
        html: None,
        clean_html: None,
        json: None,
        metadata: Some(metadata),
        headings: None,
        links: None,
        images: None,
        tables: Some(vec![table]),
        pages: None,
        http: rustcrawl::models::HttpMetadata {
            status: 200,
            content_type: "text/html".to_string(),
            content_length: 500,
            fetch_time_ms: 5,
            parse_time_ms: 1,
            extract_time_ms: 1,
            total_time_ms: 7,
        },
        renderer: Some("http".to_string()),
        render_reason: None,
        render_diagnostics: None,
        network_responses: None,
        warnings: Vec::new(),
    };

    let pool = CandidatePool::from_scrape_result(&scrape_res);
    let target_url = Url::parse(&scrape_res.url).unwrap();

    let schema = json!({
        "type": "object",
        "properties": {
            "title": { "type": "string" },
            "description": { "type": "string" },
            "image": { "type": "string" }
        },
        "required": ["title", "description"]
    });

    let (extracted, provenance, is_confident) = DeterministicExtractor::extract(
        &pool,
        Some(&schema),
        None,
        &target_url,
        MissingFieldBehavior::Null,
    );

    assert!(is_confident);
    assert!(extracted.is_some());
    let data = extracted.unwrap();

    assert_eq!(data["title"], "Acme Corp Cloud Platform");
    assert_eq!(data["description"], "Scalable cloud infrastructure");
    assert_eq!(data["image"], "https://acme.com/og.png");
    assert_eq!(provenance.len(), 3);
}
