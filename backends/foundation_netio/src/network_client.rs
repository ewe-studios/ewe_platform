//! Platform-agnostic network client surface (F51 + F52).
//!
//! Provides the builder (`HttpClientBuilder`) that constructs a concrete platform
//! client, plus the combined client handle types (`NetClient` / `DynNetClient`)
//! that cover both HTTP and WebSocket — transports hold one `Arc<dyn NetClient>`
//! for all protocol needs.
//!
//! WHY: F51 Stage 2 — a single builder surface replaces the native-only
//! `SimpleHttpClient`. F52 adds the combined trait + type alias so transports
//! don't leak platform-specific types.
//!
//! WHAT: `HttpClientBuilder` accumulates `ClientConfig` and dispatches to
//! `NativeHttpClient` or `FetchHttpClient` at `build()` time. `NetClient` is
//! the supertrait of `HttpClient + WebSocketConnector` and `DynNetClient` is
//! its `Arc<dyn ...>` handle.
//!
//! HOW: The builder is platform-neutral; `build()` is cfg-gated internally.
//! `NetClient` is a blanket impl — every platform client automatically satisfies it.

use std::sync::Arc;
use std::time::Duration;

use crate::shared::client::http_client::HttpClient;
use crate::shared::client::ClientConfig;
// SystemDnsResolver is only used by the native `build()` branch.
#[cfg(all(feature = "multi", not(target_family = "wasm")))]
use crate::shared::client::SystemDnsResolver;
use crate::shared::http::timeout::TimeoutCalculator;
use crate::websocket::shared::connector::WebSocketConnector;

/// A client that handles **both** HTTP requests and WebSocket connections.
///
/// Every platform client (`NativeHttpClient`, `FetchHttpClient`) implements
/// both `HttpClient` and `WebSocketConnector`. This supertrait + type alias
/// lets transports hold one `Arc<dyn NetClient>` and use it for both protocols
/// — no platform-specific types leak.
pub trait NetClient: HttpClient + WebSocketConnector {}
impl<T: HttpClient + WebSocketConnector> NetClient for T {}

/// Shared handle to a platform client (HTTP + WebSocket). Clone is cheap (`Arc`).
pub type DynNetClient = Arc<dyn NetClient>;
use crate::shared::http::{HttpClientError, SimpleHeader, SimpleHeaders};

