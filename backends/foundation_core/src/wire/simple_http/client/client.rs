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
    ClientRequest, ClientRequestBuilder, ConnectionPool, DnsResolver, HttpClientError,
    HttpConnectionPool, OpTimeout, SystemDnsResolver,
};
use crate::wire::simple_http::SimpleHeaders;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

/// Configuration for HTTP client.
///
/// WHY: Centralizes all client configuration in one place. Makes it easy to
/// share configuration across requests or customize per-instance.
///
/// WHAT: Holds timeouts, redirect settings, default headers, and connection pool settings.
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
}

impl ClientConfig {
    pub fn get_op_timeout(&self) -> OpTimeout {
        if self.connect_timeout.is_some() && self.read_timeout.is_some() {
            OpTimeout::new(
                self.connect_timeout.unwrap(),
                self.read_timeout.unwrap(),
                self.write_timeout.unwrap(),
            )
        } else {
            OpTimeout::default()
        }
    }
}

impl Default for ClientConfig {
    /// Creates default client configuration.
    ///
    /// WHY: Sensible defaults for most use cases. Users can customize via builder.
    ///
    /// WHAT: Default timeouts (30s connect, 30s read, 30s write), 5 redirects,
    /// no default headers, pooling disabled.
    fn default() -> Self {
        Self {
            connect_timeout: Some(Duration::from_secs(30)),
            read_timeout: Some(Duration::from_secs(30)),
            write_timeout: Some(Duration::from_secs(30)),
            default_headers: BTreeMap::default(),
            max_redirects: 5,
            pool_enabled: false,
            pool_max_connections: 10,
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
    fn new(config: ClientConfig, pool: Arc<HttpConnectionPool<R>>) -> Self {
        Self {
            config,
            pool: Some(pool),
        }
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
        }
    }
}

impl<R: DnsResolver + Clone> SimpleHttpClient<R> {
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

    /// Creates a GET request and returns a `ClientRequest` ready to execute.
    pub fn get(&self, url: &str) -> Result<ClientRequest<R>, HttpClientError> {
        let builder = ClientRequestBuilder::get(url)?;
        let prepared = builder.build()?;
        Ok(ClientRequest::new(
            prepared,
            self.config.clone(),
            self.pool.clone().expect("Pool should be initialized"),
        ))
    }

    /// Creates a POST request and returns a `ClientRequest` ready to execute.
    pub fn post(&self, url: &str) -> Result<ClientRequest<R>, HttpClientError> {
        let builder = ClientRequestBuilder::post(url)?;
        let prepared = builder.build()?;
        Ok(ClientRequest::new(
            prepared,
            self.config.clone(),
            self.pool.clone().expect("Pool should be initialized"),
        ))
    }

    /// Creates a PUT request and returns a `ClientRequest` ready to execute.
    pub fn put(&self, url: &str) -> Result<ClientRequest<R>, HttpClientError> {
        let builder = ClientRequestBuilder::put(url)?;
        let prepared = builder.build()?;
        Ok(ClientRequest::new(
            prepared,
            self.config.clone(),
            self.pool.clone().expect("Pool should be initialized"),
        ))
    }

    /// Creates a DELETE request and returns a `ClientRequest` ready to execute.
    pub fn delete(&self, url: &str) -> Result<ClientRequest<R>, HttpClientError> {
        let builder = ClientRequestBuilder::delete(url)?;
        let prepared = builder.build()?;
        Ok(ClientRequest::new(
            prepared,
            self.config.clone(),
            self.pool.clone().expect("Pool should be initialized"),
        ))
    }

    /// Creates a PATCH request and returns a `ClientRequest` ready to execute.
    pub fn patch(&self, url: &str) -> Result<ClientRequest<R>, HttpClientError> {
        let builder = ClientRequestBuilder::patch(url)?;
        let prepared = builder.build()?;
        Ok(ClientRequest::new(
            prepared,
            self.config.clone(),
            self.pool.clone().expect("Pool should be initialized"),
        ))
    }

    /// Creates a HEAD request and returns a `ClientRequest` ready to execute.
    pub fn head(&self, url: &str) -> Result<ClientRequest<R>, HttpClientError> {
        let builder = ClientRequestBuilder::head(url)?;
        let prepared = builder.build()?;
        Ok(ClientRequest::new(
            prepared,
            self.config.clone(),
            self.pool.clone().expect("Pool should be initialized"),
        ))
    }

    /// Creates an OPTIONS request and returns a `ClientRequest` ready to execute.
    pub fn options(&self, url: &str) -> Result<ClientRequest<R>, HttpClientError> {
        let builder = ClientRequestBuilder::options(url)?;
        let prepared = builder.build()?;
        Ok(ClientRequest::new(
            prepared,
            self.config.clone(),
            self.pool.clone().expect("Pool should be initialized"),
        ))
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
    pub fn request(
        &self,
        builder: ClientRequestBuilder,
    ) -> Result<ClientRequest<R>, HttpClientError> {
        let prepared = builder.build()?;
        Ok(ClientRequest::new(
            prepared,
            self.config.clone(),
            self.pool.clone().expect("should have pool"),
        ))
    }
}
