//! High-level HTTP client with clean public API.
//!
//! WHY: Provides user-facing API that hides all `TaskIterator` complexity.
//! Users interact with a simple, ergonomic interface.
//!
//! WHAT: Implements `SimpleHttpClient` with generic DNS resolver parameter,
//! configurable timeouts/redirects/pooling, and convenience methods for all HTTP verbs.
//!
//! HOW: Wraps TaskIterator-based execution in clean API. Builder pattern for configuration.
//! Generic type parameter for DNS resolver (no boxing).

use crate::wire::simple_http::client::{
    ClientRequest, ClientRequestBuilder, ConnectionPool, DnsResolver, HttpConnectionPool,
    MiddlewareChain, OpTimeout, ProxyConfig, SystemDnsResolver,
};
use crate::wire::simple_http::{HttpClientError, SimpleHeaders};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

/// Configuration for HTTP client.
///
/// WHY: Centralizes all client configuration in one place. Makes it easy to
/// share configuration across requests or customize per-instance.
///
/// WHAT: Holds timeouts, redirect settings, default headers, connection pool settings,
/// and proxy configuration.
///
/// HOW: Created via Default or explicit construction. Passed to `SimpleHttpClient`
/// via builder pattern.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Connection timeout duration
    pub connect_timeout: Option<Duration>,
    /// Read timeout duration
    pub read_timeout: Option<Duration>,
    /// Write timeout duration
    pub write_timeout: Option<Duration>,
    /// Maximum number of redirects to follow (0 = no redirects)
    pub max_redirects: u8,
    /// Headers to include in every request
    pub default_headers: SimpleHeaders,
    /// Whether connection pooling is enabled
    pub pool_enabled: bool,
    /// Maximum connections in pool (if pooling enabled)
    pub pool_max_connections: usize,
    /// Optional proxy configuration
    pub proxy: Option<ProxyConfig>,
    /// Whether to automatically detect proxy from environment variables
    pub proxy_from_env: bool,
}

impl ClientConfig {
    /// Returns operation timeout configuration.
    ///
    /// WHY: Converts optional timeout durations into `OpTimeout` struct used by internal tasks.
    ///
    /// WHAT: If all timeouts are configured, creates `OpTimeout` with specified values.
    /// Otherwise returns default `OpTimeout`.
    ///
    /// # Returns
    ///
    /// `OpTimeout` with configured or default timeout values.
    #[must_use]
    pub fn get_op_timeout(&self) -> OpTimeout {
        match (self.connect_timeout, self.read_timeout, self.write_timeout) {
            (Some(connect), Some(read), Some(write)) => OpTimeout::new(connect, read, write),
            _ => OpTimeout::default(),
        }
    }
}

impl Default for ClientConfig {
    /// Creates default client configuration.
    ///
    /// WHY: Sensible defaults for most use cases. Users can customize via builder.
    ///
    /// WHAT: Default timeouts (30s connect, 30s read, 30s write), 5 redirects,
    /// no default headers, pooling disabled, no proxy.
    fn default() -> Self {
        Self {
            connect_timeout: Some(Duration::from_secs(30)),
            read_timeout: Some(Duration::from_secs(30)),
            write_timeout: Some(Duration::from_secs(30)),
            default_headers: BTreeMap::default(),
            max_redirects: 5,
            pool_enabled: false,
            pool_max_connections: 10,
            proxy: None,
            proxy_from_env: false,
        }
    }
}

/// High-level HTTP client with clean API.
///
/// WHY: Provides user-facing API that completely hides `TaskIterator` complexity.
/// Users work with simple methods like `.get(url).send()`.
///
/// WHAT: Generic HTTP client with pluggable DNS resolver. Supports all HTTP methods,
/// configurable timeouts/redirects, optional connection pooling.
///
/// HOW: Wraps `ClientRequestBuilder` and `TaskIterator` execution. Builder pattern
/// for configuration. Generic type parameter for DNS resolver flexibility.
///
/// # Type Parameters
///
/// * `R` - DNS resolver type implementing `DnsResolver` trait. Defaults to `SystemDnsResolver`.
///
/// # Examples
///
/// ```ignore
/// // Basic usage
/// let client = SimpleHttpClient::from_system();
/// let response = client.get("http://example.com")?.send()?;
///
/// // With custom resolver
/// let client = SimpleHttpClient::with_resolver(MyResolver::new());
///
/// // With configuration
/// let client = SimpleHttpClient::from_system()
///     .connect_timeout(Duration::from_secs(10))
///     .max_redirects(3);
/// ```
pub struct SimpleHttpClient<R: DnsResolver = SystemDnsResolver> {
    config: ClientConfig,
    pool: Option<Arc<HttpConnectionPool<R>>>,
    middleware_chain: Arc<MiddlewareChain>,
}

