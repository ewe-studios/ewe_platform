//! Native HTTP client — the single concrete client for non-wasm targets.
//!
//! WHY: F51 dissolved `SimpleHttpClient` into `NativeHttpClient` so there is one
//! native HTTP client owning the pool, config, TLS connector, verb builders,
//! timeouts, proxy, redirect, and retry logic. It implements `HttpClient` and
//! provides a builder-pattern configuration surface identical to what
//! `SimpleHttpClient` offered.
//!
//! WHAT: `NativeHttpClient<R>` with generic DNS resolver `R`. Methods that take
//! `&self` (verb builders, `request`) are call-compatible with generated code's
//! `&SimpleHttpClient<R>` usage (the `SimpleHttpClient` alias is gone, #140).
//!
//! HOW: Owns `ClientConfig`, `HttpConnectionPool<R>`, and a `resolver: R` for
//! SSE task construction. All configuration is builder-pattern on `self`.

use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{execute, StreamIteratorExt};

use crate::event_source::{ReconnectingEventSourceTask, ReconnectingProgress};
use crate::http::pool::ConnectionPool;
use crate::http::tasks::HttpExchangeTask;
use crate::http::{ClientRequest, ClientRequestBuilder, HttpConnectionPool};
use crate::shared::client::http_client::{
    BoxedSseFutureStream, BoxedSseIterator, HttpClient, SseProgress,
};
use crate::shared::client::request::PreparedRequest;
use crate::shared::client::request_task::HttpExchangeClientTask;
use crate::shared::client::{ClientConfig, DnsResolver, SystemDnsResolver};
use crate::shared::http::timeout::TimeoutCalculator;
use crate::shared::http::{
    HttpClientError, SendSafeBody, SimpleHeader, SimpleMethod, SimpleResponse,
};

/// The native HTTP client — owns the connection pool, configuration, and resolver.
///
/// WHY: F51 merged `SimpleHttpClient` into this type. It is the single concrete
/// native client implementing `HttpClient`. All verb builders, timeouts, proxy,
/// redirect, and retry configuration live here.
///
/// WHAT: Generic over `R: DnsResolver`. Defaults to `SystemDnsResolver`.
/// Constructed via `::new()`, `::with_resolver()`, `::default()`, or
/// `::from_system()`. Configuration methods return `Self` (builder pattern).
///
/// # Type Parameters
///
/// * `R` - DNS resolver type implementing `DnsResolver` trait.
///   Defaults to `SystemDnsResolver`.
pub struct NativeHttpClient<R: DnsResolver + Clone + Send + 'static = SystemDnsResolver> {
    config: ClientConfig,
    pool: Option<Arc<HttpConnectionPool<R>>>,
    /// Resolver kept for SSE task construction (does not share the pool's resolver).
    resolver: R,
}

// ---------------------------------------------------------------------------
// SystemDnsResolver specialisation
// ---------------------------------------------------------------------------

impl NativeHttpClient<SystemDnsResolver> {
    /// Create a client that resolves via the OS resolver and uses a default pool.
    #[must_use]
    pub fn from_system() -> Self {
        Self {
            config: ClientConfig::default(),
            pool: Some(Arc::new(HttpConnectionPool::default())),
            resolver: SystemDnsResolver,
        }
    }
}

impl Default for NativeHttpClient<SystemDnsResolver> {
    fn default() -> Self {
        Self {
            config: ClientConfig::default(),
            pool: Some(Arc::new(HttpConnectionPool::new(
                ConnectionPool::default(),
                SystemDnsResolver,
            ))),
            resolver: SystemDnsResolver,
        }
    }
}

// ---------------------------------------------------------------------------
// Core constructors
// ---------------------------------------------------------------------------

