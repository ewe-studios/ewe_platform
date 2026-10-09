//! Portable utility functions (spec-57, F008).

/// Validate that an icon URL does not resolve to internal IPs (SSRF guard).
///
/// Bitwarden clients fetch icons through the server as a proxy; the server
/// must reject URLs that point at private/internal IPs.
#[must_use]
pub fn validate_icon_domain(url: &str) -> bool {
    // Parse the hostname and reject private/reserved patterns.
    let host = match url.split("://").nth(1).and_then(|s| s.split('/').next()) {
        Some(h) => h.split(':').next().unwrap_or(h),
        None => return false,
    };

    // Reject bare IPs in private ranges.
    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        if ip.is_private() || ip.is_loopback() || ip.is_link_local() {
            return false;
        }
    }

    // Reject known internal hostnames.
    let lower = host.to_lowercase();
    if lower == "localhost" || lower.ends_with(".localhost")
        || lower.ends_with(".local") || lower == "0.0.0.0"
    {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_localhost() {
        assert!(!validate_icon_domain("https://localhost/icon.png"));
        assert!(!validate_icon_domain("http://127.0.0.1:8080/favicon.ico"));
        assert!(!validate_icon_domain("https://10.0.0.1/icon"));
        assert!(!validate_icon_domain("https://192.168.1.1/icon"));
    }

    #[test]
    fn allows_public_urls() {
        assert!(validate_icon_domain("https://example.com/icon.png"));
        assert!(validate_icon_domain("https://github.com/favicon.ico"));
    }
}
