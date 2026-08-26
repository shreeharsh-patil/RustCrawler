use rustcrawl::config::Config;
use rustcrawl::detect::ContentDetector;
use rustcrawl::document::parser::{ContentParser, ParseInput};
use rustcrawl::document::parsers::feed::FeedDocumentParser;
use rustcrawl::document::parsers::xml::XmlDocumentParser;
use rustcrawl::models::ScrapeOptions;
use url::Url;

#[tokio::test]
async fn test_xml_parser_basic_and_anti_xxe() {
    let url = Url::parse("https://example.com/data.xml").unwrap();
    let xml_bytes = br#"<?xml version="1.0" encoding="UTF-8"?>
    <catalog>
        <book id="bk101">
            <author>Gambardella, Matthew</author>
            <title>XML Developer's Guide</title>
            <price>44.95</price>
            <link>https://example.com/books/bk101</link>
        </book>
    </catalog>"#;

    let detected = ContentDetector::detect(&url, Some("application/xml"), xml_bytes);
    let parser = XmlDocumentParser::new();
    let config = Config::default();
    let options = ScrapeOptions::default();

    let input = ParseInput {
        url: &url,
        final_url: &url,
        bytes: xml_bytes,
        detected: &detected,
        options: &options,
        config: &config,
    };

    let doc = parser.parse(input).await.expect("XML parse failed");
    assert!(doc.structured_data.is_some());
    assert!(doc.markdown.is_some());
    assert_eq!(doc.links.len(), 1);
    assert_eq!(doc.links[0].url, "https://example.com/books/bk101");
}

#[tokio::test]
async fn test_rss_feed_parser_entries_and_metadata() {
    let url = Url::parse("https://example.com/rss.xml").unwrap();
    let rss_bytes = br#"<?xml version="1.0" encoding="UTF-8" ?>
    <rss version="2.0">
    <channel>
        <title>Tech News Daily</title>
        <link>https://example.com</link>
        <description>The latest technology updates</description>
        <item>
            <title>Rust 2026 Released</title>
            <link>https://example.com/rust-2026</link>
            <description>Exciting new features in Rust!</description>
            <pubDate>Mon, 25 Aug 2026 12:00:00 GMT</pubDate>
        </item>
        <item>
            <title>New Async Engine</title>
            <link>https://example.com/async-engine</link>
            <description>A deep dive into lightweight async architectures.</description>
        </item>
    </channel>
    </rss>"#;

    let detected = ContentDetector::detect(&url, None, rss_bytes);
    let parser = FeedDocumentParser::new();
    let config = Config::default();
    let options = ScrapeOptions::default();

    let input = ParseInput {
        url: &url,
        final_url: &url,
        bytes: rss_bytes,
        detected: &detected,
        options: &options,
        config: &config,
    };

    let doc = parser.parse(input).await.expect("RSS parse failed");
    assert_eq!(doc.metadata.title.as_deref(), Some("Tech News Daily"));
    assert_eq!(doc.links.len(), 2);
    assert_eq!(doc.links[0].url, "https://example.com/rust-2026");
    assert_eq!(doc.links[1].url, "https://example.com/async-engine");

    let md = doc.markdown.expect("Missing markdown");
    assert!(md.contains("# Tech News Daily"));
    assert!(md.contains("## Rust 2026 Released"));
    assert!(md.contains("- **URL:** https://example.com/rust-2026"));
}
