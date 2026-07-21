//! Icons API — SSRF-guarded favicon proxy (spec-57, F008 Stage 1).
//!
//! WHY: Bitwarden clients fetch site icons through the server so the client never
//! talks to arbitrary hosts directly. The server must refuse to be used as an
//! SSRF gadget against internal addresses. Ported from OrangeVault `api/icons.rs`.
//!
//! WHAT: `fetch_icon(domain)` validates the domain, fetches
//! `https://<domain>/favicon.ico` over the shared cross-platform HTTP client, and
//! returns the bytes + content type. Caching is a backend concern (KV / R2) layered
//! on by the transport.
//!
//! HOW: the SSRF guard rejects userinfo/port/path/IP-literal/`localhost`/`.local`
//! shapes before any network call, via [`crate::core::util::validate_icon_domain`]
//! plus explicit structural checks.

use foundation_core::url::Uri;
use foundation_netio::default_http_client;
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::client::request::PreparedRequest;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod};

use crate::core::error::{AppError, AppResult};
use crate::core::util::validate_icon_domain;

/// A fetched favicon.
#[derive(Debug, Clone)]
pub struct IconData {
    pub content_type: String,
    pub bytes: Vec<u8>,
}

/// Reject anything that isn't a bare registrable hostname before we build a URL.
fn is_plausible_domain(domain: &str) -> bool {
    !domain.is_empty()
        && domain.len() <= 253
        && !domain.contains('/')
        && !domain.contains('@')
        && !domain.contains(':')
        && !domain.contains("..")
        && domain.contains('.')
        && domain.parse::<std::net::IpAddr>().is_err()
}

/// `GET /icons/:domain/icon.png` — fetch a site's favicon (SSRF-guarded).
pub async fn fetch_icon(domain: &str) -> AppResult<IconData> {
    let domain = domain.trim().to_lowercase();
    if !is_plausible_domain(&domain) {
        return Err(AppError::BadRequest("invalid icon domain".into()));
    }
    let url = format!("https://{domain}/favicon.ico");
    if !validate_icon_domain(&url) {
        return Err(AppError::Forbidden);
    }

    let client = default_http_client();
    let uri = Uri::parse(&url).map_err(|e| AppError::BadRequest(format!("invalid domain URL: {e}")))?;
    // Belt-and-suspenders: the parsed URL must still be a plain https host.
    if uri.host_str().as_deref() != Some(domain.as_str()) || uri.port().is_some() {
        return Err(AppError::Forbidden);
    }

    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::ACCEPT, vec!["image/*".into()]);
    let req = PreparedRequest {
        method: SimpleMethod::GET,
        url: uri,
        headers,
        body: SendSafeBody::None,
        extensions: Default::default(),
    };

    let resp = client
        .send_async(req)
        .await
        .map_err(|e| AppError::NotFound(format!("icon fetch failed: {e}")))?;
    let status: usize = resp.get_status().into();
    if !(200..300).contains(&status) {
        return Err(AppError::NotFound(format!("icon not found (HTTP {status})")));
    }
    let content_type = resp
        .get_headers_ref()
        .get(&SimpleHeader::CONTENT_TYPE)
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_else(|| "image/x-icon".to_string());
    let bytes = try_collect_bytes(resp.take_body())
        .map_err(|e| AppError::NotFound(format!("read icon body: {e}")))?;

    Ok(IconData { content_type, bytes })
}
