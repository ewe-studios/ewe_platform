//! HTTP backend for remote fetch (F29 Stage 2).
//!
//! Uses `foundation_netio::HttpClientBuilder` — the same HTTP stack
//! used across the platform. No external HTTP dependencies.

use std::collections::HashMap;
use std::sync::Mutex;

use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};
use foundation_netio::shared::http::SimpleMethod;
use foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe;

/// HTTP client for remote content fetching.
///
/// Wraps `foundation_netio::DynNetClient`. Auth tokens injected
/// from session state — they never cross the WASM boundary.
pub struct HttpBackend {
    client: DynNetClient,
    auth_tokens: Mutex<HashMap<String, String>>,
}

impl HttpBackend {
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: HttpClientBuilder::new().build(),
            auth_tokens: Mutex::new(HashMap::new()),
        }
    }

    pub fn set_auth(&self, domain: &str, token: &str) {
        self.auth_tokens.lock().unwrap().insert(domain.to_string(), token.to_string());
    }

    /// Fetch content from a URL. Returns `(body_bytes, content_type)`.
    pub fn fetch(&self, url: &str) -> Result<(Vec<u8>, String), String> {
        let builder = PreparedRequestBuilder::new(SimpleMethod::GET, url)
            .map_err(|e| format!("bad URL: {e}"))?;

        let response = self.client.send(builder.build())
            .map_err(|e| format!("HTTP fetch failed: {e}"))?;

        let status_code: usize = response.get_status().into();
        if status_code < 200 || status_code >= 400 {
            return Err(format!("HTTP {status_code} for {url}"));
        }

        let body = collect_bytes_from_send_safe(response.take_body());
        Ok((body, "text/html".to_string()))
    }
}

fn extract_host(url: &str) -> Option<String> {
    let without = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
    Some(without.split('/').next()?.to_string())
}

impl Default for HttpBackend {
    fn default() -> Self { Self::new() }
}

impl std::fmt::Debug for HttpBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpBackend").finish()
    }
}
