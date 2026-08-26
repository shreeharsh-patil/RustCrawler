use crate::models::{ScrapeWarning, WarningCode};
use crate::parser::document::ParsedDocument;
use scraper::Selector;
use std::sync::LazyLock;

static JSONLD_SEL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"script[type*="application/ld+json"]"#).unwrap());

/// Extracts and parses JSON-LD structured data blocks from `<script type="application/ld+json">`.
pub fn extract_json_ld(
    doc: &ParsedDocument,
    warnings: &mut Vec<ScrapeWarning>,
) -> Vec<serde_json::Value> {
    let mut structured_data = Vec::new();

    for elem in doc.html.select(&JSONLD_SEL) {
        let raw_text = elem.text().collect::<Vec<_>>().join("");
        let trimmed = raw_text.trim();
        if trimmed.is_empty() {
            continue;
        }

        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(value) => {
                match value {
                    serde_json::Value::Array(arr) => {
                        structured_data.extend(arr);
                    }
                    serde_json::Value::Object(obj) => {
                        // Check if it's a top-level @graph container
                        if let Some(serde_json::Value::Array(graph)) = obj.get("@graph") {
                            // Include individual graph items while preserving the top container
                            structured_data.push(serde_json::Value::Object(obj.clone()));
                            for item in graph {
                                if !structured_data.contains(item) {
                                    structured_data.push(item.clone());
                                }
                            }
                        } else {
                            structured_data.push(serde_json::Value::Object(obj));
                        }
                    }
                    other => {
                        structured_data.push(other);
                    }
                }
            }
            Err(err) => {
                warnings.push(ScrapeWarning::new(
                    WarningCode::InvalidJsonLd,
                    format!("Malformed JSON-LD content: {err}"),
                ));
            }
        }
    }

    structured_data
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn test_extract_json_ld_valid() {
        let html = r#"
            <html>
            <head>
                <script type="application/ld+json">
                {
                    "@context": "https://schema.org",
                    "@type": "Article",
                    "headline": "Rust Web Scraping",
                    "author": {
                        "@type": "Person",
                        "name": "Jane Developer"
                    }
                }
                </script>
                <script type="application/ld+json">
                [
                    {
                        "@context": "https://schema.org",
                        "@type": "BreadcrumbList"
                    }
                ]
                </script>
            </head>
            <body></body>
            </html>
        "#;

        let base = Url::parse("https://example.com").unwrap();
        let doc = ParsedDocument::new(html, base);
        let mut warnings = Vec::new();
        let json_lds = extract_json_ld(&doc, &mut warnings);

        assert_eq!(json_lds.len(), 2);
        assert_eq!(json_lds[0]["headline"], "Rust Web Scraping");
        assert_eq!(json_lds[1]["@type"], "BreadcrumbList");
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_extract_json_ld_invalid() {
        let html = r#"
            <html>
            <head>
                <script type="application/ld+json">
                { "broken": json here, }
                </script>
            </head>
            <body></body>
            </html>
        "#;

        let base = Url::parse("https://example.com").unwrap();
        let doc = ParsedDocument::new(html, base);
        let mut warnings = Vec::new();
        let json_lds = extract_json_ld(&doc, &mut warnings);

        assert!(json_lds.is_empty());
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, WarningCode::InvalidJsonLd);
    }
}
