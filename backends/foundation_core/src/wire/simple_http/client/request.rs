//! Request building and prepared request types for HTTP client.
//!
//! This module provides:
//! - `PreparedRequest` - Internal type holding all request data
//! - `ClientRequestBuilder` - Fluent API for building requests

use crate::netcap::SocketAddr;
use crate::wire::simple_http::client::connection::ParsedUrl;
use crate::wire::simple_http::client::errors::HttpClientError;
use crate::wire::simple_http::client::DnsResolver;
use crate::wire::simple_http::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleUrl,
};
use serde::Serialize;
use std::collections::BTreeMap;

/// Prepared HTTP request ready to send.
///
/// Internal type that holds all request data. Convert to `SimpleIncomingRequest`
/// via `into_simple_incoming_request()` to use with HTTP rendering.
pub struct PreparedRequest {
    pub method: SimpleMethod,
    pub url: ParsedUrl,
    pub headers: SimpleHeaders,
    pub body: SendSafeBody,
    pub socket_addrs: Vec<SocketAddr>,
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
            .with_socket_addrs(self.socket_addrs)
            .with_some_body(Some(self.body.into()))
            .build()
            .map_err(|e| HttpClientError::Other(Box::new(e)))?;

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
/// use foundation_core::wire::simple_http::client::{ClientRequestBuilder, StaticSocketAddr};
/// use foundation_core::wire::simple_http::SimpleHeader;
///
/// // Provide a resolver (StaticSocketAddr implements the DnsResolver trait).
/// let resolver = StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80)));
///
/// let request = ClientRequestBuilder::get(resolver, "http://example.com/api")
///     .unwrap()
///     .header(SimpleHeader::HOST, "example.com")
///     .body_text("{\"key\": \"value\"}")
///     .build();
/// ```
pub struct ClientRequestBuilder<R: DnsResolver + 'static> {
    method: SimpleMethod,
    url: ParsedUrl,
    resolver: R,
    headers: SimpleHeaders,
    body: Option<SendSafeBody>,
    socket_addrs: Option<Vec<SocketAddr>>,
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
    ///
    /// # Panics
    ///
    /// If the relevant socket address is not valid or provided.
    ///
    /// Returns `HttpClientError` if the URL is invalid.
    pub fn new(resolver: R, method: SimpleMethod, url: &str) -> Result<Self, HttpClientError> {
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
            resolver,
            url: parsed_url,
            body: None,
            socket_addrs: None,
        })
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

    #[must_use]
    pub fn socket_addr(mut self, addr: SocketAddr) -> Self {
        if let Some(addrs) = &mut self.socket_addrs {
            addrs.push(addr);
        } else {
            self.socket_addrs = Some(vec![addr]);
        }
        self
    }

    #[must_use]
    pub fn socket_addrs(mut self, addrs: Vec<SocketAddr>) -> Self {
        self.socket_addrs = Some(addrs);
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
            serde_json::to_string(value).map_err(|e| HttpClientError::Other(Box::new(e)))?;
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
                            .map(|b| format!("%{b:02X}"))
                            .collect::<String>()
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

    /// Builds the final prepared request.
    ///
    /// Consumes the builder and returns a `PreparedRequest` ready to send.
    #[must_use]
    pub fn build(mut self) -> Result<PreparedRequest, HttpClientError> {
        let host_port = self.url.port_or_default();
        let host_str = self
            .url
            .host_str()
            .ok_or_else(|| HttpClientError::InvalidUrl("Missing host in URL".to_string()))?;

        let socket_addrs = if let Some(addrs) = self.socket_addrs.take() {
            addrs
        } else {
            self.resolver
                .resolve(host_str.as_str(), host_port)
                .map(|addrs| addrs.into_iter().map(SocketAddr::Tcp).collect())
                .map_err(HttpClientError::DnsError)?
        };

        Ok(PreparedRequest {
            socket_addrs,
            method: self.method,
            url: self.url,
            headers: self.headers,
            body: self.body.unwrap_or(SendSafeBody::None),
        })
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
    pub fn get(resolver: R, url: &str) -> Result<Self, HttpClientError> {
        Self::new(resolver, SimpleMethod::GET, url)
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
    pub fn post(resolver: R, url: &str) -> Result<Self, HttpClientError> {
        Self::new(resolver, SimpleMethod::POST, url)
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
    pub fn put(resolver: R, url: &str) -> Result<Self, HttpClientError> {
        Self::new(resolver, SimpleMethod::PUT, url)
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
    pub fn delete(resolver: R, url: &str) -> Result<Self, HttpClientError> {
        Self::new(resolver, SimpleMethod::DELETE, url)
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
    pub fn patch(resolver: R, url: &str) -> Result<Self, HttpClientError> {
        Self::new(resolver, SimpleMethod::PATCH, url)
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
    pub fn head(resolver: R, url: &str) -> Result<Self, HttpClientError> {
        Self::new(resolver, SimpleMethod::HEAD, url)
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
    pub fn options(resolver: R, url: &str) -> Result<Self, HttpClientError> {
        Self::new(resolver, SimpleMethod::OPTIONS, url)
    }
}