impl<R: DnsResolver + Clone + Send + 'static> NativeHttpClient<R> {
    /// Create a client from a resolver. Creates a default config and fresh pool.
    ///
    /// This is the primary constructor, matching the pre-F51 `NativeHttpClient::new(resolver)`.
    #[must_use]
    pub fn new(resolver: R) -> Self {
        Self {
            config: ClientConfig::default(),
            pool: Some(Arc::new(HttpConnectionPool::new(
                ConnectionPool::default(),
                resolver.clone(),
            ))),
            resolver,
        }
    }

    /// Create a client with explicit config, pool, and resolver.
    #[must_use]
    pub fn with_config_and_pool(
        config: ClientConfig,
        pool: Arc<HttpConnectionPool<R>>,
        resolver: R,
    ) -> Self {
        Self {
            config,
            pool: Some(pool),
            resolver,
        }
    }

    /// Create a client that resolves via `resolver` and uses a default pool.
    ///
    /// This is the pre-F51 `SimpleHttpClient::with_resolver(resolver)`.
    #[must_use]
    pub fn with_resolver(resolver: R) -> Self {
        Self {
            config: ClientConfig::default(),
            pool: Some(Arc::new(HttpConnectionPool::new(
                ConnectionPool::default(),
                resolver.clone(),
            ))),
            resolver,
        }
    }

    /// Create a client with a custom expect-continue read timeout.
    #[must_use]
    pub fn with_expect_continue_timeout(resolver: R, timeout: Duration) -> Self {
        let config = ClientConfig {
            expect_continue_read_timeout: timeout,
            ..ClientConfig::default()
        };
        Self {
            config,
            pool: Some(Arc::new(HttpConnectionPool::new(
                ConnectionPool::default(),
                resolver.clone(),
            ))),
            resolver,
        }
    }

    // -- accessors -------------------------------------------------------

    #[must_use]
    pub fn client_config(&self) -> ClientConfig {
        self.config.clone()
    }

    #[must_use]
    pub fn client_pool(&self) -> Option<Arc<HttpConnectionPool<R>>> {
        self.pool.clone()
    }

    /// Access the resolver for internal use (e.g. WebSocket handshake — Stage 5/6).
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn resolver(&self) -> &R {
        &self.resolver
    }

    // -- builder helpers -------------------------------------------------

    /// Build a `ClientRequestBuilder` from a `PreparedRequest`, routing by method.
    fn build_request(
        &self,
        req: &PreparedRequest,
    ) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        let url_str = req.url.to_string();
        let mut builder = match req.method {
            SimpleMethod::GET => self.get(&url_str)?,
            SimpleMethod::POST => self.post(&url_str)?,
            SimpleMethod::PUT => self.put(&url_str)?,
            SimpleMethod::DELETE => self.delete(&url_str)?,
            SimpleMethod::PATCH => self.patch(&url_str)?,
            SimpleMethod::HEAD => self.head(&url_str)?,
            SimpleMethod::OPTIONS => self.options(&url_str)?,
            _ => return Err(HttpClientError::NotSupported),
        };

        for (key, values) in &req.headers {
            // Host is already extracted from the URL by self.get() / self.post() /
            // etc. above. Passing it again creates a duplicate value, which causes
            // strict servers (including Docker's Engine API) to reject the request
            // with HTTP 400.
            if *key == SimpleHeader::HOST {
                continue;
            }
            for value in values {
                builder = builder.header(key.clone(), value.clone());
            }
        }

        Ok(builder)
    }

    /// Build an SSE task using the stored resolver.
    fn build_sse_task(
        &self,
        req: &PreparedRequest,
    ) -> Result<ReconnectingEventSourceTask<R>, HttpClientError> {
        let url_str = req.url.to_string();
        let mut task = ReconnectingEventSourceTask::connect(self.resolver.clone(), &url_str)
            .map_err(|e| HttpClientError::Reason(format!("SSE connection failed: {e}")))?;

        for (key, values) in &req.headers {
            for value in values {
                task = task.with_header(key.clone(), value.clone());
            }
        }

        Ok(task)
    }
}

// ---------------------------------------------------------------------------
// Clone
// ---------------------------------------------------------------------------

impl<R: DnsResolver + Clone + Send + 'static> Clone for NativeHttpClient<R> {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            pool: self.pool.clone(),
            resolver: self.resolver.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Pool / TLS configuration (consume-self builder)
// ---------------------------------------------------------------------------

impl<R: DnsResolver + Default + Clone + Send + 'static> NativeHttpClient<R> {
    /// Ensure a connection pool is present (creates default if none).
    #[must_use]
    pub fn with_connection_pool(mut self) -> Self {
        self.pool = Some(Arc::new(HttpConnectionPool::new(
            ConnectionPool::default(),
            R::default(),
        )));
        self
    }
}

impl<R: DnsResolver + Clone + Send + 'static> NativeHttpClient<R> {
    /// Replace the TLS connector on the pool.
    #[must_use]
    pub fn with_tls_connector(mut self, connector: crate::netcap::ssl::SSLConnector) -> Self {
        if let Some(pool) = self.pool.take() {
            let pool_inner = Arc::try_unwrap(pool).unwrap_or_else(|arc| (*arc).clone());
            self.pool = Some(Arc::new(pool_inner.with_tls_connector(connector)));
        }
        self
    }

    /// Route requests over a custom transport connector (`WireGuard` overlay, SSH
    /// `docker system dial-stdio`) instead of dialing DNS+TCP.
    #[must_use]
    pub fn with_connector(
        mut self,
        connector: Arc<dyn crate::native::connection::Connector>,
    ) -> Self {
        if let Some(pool) = self.pool.take() {
            let pool_inner = Arc::try_unwrap(pool).unwrap_or_else(|arc| (*arc).clone());
            self.pool = Some(Arc::new(pool_inner.with_connector(connector)));
        }
        self
    }
}