/// Platform-agnostic builder for `Arc<dyn HttpClient>`.
///
/// # Examples
///
/// ```ignore
/// let client = HttpClientBuilder::new()
///     .max_redirects(3)
///     .connect_timeout(Duration::from_secs(10))
///     .build();
/// let response = client.send_async(req).await?;
/// ```
pub struct HttpClientBuilder {
    config: ClientConfig,
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClientBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: ClientConfig::default(),
        }
    }

    // -- timeouts ---------------------------------------------------------

    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        let mut c = *self.config.timeout_calculator.config();
        c.connect_timeout = timeout;
        self.config.timeout_calculator = TimeoutCalculator::with_config(c);
        self
    }

    #[must_use]
    pub fn read_timeout(mut self, timeout: Duration) -> Self {
        let mut c = *self.config.timeout_calculator.config();
        c.min_read_timeout = timeout;
        if c.max_read_timeout < timeout {
            c.max_read_timeout = timeout;
        }
        self.config.timeout_calculator = TimeoutCalculator::with_config(c);
        self
    }

    #[must_use]
    pub fn write_timeout(mut self, timeout: Duration) -> Self {
        let mut c = *self.config.timeout_calculator.config();
        c.write_timeout_per_kb = timeout;
        self.config.timeout_calculator = TimeoutCalculator::with_config(c);
        self
    }

    // -- redirects --------------------------------------------------------

    #[must_use]
    pub fn max_redirects(mut self, max: u8) -> Self {
        self.config.max_redirects = max;
        self
    }

    /// Control whether the client follows non-standard redirect responses
    /// (i.e. not 301/302/303/307/308).
    #[must_use]
    pub fn follow_other_redirects_response(mut self, follow: bool) -> Self {
        self.config.redirect.follow_other_redirects_response = follow;
        self
    }

    // -- headers ----------------------------------------------------------

    /// Add a default header sent with every request. Request-level headers
    /// take precedence over builder defaults.
    #[must_use]
    pub fn default_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key: String = key.into();
        self.config
            .headers_to_add
            .get_or_insert_with(Default::default)
            .entry(SimpleHeader::custom(&key))
            .or_default()
            .push(value.into());
        self
    }

    /// Set the full map of default headers added to every request.
    ///
    /// Replaces any headers set via `default_header()`. Request-level headers
    /// take precedence over these defaults.
    #[must_use]
    pub fn headers_to_add(mut self, headers: SimpleHeaders) -> Self {
        self.config.headers_to_add = Some(headers);
        self
    }

    /// Set the set of header names that are preserved across redirects.
    ///
    /// When following a redirect, only headers in this set are forwarded
    /// to the new target.
    #[must_use]
    pub fn headers_to_pass_on_redirect(mut self, headers: Vec<SimpleHeader>) -> Self {
        self.config.headers_to_pass_on_redirect = Some(headers);
        self
    }

    // -- auth -------------------------------------------------------------

    /// Set a `Basic` auth header. Encodes `username:password` as base64.
    #[must_use]
    pub fn basic_auth(self, username: &str, password: &str) -> Self {
        use base64::prelude::*;
        let credentials = format!("{username}:{password}");
        let encoded = BASE64_STANDARD.encode(credentials.as_bytes());
        self.default_header("Authorization", format!("Basic {encoded}"))
    }

    /// Set a `Bearer` token header.
    #[must_use]
    pub fn bearer_token(self, token: &str) -> Self {
        self.default_header("Authorization", format!("Bearer {token}"))
    }

    /// Set an arbitrary API key in a custom header.
    #[must_use]
    pub fn api_key(self, header_name: &str, key: &str) -> Self {
        self.default_header(header_name, key)
    }

    /// Set the `X-API-Key` header.
    #[must_use]
    pub fn x_api_key(self, key: &str) -> Self {
        self.default_header("X-API-Key", key)
    }

    /// Set an `Authorization` header with an arbitrary scheme and credentials.
    #[must_use]
    pub fn authorization(self, scheme: &str, credentials: &str) -> Self {
        self.default_header("Authorization", format!("{scheme} {credentials}"))
    }

    // -- body limits ------------------------------------------------------

    #[must_use]
    pub fn max_body_size(mut self, max_body_size: Option<u64>) -> Self {
        self.config.max_body_size = max_body_size;
        self
    }

    #[must_use]
    pub fn full_body_threshold(mut self, threshold: u64) -> Self {
        self.config.full_body_threshold = threshold;
        self
    }

    // -- retries ----------------------------------------------------------

    #[must_use]
    pub fn max_retries(mut self, max_retries: usize) -> Self {
        self.config.max_retries = max_retries;
        self
    }

    // -- misc -------------------------------------------------------------

    #[must_use]
    pub fn batch_size(mut self, batch_size: usize) -> Self {
        self.config.batch_size = batch_size;
        self
    }

    #[must_use]
    pub fn expect_continue(mut self, enabled: bool) -> Self {
        self.config.expect_continue_enabled = enabled;
        self
    }

    // -- proxy (native-only, no-op on wasm) -------------------------------

    /// Configure an HTTP proxy. On native this wires real proxy support; on
    /// wasm the browser owns the network stack and this is a documented no-op.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if the proxy URL is malformed.
    // On wasm the mutation below is cfg'd out, so `mut self` is not needed there.
    #[cfg_attr(target_family = "wasm", allow(unused_mut))]
    pub fn proxy(mut self, proxy_url: &str) -> Result<Self, HttpClientError> {
        #[cfg(not(target_family = "wasm"))]
        {
            use crate::shared::client::proxy::ProxyConfig;
            let proxy_config = ProxyConfig::parse(proxy_url)?;
            self.config.proxy = Some(proxy_config);
        }
        let _ = proxy_url;
        Ok(self)
    }

    /// Enable proxy auto-detection from environment variables.
    #[must_use]
    pub fn proxy_from_env(mut self) -> Self {
        self.config.proxy_from_env = true;
        self
    }

    // -- build ------------------------------------------------------------

    /// Build a [`DynNetClient`] (`Arc<dyn NetClient>` — HTTP + WebSocket).
    ///
    /// On native: creates a `NativeHttpClient` with a fresh pool and the
    /// configured resolver (defaults to `SystemDnsResolver`).
    ///
    /// On wasm: creates a `FetchHttpClient` with the accumulated config.
    /// Pool, TLS, and resolver settings are ignored (the browser owns them).
    ///
    /// The concrete clients implement both `HttpClient` and `WebSocketConnector`,
    /// so they satisfy `NetClient` via the blanket impl and coerce into
    /// `Arc<dyn NetClient>`. Callers that only need `HttpClient` still work —
    /// its methods are reachable through the `NetClient` supertrait.
    #[must_use]
    pub fn build(self) -> DynNetClient {
        #[cfg(all(feature = "multi", not(target_family = "wasm")))]
        {
            Arc::new(crate::http::NativeHttpClient::new(SystemDnsResolver).config(self.config))
        }
        #[cfg(all(target_family = "wasm", feature = "wasm-fetch"))]
        {
            Arc::new(crate::wasm::client::FetchHttpClient::with_config(
                self.config,
            ))
        }
        #[cfg(not(any(
            all(feature = "multi", not(target_family = "wasm")),
            all(target_family = "wasm", feature = "wasm-fetch")
        )))]
        {
            let _ = self;
            unimplemented!("HttpClientBuilder::build: no HTTP client backend available")
        }
    }
}