impl SimpleHttpClient<SystemDnsResolver> {
    /// Creates a new HTTP client with default system DNS resolver.
    ///
    /// WHY: Most users want system DNS resolution. This provides zero-config usage.
    ///
    /// WHAT: Creates client with `SystemDnsResolver` and default configuration.
    ///
    /// # Returns
    ///
    /// A new `SimpleHttpClient` ready to make requests.
    #[must_use]
    pub fn from_system() -> Self {
        Self::new(
            ClientConfig::default(),
            Arc::new(HttpConnectionPool::default()),
        )
    }
}

impl<R: DnsResolver> SimpleHttpClient<R> {
    #[must_use]
    pub fn new(config: ClientConfig, pool: Arc<HttpConnectionPool<R>>) -> Self {
        Self {
            config,
            pool: Some(pool),
            middleware_chain: Arc::new(MiddlewareChain::new()),
        }
    }

    /// Sets custom middleware chain for this client.
    ///
    /// WHY: Allow users to configure request/response interception.
    ///
    /// WHAT: Replaces current middleware chain with provided one.
    ///
    /// HOW: Wraps chain in Arc, returns self for builder pattern.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn middleware(mut self, chain: MiddlewareChain) -> Self {
        self.middleware_chain = Arc::new(chain);
        self
    }
}

impl<R: DnsResolver + Default> Default for SimpleHttpClient<R> {
    fn default() -> Self {
        Self::new(
            ClientConfig::default(),
            Arc::new(HttpConnectionPool::new(
                ConnectionPool::default(),
                R::default(),
            )),
        )
    }
}

impl<R: DnsResolver + Clone> Clone for SimpleHttpClient<R> {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            pool: self.pool.clone(),
            middleware_chain: self.middleware_chain.clone(),
        }
    }
}

impl<R: DnsResolver + Clone> SimpleHttpClient<R> {
    #[must_use]
    pub fn client_config(&self) -> ClientConfig {
        self.config.clone()
    }
}

impl<R: DnsResolver + Default + Clone> SimpleHttpClient<R> {
    /// Enables connection pooling with specified max connections and creates a pool instance.
    #[must_use]
    pub fn enable_pool(mut self, max_connections: usize) -> Self {
        self.config.pool_enabled = true;
        self.config.pool_max_connections = max_connections;
        // create a simple pool with default idle timeout (300s)
        self.pool = Some(Arc::new(HttpConnectionPool::new(
            ConnectionPool::default(),
            R::default(),
        )));
        self
    }
}

impl<R: DnsResolver + Clone> SimpleHttpClient<R> {
    // Convenience methods for common HTTP verbs that return prepared ClientRequest
    // which wraps the task machinery and can be executed by the caller.

