use rustcrawl::config::Config;
use rustcrawl::models::{OutputFormat, ScrapeOptions};
use rustcrawl::service::ScraperService;

#[tokio::test]
async fn test_parse_file_local_csv() {
    let mut temp_file = std::env::temp_dir();
    temp_file.push(format!("test_data_{}.csv", uuid::Uuid::new_v4()));

    let csv_content = "Product,Price,Quantity\nWidget A,19.99,100\nWidget B,29.99,50\n";
    tokio::fs::write(&temp_file, csv_content).await.unwrap();

    let config = Config {
        browser_enabled: false,
        ..Default::default()
    };
    let service = ScraperService::new(config).unwrap();

    let options = ScrapeOptions {
        formats: vec![OutputFormat::Markdown, OutputFormat::Tables],
        ..Default::default()
    };

    let result = service
        .parse_file(&temp_file, &options)
        .await
        .expect("Local file parsing failed");

    assert_eq!(result.content_type.as_deref(), Some("csv"));
    assert!(result.markdown.is_some());
    assert!(result.tables.is_some());

    let _ = tokio::fs::remove_file(&temp_file).await;
}

#[tokio::test]
async fn test_parse_file_local_json() {
    let mut temp_file = std::env::temp_dir();
    temp_file.push(format!("test_data_{}.json", uuid::Uuid::new_v4()));

    let json_content = r#"{"name": "RustCrawler", "version": "0.1.0", "active": true}"#;
    tokio::fs::write(&temp_file, json_content).await.unwrap();

    let config = Config {
        browser_enabled: false,
        ..Default::default()
    };
    let service = ScraperService::new(config).unwrap();

    let options = ScrapeOptions {
        formats: vec![OutputFormat::Markdown, OutputFormat::Json],
        ..Default::default()
    };

    let result = service
        .parse_file(&temp_file, &options)
        .await
        .expect("Local JSON parsing failed");

    assert_eq!(result.content_type.as_deref(), Some("json"));
    assert!(result.json.is_some());
    assert!(result.markdown.is_some());

    let _ = tokio::fs::remove_file(&temp_file).await;
}
