use crate::models::PageImage;
use crate::parser::document::ParsedDocument;
use crate::utils::urls::resolve_relative_url;
use scraper::Selector;
use std::sync::LazyLock;

static IMG_SEL: LazyLock<Selector> = LazyLock::new(|| Selector::parse("img").unwrap());

/// Extracts image elements with metadata and resolved URLs.
pub fn extract_images(doc: &ParsedDocument) -> Vec<PageImage> {
    let mut images = Vec::new();

    for elem in doc.html.select(&IMG_SEL) {
        let value = elem.value();

        let raw_src = value
            .attr("src")
            .or_else(|| value.attr("data-src"))
            .or_else(|| value.attr("data-original"))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());

        let raw_src = match raw_src {
            Some(s) => s,
            None => continue,
        };

        let resolved_src = match resolve_relative_url(&doc.base_url, raw_src) {
            Some(u) => u.to_string(),
            None => raw_src.to_string(),
        };

        let srcset = value.attr("srcset").map(|s| s.trim().to_string());

        let data_src = value
            .attr("data-src")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .and_then(|s| resolve_relative_url(&doc.base_url, s).map(|u| u.to_string()));

        let alt = value
            .attr("alt")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let title = value
            .attr("title")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let width = value
            .attr("width")
            .and_then(|w| w.trim().parse::<u32>().ok());

        let height = value
            .attr("height")
            .and_then(|h| h.trim().parse::<u32>().ok());

        let loading = value
            .attr("loading")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        images.push(PageImage {
            src: resolved_src,
            srcset,
            data_src,
            alt,
            title,
            width,
            height,
            loading,
        });
    }

    images
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn test_extract_images() {
        let html = r#"
            <div>
                <img src="/img/logo.png" alt="Company Logo" width="200" height="50" loading="lazy">
                <img data-src="../assets/photo.jpg" title="Photo" srcset="photo-1x.jpg 1x, photo-2x.jpg 2x">
                <img src="https://cdn.example.com/banner.webp" alt="Banner">
            </div>
        "#;

        let base = Url::parse("https://example.com/blog/post1").unwrap();
        let doc = ParsedDocument::new(html, base);
        let images = extract_images(&doc);

        assert_eq!(images.len(), 3);

        assert_eq!(images[0].src, "https://example.com/img/logo.png");
        assert_eq!(images[0].alt.as_deref(), Some("Company Logo"));
        assert_eq!(images[0].width, Some(200));
        assert_eq!(images[0].height, Some(50));
        assert_eq!(images[0].loading.as_deref(), Some("lazy"));

        assert_eq!(images[1].src, "https://example.com/assets/photo.jpg");
        assert_eq!(images[1].title.as_deref(), Some("Photo"));
        assert_eq!(
            images[1].srcset.as_deref(),
            Some("photo-1x.jpg 1x, photo-2x.jpg 2x")
        );

        assert_eq!(images[2].src, "https://cdn.example.com/banner.webp");
        assert_eq!(images[2].alt.as_deref(), Some("Banner"));
    }
}
