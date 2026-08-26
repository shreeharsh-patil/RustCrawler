use crate::crawl::dedupe::{is_static_asset_url_filtered, normalize_crawl_url, UrlDeduplicator};
use crate::crawl::models::CrawlOptions;
use crate::fetch::validation::validate_url_host_ssrf_preflight;
use crate::utils::domain::is_same_domain;
use crate::utils::patterns::is_path_allowed;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterDecision {
    Allowed { normalized_url: String },
    Disallowed(FilterReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterReason {
    InvalidScheme,
    PrivateNetwork,
    StaticAsset,
    DomainMismatch,
    PathFiltered,
    Duplicate,
    DepthExceeded,
}

/// Evaluates whether a candidate URL should be enqueued into the crawl frontier.
pub fn evaluate_url_for_crawl(
    candidate: &Url,
    depth: u32,
    seed_url: &Url,
    options: &CrawlOptions,
    deduplicator: &UrlDeduplicator,
    allow_private: bool,
) -> FilterDecision {
    // 1. Scheme check
    if candidate.scheme() != "http" && candidate.scheme() != "https" {
        return FilterDecision::Disallowed(FilterReason::InvalidScheme);
    }

    // 2. Depth check
    if depth > options.max_depth {
        return FilterDecision::Disallowed(FilterReason::DepthExceeded);
    }

    // 3. SSRF preflight host safety
    if validate_url_host_ssrf_preflight(candidate, allow_private).is_err() {
        return FilterDecision::Disallowed(FilterReason::PrivateNetwork);
    }

    // 4. Static asset check (considering enabled document types)
    if is_static_asset_url_filtered(candidate, &options.document_types) {
        return FilterDecision::Disallowed(FilterReason::StaticAsset);
    }

    // 5. Domain & subdomain rules
    if !is_same_domain(seed_url, candidate, options.allow_subdomains) {
        return FilterDecision::Disallowed(FilterReason::DomainMismatch);
    }

    // 6. Include / Exclude path patterns
    if !is_path_allowed(
        candidate.path(),
        &options.include_paths,
        &options.exclude_paths,
    ) {
        return FilterDecision::Disallowed(FilterReason::PathFiltered);
    }

    // 7. Normalization & Deduplication
    let normalized = normalize_crawl_url(
        candidate,
        options.ignore_query_parameters,
        options.remove_tracking_parameters,
    );

    if !deduplicator.insert(&normalized) {
        return FilterDecision::Disallowed(FilterReason::Duplicate);
    }

    FilterDecision::Allowed {
        normalized_url: normalized,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_url_pipeline() {
        let seed = Url::parse("https://example.com/docs").unwrap();
        let dedupe = UrlDeduplicator::new();
        let options = CrawlOptions {
            max_depth: 2,
            include_paths: vec!["/docs/**".to_string()],
            exclude_paths: vec!["/docs/secret/**".to_string()],
            ..Default::default()
        };

        // Allowed
        let u1 = Url::parse("https://example.com/docs/getting-started").unwrap();
        assert!(matches!(
            evaluate_url_for_crawl(&u1, 1, &seed, &options, &dedupe, false),
            FilterDecision::Allowed { .. }
        ));

        // Duplicate
        assert_eq!(
            evaluate_url_for_crawl(&u1, 1, &seed, &options, &dedupe, false),
            FilterDecision::Disallowed(FilterReason::Duplicate)
        );

        // Path excluded
        let u_excluded = Url::parse("https://example.com/docs/secret/keys").unwrap();
        assert_eq!(
            evaluate_url_for_crawl(&u_excluded, 1, &seed, &options, &dedupe, false),
            FilterDecision::Disallowed(FilterReason::PathFiltered)
        );

        // Path not included
        let u_other_path = Url::parse("https://example.com/blog/post-1").unwrap();
        assert_eq!(
            evaluate_url_for_crawl(&u_other_path, 1, &seed, &options, &dedupe, false),
            FilterDecision::Disallowed(FilterReason::PathFiltered)
        );

        // Subdomain disallowed
        let u_sub = Url::parse("https://blog.example.com/docs/1").unwrap();
        assert_eq!(
            evaluate_url_for_crawl(&u_sub, 1, &seed, &options, &dedupe, false),
            FilterDecision::Disallowed(FilterReason::DomainMismatch)
        );

        // Static asset
        let u_asset = Url::parse("https://example.com/docs/logo.png").unwrap();
        assert_eq!(
            evaluate_url_for_crawl(&u_asset, 1, &seed, &options, &dedupe, false),
            FilterDecision::Disallowed(FilterReason::StaticAsset)
        );

        // Depth exceeded
        let u_depth = Url::parse("https://example.com/docs/deep").unwrap();
        assert_eq!(
            evaluate_url_for_crawl(&u_depth, 3, &seed, &options, &dedupe, false),
            FilterDecision::Disallowed(FilterReason::DepthExceeded)
        );

        // Loopback / SSRF
        let u_loopback = Url::parse("http://127.0.0.1/docs/1").unwrap();
        assert_eq!(
            evaluate_url_for_crawl(&u_loopback, 1, &seed, &options, &dedupe, false),
            FilterDecision::Disallowed(FilterReason::PrivateNetwork)
        );
    }
}
