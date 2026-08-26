use crate::error::CrawlerError;
use url::Url;

/// Validates raw URL string syntax and scheme.
/// Only `http://` and `https://` schemes are permitted.
pub fn validate_url_syntax(raw_url: &str) -> Result<Url, CrawlerError> {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return Err(CrawlerError::InvalidUrl("URL cannot be empty".to_string()));
    }

    // Explicit check for dangerous / unsupported schemes before url parser
    let lower = trimmed.to_lowercase();
    if lower.starts_with("file:")
        || lower.starts_with("ftp:")
        || lower.starts_with("data:")
        || lower.starts_with("javascript:")
        || lower.starts_with("chrome:")
        || lower.starts_with("about:")
        || lower.starts_with("blob:")
    {
        let scheme = lower.split(':').next().unwrap_or("unknown");
        return Err(CrawlerError::UnsupportedScheme(scheme.to_string()));
    }

    let parsed = Url::parse(trimmed).map_err(|e| CrawlerError::InvalidUrl(e.to_string()))?;

    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(CrawlerError::UnsupportedScheme(scheme.to_string()));
    }

    if parsed.host().is_none() {
        return Err(CrawlerError::InvalidUrl(
            "URL must contain a valid host name".to_string(),
        ));
    }

    Ok(parsed)
}

/// Resolves a potentially relative URL string against a base URL.
/// Returns None if the URL uses an ignorable or unsupported non-HTTP scheme.
pub fn resolve_relative_url(base_url: &Url, relative_or_absolute: &str) -> Option<Url> {
    let trimmed = relative_or_absolute.trim();
    if trimmed.is_empty() {
        return None;
    }

    if is_ignorable_href_scheme(trimmed) {
        return None;
    }

    base_url.join(trimmed).ok()
}

/// Determines whether a scheme is ignorable for crawlable links (mailto, tel, javascript, etc.)
pub fn is_ignorable_href_scheme(href: &str) -> bool {
    let lower = href.trim_start().to_lowercase();
    lower.starts_with("javascript:")
        || lower.starts_with("mailto:")
        || lower.starts_with("tel:")
        || lower.starts_with("data:")
        || lower.starts_with("sms:")
        || lower.starts_with("callto:")
        || lower.starts_with("fax:")
        || lower.starts_with("magnet:")
        || lower.starts_with('#') && lower.len() == 1
}

/// Determines if a target URL belongs to an external host compared to the base URL.
pub fn is_external_url(base_url: &Url, target_url: &Url) -> bool {
    match (base_url.host_str(), target_url.host_str()) {
        (Some(b), Some(t)) => !b.eq_ignore_ascii_case(t),
        _ => true,
    }
}

/// Normalizes a URL for deduplication and consistent representation.
pub fn normalize_url(url: &Url) -> String {
    let mut normalized = url.clone();
    // Strip trailing empty fragments or default ports
    if (normalized.port() == Some(80) && normalized.scheme() == "http")
        || (normalized.port() == Some(443) && normalized.scheme() == "https")
    {
        let _ = normalized.set_port(None);
    }
    normalized.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_url_syntax_valid() {
        assert!(validate_url_syntax("https://example.com").is_ok());
        assert!(validate_url_syntax("http://example.com/test?a=1&b=2#section").is_ok());
        assert!(validate_url_syntax("  https://sub.domain.org/path/  ").is_ok());
    }

    #[test]
    fn test_validate_url_syntax_unsupported_schemes() {
        assert!(matches!(
            validate_url_syntax("ftp://example.com"),
            Err(CrawlerError::UnsupportedScheme(_))
        ));
        assert!(matches!(
            validate_url_syntax("file:///etc/passwd"),
            Err(CrawlerError::UnsupportedScheme(_))
        ));
        assert!(matches!(
            validate_url_syntax("data:text/html,<h1>Hello</h1>"),
            Err(CrawlerError::UnsupportedScheme(_))
        ));
        assert!(matches!(
            validate_url_syntax("javascript:alert(1)"),
            Err(CrawlerError::UnsupportedScheme(_))
        ));
        assert!(matches!(
            validate_url_syntax("chrome://settings"),
            Err(CrawlerError::UnsupportedScheme(_))
        ));
    }

    #[test]
    fn test_validate_url_syntax_invalid() {
        assert!(matches!(
            validate_url_syntax("not-a-url"),
            Err(CrawlerError::InvalidUrl(_))
        ));
        assert!(matches!(
            validate_url_syntax(""),
            Err(CrawlerError::InvalidUrl(_))
        ));
        assert!(matches!(
            validate_url_syntax("   "),
            Err(CrawlerError::InvalidUrl(_))
        ));
    }

    #[test]
    fn test_resolve_relative_url() {
        let base = Url::parse("https://example.com/articles/2026/test.html?v=1").unwrap();

        assert_eq!(
            resolve_relative_url(&base, "/about").unwrap().as_str(),
            "https://example.com/about"
        );
        assert_eq!(
            resolve_relative_url(&base, "../other.html")
                .unwrap()
                .as_str(),
            "https://example.com/articles/other.html"
        );
        assert_eq!(
            resolve_relative_url(&base, "?v=2").unwrap().as_str(),
            "https://example.com/articles/2026/test.html?v=2"
        );
        assert_eq!(
            resolve_relative_url(&base, "#comments").unwrap().as_str(),
            "https://example.com/articles/2026/test.html?v=1#comments"
        );
        assert_eq!(
            resolve_relative_url(&base, "//cdn.example.com/lib.js")
                .unwrap()
                .as_str(),
            "https://cdn.example.com/lib.js"
        );

        assert!(resolve_relative_url(&base, "javascript:void(0)").is_none());
        assert!(resolve_relative_url(&base, "mailto:user@example.com").is_none());
        assert!(resolve_relative_url(&base, "tel:+1234567890").is_none());
    }

    #[test]
    fn test_is_external_url() {
        let base = Url::parse("https://example.com/page").unwrap();
        let internal = Url::parse("https://example.com/other").unwrap();
        let internal_case = Url::parse("https://EXAMPLE.COM/other").unwrap();
        let external = Url::parse("https://sub.example.com/other").unwrap();
        let external2 = Url::parse("https://another.org").unwrap();

        assert!(!is_external_url(&base, &internal));
        assert!(!is_external_url(&base, &internal_case));
        assert!(is_external_url(&base, &external));
        assert!(is_external_url(&base, &external2));
    }
}
