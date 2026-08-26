use crate::error::CrawlerError;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use url::Url;

/// Checks if an IPv4 address is in a private, loopback, link-local, multicast, or reserved range.
pub fn is_ipv4_blocked(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();

    // 0.0.0.0/8 (Current network / Unspecified)
    if octets[0] == 0 {
        return true;
    }

    // 10.0.0.0/8 (Private network RFC 1918)
    if octets[0] == 10 {
        return true;
    }

    // 100.64.0.0/10 (Shared Address Space / CGNAT RFC 6598)
    if octets[0] == 100 && (octets[1] & 0xC0) == 64 {
        return true;
    }

    // 127.0.0.0/8 (Loopback RFC 1122)
    if octets[0] == 127 {
        return true;
    }

    // 169.254.0.0/16 (Link Local RFC 3927 - includes 169.254.169.254 cloud metadata)
    if octets[0] == 169 && octets[1] == 254 {
        return true;
    }

    // 172.16.0.0/12 (Private network RFC 1918)
    if octets[0] == 172 && (octets[1] >= 16 && octets[1] <= 31) {
        return true;
    }

    // 192.0.0.0/24 (IETF Protocol Assignments RFC 6890)
    if octets[0] == 192 && octets[1] == 0 && octets[2] == 0 {
        return true;
    }

    // 192.0.2.0/24 (TEST-NET-1 RFC 5737)
    if octets[0] == 192 && octets[1] == 0 && octets[2] == 2 {
        return true;
    }

    // 192.168.0.0/16 (Private network RFC 1918)
    if octets[0] == 192 && octets[1] == 168 {
        return true;
    }

    // 198.18.0.0/15 (Benchmarking RFC 2544)
    if octets[0] == 198 && (octets[1] == 18 || octets[1] == 19) {
        return true;
    }

    // 198.51.100.0/24 (TEST-NET-2 RFC 5737)
    if octets[0] == 198 && octets[1] == 51 && octets[2] == 100 {
        return true;
    }

    // 203.0.113.0/24 (TEST-NET-3 RFC 5737)
    if octets[0] == 203 && octets[1] == 0 && octets[2] == 113 {
        return true;
    }

    // 224.0.0.0/4 (Multicast RFC 5771)
    if octets[0] >= 224 && octets[0] <= 239 {
        return true;
    }

    // 240.0.0.0/4 (Reserved RFC 1112)
    if octets[0] >= 240 {
        return true;
    }

    // 255.255.255.255 (Broadcast)
    if ip.is_broadcast() {
        return true;
    }

    false
}

/// Checks if an IPv6 address is in a private, loopback, link-local, multicast, or reserved range.
pub fn is_ipv6_blocked(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    let octets = ip.octets();

    // :: (Unspecified)
    if ip.is_unspecified() {
        return true;
    }

    // ::1 (Loopback)
    if ip.is_loopback() {
        return true;
    }

    // Check for IPv4-mapped IPv6 addresses (::ffff:0:0/96 or ::ffff:a.b.c.d)
    if let Some(ipv4) = ip.to_ipv4_mapped() {
        return is_ipv4_blocked(ipv4);
    }
    // Also check standard IPv4-compatible (deprecated ::a.b.c.d)
    if segments[0] == 0
        && segments[1] == 0
        && segments[2] == 0
        && segments[3] == 0
        && segments[4] == 0
        && segments[5] == 0
    {
        let ipv4 = Ipv4Addr::new(octets[12], octets[13], octets[14], octets[15]);
        return is_ipv4_blocked(ipv4);
    }

    // fc00::/7 (Unique Local Address - ULA RFC 4193)
    if (octets[0] & 0xFE) == 0xFC {
        return true;
    }

    // fe80::/10 (Link-Local Unicast RFC 4291)
    if octets[0] == 0xFE && (octets[1] & 0xC0) == 0x80 {
        return true;
    }

    // 2001:db8::/32 (Documentation RFC 3849)
    if segments[0] == 0x2001 && segments[1] == 0x0DB8 {
        return true;
    }

    // ff00::/8 (Multicast)
    if ip.is_multicast() {
        return true;
    }

    false
}

/// Checks if an IP address is blocked under SSRF policies.
pub fn is_ip_blocked(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_ipv4_blocked(v4),
        IpAddr::V6(v6) => is_ipv6_blocked(v6),
    }
}

