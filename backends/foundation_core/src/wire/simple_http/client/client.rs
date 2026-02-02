//! High-level HTTP client with clean public API.
//!
//! WHY: Provides user-facing API that hides all TaskIterator complexity.
//! Users interact with a simple, ergonomic interface.
//!
//! WHAT: Implements `SimpleHttpClient` with generic DNS resolver parameter,
//! configurable timeouts/redirects/pooling, and convenience methods for all HTTP verbs.
//!
//! HOW: Wraps TaskIterator-based execution in clean API. Builder pattern for configuration.
//! Generic type parameter for DNS resolver (no boxing).

use crate::wire::simple_http::client::{
    ClientRequestBuilder, DnsResolver, HttpClientError, SystemDnsResolver,
};
use crate::wire::simple_http::SimpleHeaders;
use std::time::Duration;

/// Configuration for HTTP client.
///
/// WHY: Centralizes all client configuration in one place. Makes it easy to
/// share configuration across requests or customize per-instance.
///
/// WHAT: Holds timeouts, redirect settings, default headers, and connection pool settings.
///
/// HOW: Created via Default or explicit construction. Passed to SimpleHttpClient
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
            max_redirects: 5,
            default_headers: Default::default(),
            pool_enabled: false,
            pool_max_connections: 10,
        }
    }
}

/// High-level HTTP client with clean API.
///
/// WHY: Provides user-facing API that completely hides TaskIterator complexity.
/// Users work with simple methods like `.get(url).send()`.
///
/// WHAT: Generic HTTP client with pluggable DNS resolver. Supports all HTTP methods,
/// configurable timeouts/redirects, optional connection pooling.
///
/// HOW: Wraps ClientRequestBuilder and TaskIterator execution. Builder pattern
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
/// let client = SimpleHttpClient::new();
/// let response = client.get("http://example.com")?.send()?;
///
/// // With custom resolver
/// let client = SimpleHttpClient::with_resolver(MyResolver::new());
///
/// // With configuration
/// let client = SimpleHttpClient::new()
///     .connect_timeout(Duration::from_secs(10))
///     .max_redirects(3);
/// ```
pub struct SimpleHttpClient<R: DnsResolver = SystemDnsResolver> {
    resolver: R,
    config: ClientConfig,
}

impl SimpleHttpClient<SystemDnsResolver> {
    /// Creates a new HTTP client with default system DNS resolver.
    ///
    /// WHY: Most users want system DNS resolution. This provides zero-config usage.
    ///
    /// WHAT: Creates client with SystemDnsResolver and default configuration.
    ///
    /// # Returns
    ///
    /// A new `SimpleHttpClient` ready to make requests.
    #[must_use]
    pub fn new() -> Self {
        Self {
            resolver: SystemDnsResolver,
            config: ClientConfig::default(),
        }
    }
}

