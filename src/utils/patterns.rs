/// Matches a URL path against a glob pattern.
///
/// Supports:
/// - `*`: matches zero or more characters within a path segment (does not cross `/`)
/// - `**`: matches zero or more characters across path segments (crosses `/`)
/// - `?`: matches exactly one character
/// - Literals: exact character match
pub fn matches_glob(pattern: &str, path: &str) -> bool {
    let p_bytes = pattern.as_bytes();
    let s_bytes = path.as_bytes();

    match_helper(p_bytes, 0, s_bytes, 0)
}

fn match_helper(pattern: &[u8], p_idx: usize, path: &[u8], s_idx: usize) -> bool {
    let mut p = p_idx;
    let mut s = s_idx;

    while p < pattern.len() {
        if p + 1 < pattern.len() && pattern[p] == b'*' && pattern[p + 1] == b'*' {
            let next_p = p + 2;
            // Handle optional trailing slash or segment in **
            if next_p == pattern.len() {
                return true;
            }
            // If pattern is **/something or **something
            let skip_slash = if next_p < pattern.len() && pattern[next_p] == b'/' {
                next_p + 1
            } else {
                next_p
            };

            for cur_s in s..=path.len() {
                if match_helper(pattern, skip_slash, path, cur_s) {
                    return true;
                }
            }
            return false;
        } else if pattern[p] == b'*' {
            // Match zero or more non-slash characters
            for cur_s in s..=path.len() {
                if cur_s > s && cur_s <= path.len() && path[cur_s - 1] == b'/' {
                    break;
                }
                if match_helper(pattern, p + 1, path, cur_s) {
                    return true;
                }
            }
            return false;
        } else if pattern[p] == b'?' {
            if s >= path.len() || path[s] == b'/' {
                return false;
            }
            p += 1;
            s += 1;
        } else {
            if s >= path.len() || pattern[p] != path[s] {
                return false;
            }
            p += 1;
            s += 1;
        }
    }

    s == path.len()
}

/// Determines whether a URL path is allowed based on include and exclude glob patterns.
/// Exclude patterns take strict precedence over include patterns.
pub fn is_path_allowed(
    path: &str,
    include_patterns: &[String],
    exclude_patterns: &[String],
) -> bool {
    let normalized_path = if path.starts_with('/') {
        path
    } else {
        // Ensure path starts with /
        return is_path_allowed(&format!("/{path}"), include_patterns, exclude_patterns);
    };

    // 1. Check exclusions first
    for exclude in exclude_patterns {
        let pattern = exclude.trim();
        if pattern.is_empty() {
            continue;
        }
        if matches_glob(pattern, normalized_path) {
            return false;
        }
    }

    // 2. If no include patterns specified, everything not excluded is allowed
    if include_patterns.is_empty() {
        return true;
    }

    // 3. Must match at least one include pattern
    for include in include_patterns {
        let pattern = include.trim();
        if pattern.is_empty() {
            continue;
        }
        if matches_glob(pattern, normalized_path) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_exact_match() {
        assert!(matches_glob("/docs", "/docs"));
        assert!(!matches_glob("/docs", "/docs/"));
        assert!(matches_glob("/docs/", "/docs/"));
    }

    #[test]
    fn test_glob_single_star() {
        assert!(matches_glob("/docs/*", "/docs/start"));
        assert!(matches_glob("/docs/*", "/docs/api"));
        assert!(!matches_glob("/docs/*", "/docs/api/v1"));
        assert!(matches_glob("/blog/*.html", "/blog/post.html"));
        assert!(!matches_glob("/blog/*.html", "/blog/dir/post.html"));
    }

    #[test]
    fn test_glob_double_star() {
        assert!(matches_glob("/docs/**", "/docs/"));
        assert!(matches_glob("/docs/**", "/docs/start"));
        assert!(matches_glob("/docs/**", "/docs/a/b/c/d"));
        assert!(matches_glob("/**", "/anything/here"));
        assert!(matches_glob("**/admin/**", "/app/admin/dashboard"));
        assert!(!matches_glob("/docs/**", "/other/page"));
    }

    #[test]
    fn test_path_filter_priority() {
        let includes = vec!["/docs/**".to_string(), "/blog/**".to_string()];
        let excludes = vec!["/docs/private/**".to_string(), "**/secret".to_string()];

        assert!(is_path_allowed(
            "/docs/getting-started",
            &includes,
            &excludes
        ));
        assert!(is_path_allowed(
            "/blog/2026/08/post-1",
            &includes,
            &excludes
        ));

        // Excluded takes precedence
        assert!(!is_path_allowed(
            "/docs/private/admin",
            &includes,
            &excludes
        ));
        assert!(!is_path_allowed("/docs/secret", &includes, &excludes));

        // Not included
        assert!(!is_path_allowed("/about", &includes, &excludes));
    }

    #[test]
    fn test_empty_includes_allows_all_except_excludes() {
        let includes = vec![];
        let excludes = vec!["/admin/**".to_string(), "/login".to_string()];

        assert!(is_path_allowed("/home", &includes, &excludes));
        assert!(is_path_allowed("/docs/api", &includes, &excludes));
        assert!(!is_path_allowed("/admin/users", &includes, &excludes));
        assert!(!is_path_allowed("/login", &includes, &excludes));
    }
}
