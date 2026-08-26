use crate::models::{PageLink, ScrapeWarning, WarningCode};
use crate::parser::document::ParsedDocument;
use crate::utils::urls::{
    is_external_url, is_ignorable_href_scheme, normalize_url, resolve_relative_url,
};
use scraper::Selector;
use std::collections::HashSet;
use std::sync::LazyLock;

static A_HREF_SEL: LazyLock<Selector> = LazyLock::new(|| Selector::parse("a[href]").unwrap());

/// Extracts, resolves, normalizes, and deduplicates all crawlable links from `<a href="...">` tags.
pub fn extract_links(doc: &ParsedDocument, warnings: &mut Vec<ScrapeWarning>) -> Vec<PageLink> {
    let mut links = Vec::new();
    let mut seen_urls = HashSet::new();

    for elem in doc.html.select(&A_HREF_SEL) {
        let value = elem.value();
        let raw_href = match value.attr("href") {
            Some(h) => h.trim(),
            None => continue,
        };

        if raw_href.is_empty() || is_ignorable_href_scheme(raw_href) {
            continue;
        }

        let resolved_url = match resolve_relative_url(&doc.base_url, raw_href) {
            Some(u) => u,
            None => {
                warnings.push(ScrapeWarning::new(
                    WarningCode::MalformedLink,
                    format!("Failed to resolve relative link: '{raw_href}'"),
                ));
                continue;
            }
        };

        let normalized = normalize_url(&resolved_url);
        if !seen_urls.insert(normalized.clone()) {
            // Already seen this URL, skip deduplicated
            continue;
        }

        let text = elem.text().collect::<Vec<_>>().join(" ");
        let trimmed_text = text.trim().to_string();

        let rel: Vec<String> = value
            .attr("rel")
            .map(|r| r.split_whitespace().map(|s| s.to_string()).collect())
            .unwrap_or_default();

        let is_external = is_external_url(&doc.base_url, &resolved_url);

        links.push(PageLink {
            text: trimmed_text,
            url: normalized,
            rel,
            is_external,
        });
    }

    links
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn test_extract_links() {
        let html = r#"
            <div>
                <a href="/about" rel="nofollow noopener">About Us</a>
                <a href="../contact.html">Contact</a>
                <a href="https://other.com/docs" rel="external">Documentation</a>
                <a href="javascript:void(0)">Click me</a>
                <a href="mailto:info@example.com">Email</a>
                <a href="/about">About Us Duplicate</a>
                <a href="https://example.com/blog?page=1#top">Blog</a>
            </div>
        "#;

        let base = Url::parse("https://example.com/pages/home").unwrap();
        let doc = ParsedDocument::new(html, base);
        let mut warnings = Vec::new();
        let links = extract_links(&doc, &mut warnings);

        assert_eq!(links.len(), 4);

        // Link 0: /about
        assert_eq!(links[0].url, "https://example.com/about");
        assert_eq!(links[0].text, "About Us");
        assert_eq!(links[0].rel, vec!["nofollow", "noopener"]);
        assert!(!links[0].is_external);

        // Link 1: ../contact.html -> https://example.com/contact.html
        assert_eq!(links[1].url, "https://example.com/contact.html");
        assert_eq!(links[1].text, "Contact");
        assert!(!links[1].is_external);

        // Link 2: https://other.com/docs
        assert_eq!(links[2].url, "https://other.com/docs");
        assert!(links[2].is_external);
        assert_eq!(links[2].rel, vec!["external"]);

        // Link 3: https://example.com/blog?page=1#top
        assert_eq!(links[3].url, "https://example.com/blog?page=1#top");
        assert!(!links[3].is_external);
    }
}
