//! Request building and prepared request types for HTTP client.
//!
//! This module provides:
//! - `PreparedRequest` - Internal type holding all request data
//! - `ClientRequestBuilder` - Fluent API for building requests

use crate::wire::simple_http::client::connection::ParsedUrl;
use crate::wire::simple_http::client::{
    ClientConfig, ClientRequest, DnsResolver, Extensions, HttpConnectionPool, MiddlewareChain,
    SystemDnsResolver,
};
use crate::wire::simple_http::HttpClientError;
use crate::wire::simple_http::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleUrl,
};
use base64::prelude::*;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Prepared HTTP request ready to send.
///
/// Internal type that holds all request data. Convert to `SimpleIncomingRequest`
/// via `into_simple_incoming_request()` to use with HTTP rendering.
pub struct PreparedRequest {
    pub method: SimpleMethod,
    pub url: ParsedUrl,
    pub headers: SimpleHeaders,
    pub body: SendSafeBody,
    pub extensions: Extensions,
}

impl PreparedRequest {
    /// Converts this prepared request into a `SimpleIncomingRequest`.
    ///
    /// The returned request can be rendered using `Http11::request(req).http_render()`.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if the request cannot be built.
    pub fn into_simple_incoming_request(self) -> Result<SimpleIncomingRequest, HttpClientError> {
        // Convert Uri to SimpleUrl
        let simple_url = if let Some(query) = self.url.query() {
            SimpleUrl::url_with_query(format!("{}?{}", self.url.path(), query))
        } else {
            SimpleUrl::url_only(self.url.to_string())
        };

        // Create SimpleIncomingRequest using builder
        let request = SimpleIncomingRequest::builder()
            .with_url(simple_url)
            .with_uri(self.url)
            .with_method(self.method)
            .with_proto(Proto::HTTP11)
            .with_headers(self.headers)
            .with_some_body(Some(self.body))
            .with_extensions(self.extensions)
            .build()
            .map_err(|e| HttpClientError::FailedWith(Box::new(e)))?;

        Ok(request)
    }
}

/// Fluent builder for HTTP requests.
///
/// Provides a convenient API for constructing HTTP requests with
/// methods, headers, and body content.
///
/// # Examples
///
/// ```
/// use foundation_core::wire::simple_http::client::{ClientRequestBuilder, SystemDnsResolver};
/// use foundation_core::wire::simple_http::SimpleHeader;
///
/// // Build a GET request
/// let request = ClientRequestBuilder::<SystemDnsResolver>::get("http://example.com/api")
///     .unwrap()
///     .header(SimpleHeader::HOST, "example.com");
///
/// // Can build the request
/// assert!(request.build().is_ok());
/// ```
pub struct ClientRequestBuilder<R: DnsResolver + 'static> {
    method: SimpleMethod,
    url: ParsedUrl,
    headers: SimpleHeaders,
    body: Option<SendSafeBody>,
    config: Option<ClientConfig>,
    pool: Option<Arc<HttpConnectionPool<R>>>,
    middleware_chain: Option<Arc<MiddlewareChain>>,
}

impl<R: DnsResolver + 'static> ClientRequestBuilder<R> {
    /// Builds the final prepared request.
    ///
    /// Consumes the builder and returns a `PreparedRequest` ready to send.
    ///
    /// # Errors
    ///
    /// Currently always returns `Ok`. Returns `Result` for future extensibility.
    #[must_use = "builder must be consumed to produce a PreparedRequest"]
    pub fn build(self) -> Result<PreparedRequest, HttpClientError> {
        Ok(PreparedRequest {
            method: self.method,
            url: self.url,
            headers: self.headers,
            body: self.body.unwrap_or(SendSafeBody::None),
            extensions: Extensions::new(),
        })
    }
}

impl ClientRequestBuilder<SystemDnsResolver> {
    /// Builds a client request with system DNS resolver.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError::NoPool` if connection pool is not set.
    #[must_use = "builder must be consumed to produce a ClientRequest"]
    pub fn system_client(self) -> Result<ClientRequest<SystemDnsResolver>, HttpClientError> {
        self.build_client()
    }
}