impl<R: DnsResolver + Default + Clone + Send + 'static> NativeHttpClient<R> {
    /// Set a specific pool (replaces any existing pool).
    #[must_use]
    pub fn with_pool(pool: Arc<HttpConnectionPool<R>>) -> Self {
        Self {
            pool: Some(pool),
            config: ClientConfig::default(),
            resolver: R::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Configuration builders (consume-self)
// ---------------------------------------------------------------------------

impl<R: DnsResolver + Clone + Send + 'static> NativeHttpClient<R> {
    #[must_use]
    pub fn config(mut self, config: ClientConfig) -> Self {
        self.config = config;
        self
    }

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

    #[must_use]
    pub fn max_redirects(mut self, max: u8) -> Self {
        self.config.max_redirects = max;
        self
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn proxy(mut self, proxy_url: &str) -> Result<Self, HttpClientError> {
        use crate::shared::client::proxy::ProxyConfig;
        let proxy_config = ProxyConfig::parse(proxy_url)?;
        self.config.proxy = Some(proxy_config);
        Ok(self)
    }

    #[must_use]
    pub fn proxy_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        use crate::shared::client::proxy::ProxyAuth;
        if let Some(ref mut proxy) = self.config.proxy {
            proxy.auth = Some(ProxyAuth::new(username, password));
        }
        self
    }

    #[must_use]
    pub fn proxy_from_env(mut self) -> Self {
        self.config.proxy_from_env = true;
        self
    }

    #[must_use]
    pub fn max_body_size(mut self, max_body_size: Option<u64>) -> Self {
        self.config.max_body_size = max_body_size;
        self
    }

    /// Caps on the frames a parser must buffer whole (response header block,
    /// chunk-size line, line, server-sent event). See
    /// [`crate::shared::http::framing::FrameLimits`].
    #[must_use]
    pub fn frame_limits(mut self, limits: crate::shared::http::framing::FrameLimits) -> Self {
        self.config.frame_limits = limits;
        self
    }

    #[must_use]
    pub fn full_body_threshold(mut self, threshold: u64) -> Self {
        self.config.full_body_threshold = threshold;
        self
    }

    #[must_use]
    pub fn batch_size(mut self, batch_size: usize) -> Self {
        self.config.batch_size = batch_size;
        self
    }

    #[must_use]
    pub fn max_retries(mut self, max_retries: usize) -> Self {
        self.config.max_retries = max_retries;
        self
    }

    #[must_use]
    pub fn preserve_auth_on_redirect(mut self, preserve: bool) -> Self {
        self.config.redirect.preserve_auth_on_redirect = preserve;
        self
    }

    #[must_use]
    pub fn preserve_cookies_on_redirect(mut self, preserve: bool) -> Self {
        self.config.redirect.preserve_cookies_on_redirect = preserve;
        self
    }

    /// Enable or disable the `Expect: 100-continue` handshake for requests with
    /// a body. When disabled, the header is never sent and bodies are written
    /// immediately, which avoids interim `100 Continue` responses from servers
    /// that do not handle the handshake well. Bodyless requests never use it.
    #[must_use]
    pub fn expect_continue(mut self, enabled: bool) -> Self {
        self.config.expect_continue_enabled = enabled;
        self
    }
}

// ---------------------------------------------------------------------------
// Verb builders (&self — call-compatible with the old SimpleHttpClient surface)
// ---------------------------------------------------------------------------

impl<R: DnsResolver + Clone + Send + 'static> NativeHttpClient<R> {
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn get(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::get(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
        })
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn post(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::post(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
        })
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn put(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::put(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
        })
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn delete(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::delete(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
        })
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn patch(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::patch(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
        })
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn head(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::head(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
        })
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn options(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::options(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
        })
    }
}

// ---------------------------------------------------------------------------
// request() — creates a ClientRequest from a builder
// ---------------------------------------------------------------------------

impl<R: DnsResolver + Clone + Send + 'static> NativeHttpClient<R> {
    /// Finalise a builder into a `ClientRequest` that can be executed.
    ///
    /// Takes the `ClientRequestBuilder`, calls `.build()`, and wraps the
    /// request with the client's config and pool.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn request(
        &self,
        builder: ClientRequestBuilder<R>,
    ) -> Result<ClientRequest<R>, HttpClientError> {
        let prepared = builder.build()?;
        let pool = self.pool.clone().ok_or(HttpClientError::NoPool)?;
        Ok(ClientRequest::new(prepared, self.config.clone(), pool))
    }
}

