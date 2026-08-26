use crate::models::Heading;
use crate::parser::document::ParsedDocument;
use scraper::Selector;
use std::sync::LazyLock;

static HEADING_SEL: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("h1, h2, h3, h4, h5, h6").unwrap());

/// Extracts ordered headings (h1-h6) from the document with their text and optional ID.
pub fn extract_headings(doc: &ParsedDocument) -> Vec<Heading> {
    let mut headings = Vec::new();

    for elem in doc.html.select(&HEADING_SEL) {
        let tag_name = elem.value().name();
        let level = match tag_name {
            "h1" => 1,
            "h2" => 2,
            "h3" => 3,
            "h4" => 4,
            "h5" => 5,
            "h6" => 6,
            _ => continue,
        };

        let text = elem.text().collect::<Vec<_>>().join(" ");
        let trimmed_text = text.trim();
        if trimmed_text.is_empty() {
            continue;
        }

        let id = elem
            .value()
            .attr("id")
            .or_else(|| elem.value().attr("name"))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        headings.push(Heading {
            level,
            text: trimmed_text.to_string(),
            id,
        });
    }

    headings
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn test_extract_headings() {
        let html = r#"
            <div>
                <h1 id="main-title">Main Title</h1>
                <p>Some content</p>
                <h2>Section 1</h2>
                <h3>Subsection 1.1</h3>
                <h6>Deep Note</h6>
            </div>
        "#;

        let base = Url::parse("https://example.com").unwrap();
        let doc = ParsedDocument::new(html, base);
        let headings = extract_headings(&doc);

        assert_eq!(headings.len(), 4);
        assert_eq!(headings[0].level, 1);
        assert_eq!(headings[0].text, "Main Title");
        assert_eq!(headings[0].id.as_deref(), Some("main-title"));

        assert_eq!(headings[1].level, 2);
        assert_eq!(headings[1].text, "Section 1");
        assert_eq!(headings[1].id, None);

        assert_eq!(headings[2].level, 3);
        assert_eq!(headings[3].level, 6);
    }
}