/// Checks if a hostname matches well-known local/internal/metadata hostnames.
pub fn is_hostname_blocked(host: &str) -> bool {
    let lower = host.trim().to_lowercase();

    if lower == "localhost"
        || lower.ends_with(".localhost")
        || lower == "local"
        || lower.ends_with(".local")
        || lower.ends_with(".internal")
        || lower == "metadata.google.internal"
        || lower == "instance-data"
    {
        return true;
    }

    false
}

/// Validates that a parsed URL does not violate basic static SSRF / host rules.
pub fn validate_url_host_ssrf_preflight(
    url: &Url,
    allow_private: bool,
) -> Result<(), CrawlerError> {
    if allow_private {
        return Ok(());
    }

    let host_str = url
        .host_str()
        .ok_or_else(|| CrawlerError::InvalidUrl("Missing host".to_string()))?;

    if is_hostname_blocked(host_str) {
        return Err(CrawlerError::PrivateNetworkBlocked(format!(
            "Hostname '{host_str}' is blocked by SSRF protection"
        )));
    }

    // If host is a literal IP address, validate it immediately
    if let Ok(ip) = host_str.parse::<IpAddr>() {
        if is_ip_blocked(ip) {
            return Err(CrawlerError::PrivateNetworkBlocked(format!(
                "IP '{ip}' is in a blocked private/reserved range"
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipv4_blocking() {
        // Loopback
        assert!(is_ipv4_blocked(Ipv4Addr::new(127, 0, 0, 1)));
        assert!(is_ipv4_blocked(Ipv4Addr::new(127, 255, 255, 254)));

        // Private RFC 1918
        assert!(is_ipv4_blocked(Ipv4Addr::new(10, 0, 0, 1)));
        assert!(is_ipv4_blocked(Ipv4Addr::new(172, 16, 0, 1)));
        assert!(is_ipv4_blocked(Ipv4Addr::new(172, 31, 255, 255)));
        assert!(!is_ipv4_blocked(Ipv4Addr::new(172, 32, 0, 1))); // Public
        assert!(is_ipv4_blocked(Ipv4Addr::new(192, 168, 1, 1)));

        // Link local & cloud metadata
        assert!(is_ipv4_blocked(Ipv4Addr::new(169, 254, 169, 254)));
        assert!(is_ipv4_blocked(Ipv4Addr::new(169, 254, 1, 1)));

        // Current network / Unspecified
        assert!(is_ipv4_blocked(Ipv4Addr::new(0, 0, 0, 0)));

        // Public IPs should NOT be blocked
        assert!(!is_ipv4_blocked(Ipv4Addr::new(8, 8, 8, 8)));
        assert!(!is_ipv4_blocked(Ipv4Addr::new(1, 1, 1, 1)));
        assert!(!is_ipv4_blocked(Ipv4Addr::new(93, 184, 216, 34))); // example.com
    }

    #[test]
    fn test_ipv6_blocking() {
        // Loopback
        assert!(is_ipv6_blocked(Ipv6Addr::LOCALHOST));
        // Unspecified
        assert!(is_ipv6_blocked(Ipv6Addr::UNSPECIFIED));
        // ULA fc00::/7
        assert!(is_ipv6_blocked("fc00::1".parse::<Ipv6Addr>().unwrap()));
        assert!(is_ipv6_blocked(
            "fd12:3456:789a::1".parse::<Ipv6Addr>().unwrap()
        ));
        // Link-local fe80::/10
        assert!(is_ipv6_blocked("fe80::1".parse::<Ipv6Addr>().unwrap()));
        // IPv4-mapped private IP
        assert!(is_ipv6_blocked(
            "::ffff:127.0.0.1".parse::<Ipv6Addr>().unwrap()
        ));
        assert!(is_ipv6_blocked(
            "::ffff:10.0.0.1".parse::<Ipv6Addr>().unwrap()
        ));
        assert!(is_ipv6_blocked(
            "::ffff:192.168.1.1".parse::<Ipv6Addr>().unwrap()
        ));
        // IPv4-mapped public IP
        assert!(!is_ipv6_blocked(
            "::ffff:8.8.8.8".parse::<Ipv6Addr>().unwrap()
        ));
        // Public IPv6
        assert!(!is_ipv6_blocked(
            "2606:4700:4700::1111".parse::<Ipv6Addr>().unwrap()
        ));
    }

    #[test]
    fn test_hostname_blocking() {
        assert!(is_hostname_blocked("localhost"));
        assert!(is_hostname_blocked("foo.localhost"));
        assert!(is_hostname_blocked("metadata.google.internal"));
        assert!(is_hostname_blocked("instance-data"));
        assert!(!is_hostname_blocked("example.com"));
        assert!(!is_hostname_blocked("google.com"));
    }
}