impl Default for SimpleHttpClient<SystemDnsResolver> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: DnsResolver> SimpleHttpClient<R> {
    /// Creates a new HTTP client with custom DNS resolver.
    ///
    /// WHY: Advanced users may need custom DNS (caching, mock for testing, etc.).
    ///
    /// WHAT: Creates client with provided resolver and default configuration.
    ///
    /// # Arguments
    ///
    /// * `resolver` - DNS resolver implementing `DnsResolver` trait
    ///
    /// # Returns
    ///
    /// A new `SimpleHttpClient` using the provided resolver.
    #[must_use]
    pub fn with_resolver(resolver: R) -> Self {
        Self {
            resolver,
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

    /// Enables connection pooling with specified max connections.
    ///
    /// WHY: Connection pooling improves performance for multiple requests.
    ///
    /// WHAT: Builder method to enable pooling and set pool size.
    ///
    /// # Arguments
    ///
    /// * `max_connections` - Maximum connections to pool
    #[must_use]
    pub fn enable_pool(mut self, max_connections: usize) -> Self {
        self.config.pool_enabled = true;
        self.config.pool_max_connections = max_connections;
        self
    }

    // Convenience methods for common HTTP verbs
    // TRANSITIONAL: These will return ClientRequest once api.rs is implemented
    // Current: Returns ClientRequestBuilder (passthrough)
    // Future: Returns ClientRequest ready to execute (requires task-iterator completion)

    /// Creates a GET request.
    ///
    /// WHY: GET is the most common HTTP method. Convenience method for ergonomics.
    ///
    /// WHAT: Creates ClientRequestBuilder for GET method.
    ///
    /// # Arguments
    ///
    /// * `url` - URL string
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if URL is invalid.
    pub fn get(&self, url: &str) -> Result<ClientRequestBuilder, HttpClientError> {
        ClientRequestBuilder::get(url)
    }

    /// Creates a POST request.
    ///
    /// WHY: POST is common for form submissions and API calls.
    ///
    /// WHAT: Creates ClientRequestBuilder for POST method.
    ///
    /// # Arguments
    ///
    /// * `url` - URL string
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if URL is invalid.
    pub fn post(&self, url: &str) -> Result<ClientRequestBuilder, HttpClientError> {
        ClientRequestBuilder::post(url)
    }

    /// Creates a PUT request.
    ///
    /// WHY: PUT is common for resource updates.
    ///
    /// WHAT: Creates ClientRequestBuilder for PUT method.
    ///
    /// # Arguments
    ///
    /// * `url` - URL string
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if URL is invalid.
    pub fn put(&self, url: &str) -> Result<ClientRequestBuilder, HttpClientError> {
        ClientRequestBuilder::put(url)
    }

    /// Creates a DELETE request.
    ///
    /// WHY: DELETE is common for resource removal.
    ///
    /// WHAT: Creates ClientRequestBuilder for DELETE method.
    ///
    /// # Arguments
    ///
    /// * `url` - URL string
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if URL is invalid.
    pub fn delete(&self, url: &str) -> Result<ClientRequestBuilder, HttpClientError> {
        ClientRequestBuilder::delete(url)
    }

    /// Creates a PATCH request.
    ///
    /// WHY: PATCH is common for partial resource updates.
    ///
    /// WHAT: Creates ClientRequestBuilder for PATCH method.
    ///
    /// # Arguments
    ///
    /// * `url` - URL string
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if URL is invalid.
    pub fn patch(&self, url: &str) -> Result<ClientRequestBuilder, HttpClientError> {
        ClientRequestBuilder::patch(url)
    }

    /// Creates a HEAD request.
    ///
    /// WHY: HEAD is useful for checking resource existence without downloading body.
    ///
    /// WHAT: Creates ClientRequestBuilder for HEAD method.
    ///
    /// # Arguments
    ///
    /// * `url` - URL string
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if URL is invalid.
    pub fn head(&self, url: &str) -> Result<ClientRequestBuilder, HttpClientError> {
        ClientRequestBuilder::head(url)
    }

    /// Creates an OPTIONS request.
    ///
    /// WHY: OPTIONS is used for CORS preflight and capability discovery.
    ///
    /// WHAT: Creates ClientRequestBuilder for OPTIONS method.
    ///
    /// # Arguments
    ///
    /// * `url` - URL string
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if URL is invalid.
    pub fn options(&self, url: &str) -> Result<ClientRequestBuilder, HttpClientError> {
        ClientRequestBuilder::options(url)
    }

    /// Creates a request from a builder.
    ///
    /// # Transitional Implementation
    ///
    /// Currently returns ClientRequestBuilder as-is pending api.rs implementation.
    /// Once task-iterator feature completes, this will return ClientRequest with
    /// .execute() method for running HTTP requests.
    ///
    /// WHY: Advanced users may want full control via ClientRequestBuilder.
    ///
    /// WHAT: Takes a pre-configured builder and prepares it for execution.
    ///
    /// # Arguments
    ///
    /// * `builder` - Pre-configured request builder
    ///
    /// # Returns
    ///
    /// A `ClientRequest` ready to execute.
    pub fn request(&self, builder: ClientRequestBuilder) -> ClientRequestBuilder {
        // Transitional: Will return ClientRequest when api.rs is complete
        builder
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::simple_http::client::MockDnsResolver;

    // ========================================================================
    // ClientConfig Tests
    // ========================================================================

    /// WHY: Verify ClientConfig::default creates valid configuration
    /// WHAT: Tests that default config has sensible values
    #[test]
    fn test_client_config_default() {
        let config = ClientConfig::default();

        assert!(config.connect_timeout.is_some());
        assert!(config.read_timeout.is_some());
        assert!(config.write_timeout.is_some());
        assert_eq!(config.max_redirects, 5);
        assert!(config.default_headers.is_empty());
        assert!(!config.pool_enabled);
        assert_eq!(config.pool_max_connections, 10);
    }

    /// WHY: Verify ClientConfig fields are public and modifiable
    /// WHAT: Tests that all config fields can be accessed and modified
    #[test]
    fn test_client_config_fields_public() {
        let mut config = ClientConfig::default();

        config.connect_timeout = Some(Duration::from_secs(10));
        config.max_redirects = 3;
        config.pool_enabled = true;

        assert_eq!(config.connect_timeout, Some(Duration::from_secs(10)));
        assert_eq!(config.max_redirects, 3);
        assert!(config.pool_enabled);
    }

    /// WHY: Verify ClientConfig implements Clone
    /// WHAT: Tests that config can be cloned
    #[test]
    fn test_client_config_clone() {
        let config = ClientConfig::default();
        let cloned = config.clone();

        assert_eq!(cloned.max_redirects, config.max_redirects);
        assert_eq!(cloned.pool_enabled, config.pool_enabled);
    }

    // ========================================================================
    // SimpleHttpClient Tests
    // ========================================================================

    /// WHY: Verify SimpleHttpClient::new creates client with system resolver
    /// WHAT: Tests that new() creates client with default config
    #[test]
    fn test_simple_http_client_new() {
        let client = SimpleHttpClient::new();

        assert_eq!(client.config.max_redirects, 5);
        assert!(!client.config.pool_enabled);
    }

    /// WHY: Verify SimpleHttpClient::with_resolver accepts custom resolver
    /// WHAT: Tests that with_resolver works with MockDnsResolver
    #[test]
    fn test_simple_http_client_with_resolver() {
        let resolver = MockDnsResolver::new();
        let client = SimpleHttpClient::with_resolver(resolver);

        assert_eq!(client.config.max_redirects, 5);
    }

    /// WHY: Verify SimpleHttpClient::config sets configuration
    /// WHAT: Tests that config() replaces configuration
    #[test]
    fn test_simple_http_client_config() {
        let mut custom_config = ClientConfig::default();
        custom_config.max_redirects = 10;

        let client = SimpleHttpClient::new().config(custom_config);

        assert_eq!(client.config.max_redirects, 10);
    }

    /// WHY: Verify SimpleHttpClient::connect_timeout sets timeout
    /// WHAT: Tests that builder method sets connect timeout
    #[test]
    fn test_simple_http_client_connect_timeout() {
        let client = SimpleHttpClient::new().connect_timeout(Duration::from_secs(5));

        assert_eq!(client.config.connect_timeout, Some(Duration::from_secs(5)));
    }

    /// WHY: Verify SimpleHttpClient::read_timeout sets timeout
    /// WHAT: Tests that builder method sets read timeout
    #[test]
    fn test_simple_http_client_read_timeout() {
        let client = SimpleHttpClient::new().read_timeout(Duration::from_secs(15));

        assert_eq!(client.config.read_timeout, Some(Duration::from_secs(15)));
    }

    /// WHY: Verify SimpleHttpClient::write_timeout sets timeout
    /// WHAT: Tests that builder method sets write timeout
    #[test]
    fn test_simple_http_client_write_timeout() {
        let client = SimpleHttpClient::new().write_timeout(Duration::from_secs(20));

        assert_eq!(client.config.write_timeout, Some(Duration::from_secs(20)));
    }

    /// WHY: Verify SimpleHttpClient::max_redirects sets redirect limit
    /// WHAT: Tests that builder method sets max redirects
    #[test]
    fn test_simple_http_client_max_redirects() {
        let client = SimpleHttpClient::new().max_redirects(3);

        assert_eq!(client.config.max_redirects, 3);
    }

    /// WHY: Verify SimpleHttpClient::enable_pool enables connection pooling
    /// WHAT: Tests that builder method enables pooling and sets max connections
    #[test]
    fn test_simple_http_client_enable_pool() {
        let client = SimpleHttpClient::new().enable_pool(20);

        assert!(client.config.pool_enabled);
        assert_eq!(client.config.pool_max_connections, 20);
    }

    /// WHY: Verify SimpleHttpClient builder methods are chainable
    /// WHAT: Tests that multiple builder methods can be chained
    #[test]
    fn test_simple_http_client_builder_chaining() {
        let client = SimpleHttpClient::new()
            .connect_timeout(Duration::from_secs(10))
            .max_redirects(3)
            .enable_pool(15);

        assert_eq!(client.config.connect_timeout, Some(Duration::from_secs(10)));
        assert_eq!(client.config.max_redirects, 3);
        assert!(client.config.pool_enabled);
        assert_eq!(client.config.pool_max_connections, 15);
    }

    /// WHY: Verify SimpleHttpClient::get creates GET request builder
    /// WHAT: Tests that get() returns ClientRequestBuilder for GET
    #[test]
    fn test_simple_http_client_get() {
        let client = SimpleHttpClient::new();
        let builder = client.get("http://example.com").unwrap();

        let request = builder.build();
        assert_eq!(request.url.host_str().unwrap(), "example.com");
    }

    /// WHY: Verify SimpleHttpClient::get validates URL
    /// WHAT: Tests that get() returns error for invalid URL
    #[test]
    fn test_simple_http_client_get_invalid_url() {
        let client = SimpleHttpClient::new();
        let result = client.get("not a url");

        assert!(result.is_err());
    }

    /// WHY: Verify SimpleHttpClient::post creates POST request builder
    /// WHAT: Tests that post() returns ClientRequestBuilder for POST
    #[test]
    fn test_simple_http_client_post() {
        let client = SimpleHttpClient::new();
        let builder = client.post("http://example.com").unwrap();

        let request = builder.build();
        assert_eq!(request.url.host_str().unwrap(), "example.com");
    }

    /// WHY: Verify SimpleHttpClient::put creates PUT request builder
    /// WHAT: Tests that put() returns ClientRequestBuilder for PUT
    #[test]
    fn test_simple_http_client_put() {
        let client = SimpleHttpClient::new();
        let builder = client.put("http://example.com").unwrap();

        let request = builder.build();
        assert_eq!(request.url.host_str().unwrap(), "example.com");
    }

    /// WHY: Verify SimpleHttpClient::delete creates DELETE request builder
    /// WHAT: Tests that delete() returns ClientRequestBuilder for DELETE
    #[test]
    fn test_simple_http_client_delete() {
        let client = SimpleHttpClient::new();
        let builder = client.delete("http://example.com").unwrap();

        let request = builder.build();
        assert_eq!(request.url.host_str().unwrap(), "example.com");
    }

    /// WHY: Verify SimpleHttpClient::patch creates PATCH request builder
    /// WHAT: Tests that patch() returns ClientRequestBuilder for PATCH
    #[test]
    fn test_simple_http_client_patch() {
        let client = SimpleHttpClient::new();
        let builder = client.patch("http://example.com").unwrap();

        let request = builder.build();
        assert_eq!(request.url.host_str().unwrap(), "example.com");
    }

    /// WHY: Verify SimpleHttpClient::head creates HEAD request builder
    /// WHAT: Tests that head() returns ClientRequestBuilder for HEAD
    #[test]
    fn test_simple_http_client_head() {
        let client = SimpleHttpClient::new();
        let builder = client.head("http://example.com").unwrap();

        let request = builder.build();
        assert_eq!(request.url.host_str().unwrap(), "example.com");
    }

    /// WHY: Verify SimpleHttpClient::options creates OPTIONS request builder
    /// WHAT: Tests that options() returns ClientRequestBuilder for OPTIONS
    #[test]
    fn test_simple_http_client_options() {
        let client = SimpleHttpClient::new();
        let builder = client.options("http://example.com").unwrap();

        let request = builder.build();
        assert_eq!(request.url.host_str().unwrap(), "example.com");
    }

    /// WHY: Verify SimpleHttpClient::request accepts pre-built builder
    /// WHAT: Tests that request() can take ClientRequestBuilder
    #[test]
    fn test_simple_http_client_request() {
        let client = SimpleHttpClient::new();
        let builder = ClientRequestBuilder::get("http://example.com").unwrap();

        let _result = client.request(builder);
        // TEST NOTE: Assertion pending ClientRequest implementation
        // Once api.rs is complete, assert on ClientRequest type and .execute() method
    }

    /// WHY: Verify SimpleHttpClient implements Default
    /// WHAT: Tests that Default trait is implemented
    #[test]
    fn test_simple_http_client_default() {
        let client = SimpleHttpClient::default();

        assert_eq!(client.config.max_redirects, 5);
    }
}
