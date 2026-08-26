use rustcrawl::agent::policy::AgentPolicy;

#[test]
fn test_security_blocks_private_ipv4_addresses() {
    let private_ips = vec![
        "http://127.0.0.1:8080/secret",
        "http://10.0.0.1/admin",
        "http://192.168.1.1/router",
        "http://172.16.0.1/internal",
        "http://169.254.169.254/latest/meta-data",
        "http://0.0.0.0:3000",
    ];

    for ip_url in private_ips {
        let result = AgentPolicy::validate_url(ip_url, false);
        assert!(
            result.is_err(),
            "Expected private URL '{ip_url}' to be rejected by AgentPolicy"
        );
    }
}

#[test]
fn test_security_blocks_localhost_and_internal_hostnames() {
    let internal_hosts = vec![
        "http://localhost:3000/api",
        "http://sub.localhost:8080",
        "http://server.local/info",
        "http://db.internal/query",
        "http://metadata.google.internal/computeMetadata/v1",
    ];

    for host_url in internal_hosts {
        let result = AgentPolicy::validate_url(host_url, false);
        assert!(
            result.is_err(),
            "Expected internal host URL '{host_url}' to be rejected by AgentPolicy"
        );
    }
}

#[test]
fn test_security_blocks_unsupported_schemes() {
    let invalid_schemes = vec![
        "file:///etc/passwd",
        "ftp://ftp.example.com/files",
        "gopher://gopher.example.com",
        "javascript:alert(1)",
        "data:text/html,<h1>test</h1>",
    ];

    for scheme_url in invalid_schemes {
        let result = AgentPolicy::validate_url(scheme_url, false);
        assert!(
            result.is_err(),
            "Expected scheme in '{scheme_url}' to be rejected"
        );
    }
}

#[test]
fn test_security_permits_valid_public_urls() {
    let valid_urls = vec![
        "https://example.com",
        "https://rust-lang.org/learn",
        "https://docs.rs/serde/latest/serde/",
        "http://info.cern.ch",
    ];

    for public_url in valid_urls {
        let result = AgentPolicy::validate_url(public_url, false);
        assert!(
            result.is_ok(),
            "Expected public URL '{public_url}' to be permitted: {:?}",
            result.err()
        );
    }
}
