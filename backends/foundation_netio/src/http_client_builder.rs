//! Platform-agnostic `HttpClientBuilder` — produces an `HttpClient` on both targets.
//!
//! WHY: F51 Stage 2 — a single builder surface replaces the native-only
//! `SimpleHttpClient` configuration surface. Callers write identical code on
//! native and wasm; the builder dispatches to `NativeHttpClient` or
//! `FetchHttpClient` at `build()` time.
//!
//! WHAT: Accumulates `ClientConfig`, optional resolver, and default headers.
//! On native, `build()` creates a `NativeHttpClient` with a fresh pool.
//! On wasm, `build()` creates a `FetchHttpClient` with the accumulated config.
//!
//! HOW: The struct is platform-neutral. `build()` is cfg-gated internally —
//! the signature is identical on both targets (`-> impl HttpClient`).

use std::sync::Arc;
use std::time::Duration;

use crate::shared::client::http_client::HttpClient;
use crate::shared::client::{ClientConfig, SystemDnsResolver};
use crate::simple_http::shared::timeout::TimeoutCalculator;
use crate::simple_http::shared::HttpClientError;

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
    default_headers: Vec<(String, String)>,
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
            default_headers: Vec::new(),
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

    // -- headers ----------------------------------------------------------

    /// Add a default header sent with every request. Request-level headers
    /// take precedence over builder defaults.
    #[must_use]
    pub fn default_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.default_headers.push((key.into(), value.into()));
        self
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

    /// Build an `Arc<dyn HttpClient>`.
    ///
    /// On native: creates a `NativeHttpClient` with a fresh pool and the
    /// configured resolver (defaults to `SystemDnsResolver`).
    ///
    /// On wasm: creates a `FetchHttpClient` with the accumulated config.
    /// Pool, TLS, and resolver settings are ignored (the browser owns them).
    #[must_use]
    pub fn build(self) -> Arc<dyn HttpClient> {
        #[cfg(all(feature = "multi", not(target_family = "wasm")))]
        {
            let config = self.finalize_config();
            Arc::new(crate::http::NativeHttpClient::new(
                SystemDnsResolver::default(),
            )
            .config(config))
        }
        #[cfg(all(target_family = "wasm", feature = "wasm-fetch"))]
        {
            Arc::new(
                crate::wasm::client::FetchHttpClient::with_config(
                    self.finalize_config(),
                ),
            )
        }
        #[cfg(not(any(
            all(feature = "multi", not(target_family = "wasm")),
            all(target_family = "wasm", feature = "wasm-fetch")
        )))]
        {
            // Neither native multi nor wasm-fetch available — return a stub
            // that will error when used. This path only compiles when both
            // features are off, which shouldn't happen in practice.
            let _ = self;
            unimplemented!("HttpClientBuilder::build: no HTTP client backend available")
        }
    }

    /// Internal: fold default headers into `ClientConfig` before building.
    fn finalize_config(mut self) -> ClientConfig {
        if !self.default_headers.is_empty() {
            use crate::simple_http::shared::SimpleHeader;
            use std::collections::BTreeMap;
            let mut headers = BTreeMap::new();
            for (key, value) in self.default_headers {
                headers
                    .entry(SimpleHeader::custom(&key))
                    .or_insert_with(Vec::new)
                    .push(value);
            }
            self.config.headers_to_add = Some(headers);
        }
        self.config
    }
}