// ---------------------------------------------------------------------------
// HttpClient trait impl
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl<R: DnsResolver + Clone + Default + Send + Sync + 'static> HttpClient for NativeHttpClient<R> {
    async fn send_async(
        &self,
        req: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        let mut builder = self.build_request(&req)?;

        if !matches!(req.body, SendSafeBody::None) {
            builder = builder.body(req.body);
        }

        let request = self.request(builder)?;
        let (response, conn, _pool) = request.send_detached_async().await?;
        let conn = Some(conn);
        let (status, headers, body) = {
            let status = response.get_status();
            let headers = response.get_headers_ref().clone();
            (status, headers, response.take_body())
        };
        // This method is asynchronous, so its caller reads the body with an
        // async reader. `ClientRequest::send_async` hands the socket back in
        // blocking mode; a blocking body read would hold the valtron worker
        // that polls it. Set the mode the read wake source needs (spec 70
        // feature 121).
        if let Some(conn) = conn.as_ref() {
            if let Err(e) = conn.set_nonblocking(true) {
                tracing::debug!(error = %e, "cannot set response socket non-blocking");
            }
        }
        let source = conn.as_ref().and_then(|c| c.read_wake_source());
        // Without a readiness source (Windows today: the reactor's fd readiness
        // is Unix-only), a non-blocking body read can only poll in a loop. A
        // short gap in the data (for example a full 64 KiB receive window on
        // Windows) then looks like a stalled peer, and the body ended early
        // (#520). Read in blocking mode instead, bounded by the body read
        // timeout that the request already set on the socket.
        if source.is_none() {
            if let Some(conn) = conn.as_ref() {
                if let Err(e) = conn.set_nonblocking(false) {
                    tracing::debug!(error = %e, "cannot set response socket blocking");
                }
            }
        }

        Ok(SimpleResponse::with_read_wake(
            status, headers, body, source,
        ))
    }

    async fn send_sse_async(
        &self,
        req: PreparedRequest,
    ) -> Result<BoxedSseFutureStream, HttpClientError> {
        let mut task = self.build_sse_task(&req)?;

        if !matches!(req.body, SendSafeBody::None) {
            task = task.with_body(req.body);
        }

        let driven = execute(task, None)
            .map_err(|e| HttpClientError::Reason(format!("SSE executor error: {e}")))?;

        let mapped = driven.map_pending(map_progress);
        let future_stream = mapped.into_pure_future_stream();
        Ok(Box::pin(future_stream))
    }

    fn send(&self, req: PreparedRequest) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        let mut builder = self.build_request(&req)?;

        if !matches!(req.body, SendSafeBody::None) {
            builder = builder.body(req.body);
        }

        let request = self.request(builder)?;
        // The caller owns the response body. No guard returns the connection
        // to the pool, so no `Ctx` is needed here.
        let (response, _conn, _pool) = request.send_detached()?;

        Ok(response)
    }

    fn send_sse(&self, req: PreparedRequest) -> Result<BoxedSseIterator, HttpClientError> {
        let mut task = self.build_sse_task(&req)?;

        if !matches!(req.body, SendSafeBody::None) {
            task = task.with_body(req.body);
        }

        let driven = execute(task, None)
            .map_err(|e| HttpClientError::Reason(format!("SSE executor error: {e}")))?;

        let mapped = driven.map_pending(map_progress);
        Ok(Box::new(mapped))
    }

    fn open_exchange(&self, req: PreparedRequest) -> HttpExchangeClientTask {
        let config = self.config.clone();
        // Use the client's own pool so a transport connector set with
        // `with_connector` (WireGuard overlay, SSH `dial-stdio`) reaches the
        // async exchange path, matching `send` and `send_async`. A fresh pool
        // here would silently drop the connector and dial DNS+TCP instead.
        let pool = self.pool.clone().unwrap_or_else(|| {
            Arc::new(HttpConnectionPool::new(
                ConnectionPool::default(),
                self.resolver.clone(),
            ))
        });
        let task = HttpExchangeTask::new(req, config.max_redirects, pool, config);
        Box::new(task)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn map_progress(p: ReconnectingProgress) -> SseProgress {
    match p {
        ReconnectingProgress::Connecting => SseProgress::Connecting,
        ReconnectingProgress::Reading => SseProgress::Reading,
        ReconnectingProgress::Reconnecting => SseProgress::Reconnecting,
    }
}

/// Create a default HTTP client (`Arc<dyn HttpClient>`) backed by the system
/// resolver.
#[must_use]
pub fn default_http_client() -> Arc<dyn HttpClient> {
    Arc::new(NativeHttpClient::<SystemDnsResolver>::default())
}