    /// Creates a GET request and returns a `ClientRequestBuilder` ready to execute.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError::InvalidUrl` if the URL cannot be parsed.
    /// Returns `HttpClientError::UnsupportedScheme` if the URL scheme is not http or https.
    pub fn get(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::get(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
                .with_middleware(self.middleware_chain.clone())
        })
    }

    /// Creates a POST request and returns a `ClientRequestBuilder` ready to execute.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError::InvalidUrl` if the URL cannot be parsed.
    /// Returns `HttpClientError::UnsupportedScheme` if the URL scheme is not http or https.
    pub fn post(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::post(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
                .with_middleware(self.middleware_chain.clone())
        })
    }

    /// Creates a PUT request and returns a `ClientRequestBuilder` ready to execute.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError::InvalidUrl` if the URL cannot be parsed.
    /// Returns `HttpClientError::UnsupportedScheme` if the URL scheme is not http or https.
    pub fn put(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::put(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
                .with_middleware(self.middleware_chain.clone())
        })
    }

    /// Creates a DELETE request and returns a `ClientRequestBuilder` ready to execute.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError::InvalidUrl` if the URL cannot be parsed.
    /// Returns `HttpClientError::UnsupportedScheme` if the URL scheme is not http or https.
    pub fn delete(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::delete(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
                .with_middleware(self.middleware_chain.clone())
        })
    }

    /// Creates a PATCH request and returns a `ClientRequestBuilder` ready to execute.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError::InvalidUrl` if the URL cannot be parsed.
    /// Returns `HttpClientError::UnsupportedScheme` if the URL scheme is not http or https.
    pub fn patch(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::patch(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
                .with_middleware(self.middleware_chain.clone())
        })
    }

    /// Creates a HEAD request and returns a `ClientRequestBuilder` ready to execute.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError::InvalidUrl` if the URL cannot be parsed.
    /// Returns `HttpClientError::UnsupportedScheme` if the URL scheme is not http or https.
    pub fn head(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::head(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
                .with_middleware(self.middleware_chain.clone())
        })
    }

    /// Creates an OPTIONS request and returns a `ClientRequestBuilder` ready to execute.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError::InvalidUrl` if the URL cannot be parsed.
    /// Returns `HttpClientError::UnsupportedScheme` if the URL scheme is not http or https.
    pub fn options(&self, url: &str) -> Result<ClientRequestBuilder<R>, HttpClientError> {
        ClientRequestBuilder::options(url).map(|builder| {
            builder
                .client_config(self.config.clone())
                .with_pool(self.pool.clone())
                .with_middleware(self.middleware_chain.clone())
        })
    }
}

impl<R: DnsResolver> SimpleHttpClient<R> {
    /// Creates a new HTTP client with provided pool.
    ///
    /// # Returns
    ///
    /// A new `SimpleHttpClient` using the provided resolver.
    #[must_use]
    pub fn with_pool(pool: Arc<HttpConnectionPool<R>>) -> Self {
        Self {
            pool: Some(pool),
            config: ClientConfig::default(),
            middleware_chain: Arc::new(MiddlewareChain::new()),
        }
    }

    /// Sets full client configuration.
    ///
    /// WHY: Allows passing pre-built configuration object.
    ///
    /// WHAT: Replaces current configuration with provided one.
    ///
    /// # Arguments
    ///
    /// * `config` - New configuration to use
    #[must_use]
    pub fn config(mut self, config: ClientConfig) -> Self {
        self.config = config;
        self
    }

