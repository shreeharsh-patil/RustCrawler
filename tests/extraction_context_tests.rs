use rustcrawl::extraction::context::ContextSelector;
use rustcrawl::models::{ScrapeResult, WarningCode};
use rustcrawl::DocumentTable;
use serde_json::json;

fn mock_scrape_result_for_context() -> ScrapeResult {
    let md = "# Technical Specifications\n\n\
    The device features a 6.7-inch OLED display running at 120Hz refresh rate.\n\n\
    ## Battery and Charging\n\n\
    Equipped with a 5000mAh battery that supports 65W fast charging.\n\n\
    ## Pricing and Availability\n\n\
    Available for pre-order starting at $899 in the United States and Europe.";

    let table = DocumentTable {
        name: None,
        headers: vec!["Component".to_string(), "Specification".to_string()],
        rows: vec![
            vec!["Processor".to_string(), "Octa-core 3.2GHz".to_string()],
            vec!["RAM".to_string(), "16GB LPDDR5X".to_string()],
        ],
    };

    ScrapeResult {
        url: "https://reviews.example.com/phone-review".to_string(),
        final_url: "https://reviews.example.com/phone-review".to_string(),
        content_type: Some("text/html".to_string()),
        content_hash: Some("mock_phone_hash".to_string()),
        markdown: Some(md.to_string()),
        text: Some(md.to_string()),
        html: None,
        clean_html: None,
        json: None,
        metadata: None,
        headings: None,
        links: None,
        images: None,
        tables: Some(vec![table]),
        pages: None,
        http: rustcrawl::models::HttpMetadata {
            status: 200,
            content_type: "text/html".to_string(),
            content_length: 1200,
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
fn test_context_selector_chunks_and_ranks_by_relevance() {
    let result = mock_scrape_result_for_context();
    let schema = json!({
        "type": "object",
        "properties": {
            "battery_capacity": { "type": "string" },
            "charging_speed": { "type": "string" }
        }
    });

    let prompt = "Extract the battery capacity and charging speed";

    let (chunks, warnings, total_chars) =
        ContextSelector::select_context(&[result], Some(&schema), Some(prompt), 100000, 50, 12000);

    assert!(!chunks.is_empty());
    assert!(warnings.is_empty());
    assert!(total_chars > 0);

    // The top chunk should be the Battery and Charging section
    let top_chunk = &chunks[0];
    let top_headings = top_chunk.heading_path.join(" ");
    assert!(
        top_headings.contains("Battery")
            || top_chunk.content.contains("battery")
            || top_chunk.content.contains("Charging"),
        "Top chunk was: {:?}",
        top_chunk
    );
}

#[test]
fn test_context_selector_budget_truncation_and_warning() {
    let result = mock_scrape_result_for_context();
    let schema = json!({ "type": "object" });

    // Restrict context to 100 characters maximum
    let (chunks, warnings, total_chars) =
        ContextSelector::select_context(&[result], Some(&schema), None, 100, 50, 12000);

    assert!(!chunks.is_empty());
    assert!(total_chars <= 100);
    assert!(warnings
        .iter()
        .any(|w| w.code == WarningCode::ExtractionContextTruncated));
}
