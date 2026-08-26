use crate::models::{OpenGraphMetadata, PageMetadata, ScrapeWarning, TwitterMetadata, WarningCode};
use crate::parser::document::ParsedDocument;
use crate::utils::urls::resolve_relative_url;
use scraper::Selector;
use std::sync::LazyLock;

static TITLE_SEL: LazyLock<Selector> = LazyLock::new(|| Selector::parse("title").unwrap());
static META_SEL: LazyLock<Selector> = LazyLock::new(|| Selector::parse("meta").unwrap());
static LINK_SEL: LazyLock<Selector> = LazyLock::new(|| Selector::parse("link").unwrap());
static HTML_SEL: LazyLock<Selector> = LazyLock::new(|| Selector::parse("html").unwrap());
static TIME_SEL: LazyLock<Selector> = LazyLock::new(|| Selector::parse("time[datetime]").unwrap());
static H1_SEL: LazyLock<Selector> = LazyLock::new(|| Selector::parse("h1").unwrap());

/// Extracts comprehensive metadata from a parsed HTML document.
pub fn extract_metadata(doc: &ParsedDocument, warnings: &mut Vec<ScrapeWarning>) -> PageMetadata {
    let mut og = OpenGraphMetadata::default();
    let mut twitter = TwitterMetadata::default();

    let mut direct_title = None;
    let mut meta_description = None;
    let mut language = None;
    let mut canonical_url = None;
    let mut charset = None;
    let mut favicon = None;
    let mut author = None;
    let mut robots = None;
    let mut published_time = None;
    let mut generator = None;
    let mut theme_color = None;

    // 1. Extract <html lang="...">
    if let Some(html_elem) = doc.html.select(&HTML_SEL).next() {
        if let Some(lang) = html_elem.value().attr("lang") {
            let trimmed = lang.trim();
            if !trimmed.is_empty() {
                language = Some(trimmed.to_string());
            }
        } else if let Some(lang) = html_elem.value().attr("xml:lang") {
            let trimmed = lang.trim();
            if !trimmed.is_empty() {
                language = Some(trimmed.to_string());
            }
        }
    }

    // 2. Extract <title>
    if let Some(title_elem) = doc.html.select(&TITLE_SEL).next() {
        let text = title_elem.text().collect::<Vec<_>>().join(" ");
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            direct_title = Some(trimmed.to_string());
        }
    }

    // 3. Scan <meta> tags
    for meta in doc.html.select(&META_SEL) {
        let value = meta.value();
        let name = value.attr("name").map(|s| s.trim().to_lowercase());
        let property = value.attr("property").map(|s| s.trim().to_lowercase());
        let http_equiv = value.attr("http-equiv").map(|s| s.trim().to_lowercase());
        let content = value.attr("content").map(|s| s.trim().to_string());

        // Charset
        if let Some(cs) = value.attr("charset") {
            let trimmed = cs.trim();
            if !trimmed.is_empty() && charset.is_none() {
                charset = Some(trimmed.to_string());
            }
        }

        if let Some(equiv) = &http_equiv {
            if equiv == "content-language" && language.is_none() {
                if let Some(cnt) = &content {
                    if !cnt.trim().is_empty() {
                        language = Some(cnt.trim().to_string());
                    }
                }
            } else if equiv == "content-type" && charset.is_none() {
                if let Some(cnt) = &content {
                    for part in cnt.split(';') {
                        if let Some(rest) = part.trim().strip_prefix("charset=") {
                            let cs = rest.trim_matches('"').trim_matches('\'').trim();
                            if !cs.is_empty() {
                                charset = Some(cs.to_string());
                            }
                        }
                    }
                }
            }
        }

        if let Some(c) = content {
            let c_trimmed = c.trim().to_string();
            if c_trimmed.is_empty() {
                continue;
            }

            // Standard Meta Names
            if let Some(ref n) = name {
                match n.as_str() {
                    "description" => {
                        if meta_description.is_none() {
                            meta_description = Some(c_trimmed.clone());
                        }
                    }
                    "author" => {
                        if author.is_none() {
                            author = Some(c_trimmed.clone());
                        }
                    }
                    "robots" | "googlebot" => {
                        if robots.is_none() {
                            robots = Some(c_trimmed.clone());
                        }
                    }
                    "generator" => {
                        if generator.is_none() {
                            generator = Some(c_trimmed.clone());
                        }
                    }
                    "theme-color" => {
                        if theme_color.is_none() {
                            theme_color = Some(c_trimmed.clone());
                        }
                    }
                    "date" | "pubdate" | "publication_date" | "article:published_time" => {
                        if published_time.is_none() {
                            published_time = Some(c_trimmed.clone());
                        }
                    }
                    // Twitter Card tags
                    "twitter:card" => twitter.card = Some(c_trimmed.clone()),
                    "twitter:title" => twitter.title = Some(c_trimmed.clone()),
                    "twitter:description" => twitter.description = Some(c_trimmed.clone()),
                    "twitter:image" | "twitter:image:src" => {
                        twitter.image = resolve_relative_url(&doc.base_url, &c_trimmed)
                            .map(|u| u.to_string())
                            .or(Some(c_trimmed.clone()));
                    }
                    "twitter:site" => twitter.site = Some(c_trimmed.clone()),
                    "twitter:creator" => twitter.creator = Some(c_trimmed.clone()),
                    _ => {}
                }
            }

            // Open Graph Properties
            if let Some(ref p) = property {
                match p.as_str() {
                    "og:title" => og.title = Some(c_trimmed.clone()),
                    "og:description" => og.description = Some(c_trimmed.clone()),
                    "og:image" | "og:image:url" => {
                        og.image = resolve_relative_url(&doc.base_url, &c_trimmed)
                            .map(|u| u.to_string())
                            .or(Some(c_trimmed.clone()));
                    }
                    "og:url" => {
                        og.url = resolve_relative_url(&doc.base_url, &c_trimmed)
                            .map(|u| u.to_string())
                            .or(Some(c_trimmed.clone()));
                    }
                    "og:type" => og.og_type = Some(c_trimmed.clone()),
                    "og:site_name" => og.site_name = Some(c_trimmed.clone()),
                    "og:locale" => og.locale = Some(c_trimmed.clone()),
                    "article:published_time" | "article:modified_time"
                        if published_time.is_none() =>
                    {
                        published_time = Some(c_trimmed.clone());
                    }
                    "article:author" if author.is_none() => {
                        author = Some(c_trimmed.clone());
                    }
                    _ => {}
                }
            }
        }
    }

    // 4. Scan <link> tags (canonical, favicon)
    for link in doc.html.select(&LINK_SEL) {
        let value = link.value();
        let rel = value.attr("rel").map(|s| s.trim().to_lowercase());
        let href = value.attr("href").map(|s| s.trim());

        if let (Some(rel_val), Some(href_val)) = (rel, href) {
            if rel_val == "canonical" && canonical_url.is_none() {
                if let Some(resolved) = resolve_relative_url(&doc.base_url, href_val) {
                    canonical_url = Some(resolved.to_string());
                } else {
                    warnings.push(ScrapeWarning::new(
                        WarningCode::InvalidCanonicalUrl,
                        format!("Invalid canonical URL href: '{href_val}'"),
                    ));
                }
            } else if (rel_val == "icon"
                || rel_val == "shortcut icon"
                || rel_val == "apple-touch-icon")
                && favicon.is_none()
            {
                if let Some(resolved) = resolve_relative_url(&doc.base_url, href_val) {
                    favicon = Some(resolved.to_string());
                }
            }
        }
    }

    // 5. Fallback for published_time using <time datetime="...">
    if published_time.is_none() {
        if let Some(time_elem) = doc.html.select(&TIME_SEL).next() {
            if let Some(dt) = time_elem.value().attr("datetime") {
                let trimmed = dt.trim();
                if !trimmed.is_empty() {
                    published_time = Some(trimmed.to_string());
                }
            }
        }
    }

    // 6. Title resolution hierarchy: <title> -> og:title -> twitter:title -> <h1>
    let final_title = direct_title
        .or_else(|| og.title.clone())
        .or_else(|| twitter.title.clone())
        .or_else(|| {
            doc.html
                .select(&H1_SEL)
                .next()
                .map(|h1| h1.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .filter(|s| !s.is_empty())
        });

    if final_title.is_none() {
        warnings.push(ScrapeWarning::new(
            WarningCode::MissingTitle,
            "Document is missing a <title> element and alternative title metadata",
        ));
    }

    // 7. Fallback description: meta description -> og:description -> twitter:description
    let final_description = meta_description
        .or_else(|| og.description.clone())
        .or_else(|| twitter.description.clone());

    PageMetadata {
        title: final_title,
        description: final_description,
        language,
        canonical_url,
        charset,
        favicon,
        author,
        subject: None,
        creator: None,
        robots,
        published_time,
        modified_time: None,
        generator,
        theme_color,
        page_count: None,
        row_count: None,
        word_count: None,
        content_hash: None,
        open_graph: Some(og),
        twitter: Some(twitter),
        json_ld: Vec::new(), // Filled by structured_data extractor
        custom: std::collections::HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn test_metadata_extraction() {
        let html = r##"
            <!DOCTYPE html>
            <html lang="en">
            <head>
                <meta charset="utf-8">
                <title>Test Page Title</title>
                <meta name="description" content="This is a test description">
                <meta name="author" content="Jane Doe">
                <meta name="robots" content="index, follow">
                <meta name="generator" content="RustCrawl Generator">
                <meta name="theme-color" content="#ff5500">
                <link rel="canonical" href="/canonical-page">
                <link rel="icon" href="/favicon.ico">
                <meta property="og:title" content="OG Title">
                <meta property="og:description" content="OG Description">
                <meta property="og:image" content="/images/og.png">
                <meta property="og:url" content="https://example.com/canonical-page">
                <meta property="og:type" content="article">
                <meta property="article:published_time" content="2026-08-25T12:00:00Z">
                <meta name="twitter:card" content="summary_large_image">
                <meta name="twitter:creator" content="@janedoe">
            </head>
            <body>
                <h1>Heading 1</h1>
            </body>
            </html>
        "##;

        let base = Url::parse("https://example.com/sub/index.html").unwrap();
        let doc = ParsedDocument::new(html, base);
        let mut warnings = Vec::new();
        let meta = extract_metadata(&doc, &mut warnings);

        assert_eq!(meta.title.as_deref(), Some("Test Page Title"));
        assert_eq!(
            meta.description.as_deref(),
            Some("This is a test description")
        );
        assert_eq!(meta.language.as_deref(), Some("en"));
        assert_eq!(
            meta.canonical_url.as_deref(),
            Some("https://example.com/canonical-page")
        );
        assert_eq!(
            meta.favicon.as_deref(),
            Some("https://example.com/favicon.ico")
        );
        assert_eq!(meta.author.as_deref(), Some("Jane Doe"));
        assert_eq!(meta.robots.as_deref(), Some("index, follow"));
        assert_eq!(meta.generator.as_deref(), Some("RustCrawl Generator"));
        assert_eq!(meta.theme_color.as_deref(), Some("#ff5500"));
        assert_eq!(meta.published_time.as_deref(), Some("2026-08-25T12:00:00Z"));

        let og = meta.open_graph.unwrap();
        assert_eq!(og.title.as_deref(), Some("OG Title"));
        assert_eq!(
            og.image.as_deref(),
            Some("https://example.com/images/og.png")
        );
        assert_eq!(og.og_type.as_deref(), Some("article"));

        let tw = meta.twitter.unwrap();
        assert_eq!(tw.card.as_deref(), Some("summary_large_image"));
        assert_eq!(tw.creator.as_deref(), Some("@janedoe"));
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_missing_title_warning() {
        let html = r#"<html><body><p>No title</p></body></html>"#;
        let base = Url::parse("https://example.com").unwrap();
        let doc = ParsedDocument::new(html, base);
        let mut warnings = Vec::new();
        let meta = extract_metadata(&doc, &mut warnings);

        assert!(meta.title.is_none());
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, WarningCode::MissingTitle);
    }
}
