use crate::error::CrawlerError;
use crate::fetch::validation::{is_hostname_blocked, is_ip_blocked};
use std::net::{IpAddr, SocketAddr};
use url::Url;

/// Resolves DNS for the given URL and validates that all resolved IP addresses are public and safe.
/// Returns the list of validated socket addresses.
pub async fn resolve_and_validate_url(
    url: &Url,
    allow_private: bool,
) -> Result<Vec<SocketAddr>, CrawlerError> {
    let host = url
        .host_str()
        .ok_or_else(|| CrawlerError::InvalidUrl("Missing host in URL".to_string()))?;

    if !allow_private && is_hostname_blocked(host) {
        return Err(CrawlerError::PrivateNetworkBlocked(format!(
            "Hostname '{host}' is blocked by SSRF policy"
        )));
    }

    let port = url
        .port_or_known_default()
        .unwrap_or(if url.scheme() == "https" { 443 } else { 80 });

    // If host is a raw IP literal
    if let Ok(ip) = host.parse::<IpAddr>() {
        if !allow_private && is_ip_blocked(ip) {
            return Err(CrawlerError::PrivateNetworkBlocked(format!(
                "Direct IP '{ip}' is blocked by SSRF policy"
            )));
        }
        return Ok(vec![SocketAddr::new(ip, port)]);
    }

    // Hostname resolution
    let host_with_port = format!("{host}:{port}");
    let resolved = match tokio::net::lookup_host(&host_with_port).await {
        Ok(addrs) => addrs.collect::<Vec<_>>(),
        Err(e) => {
            return Err(CrawlerError::DnsResolutionFailed(format!(
                "Failed to resolve host '{host}': {e}"
            )));
        }
    };

    if resolved.is_empty() {
        return Err(CrawlerError::DnsResolutionFailed(format!(
            "No IP addresses resolved for host '{host}'"
        )));
    }

    // Validate every resolved IP
    if !allow_private {
        for addr in &resolved {
            let ip = addr.ip();
            if is_ip_blocked(ip) {
                return Err(CrawlerError::PrivateNetworkBlocked(format!(
                    "Resolved IP '{ip}' for host '{host}' is blocked by SSRF policy"
                )));
            }
        }
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dns_blocks_localhost() {
        let url = Url::parse("http://127.0.0.1:8080").unwrap();
        let res = resolve_and_validate_url(&url, false).await;
        assert!(matches!(res, Err(CrawlerError::PrivateNetworkBlocked(_))));
    }

    #[tokio::test]
    async fn test_dns_blocks_cloud_metadata() {
        let url = Url::parse("http://169.254.169.254/latest/meta-data/").unwrap();
        let res = resolve_and_validate_url(&url, false).await;
        assert!(matches!(res, Err(CrawlerError::PrivateNetworkBlocked(_))));
    }
}
