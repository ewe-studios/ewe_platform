//! Real HTTP backend for remote fetch (F29 Stage 2).
//!
//! WHY: The `BackendTransport::fetch_remote()` stub returned fake JSON.
//! Real apps need to fetch content from the internet — proxied through
//! the platform shell so auth tokens never enter the WebView.
//!
//! WHAT: `HttpBackend` using `reqwest`. Both sync (blocking) for use
//! inside Tauri commands, and async for use with Tauri's async runtime.

use std::collections::HashMap;
use std::sync::Mutex;

/// An HTTP client for remote content fetching.
///
/// Wraps `reqwest::blocking::Client` for simple synchronous use.
/// Auth tokens can be injected from session state — they never
/// cross the WASM/WebView boundary.
pub struct HttpBackend {
    client: reqwest::blocking::Client,
    /// Optional auth token map: domain → header value.
    /// E.g. "api.example.com" → "Bearer xyz".
    auth_tokens: Mutex<HashMap<String, String>>,
}

impl HttpBackend {
    /// Create a new HTTP backend with a default client.
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::builder()
                .user_agent("ewe-platform/0.1")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest client should build"),
            auth_tokens: Mutex::new(HashMap::new()),
        }
    }

    /// Set an auth token for a domain.
    pub fn set_auth(&self, domain: &str, token: &str) {
        self.auth_tokens
            .lock()
            .unwrap()
            .insert(domain.to_string(), token.to_string());
    }

    /// Fetch content from a URL. Returns `(body_bytes, content_type)`.
    ///
    /// Auth tokens are attached for matching domains before the request
    /// leaves the platform shell.
    ///
    /// # Errors
    /// Returns an error string if the request fails.
    pub fn fetch(&self, url: &str) -> Result<(Vec<u8>, String), String> {
        let mut request = self.client.get(url);

        // Inject auth tokens for matching domains
        if let Ok(parsed) = reqwest::Url::parse(url) {
            if let Some(host) = parsed.host_str() {
                let guard = self.auth_tokens.lock().unwrap();
                if let Some(token) = guard.get(host) {
                    request = request.header("Authorization", token.as_str());
                }
            }
        }

        let response = request.send().map_err(|e| format!("HTTP fetch failed: {e}"))?;

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/html")
            .to_string();

        let status = response.status();
        if !status.is_success() {
            return Err(format!("HTTP {} for {url}", status.as_u16()));
        }

        let body = response
            .bytes()
            .map_err(|e| format!("Failed to read response body: {e}"))?;

        Ok((body.to_vec(), content_type))
    }

    /// Fetch content as a `tauri::http::Response<Vec<u8>>`.
    ///
    /// Used directly by route responders that serve remote content.
    pub fn fetch_response(&self, url: &str) -> tauri::http::Response<Vec<u8>> {
        match self.fetch(url) {
            Ok((body, content_type)) => tauri::http::Response::builder()
                .status(200)
                .header("Content-Type", content_type.as_str())
                .body(body)
                .unwrap(),
            Err(e) => {
                let msg = format!("Remote fetch error: {e}").into_bytes();
                tauri::http::Response::builder()
                    .status(502)
                    .header("Content-Type", "text/plain")
                    .body(msg)
                    .unwrap()
            }
        }
    }
}

impl Default for HttpBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for HttpBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpBackend")
            .field("auth_domains", &self.auth_tokens.lock().unwrap().len())
            .finish()
    }
}
