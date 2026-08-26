use crate::cache::store::CacheEntry;
use reqwest::header::{
    HeaderMap, HeaderValue, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED,
};
use reqwest::StatusCode;

pub struct HttpCacheUtils;

impl HttpCacheUtils {
    /// Computes a safe, collision-resistant cache key using a namespace and URL.
    pub fn compute_key(namespace: &str, url_str: &str) -> String {
        let hash = blake3::hash(url_str.as_bytes()).to_hex().to_string();
        format!("{namespace}:{hash}")
    }

    /// Injects conditional HTTP headers (If-None-Match, If-Modified-Since) if present in the cache entry.
    pub fn apply_conditional_headers(headers: &mut HeaderMap, cached: &CacheEntry) {
        if let Some(ref etag) = cached.etag {
            if let Ok(val) = HeaderValue::from_str(etag) {
                headers.insert(IF_NONE_MATCH, val);
            }
        }
        if let Some(ref last_mod) = cached.last_modified {
            if let Ok(val) = HeaderValue::from_str(last_mod) {
                headers.insert(IF_MODIFIED_SINCE, val);
            }
        }
    }

    /// Checks if an HTTP status code signifies 304 Not Modified.
    pub fn is_not_modified(status: StatusCode) -> bool {
        status == StatusCode::NOT_MODIFIED
    }

    /// Extracts ETag and Last-Modified headers from an HTTP response.
    pub fn extract_validators(headers: &HeaderMap) -> (Option<String>, Option<String>) {
        let etag = headers
            .get(ETAG)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let last_modified = headers
            .get(LAST_MODIFIED)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        (etag, last_modified)
    }
}