impl<R: DnsResolver + Default + 'static> ClientRequestBuilder<R> {
    /// Builds the final prepared request with a client wrapper.
    ///
    /// Consumes the builder and returns a `ClientRequest` ready to execute.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError::NoPool` if connection pool is not configured.
    #[must_use = "builder must be consumed to produce a ClientRequest"]
    pub fn build_client(self) -> Result<ClientRequest<R>, HttpClientError> {
        let prepared = PreparedRequest {
            method: self.method,
            url: self.url,
            headers: self.headers,
            body: self.body.unwrap_or(SendSafeBody::None),
            extensions: Extensions::new(),
        };

        let pool = self.pool.unwrap_or_default();
        let config = self.config.unwrap_or_default();
        let middleware_chain = self
            .middleware_chain
            .unwrap_or_else(|| Arc::new(MiddlewareChain::new()));
        Ok(ClientRequest::new(prepared, config, pool, middleware_chain))
    }

    pub fn build_send_request(self) -> Result<super::SendRequestTask<R>, HttpClientError> {
        let prepared_request = PreparedRequest {
            method: self.method,
            url: self.url,
            headers: self.headers,
            body: self.body.unwrap_or(SendSafeBody::None),
            extensions: Extensions::new(),
        };

        let pool = self.pool.unwrap_or_default();
        let config = self.config.unwrap_or_default();

        Ok(super::SendRequestTask::new(
            prepared_request,
            config.max_redirects,
            pool,
            config,
        ))
    }
}

