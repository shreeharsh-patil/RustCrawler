use rustcrawl::config::Config;
use rustcrawl::detect::ContentDetector;
use rustcrawl::document::parser::{ContentParser, ParseInput};
use rustcrawl::document::parsers::json::JsonDocumentParser;
use rustcrawl::models::ScrapeOptions;
use url::Url;

#[tokio::test]
async fn test_json_parser_basic_and_markdown() {
    let url = Url::parse("https://api.example.com/items").unwrap();
    let json_bytes = br#"{
        "store": "Main Branch",
        "active": true,
        "items": [
            {"id": 101, "name": "Mechanical Keyboard", "price": 129.99, "link": "https://example.com/kb"},
            {"id": 102, "name": "Wireless Mouse", "price": 49.99, "link": "https://example.com/mouse"}
        ]
    }"#;

    let detected = ContentDetector::detect(&url, Some("application/json"), json_bytes);
    let parser = JsonDocumentParser::new();
    let config = Config::default();
    let options = ScrapeOptions::default();

    let input = ParseInput {
        url: &url,
        final_url: &url,
        bytes: json_bytes,
        detected: &detected,
        options: &options,
        config: &config,
    };

    let doc = parser.parse(input).await.expect("JSON parse failed");

    assert!(doc.structured_data.is_some());
    assert!(doc.markdown.is_some());
    let md = doc.markdown.unwrap();
    assert!(md.contains("Mechanical Keyboard"));
    assert!(md.contains("129.99"));
    assert_eq!(doc.links.len(), 2);
    assert_eq!(doc.links[0].url, "https://example.com/kb");
    assert_eq!(doc.links[1].url, "https://example.com/mouse");
}

#[tokio::test]
async fn test_json_parser_jsonpath_filtering() {
    let url = Url::parse("https://api.example.com/data").unwrap();
    let json_bytes = br#"{
        "data": {
            "products": [
                {"name": "Widget A", "score": 95},
                {"name": "Widget B", "score": 82}
            ]
        }
    }"#;

    let detected = ContentDetector::detect(&url, Some("application/json"), json_bytes);
    let parser = JsonDocumentParser::new();
    let config = Config::default();
    let options = ScrapeOptions {
        json_path: Some("$.data.products[*].name".to_string()),
        ..Default::default()
    };

    let input = ParseInput {
        url: &url,
        final_url: &url,
        bytes: json_bytes,
        detected: &detected,
        options: &options,
        config: &config,
    };

    let doc = parser.parse(input).await.expect("JSONPath parse failed");
    let structured = doc.structured_data.expect("Missing structured data");
    assert!(structured.is_array());
    let arr = structured.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0], "Widget A");
    assert_eq!(arr[1], "Widget B");
}
