use crate::error::CrawlerError;
use crate::fetch::validation::validate_url_host_ssrf_preflight;
use url::Url;

/// Validates top-level navigation URL before browser navigation.
pub fn validate_browser_navigation_url(url: &Url, allow_private: bool) -> Result<(), CrawlerError> {
    // 1. Scheme check
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(CrawlerError::UnsupportedScheme(format!(
            "Unsupported browser navigation scheme '{}'. Only http:// and https:// are permitted.",
            url.scheme()
        )));
    }

    // 2. SSRF preflight
    validate_url_host_ssrf_preflight(url, allow_private)?;

    Ok(())
}

/// Checks if a subresource request URL should be blocked by browser security policies.
pub fn is_subresource_url_blocked(url_str: &str, allow_private: bool) -> bool {
    let url = match Url::parse(url_str) {
        Ok(u) => u,
        Err(_) => return true,
    };

    // Block non-web schemes
    if url.scheme() != "http" && url.scheme() != "https" && url.scheme() != "data" {
        return true;
    }

    if url.scheme() == "data" {
        return false;
    }

    if allow_private {
        return false;
    }

    // SSRF preflight for subresources
    validate_url_host_ssrf_preflight(&url, false).is_err()
}
