//! Portable utility functions (spec-57, F008).

/// Validate that an icon URL does not resolve to internal IPs (SSRF guard).
///
/// Bitwarden clients fetch icons through the server as a proxy; the server
/// must reject URLs that point at private/internal IPs.
#[must_use]
// The suffixes are hostname labels, not file extensions.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
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
    !(lower == "localhost"
        || lower.ends_with(".localhost")
        || lower.ends_with(".local")
        || lower == "0.0.0.0")
}
