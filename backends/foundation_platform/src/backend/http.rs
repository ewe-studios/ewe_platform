//! HTTP backend for remote fetch (F29 Stage 2).
//!
//! Uses `foundation_netio::HttpClientBuilder` with rustls-ring TLS —
//! our own HTTP stack. Compiles for desktop, Android, iOS, and wasm.

use std::collections::HashMap;
use std::sync::Mutex;

use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};
use foundation_netio::shared::http::SimpleMethod;
use foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe;

/// HTTP client for remote content fetching.
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
        tracing::debug!(%url, "HttpBackend::fetch");

        let builder = PreparedRequestBuilder::new(SimpleMethod::GET, url)
            .map_err(|e| format!("bad URL '{url}': {e}"))?;

        let response = self.client.send(builder.build())
            .map_err(|e| format!("fetch '{url}' failed: {e}"))?;

        let status_code: usize = response.get_status().into();
        if status_code < 200 || status_code >= 400 {
            return Err(format!("HTTP {status_code} for {url}"));
        }

        let body = collect_bytes_from_send_safe(response.take_body());
        Ok((body, "text/html".to_string()))
    }
}

impl Default for HttpBackend {
    fn default() -> Self { Self::new() }
}

impl std::fmt::Debug for HttpBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpBackend").finish()
    }
}