impl<R: DnsResolver + 'static> ClientRequestBuilder<R> {
    /// Creates a new request builder with the given method and URL.
    ///
    /// # Arguments
    ///
    /// * `method` - HTTP method (GET, POST, etc.)
    /// * `url` - URL string to parse
    ///
    /// # Returns
    ///
    /// A new `ClientRequestBuilder` ready to configure.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError::InvalidUrl` if the URL cannot be parsed.
    /// Returns `HttpClientError::UnsupportedScheme` if the URL scheme is not http or https.
    ///
    /// # Panics
    ///
    /// Panics if the socket address cannot be determined from the URL.
    pub fn new(method: SimpleMethod, url: &str) -> Result<Self, HttpClientError> {
        let parsed_url = ParsedUrl::parse(url)?;
        let mut headers = BTreeMap::new();

        // Add required Host header
        let host_str = parsed_url
            .host_str()
            .ok_or_else(|| HttpClientError::InvalidUrl("Missing host in URL".to_string()))?;

        let host = if parsed_url.port().is_some() {
            format!("{}:{}", host_str, parsed_url.port_or_default())
        } else {
            host_str.clone()
        };
        headers.insert(SimpleHeader::HOST, vec![host]);

        Ok(Self {
            method,
            headers,
            url: parsed_url,
            body: None,
            pool: None,
            config: None,
            middleware_chain: None,
        })
    }

    #[must_use]
    pub fn with_pool(mut self, pool: Option<Arc<HttpConnectionPool<R>>>) -> Self {
        self.pool = pool;
        self
    }

    /// Explicitly disable connection pooling for this request by setting pool to None.
    #[must_use]
    pub fn without_pool(mut self) -> Self {
        self.pool = None;
        self
    }

    #[must_use]
    pub fn with_middleware(mut self, chain: Arc<MiddlewareChain>) -> Self {
        self.middleware_chain = Some(chain);
        self
    }

    #[must_use]
    pub fn pool(mut self, pool: Arc<HttpConnectionPool<R>>) -> Self {
        self.pool = Some(pool);
        self
    }

    #[must_use]
    pub fn client_config(mut self, config: ClientConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Adds a single header to the request.
    ///
    /// # Arguments
    ///
    /// * `key` - Header name
    /// * `value` - Header value
    #[must_use]
    pub fn header(mut self, key: SimpleHeader, value: impl Into<String>) -> Self {
        self.headers.entry(key).or_default().push(value.into());
        self
    }

    #[must_use]
    pub fn add_header(mut self, key: impl Into<SimpleHeader>, value: impl Into<String>) -> Self {
        self.headers
            .entry(key.into())
            .or_default()
            .push(value.into());
        self
    }

    /// Replaces all headers with the given headers.
    ///
    /// # Arguments
    ///
    /// * `headers` - New headers to use
    #[must_use]
    pub fn headers(mut self, headers: SimpleHeaders) -> Self {
        self.headers = headers;
        self
    }

    /// Sets the body as plain text.
    ///
    /// Automatically sets Content-Type to text/plain if not already set.
    ///
    /// # Arguments
    ///
    /// * `text` - Text content
    #[must_use]
    pub fn body_text(mut self, text: impl Into<String>) -> Self {
        let text_string = text.into();
        let content_length = text_string.len().to_string();

        self.headers
            .entry(SimpleHeader::CONTENT_TYPE)
            .or_insert_with(|| vec!["text/plain".to_string()]);
        self.headers
            .insert(SimpleHeader::CONTENT_LENGTH, vec![content_length]);

        self.body = Some(SendSafeBody::Text(text_string));
        self
    }

    /// Sets the body as raw bytes.
    ///
    /// Automatically sets Content-Type to application/octet-stream if not already set.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Binary content
    #[must_use]
    pub fn body_bytes(mut self, bytes: Vec<u8>) -> Self {
        let content_length = bytes.len().to_string();

        self.headers
            .entry(SimpleHeader::CONTENT_TYPE)
            .or_insert_with(|| vec!["application/octet-stream".to_string()]);
        self.headers
            .insert(SimpleHeader::CONTENT_LENGTH, vec![content_length]);

        self.body = Some(SendSafeBody::Bytes(bytes));
        self
    }

    /// Sets the body as JSON.
    ///
    /// Automatically sets Content-Type to application/json.
    ///
    /// # Arguments
    ///
    /// * `value` - Value to serialize to JSON
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if JSON serialization fails.
    pub fn body_json<T: Serialize>(mut self, value: &T) -> Result<Self, HttpClientError> {
        let json_string =
            serde_json::to_string(value).map_err(|e| HttpClientError::FailedWith(Box::new(e)))?;
        let content_length = json_string.len().to_string();

        self.headers.insert(
            SimpleHeader::CONTENT_TYPE,
            vec!["application/json".to_string()],
        );
        self.headers
            .insert(SimpleHeader::CONTENT_LENGTH, vec![content_length]);

        self.body = Some(SendSafeBody::Text(json_string));
        Ok(self)
    }

    /// Sets the body as form data.
    ///
    /// Automatically sets Content-Type to application/x-www-form-urlencoded.
    ///
    /// # Arguments
    ///
    /// * `params` - Form parameters as key-value pairs
    #[must_use]
    pub fn body_form(mut self, params: &[(String, String)]) -> Self {
        // Simple URL encoding (percent-encoding for form data)
        fn urlencode(s: &str) -> String {
            s.chars()
                .map(|c| match c {
                    'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                    ' ' => "+".to_string(), // Space becomes +
                    _ => {
                        let bytes = c.to_string().into_bytes();
                        bytes
                            .iter()
                            .fold(String::new(), |acc, b| format!("{acc:}%{b:02X}"))
                    }
                })
                .collect()
        }

        let form_string = params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencode(k), urlencode(v)))
            .collect::<Vec<_>>()
            .join("&");
        let content_length = form_string.len().to_string();

        self.headers.insert(
            SimpleHeader::CONTENT_TYPE,
            vec!["application/x-www-form-urlencoded".to_string()],
        );
        self.headers
            .insert(SimpleHeader::CONTENT_LENGTH, vec![content_length]);

        self.body = Some(SendSafeBody::Text(form_string));
        self
    }

    // Convenience methods for common HTTP methods

    /// Creates a GET request builder.
    ///
    /// # Arguments
    ///
    /// * `url` - URL string
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if the URL is invalid.
    pub fn get(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::GET, url)
    }

    /// Creates a POST request builder.
    ///
    /// # Arguments
    ///
    /// * `url` - URL string
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if the URL is invalid.
    pub fn post(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::POST, url)
    }

    /// Creates a PUT request builder.
    ///
    /// # Arguments
    ///
    /// * `url` - URL string
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if the URL is invalid.
    pub fn put(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::PUT, url)
    }

    /// Creates a DELETE request builder.
    ///
    /// # Arguments
    ///
    /// * `url` - URL string
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if the URL is invalid.
    pub fn delete(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::DELETE, url)
    }

    /// Creates a PATCH request builder.
    ///
    /// # Arguments
    ///
    /// * `url` - URL string
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if the URL is invalid.
    pub fn patch(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::PATCH, url)
    }

    /// Creates a HEAD request builder.
    ///
    /// # Arguments
    ///
    /// * `url` - URL string
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if the URL is invalid.
    pub fn head(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::HEAD, url)
    }

    /// Creates an OPTIONS request builder.
    ///
    /// # Arguments
    ///
    /// * `url` - URL string
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if the URL is invalid.
    pub fn options(url: &str) -> Result<Self, HttpClientError> {
        Self::new(SimpleMethod::OPTIONS, url)
    }

    // Authentication helper methods

    /// Sets HTTP Basic Authentication header.
    ///
    /// # Purpose (WHY)
    ///
    /// Provides convenient API for HTTP Basic Authentication (RFC 7617).
    /// Automatically encodes username:password in base64 and sets the
    /// Authorization header.
    ///
    /// # Arguments (WHAT)
    ///
    /// * `username` - Username for authentication
    /// * `password` - Password for authentication
    ///
    /// # Returns (HOW)
    ///
    /// Returns self for method chaining.
    ///
    /// # Panics
    ///
    /// This function cannot panic.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::client::ClientRequestBuilder;
    /// use foundation_core::wire::simple_http::client::SystemDnsResolver;
    ///
    /// let request = ClientRequestBuilder::<SystemDnsResolver>::get("http://example.com")
    ///     .unwrap()
    ///     .basic_auth("username", "password");
    /// ```
    #[must_use]
    pub fn basic_auth(self, username: &str, password: &str) -> Self {
        let credentials = format!("{username}:{password}");
        let encoded = BASE64_STANDARD.encode(credentials.as_bytes());
        self.header(SimpleHeader::AUTHORIZATION, format!("Basic {encoded}"))
    }

    /// Sets HTTP Basic Authentication header with optional password.
    ///
    /// # Purpose (WHY)
    ///
    /// Provides API for Basic Authentication when password is optional.
    /// If password is None, uses empty string (per RFC 7617 section 2).
    ///
    /// # Arguments (WHAT)
    ///
    /// * `username` - Username for authentication
    /// * `password` - Optional password (empty string if None)
    ///
    /// # Returns (HOW)
    ///
    /// Returns self for method chaining.
    ///
    /// # Panics
    ///
    /// This function cannot panic.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::client::ClientRequestBuilder;
    /// use foundation_core::wire::simple_http::client::SystemDnsResolver;
    ///
    /// // With password
    /// let request = ClientRequestBuilder::<SystemDnsResolver>::get("http://example.com")
    ///     .unwrap()
    ///     .basic_auth_opt("username", Some("password"));
    ///
    /// // Without password (empty string)
    /// let request2 = ClientRequestBuilder::<SystemDnsResolver>::get("http://example.com")
    ///     .unwrap()
    ///     .basic_auth_opt("username", None);
    /// ```
    #[must_use]
    pub fn basic_auth_opt(self, username: &str, password: Option<&str>) -> Self {
        self.basic_auth(username, password.unwrap_or(""))
    }

    /// Sets HTTP Bearer Token Authentication header.
    ///
    /// # Purpose (WHY)
    ///
    /// Provides convenient API for OAuth 2.0 Bearer Token authentication (RFC 6750).
    /// Automatically formats the token with Bearer prefix and sets the
    /// Authorization header.
    ///
    /// # Arguments (WHAT)
    ///
    /// * `token` - Bearer token (JWT, OAuth token, etc.)
    ///
    /// # Returns (HOW)
    ///
    /// Returns self for method chaining.
    ///
    /// # Panics
    ///
    /// This function cannot panic.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::client::ClientRequestBuilder;
    /// use foundation_core::wire::simple_http::client::SystemDnsResolver;
    ///
    /// let request = ClientRequestBuilder::<SystemDnsResolver>::get("http://api.example.com")
    ///     .unwrap()
    ///     .bearer_token("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...");
    /// ```
    #[must_use]
    pub fn bearer_token(self, token: &str) -> Self {
        self.header(SimpleHeader::AUTHORIZATION, format!("Bearer {token}"))
    }

    /// Alias for `bearer_token()`.
    ///
    /// # Purpose (WHY)
    ///
    /// Provides alternative method name for Bearer authentication.
    /// Some users prefer "`bearer_auth`" naming convention.
    ///
    /// # Arguments (WHAT)
    ///
    /// * `token` - Bearer token (JWT, OAuth token, etc.)
    ///
    /// # Returns (HOW)
    ///
    /// Returns self for method chaining.
    ///
    /// # Panics
    ///
    /// This function cannot panic.
    #[must_use]
    pub fn bearer_auth(self, token: &str) -> Self {
        self.bearer_token(token)
    }

    /// Sets a custom API key header.
    ///
    /// # Purpose (WHY)
    ///
    /// Provides convenient API for custom API key authentication.
    /// Many REST APIs use custom headers like X-API-Key, X-Auth-Token, etc.
    ///
    /// # Arguments (WHAT)
    ///
    /// * `header_name` - Name of the custom header (e.g., "X-API-Key")
    /// * `key` - API key value
    ///
    /// # Returns (HOW)
    ///
    /// Returns self for method chaining.
    ///
    /// # Panics
    ///
    /// This function cannot panic.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::client::ClientRequestBuilder;
    /// use foundation_core::wire::simple_http::client::SystemDnsResolver;
    ///
    /// let request = ClientRequestBuilder::<SystemDnsResolver>::get("http://api.example.com")
    ///     .unwrap()
    ///     .api_key("X-API-Key", "secret-key-123");
    /// ```
    #[must_use]
    pub fn api_key(self, header_name: &str, key: &str) -> Self {
        self.add_header(header_name.to_string(), key.to_string())
    }

    /// Sets the X-API-Key header.
    ///
    /// # Purpose (WHY)
    ///
    /// Convenience method for the common X-API-Key header pattern.
    /// Many REST APIs use X-API-Key as the standard header name.
    ///
    /// # Arguments (WHAT)
    ///
    /// * `key` - API key value
    ///
    /// # Returns (HOW)
    ///
    /// Returns self for method chaining.
    ///
    /// # Panics
    ///
    /// This function cannot panic.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::client::ClientRequestBuilder;
    /// use foundation_core::wire::simple_http::client::SystemDnsResolver;
    ///
    /// let request = ClientRequestBuilder::<SystemDnsResolver>::get("http://api.example.com")
    ///     .unwrap()
    ///     .x_api_key("secret-key-123");
    /// ```
    #[must_use]
    pub fn x_api_key(self, key: &str) -> Self {
        self.api_key("X-API-Key", key)
    }

    /// Sets Authorization header with custom scheme and credentials.
    ///
    /// # Purpose (WHY)
    ///
    /// Provides flexible API for non-standard authentication schemes.
    /// Allows using custom authentication schemes beyond Basic/Bearer.
    ///
    /// # Arguments (WHAT)
    ///
    /// * `scheme` - Authentication scheme (e.g., "Digest", "HOBA", "AWS4-HMAC-SHA256")
    /// * `credentials` - Credentials or token for the scheme
    ///
    /// # Returns (HOW)
    ///
    /// Returns self for method chaining.
    ///
    /// # Panics
    ///
    /// This function cannot panic.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::client::ClientRequestBuilder;
    /// use foundation_core::wire::simple_http::client::SystemDnsResolver;
    ///
    /// // Custom auth scheme
    /// let request = ClientRequestBuilder::<SystemDnsResolver>::get("http://api.example.com")
    ///     .unwrap()
    ///     .authorization("CustomScheme", "token123");
    /// ```
    #[must_use]
    pub fn authorization(self, scheme: &str, credentials: &str) -> Self {
        self.header(
            SimpleHeader::AUTHORIZATION,
            format!("{scheme} {credentials}"),
        )
    }
}
