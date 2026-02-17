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
            SimpleUrl::url_only(self.url.path().to_string())
        };

        // Create SimpleIncomingRequest using builder
        let request = SimpleIncomingRequest::builder()
            .with_url(simple_url)
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
/// use foundation_core::wire::simple_http::client::ClientRequestBuilder;
///
/// let request = ClientRequestBuilder::get("http://example.com/api")
///     .unwrap()
///     .header(
///         foundation_core::wire::simple_http::SimpleHeader::HOST,
///         "example.com"
///     )
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
        let host_port = parsed_url.port_or_default();
        let host_str = parsed_url
            .host_str()
            .ok_or_else(|| HttpClientError::InvalidUrl("Missing host in URL".to_string()))?;

        let host = if parsed_url.port().is_some() {
            format!("{}:{}", host_str, parsed_url.port_or_default())
        } else {
            host_str.clone()
        };
        headers.insert(SimpleHeader::HOST, vec![host]);

        let socket_addrs = resolver
            .resolve(host_str.as_str(), host_port)
            .map(|addrs| addrs.into_iter().map(SocketAddr::Tcp).collect())
            .map_err(HttpClientError::DnsError)?;

        Ok(Self {
            method,
            headers,
            resolver,
            url: parsed_url,
            body: None,
            socket_addrs: Some(socket_addrs),
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
    pub fn build(self) -> PreparedRequest {
        PreparedRequest {
            method: self.method,
            url: self.url,
            headers: self.headers,
            body: self.body.unwrap_or(SendSafeBody::None),
            socket_addrs: self.socket_addrs.expect("Expected socket addr"),
        }
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

#[cfg(test)]
mod tests {
    use crate::wire::simple_http::client::StaticSocketAddr;

    use super::*;

    /// WHY: Verify ClientRequestBuilder::new creates builder
    /// WHAT: Tests that new creates a request builder with URL and method
    #[test]
    fn test_client_request_builder_new() {
        let builder = ClientRequestBuilder::new(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            SimpleMethod::GET,
            "http://example.com",
        )
        .unwrap();
        assert_eq!(builder.url.host_str().unwrap(), "example.com");
        assert_eq!(builder.url.port_or_default(), 80);
        assert!(matches!(builder.method, SimpleMethod::GET));
    }

    /// WHY: Verify ClientRequestBuilder::new validates URL
    /// WHAT: Tests that invalid URLs return error
    #[test]
    fn test_client_request_builder_new_invalid_url() {
        let result = ClientRequestBuilder::new(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            SimpleMethod::GET,
            "not a url",
        );
        assert!(result.is_err());
    }

    /// WHY: Verify ClientRequestBuilder::header adds header
    /// WHAT: Tests that header method adds header to request
    #[test]
    fn test_client_request_builder_header() {
        let builder = ClientRequestBuilder::get(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com",
        )
        .unwrap()
        .header(SimpleHeader::CONTENT_TYPE, "application/json");

        assert!(builder.headers.contains_key(&SimpleHeader::CONTENT_TYPE));
        assert_eq!(
            builder.headers.get(&SimpleHeader::CONTENT_TYPE).unwrap()[0],
            "application/json"
        );
    }

    /// WHY: Verify ClientRequestBuilder::body_text sets text body
    /// WHAT: Tests that body_text sets body and content headers
    #[test]
    fn test_client_request_builder_body_text() {
        let builder = ClientRequestBuilder::post(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com",
        )
        .unwrap()
        .body_text("Hello, World!");

        assert!(matches!(builder.body, Some(SendSafeBody::Text(_))));
        assert!(builder.headers.contains_key(&SimpleHeader::CONTENT_LENGTH));
        assert!(builder.headers.contains_key(&SimpleHeader::CONTENT_TYPE));
    }

    /// WHY: Verify ClientRequestBuilder::body_bytes sets binary body
    /// WHAT: Tests that body_bytes sets body and content headers
    #[test]
    fn test_client_request_builder_body_bytes() {
        let builder = ClientRequestBuilder::post(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com",
        )
        .unwrap()
        .body_bytes(vec![1, 2, 3, 4]);

        assert!(matches!(builder.body, Some(SendSafeBody::Bytes(_))));
        assert!(builder.headers.contains_key(&SimpleHeader::CONTENT_LENGTH));
    }

    /// WHY: Verify ClientRequestBuilder::body_json serializes to JSON
    /// WHAT: Tests that body_json creates JSON body
    #[test]
    fn test_client_request_builder_body_json() {
        #[derive(Serialize)]
        struct TestData {
            key: String,
        }

        let data = TestData {
            key: "value".to_string(),
        };

        let builder = ClientRequestBuilder::post(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com",
        )
        .unwrap()
        .body_json(&data)
        .unwrap();

        assert!(matches!(builder.body, Some(SendSafeBody::Text(_))));
        if let Some(SendSafeBody::Text(json)) = &builder.body {
            assert!(json.contains("\"key\""));
            assert!(json.contains("\"value\""));
        }
    }

    /// WHY: Verify ClientRequestBuilder::body_form encodes form data
    /// WHAT: Tests that body_form creates URL-encoded body
    #[test]
    fn test_client_request_builder_body_form() {
        let params = vec![
            ("key1".to_string(), "value1".to_string()),
            ("key2".to_string(), "value2".to_string()),
        ];

        let builder = ClientRequestBuilder::post(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com",
        )
        .unwrap()
        .body_form(&params);

        assert!(matches!(builder.body, Some(SendSafeBody::Text(_))));
        if let Some(SendSafeBody::Text(form)) = &builder.body {
            assert!(form.contains("key1=value1"));
            assert!(form.contains("key2=value2"));
        }
    }

    /// WHY: Verify ClientRequestBuilder::build creates PreparedRequest
    /// WHAT: Tests that build consumes builder and creates prepared request
    #[test]
    fn test_client_request_builder_build() {
        let resolver = StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80)));
        let prepared = ClientRequestBuilder::get(resolver, "http://example.com")
            .unwrap()
            .build();

        assert_eq!(prepared.url.host_str().unwrap(), "example.com");
        assert!(matches!(prepared.method, SimpleMethod::GET));
        assert!(matches!(prepared.body, SendSafeBody::None));
    }

    /// WHY: Verify convenience methods create correct builders
    /// WHAT: Tests get, post, put, delete, patch, head, options methods
    #[test]
    fn test_client_request_builder_convenience_methods() {
        let get = ClientRequestBuilder::get(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com",
        )
        .unwrap();
        assert!(matches!(get.method, SimpleMethod::GET));

        let post = ClientRequestBuilder::post(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com",
        )
        .unwrap();
        assert!(matches!(post.method, SimpleMethod::POST));

        let put = ClientRequestBuilder::put(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com",
        )
        .unwrap();
        assert!(matches!(put.method, SimpleMethod::PUT));

        let delete = ClientRequestBuilder::delete(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com",
        )
        .unwrap();
        assert!(matches!(delete.method, SimpleMethod::DELETE));

        let patch = ClientRequestBuilder::patch(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com",
        )
        .unwrap();
        assert!(matches!(patch.method, SimpleMethod::PATCH));

        let head = ClientRequestBuilder::head(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com",
        )
        .unwrap();
        assert!(matches!(head.method, SimpleMethod::HEAD));

        let options = ClientRequestBuilder::options(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com",
        )
        .unwrap();
        assert!(matches!(options.method, SimpleMethod::OPTIONS));
    }

    /// WHY: Verify Host header is automatically added
    /// WHAT: Tests that Host header is set from URL
    #[test]
    fn test_client_request_builder_auto_host_header() {
        let builder = ClientRequestBuilder::get(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com",
        )
        .unwrap();
        assert!(builder.headers.contains_key(&SimpleHeader::HOST));
        assert_eq!(
            builder.headers.get(&SimpleHeader::HOST).unwrap()[0],
            "example.com"
        );

        let builder2 = ClientRequestBuilder::get(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com:8080",
        )
        .unwrap();
        assert_eq!(
            builder2.headers.get(&SimpleHeader::HOST).unwrap()[0],
            "example.com:8080"
        );
    }

    /// WHY: Verify PreparedRequest::into_simple_incoming_request works
    /// WHAT: Tests that prepared request can be converted to SimpleIncomingRequest
    #[test]
    fn test_prepared_request_into_simple_incoming_request() {
        let prepared = ClientRequestBuilder::get(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com/path",
        )
        .unwrap()
        .build();

        let simple_request = prepared.into_simple_incoming_request().unwrap();
        assert_eq!(simple_request.method, SimpleMethod::GET);
        assert_eq!(simple_request.proto, Proto::HTTP11);
    }

    /// WHY: Verify PreparedRequest with query string works
    /// WHAT: Tests that query strings are preserved
    #[test]
    fn test_prepared_request_with_query() {
        let prepared = ClientRequestBuilder::get(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com/path?foo=bar",
        )
        .unwrap()
        .build();

        let simple_request = prepared.into_simple_incoming_request().unwrap();
        assert!(simple_request.request_url.url.contains("?foo=bar"));
    }

    /// WHY: Verify PreparedRequest is Send
    /// WHAT: Compile-time test that PreparedRequest can be sent across threads
    #[test]
    fn test_prepared_request_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<PreparedRequest>();
    }
}
