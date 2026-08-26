use url::Url;

/// Checks if a target URL belongs to the same domain as the seed URL according to subdomain rules.
///
/// If `allow_subdomains` is false:
/// - `target_url` must have the exact same host as `seed_url` (case-insensitive).
///
/// If `allow_subdomains` is true:
/// - `target_url` host must equal `seed_url` host OR end with `.<seed_url_host>`.
/// - Prevents domain confusion like `evil-example.com` matching `example.com`.
pub fn is_same_domain(seed_url: &Url, target_url: &Url, allow_subdomains: bool) -> bool {
    let seed_host = match seed_url.host_str() {
        Some(h) => h.to_lowercase(),
        None => return false,
    };

    let target_host = match target_url.host_str() {
        Some(h) => h.to_lowercase(),
        None => return false,
    };

    if seed_host == target_host {
        return true;
    }

    if allow_subdomains {
        // Must end with .<seed_host>
        if target_host.ends_with(&format!(".{seed_host}")) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_domain_strict() {
        let seed = Url::parse("https://example.com/docs").unwrap();
        let exact = Url::parse("https://example.com/blog").unwrap();
        let exact_case = Url::parse("https://EXAMPLE.COM/about").unwrap();
        let sub = Url::parse("https://docs.example.com/start").unwrap();
        let evil = Url::parse("https://evil-example.com/").unwrap();
        let other = Url::parse("https://google.com/").unwrap();

        assert!(is_same_domain(&seed, &exact, false));
        assert!(is_same_domain(&seed, &exact_case, false));
        assert!(!is_same_domain(&seed, &sub, false));
        assert!(!is_same_domain(&seed, &evil, false));
        assert!(!is_same_domain(&seed, &other, false));
    }

    #[test]
    fn test_same_domain_allow_subdomains() {
        let seed = Url::parse("https://example.com/docs").unwrap();
        let exact = Url::parse("https://example.com/blog").unwrap();
        let sub1 = Url::parse("https://docs.example.com/start").unwrap();
        let sub2 = Url::parse("https://api.v2.example.com/").unwrap();
        let evil = Url::parse("https://evil-example.com/").unwrap();
        let evil_suffix = Url::parse("https://notexample.com/").unwrap();
        let other = Url::parse("https://google.com/").unwrap();

        assert!(is_same_domain(&seed, &exact, true));
        assert!(is_same_domain(&seed, &sub1, true));
        assert!(is_same_domain(&seed, &sub2, true));
        assert!(!is_same_domain(&seed, &evil, true));
        assert!(!is_same_domain(&seed, &evil_suffix, true));
        assert!(!is_same_domain(&seed, &other, true));
    }
}
