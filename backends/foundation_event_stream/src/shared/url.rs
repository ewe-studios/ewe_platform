//! Webhook uniform resource locator (URL) validation.
//!
//! The check reduces server-side request forgery (SSRF) risk. A production webhook must use
//! `https`. A development webhook may use `http` on `localhost` or `127.0.0.x`. Private,
//! link-local, loopback and other local network targets are refused.

use std::net::{Ipv4Addr, Ipv6Addr};
use url::Url;

/// Validate a webhook uniform resource locator (URL).
///
/// # Errors
///
/// Returns the reason the uniform resource locator (URL) is not safe to call.
pub fn validate_webhook_url(raw: &str) -> Result<(), String> {
    let url = Url::parse(raw).map_err(|_| "webhook.url must be a valid URL".to_string())?;
    let Some(host) = url.host_str() else {
        return Err("webhook.url must name a host".to_string());
    };
    let host = host.to_ascii_lowercase();

    match url.scheme() {
        "http" => {
            if is_local_dev_host(&host) {
                Ok(())
            } else {
                Err("http webhook URLs are only allowed for localhost or 127.0.0.x".to_string())
            }
        }
        "https" => {
            if host == "localhost" {
                return Err("localhost webhook URLs must use http for development".to_string());
            }
            if ipv4(&host).is_some_and(is_private_or_link_local_ipv4) {
                return Err("webhook.url must not target private or link-local hosts".to_string());
            }
            if is_ipv6(&host) {
                return Err("IPv6 webhook hosts are not accepted".to_string());
            }
            Ok(())
        }
        _ => Err("webhook.url must use https".to_string()),
    }
}

fn is_local_dev_host(host: &str) -> bool {
    host == "localhost" || host.starts_with("127.")
}

fn ipv4(host: &str) -> Option<Ipv4Addr> {
    host.parse::<Ipv4Addr>().ok()
}

fn is_ipv6(host: &str) -> bool {
    let trimmed = host.trim_start_matches('[').trim_end_matches(']');
    trimmed.parse::<Ipv6Addr>().is_ok()
}

fn is_private_or_link_local_ipv4(host: Ipv4Addr) -> bool {
    let [a, b, _, _] = host.octets();
    a == 10
        || a == 127
        || a == 0
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 168)
        || (a == 169 && b == 254)
}