    /// Sets connection timeout.
    ///
    /// WHY: Users often need to customize timeout without rebuilding entire config.
    ///
    /// WHAT: Builder method to set connection timeout.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Connection timeout duration
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.config.connect_timeout = Some(timeout);
        self
    }

    /// Sets read timeout.
    ///
    /// WHY: Users often need to customize timeout without rebuilding entire config.
    ///
    /// WHAT: Builder method to set read timeout.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Read timeout duration
    #[must_use]
    pub fn read_timeout(mut self, timeout: Duration) -> Self {
        self.config.read_timeout = Some(timeout);
        self
    }

    /// Sets write timeout.
    ///
    /// WHY: Users often need to customize timeout without rebuilding entire config.
    ///
    /// WHAT: Builder method to set write timeout.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Write timeout duration
    #[must_use]
    pub fn write_timeout(mut self, timeout: Duration) -> Self {
        self.config.write_timeout = Some(timeout);
        self
    }

    /// Sets maximum number of redirects to follow.
    ///
    /// WHY: Some use cases require no redirects or limited redirects.
    ///
    /// WHAT: Builder method to set max redirects (0 = no redirects).
    ///
    /// # Arguments
    ///
    /// * `max` - Maximum redirects (0-255)
    #[must_use]
    pub fn max_redirects(mut self, max: u8) -> Self {
        self.config.max_redirects = max;
        self
    }

    /// Sets proxy from URL.
    ///
    /// WHY: Users need to configure HTTP/HTTPS/SOCKS5 proxy for their requests.
    ///
    /// WHAT: Parses proxy URL and sets it in client configuration.
    ///
    /// HOW: Parses URL with `ProxyConfig::parse()`, supports authentication
    /// in URL format (http://user:pass@proxy.com:8080).
    ///
    /// # Arguments
    ///
    /// * `proxy_url` - Proxy URL (e.g., "http://proxy.example.com:8080")
    ///
    /// # Returns
    ///
    /// Self for builder pattern chaining.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError::InvalidProxyUrl` if URL parsing fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let client = SimpleHttpClient::from_system()
    ///     .proxy("http://proxy.example.com:8080")?;
    ///
    /// // With authentication
    /// let client = SimpleHttpClient::from_system()
    ///     .proxy("http://user:pass@proxy.example.com:8080")?;
    /// ```
    pub fn proxy(mut self, proxy_url: &str) -> Result<Self, HttpClientError> {
        let proxy_config = ProxyConfig::parse(proxy_url)?;
        self.config.proxy = Some(proxy_config);
        Ok(self)
    }

    /// Sets proxy authentication credentials.
    ///
    /// WHY: Users may want to configure proxy auth separately from URL.
    ///
    /// WHAT: Sets username and password for existing proxy configuration.
    ///
    /// HOW: Updates auth field on existing proxy config. If no proxy is configured,
    /// this method does nothing (proxy must be set first via `.proxy()`).
    ///
    /// # Arguments
    ///
    /// * `username` - Proxy username
    /// * `password` - Proxy password
    ///
    /// # Returns
    ///
    /// Self for builder pattern chaining.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let client = SimpleHttpClient::from_system()
    ///     .proxy("http://proxy.example.com:8080")?
    ///     .proxy_auth("user", "password");
    /// ```
    #[must_use]
    pub fn proxy_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        if let Some(ref mut proxy) = self.config.proxy {
            proxy.auth = Some(crate::wire::simple_http::client::ProxyAuth::new(
                username, password,
            ));
        }
        self
    }

    /// Enables automatic proxy detection from environment variables.
    ///
    /// WHY: Users often configure proxies via HTTP_PROXY/HTTPS_PROXY environment variables.
    ///
    /// WHAT: Sets flag to automatically detect proxy from environment for each request.
    ///
    /// HOW: When enabled, client checks HTTP_PROXY/HTTPS_PROXY/NO_PROXY environment
    /// variables per-request based on target URL scheme.
    ///
    /// # Returns
    ///
    /// Self for builder pattern chaining.
    ///
    /// # Environment Variables
    ///
    /// - `HTTP_PROXY` / `http_proxy` - Proxy for HTTP requests
    /// - `HTTPS_PROXY` / `https_proxy` - Proxy for HTTPS requests
    /// - `NO_PROXY` / `no_proxy` - Comma-separated list of hosts to bypass
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Set environment variables
    /// std::env::set_var("HTTP_PROXY", "http://proxy.example.com:8080");
    /// std::env::set_var("NO_PROXY", "localhost,.internal.com");
    ///
    /// let client = SimpleHttpClient::from_system()
    ///     .proxy_from_env();
    ///
    /// // Client will automatically use proxy from environment
    /// let response = client.get("http://example.com")?.send()?;
    /// ```
    #[must_use]
    pub fn proxy_from_env(mut self) -> Self {
        self.config.proxy_from_env = true;
        self
    }

    /* duplicate enable_pool removed (kept the earlier implementation that constructs a pool) */

    /// Creates a request from a builder and returns a `ClientRequest`.
    ///
    /// WHY: Advanced users may want full control via `ClientRequestBuilder`.
    ///
    /// WHAT: Takes a pre-configured builder and prepares it for execution by
    /// converting it into an internal `PreparedRequest`.
    ///
    /// # Arguments
    ///
    /// * `builder` - Pre-configured request builder
    ///
    /// # Returns
    ///
    /// A `ClientRequest` ready to execute.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError::InvalidUrl` if the URL in the builder is invalid.
    /// Returns `HttpClientError::NoPool` if the connection pool is not initialized.
    pub fn request(
        &self,
        builder: ClientRequestBuilder<R>,
    ) -> Result<ClientRequest<R>, HttpClientError> {
        let prepared = builder.build()?;
        let pool = self.pool.clone().ok_or(HttpClientError::NoPool)?;
        Ok(ClientRequest::new(
            prepared,
            self.config.clone(),
            pool,
            self.middleware_chain.clone(),
        ))
    }
}
