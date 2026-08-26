use scraper::Html;
use url::Url;

/// Represents a parsed HTML document backed by a DOM tree.
pub struct ParsedDocument {
    pub html: Html,
    pub base_url: Url,
}

impl ParsedDocument {
    pub fn new(html_content: &str, base_url: Url) -> Self {
        let html = Html::parse_document(html_content);
        Self { html, base_url }
    }
}
