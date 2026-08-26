use dashmap::DashSet;
use std::collections::BTreeMap;
use url::Url;

const TRACKING_PARAMS: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "utm_id",
    "gclid",
    "fbclid",
    "msclkid",
    "mc_eid",
    "ref",
    "ref_src",
    "igshid",
    "yclid",
];

const ASSET_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "svg", "ico", "bmp", "tiff", "mp4", "webm", "mp3", "wav",
    "ogg", "zip", "rar", "7z", "tar", "gz", "woff", "woff2", "ttf", "eot", "css", "js", "map",
    "pdf", "docx", "xlsx", "pptx", "exe", "dmg", "pkg", "deb", "rpm", "iso",
];

/// Normalizes a URL for crawling and deduplication according to configurable rules.
pub fn normalize_crawl_url(
    url: &Url,
    ignore_query_parameters: bool,
    remove_tracking_parameters: bool,
) -> String {
    let mut normalized = url.clone();

    // 1. Remove fragment
    normalized.set_fragment(None);

    // 2. Remove default ports
    if (normalized.port() == Some(80) && normalized.scheme() == "http")
        || (normalized.port() == Some(443) && normalized.scheme() == "https")
    {
        let _ = normalized.set_port(None);
    }

    // 3. Normalize empty path to "/"
    if normalized.path().is_empty() {
        normalized.set_path("/");
    }

    // 4. Handle query parameters
    if ignore_query_parameters {
        normalized.set_query(None);
    } else {
        let query_pairs: Vec<(String, String)> = normalized.query_pairs().into_owned().collect();
        if query_pairs.is_empty() {
            normalized.set_query(None);
        } else {
            let mut filtered: BTreeMap<String, String> = BTreeMap::new();
            for (key, val) in query_pairs {
                let lower_key = key.to_ascii_lowercase();
                if remove_tracking_parameters && TRACKING_PARAMS.contains(&lower_key.as_str()) {
                    continue;
                }
                filtered.insert(key, val);
            }

            if filtered.is_empty() {
                normalized.set_query(None);
            } else {
                let mut serializer = url::form_urlencoded::Serializer::new(String::new());
                for (k, v) in filtered {
                    serializer.append_pair(&k, &v);
                }
                normalized.set_query(Some(&serializer.finish()));
            }
        }
    }

    normalized.to_string()
}

/// Checks if a URL points to a static media or code asset file extension, taking into account enabled document types.
pub fn is_static_asset_url_filtered(url: &Url, allowed_document_types: &[String]) -> bool {
    let path = url.path();
    if let Some(pos) = path.rfind('.') {
        let ext = &path[pos + 1..];
        if !ext.contains('/') {
            let lower = ext.to_ascii_lowercase();

            // If the extension matches an explicitly allowed document type, do not filter it!
            for dt in allowed_document_types {
                let dt_clean = dt.to_lowercase();
                if dt_clean == lower
                    || (dt_clean == "plain_text"
                        && (lower == "txt" || lower == "text" || lower == "md"))
                    || (dt_clean == "feed" && (lower == "rss" || lower == "atom"))
                {
                    return false;
                }
            }

            return ASSET_EXTENSIONS.contains(&lower.as_str());
        }
    }
    false
}

/// Checks if a URL points to a static media or code asset file extension (HTML-only default).
pub fn is_static_asset_url(url: &Url) -> bool {
    is_static_asset_url_filtered(url, &["html".to_string()])
}

/// High-performance thread-safe URL deduplicator backed by a sharded DashSet.
#[derive(Debug, Default)]
pub struct UrlDeduplicator {
    visited: DashSet<String>,
}

impl UrlDeduplicator {
    pub fn new() -> Self {
        Self {
            visited: DashSet::new(),
        }
    }

    /// Attempts to insert a normalized URL. Returns `true` if newly inserted, `false` if duplicate.
    pub fn insert(&self, normalized_url: &str) -> bool {
        self.visited.insert(normalized_url.to_string())
    }

    pub fn contains(&self, normalized_url: &str) -> bool {
        self.visited.contains(normalized_url)
    }

    pub fn len(&self) -> usize {
        self.visited.len()
    }

    pub fn is_empty(&self) -> bool {
        self.visited.is_empty()
    }

    pub fn clear(&self) {
        self.visited.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_crawl_url_basic() {
        let u = Url::parse("HTTP://Example.COM:80/docs/../docs/index.html#intro").unwrap();
        let norm = normalize_crawl_url(&u, false, false);
        assert_eq!(norm, "http://example.com/docs/index.html");
    }

    #[test]
    fn test_normalize_crawl_url_query_sorting_and_tracking() {
        let u = Url::parse("https://example.com/search?b=2&utm_source=twitter&a=1&gclid=12345")
            .unwrap();
        let norm = normalize_crawl_url(&u, false, true);
        assert_eq!(norm, "https://example.com/search?a=1&b=2");

        let norm_ignore_all = normalize_crawl_url(&u, true, true);
        assert_eq!(norm_ignore_all, "https://example.com/search");
    }

    #[test]
    fn test_is_static_asset_url() {
        let u1 = Url::parse("https://example.com/image.png").unwrap();
        let u2 = Url::parse("https://example.com/style.css").unwrap();
        let u3 = Url::parse("https://example.com/doc.pdf").unwrap();
        let u4 = Url::parse("https://example.com/about").unwrap();
        let u5 = Url::parse("https://example.com/index.html").unwrap();

        assert!(is_static_asset_url(&u1));
        assert!(is_static_asset_url(&u2));
        assert!(is_static_asset_url(&u3));
        assert!(!is_static_asset_url(&u4));
        assert!(!is_static_asset_url(&u5));

        // When PDF is allowed
        assert!(!is_static_asset_url_filtered(&u3, &["pdf".to_string()]));
    }

    #[test]
    fn test_deduplicator() {
        let dedupe = UrlDeduplicator::new();
        assert!(dedupe.insert("https://example.com/a"));
        assert!(!dedupe.insert("https://example.com/a"));
        assert!(dedupe.insert("https://example.com/b"));
        assert_eq!(dedupe.len(), 2);
    }
}
