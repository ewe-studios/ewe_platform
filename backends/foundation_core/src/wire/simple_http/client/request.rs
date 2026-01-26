//! Request building and prepared request types for HTTP client.
//!
//! This module provides:
//! - `PreparedRequest` - Internal type holding all request data
//! - `ClientRequestBuilder` - Fluent API for building requests

use crate::wire::simple_http::client::connection::ParsedUrl;
use crate::wire::simple_http::client::errors::HttpClientError;
use crate::wire::simple_http::{
    Proto, SimpleBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod, SimpleUrl,
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
    pub body: SimpleBody,
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
        // Convert ParsedUrl to SimpleUrl
        let simple_url = if let Some(query) = &self.url.query {
            SimpleUrl::url_with_query(format!("{}?{}", self.url.path, query))
        } else {
            SimpleUrl::url_only(self.url.path.clone())
        };

        // Create SimpleIncomingRequest using builder
        let request = SimpleIncomingRequest::builder()
            .with_url(simple_url)
            .with_method(self.method)
            .with_proto(Proto::HTTP11)
            .with_headers(self.headers)
            .with_some_body(Some(self.body))
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
pub struct ClientRequestBuilder {
    method: SimpleMethod,
    url: ParsedUrl,
    headers: SimpleHeaders,
    body: Option<SimpleBody>,
}

impl ClientRequestBuilder {
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
    /// Returns `HttpClientError` if the URL is invalid.
    pub fn new(method: SimpleMethod, url: &str) -> Result<Self, HttpClientError> {
        let parsed_url = ParsedUrl::parse(url)?;
        let mut headers = BTreeMap::new();

        // Add required Host header
        let host = if parsed_url.scheme.default_port() == parsed_url.port {
            parsed_url.host.clone()
        } else {
            format!("{}:{}", parsed_url.host, parsed_url.port)
        };
        headers.insert(SimpleHeader::HOST, vec![host]);

        Ok(Self {
            method,
            url: parsed_url,
            headers,
            body: None,
        })
    }

    /// Adds a single header to the request.
    ///
    /// # Arguments
    ///
    /// * `key` - Header name
    /// * `value` - Header value
    pub fn header(mut self, key: SimpleHeader, value: impl Into<String>) -> Self {
        self.headers
            .entry(key)
            .or_insert_with(Vec::new)
            .push(value.into());
        self
    }

    /// Replaces all headers with the given headers.
    ///
    /// # Arguments
    ///
    /// * `headers` - New headers to use
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
    pub fn body_text(mut self, text: impl Into<String>) -> Self {
        let text_string = text.into();
        let content_length = text_string.len().to_string();

        self.headers
            .entry(SimpleHeader::CONTENT_TYPE)
            .or_insert_with(|| vec!["text/plain".to_string()]);
        self.headers
            .insert(SimpleHeader::CONTENT_LENGTH, vec![content_length]);

        self.body = Some(SimpleBody::Text(text_string));
        self
    }

    /// Sets the body as raw bytes.
    ///
    /// Automatically sets Content-Type to application/octet-stream if not already set.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Binary content
    pub fn body_bytes(mut self, bytes: Vec<u8>) -> Self {
        let content_length = bytes.len().to_string();

        self.headers
            .entry(SimpleHeader::CONTENT_TYPE)
            .or_insert_with(|| vec!["application/octet-stream".to_string()]);
        self.headers
            .insert(SimpleHeader::CONTENT_LENGTH, vec![content_length]);

        self.body = Some(SimpleBody::Bytes(bytes));
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

        self.body = Some(SimpleBody::Text(json_string));
        Ok(self)
    }

    /// Sets the body as form data.
    ///
    /// Automatically sets Content-Type to application/x-www-form-urlencoded.
    ///
    /// # Arguments
    ///
    /// * `params` - Form parameters as key-value pairs
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
                            .map(|b| format!("%{:02X}", b))
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

        self.body = Some(SimpleBody::Text(form_string));
        self
    }

    /// Builds the final prepared request.
    ///
    /// Consumes the builder and returns a `PreparedRequest` ready to send.
    pub fn build(self) -> PreparedRequest {
        PreparedRequest {
            method: self.method,
            url: self.url,
            headers: self.headers,
            body: self.body.unwrap_or(SimpleBody::None),
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WHY: Verify ClientRequestBuilder::new creates builder
    /// WHAT: Tests that new creates a request builder with URL and method
    #[test]
    fn test_client_request_builder_new() {
        let builder = ClientRequestBuilder::new(SimpleMethod::GET, "http://example.com").unwrap();
        assert_eq!(builder.url.host, "example.com");
        assert_eq!(builder.url.port, 80);
        assert!(matches!(builder.method, SimpleMethod::GET));
    }

    /// WHY: Verify ClientRequestBuilder::new validates URL
    /// WHAT: Tests that invalid URLs return error
    #[test]
    fn test_client_request_builder_new_invalid_url() {
        let result = ClientRequestBuilder::new(SimpleMethod::GET, "not a url");
        assert!(result.is_err());
    }

    /// WHY: Verify ClientRequestBuilder::header adds header
    /// WHAT: Tests that header method adds header to request
    #[test]
    fn test_client_request_builder_header() {
        let builder = ClientRequestBuilder::get("http://example.com")
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
        let builder = ClientRequestBuilder::post("http://example.com")
            .unwrap()
            .body_text("Hello, World!");

        assert!(matches!(builder.body, Some(SimpleBody::Text(_))));
        assert!(builder.headers.contains_key(&SimpleHeader::CONTENT_LENGTH));
        assert!(builder.headers.contains_key(&SimpleHeader::CONTENT_TYPE));
    }

    /// WHY: Verify ClientRequestBuilder::body_bytes sets binary body
    /// WHAT: Tests that body_bytes sets body and content headers
    #[test]
    fn test_client_request_builder_body_bytes() {
        let builder = ClientRequestBuilder::post("http://example.com")
            .unwrap()
            .body_bytes(vec![1, 2, 3, 4]);

        assert!(matches!(builder.body, Some(SimpleBody::Bytes(_))));
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

        let builder = ClientRequestBuilder::post("http://example.com")
            .unwrap()
            .body_json(&data)
            .unwrap();

        assert!(matches!(builder.body, Some(SimpleBody::Text(_))));
        if let Some(SimpleBody::Text(json)) = &builder.body {
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

        let builder = ClientRequestBuilder::post("http://example.com")
            .unwrap()
            .body_form(&params);

        assert!(matches!(builder.body, Some(SimpleBody::Text(_))));
        if let Some(SimpleBody::Text(form)) = &builder.body {
            assert!(form.contains("key1=value1"));
            assert!(form.contains("key2=value2"));
        }
    }

    /// WHY: Verify ClientRequestBuilder::build creates PreparedRequest
    /// WHAT: Tests that build consumes builder and creates prepared request
    #[test]
    fn test_client_request_builder_build() {
        let prepared = ClientRequestBuilder::get("http://example.com")
            .unwrap()
            .build();

        assert_eq!(prepared.url.host, "example.com");
        assert!(matches!(prepared.method, SimpleMethod::GET));
        assert!(matches!(prepared.body, SimpleBody::None));
    }

    /// WHY: Verify convenience methods create correct builders
    /// WHAT: Tests get, post, put, delete, patch, head, options methods
    #[test]
    fn test_client_request_builder_convenience_methods() {
        let get = ClientRequestBuilder::get("http://example.com").unwrap();
        assert!(matches!(get.method, SimpleMethod::GET));

        let post = ClientRequestBuilder::post("http://example.com").unwrap();
        assert!(matches!(post.method, SimpleMethod::POST));

        let put = ClientRequestBuilder::put("http://example.com").unwrap();
        assert!(matches!(put.method, SimpleMethod::PUT));

        let delete = ClientRequestBuilder::delete("http://example.com").unwrap();
        assert!(matches!(delete.method, SimpleMethod::DELETE));

        let patch = ClientRequestBuilder::patch("http://example.com").unwrap();
        assert!(matches!(patch.method, SimpleMethod::PATCH));

        let head = ClientRequestBuilder::head("http://example.com").unwrap();
        assert!(matches!(head.method, SimpleMethod::HEAD));

        let options = ClientRequestBuilder::options("http://example.com").unwrap();
        assert!(matches!(options.method, SimpleMethod::OPTIONS));
    }

    /// WHY: Verify Host header is automatically added
    /// WHAT: Tests that Host header is set from URL
    #[test]
    fn test_client_request_builder_auto_host_header() {
        let builder = ClientRequestBuilder::get("http://example.com").unwrap();
        assert!(builder.headers.contains_key(&SimpleHeader::HOST));
        assert_eq!(
            builder.headers.get(&SimpleHeader::HOST).unwrap()[0],
            "example.com"
        );

        let builder2 = ClientRequestBuilder::get("http://example.com:8080").unwrap();
        assert_eq!(
            builder2.headers.get(&SimpleHeader::HOST).unwrap()[0],
            "example.com:8080"
        );
    }

    /// WHY: Verify PreparedRequest::into_simple_incoming_request works
    /// WHAT: Tests that prepared request can be converted to SimpleIncomingRequest
    #[test]
    fn test_prepared_request_into_simple_incoming_request() {
        let prepared = ClientRequestBuilder::get("http://example.com/path")
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
        let prepared = ClientRequestBuilder::get("http://example.com/path?foo=bar")
            .unwrap()
            .build();

        let simple_request = prepared.into_simple_incoming_request().unwrap();
        assert!(simple_request.request_url.url.contains("?foo=bar"));
    }
}
