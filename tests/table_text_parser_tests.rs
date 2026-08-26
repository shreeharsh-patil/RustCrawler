use rustcrawl::config::Config;
use rustcrawl::detect::ContentDetector;
use rustcrawl::document::parser::{ContentParser, ParseInput};
use rustcrawl::document::parsers::csv::CsvDocumentParser;
use rustcrawl::document::parsers::text::TextDocumentParser;
use rustcrawl::models::ScrapeOptions;
use url::Url;

#[tokio::test]
async fn test_csv_table_parser_auto_delimiter_and_markdown() {
    let url = Url::parse("https://example.com/employees.csv").unwrap();
    let csv_bytes = b"ID,Name,Department,Salary\n101,John Doe,Engineering,120000\n102,Jane Smith,Design,110000\n103,Bob Wilson,Marketing,95000\n";

    let detected = ContentDetector::detect(&url, Some("text/csv"), csv_bytes);
    let parser = CsvDocumentParser::new();
    let config = Config::default();
    let options = ScrapeOptions::default();

    let input = ParseInput {
        url: &url,
        final_url: &url,
        bytes: csv_bytes,
        detected: &detected,
        options: &options,
        config: &config,
    };

    let doc = parser.parse(input).await.expect("CSV parse failed");
    assert_eq!(doc.tables.len(), 1);
    let table = &doc.tables[0];
    assert_eq!(table.headers, vec!["ID", "Name", "Department", "Salary"]);
    assert_eq!(table.rows.len(), 3);
    assert_eq!(
        table.rows[0],
        vec!["101", "John Doe", "Engineering", "120000"]
    );

    let md = doc.markdown.expect("Missing markdown table");
    assert!(md.contains("| ID | Name | Department | Salary |"));
    assert!(md.contains("| 101 | John Doe | Engineering | 120000 |"));
}

#[tokio::test]
async fn test_plain_text_parser_encoding_and_links() {
    let url = Url::parse("https://example.com/readme.txt").unwrap();
    let text_bytes = b"Welcome to the RustCrawler Universal Engine!\r\nDocumentation is available at https://docs.example.com/guide.\r\nHave fun!\r\n";

    let detected = ContentDetector::detect(&url, Some("text/plain"), text_bytes);
    let parser = TextDocumentParser::new();
    let config = Config::default();
    let options = ScrapeOptions::default();

    let input = ParseInput {
        url: &url,
        final_url: &url,
        bytes: text_bytes,
        detected: &detected,
        options: &options,
        config: &config,
    };

    let doc = parser.parse(input).await.expect("PlainText parse failed");
    let text = doc.text.expect("Missing text");
    assert!(text.contains("Welcome to the RustCrawler"));
    assert_eq!(doc.links.len(), 1);
    assert_eq!(doc.links[0].url, "https://docs.example.com/guide");
}
