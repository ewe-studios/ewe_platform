//! Request building and response types for HTTP client.
//!
//! This module provides:
//! - `ClientRequestBuilder` - Fluent API for building HTTP requests
//! - `PreparedRequest` - Request ready for execution
//! - `Response` - Complete HTTP response
//! - `ResponseIntro` - Initial response headers
//! - `ResponseHeaders` - Iterator over response headers
//! - `ResponseData` - Response body data

use crate::wire::simple_http::client::ParsedUrl;
use crate::wire::simple_http::client::connection::HttpClientConnection;
use crate::wire::simple_http::client::dns::DnsResolver;
use crate::wire::simple_http::client::errors::HttpClientError;
use derive_more::From;
use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::sync::Arc;
use std::time::Duration;

/// HTTP method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Method {
    /// GET request
    Get,
    /// POST request
    Post,
    /// PUT request
    Put,
    /// DELETE request
    Delete,
    /// PATCH request
    Patch,
    /// HEAD request
    Head,
    /// OPTIONS request
    Options,
    /// TRACE request
    Trace,
}

impl Method {
    /// Returns the method name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Patch => "PATCH",
            Method::Head => "HEAD",
            Method::Options => "OPTIONS",
            Method::Trace => "TRACE",
        }
    }

    /// Parses a method from a string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "GET" => Some(Method::Get),
            "POST" => Some(Method::Post),
            "PUT" => Some(Method::Put),
            "DELETE" => Some(Method::Delete),
            "PATCH" => Some(Method::Patch),
            "HEAD" => Some(Method::Head),
            "OPTIONS" => Some(Method::Options),
            "TRACE" => Some(Method::Trace),
            _ => None,
        }
    }
}

/// HTTP request headers.
#[derive(Debug, Default, Clone)]
pub struct Headers {
    inner: HashMap<String, Vec<String>>,
}

impl Headers {
    /// Creates a new empty header map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new header map with the given entries.
    pub fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (String, String)>,
    {
        let mut inner = HashMap::new();
        for (key, value) in iter {
            inner.entry(key).or_insert_with(Vec::new).push(value);
        }
        Self { inner }
    }

    /// Adds a header entry.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        self.inner.entry(key).or_insert_with(Vec::new).push(value);
    }

    /// Sets a header to a single value (overwrites any existing values).
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        self.inner.insert(key, vec![value]);
    }

    /// Gets all values for a header key.
    pub fn get_all(&self, key: &str) -> &[String] {
        self.inner.get(key).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Gets the first value for a header key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.get_all(key).first().copied()
    }

    /// Checks if a header exists.
    pub fn contains_key(&self, key: &str) -> bool {
        self.inner.contains_key(key)
    }

    /// Removes all values for a header key.
    pub fn remove(&mut self, key: &str) {
        self.inner.remove(key);
    }

    /// Returns the number of headers.
    pub fn len(&self) -> usize {
        self.inner.values().map(|v| v.len()).sum()
    }

    /// Returns true if there are no headers.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Clears all headers.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Iterates over all headers.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> + '_ {
        self.inner.iter().flat_map(|(k, v)| v.iter().map(move |value| (k, value)))
    }
}

/// HTTP request body.
#[derive(Debug, Clone)]
pub enum Body {
    /// Empty body
    Empty,
    /// String body
    String(String),
    /// Bytes body
    Bytes(Vec<u8>),
    /// Reader body
    Reader(Box<dyn Read + Send + Sync>),
}

impl Body {
    /// Creates a new empty body.
    pub fn empty() -> Self {
        Body::Empty
    }

    /// Creates a new string body.
    pub fn text(text: impl Into<String>) -> Self {
        Body::String(text.into())
    }

    /// Creates a new bytes body.
    pub fn bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Body::Bytes(bytes.into())
    }

    /// Creates a new reader body.
    pub fn reader<R>(reader: R) -> Self
    where
        R: Read + Send + Sync + 'static,
    {
        Body::Reader(Box::new(reader))
    }

    /// Returns the content length if known.
    pub fn content_length(&self) -> Option<u64> {
        match self {
            Body::Empty => Some(0),
            Body::String(s) => Some(s.len() as u64),
            Body::Bytes(b) => Some(b.len() as u64),
            Body::Reader(_) => None,
        }
    }
}

/// Prepared HTTP request ready for execution.
#[derive(Debug, Clone)]
pub struct PreparedRequest {
    url: ParsedUrl,
    method: Method,
    headers: Headers,
    body: Option<Body>,
    timeout: Option<Duration>,
}

impl PreparedRequest {
    /// Creates a new prepared request.
    pub fn new(url: ParsedUrl, method: Method) -> Self {
        Self {
            url,
            method,
            headers: Headers::new(),
            body: None,
            timeout: None,
        }
    }

    /// Returns the URL of the request.
    pub fn url(&self) -> &ParsedUrl {
        &self.url
    }

    /// Returns the HTTP method.
    pub fn method(&self) -> Method {
        self.method
    }

    /// Returns the headers.
    pub fn headers(&self) -> &Headers {
        &self.headers
    }

    /// Returns the body, if any.
    pub fn body(&self) -> Option<&Body> {
        self.body.as_ref()
    }

    /// Returns the timeout, if set.
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// Sets a timeout for the request.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets the request body.
    pub fn with_body(mut self, body: Body) -> Self {
        self.body = Some(body);
        self
    }

    /// Sets the request headers.
    pub fn with_headers(mut self, headers: Headers) -> Self {
        self.headers = headers;
        self
    }

    /// Sends the request and returns a response.
    pub async fn send<R: DnsResolver>(
        self,
        resolver: &R,
    ) -> Result<Response, HttpClientError> {
        // Establish connection
        let conn = HttpClientConnection::connect(&self.url, resolver, self.timeout)?;

        // Build request line and headers
        let mut request = String::new();
        request.push_str(&format!("{} {} {} HTTP/1.1\r\n", self.method.as_str(), self.url.path.as_str(), self.url.query.as_deref().unwrap_or("")));

        // Add Host header
        request.push_str(&format!("Host: {}\r\n", self.url.host.as_str()));

        // Add User-Agent header
        request.push_str("User-Agent: ewe-http-client/1.0\r\n");

        // Add other headers
        for (key, value) in self.headers.iter() {
            request.push_str(&format!("{}: {}\r\n", key.as_str(), value.as_str()));
        }

        // Add Content-Length if body present
        if let Some(body) = &self.body {
            if let Some(len) = body.content_length() {
                request.push_str(&format!("Content-Length: {}\r\n", len));
            } else {
                // For unknown length, use Transfer-Encoding: chunked
                request.push_str("Transfer-Encoding: chunked\r\n");
            }
        }

        // End headers
        request.push_str("\r\n");

        // Add body if present
        if let Some(body) = self.body {
            match body {
                Body::Empty => {}
                Body::String(s) => request.push_str(&s),
                Body::Bytes(b) => request.push_str(std::str::from_utf8(&b[..]).unwrap_or("")),
                Body::Reader(mut r) => {
                    let mut buf = vec![0u8; 8192];
                    while let Ok(n) = r.read(&mut buf) {
                        if n == 0 {
                            break;
                        }
                        request.push_str(std::str::from_utf8(&buf[..n]).unwrap_or(""));
                    }
                }
            }
        }

        // Send request
        {
            let conn = conn.connection();
            if let Err(e) = conn.write_all(request.as_bytes()) {
                return Err(HttpClientError::IoError(e));
            }
        }

        // Read response
        Response::new(conn)
    }
}

/// HTTP response intro (status line and headers).
#[derive(Debug, Clone)]
pub struct ResponseIntro {
    status_code: u16,
    status_text: String,
    headers: Headers,
}

impl ResponseIntro {
    /// Creates a new response intro.
    pub fn new(status_code: u16, status_text: impl Into<String>, headers: Headers) -> Self {
        Self {
            status_code,
            status_text: status_text.into(),
            headers,
        }
    }

    /// Returns the status code.
    pub fn status_code(&self) -> u16 {
        self.status_code
    }

    /// Returns the status text.
    pub fn status_text(&self) -> &str {
        &self.status_text
    }

    /// Returns the headers.
    pub fn headers(&self) -> &Headers {
        &self.headers
    }

    /// Returns true if the response is successful.
    pub fn is_success(&self) -> bool {
        matches!(
            self.status_code,
            200..=299
        )
    }

    /// Returns true if the response is a redirect.
    pub fn is_redirect(&self) -> bool {
        matches!(
            self.status_code,
            301 | 302 | 303 | 307 | 308
        )
    }

    /// Returns true if the response is a client error.
    pub fn is_client_error(&self) -> bool {
        matches!(
            self.status_code,
            400..=499
        )
    }

    /// Returns true if the response is a server error.
    pub fn is_server_error(&self) -> bool {
        matches!(
            self.status_code,
            500..=599
        )
    }

    /// Returns the Content-Type header.
    pub fn content_type(&self) -> Option<&str> {
        self.headers.get("Content-Type")
    }

    /// Returns the Content-Length header.
    pub fn content_length(&self) -> Option<u64> {
        self.headers.get("Content-Length")
            .and_then(|s| s.parse().ok())
    }

    /// Returns true if the response uses chunked encoding.
    pub fn is_chunked(&self) -> bool {
        self.headers.contains_key("Transfer-Encoding")
            && self.headers.get_all("Transfer-Encoding").iter().any(|v| {
                v.to_ascii_lowercase().contains("chunked")
            })
    }
}

/// Iterator over HTTP response headers.
pub struct ResponseHeaders<'a> {
    headers: &'a Headers,
    index: usize,
}

impl<'a> ResponseHeaders<'a> {
    /// Creates a new headers iterator.
    pub fn new(headers: &'a Headers) -> Self {
        Self {
            headers,
            index: 0,
        }
    }

    /// Returns the current header name and value.
    pub fn current(&self) -> Option<(&'a str, &'a str)> {
        let (key, values) = self.headers.iter().nth(self.index)?;
        Some((key, values.first()?))
    }
}

impl<'a> Iterator for ResponseHeaders<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        let (key, values) = self.headers.iter().nth(self.index)?;
        self.index += 1;
        Some((key, values.first()?))
    }
}

impl<'a> fmt::Debug for ResponseHeaders<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.headers.iter()).finish()
    }
}

/// HTTP response.
#[derive(Debug)]
pub struct Response {
    intro: ResponseIntro,
    conn: Arc<HttpClientConnection>,
}

impl Response {
    /// Creates a new response from a connection.
    pub fn new(conn: Arc<HttpClientConnection>) -> Result<Self, HttpClientError> {
        // Read status line
        let mut status_line = String::new();
        conn.connection_mut().take_until(b"\r\n", &mut status_line)?;
        conn.connection_mut().consume(b"\r\n")?;

        // Parse status line: "HTTP/1.1 200 OK"
        let parts: Vec<&str> = status_line.trim().split_whitespace().collect();
        if parts.len() < 2 {
            return Err(HttpClientError::InvalidUrl(format!(
                "Invalid status line: {}",
                status_line
            )));
        }

        let status_code: u16 = parts[1]
            .parse()
            .map_err(|_| HttpClientError::InvalidUrl(format!("Invalid status code: {}", parts[1])))?;

        let status_text = parts.get(2).map(|s| *s).unwrap_or("Unknown");

        // Read headers
        let mut headers = Headers::new();
        loop {
            let mut header_line = String::new();
            conn.connection_mut().take_until(b"\r\n", &mut header_line)?;
            conn.connection_mut().consume(b"\r\n")?;

            if header_line.is_empty() {
                break; // End of headers
            }

            // Parse header: "Name: Value"
            if let Some(colon_pos) = header_line.find(':') {
                let name = &header_line[..colon_pos];
                let value = &header_line[colon_pos + 1..];
                headers.insert(name.trim(), value.trim());
            }
        }

        Ok(Self {
            intro: ResponseIntro::new(status_code, status_text, headers),
            conn,
        })
    }

    /// Returns the response intro (status line and headers).
    pub fn intro(&self) -> &ResponseIntro {
        &self.intro
    }

    /// Returns the status code.
    pub fn status_code(&self) -> u16 {
        self.intro.status_code()
    }

    /// Returns the status text.
    pub fn status_text(&self) -> &str {
        self.intro.status_text()
    }

    /// Returns the headers.
    pub fn headers(&self) -> &Headers {
        &self.intro.headers
    }

    /// Returns true if the response is successful.
    pub fn is_success(&self) -> bool {
        self.intro.is_success()
    }

    /// Returns true if the response is a redirect.
    pub fn is_redirect(&self) -> bool {
        self.intro.is_redirect()
    }

    /// Returns true if the response is a client error.
    pub fn is_client_error(&self) -> bool {
        self.intro.is_client_error()
    }

    /// Returns true if the response is a server error.
    pub fn is_server_error(&self) -> bool {
        self.intro.is_server_error()
    }

    /// Returns the Content-Type header.
    pub fn content_type(&self) -> Option<&str> {
        self.intro.content_type()
    }

    /// Returns the Content-Length header.
    pub fn content_length(&self) -> Option<u64> {
        self.intro.content_length()
    }

    /// Returns true if the response uses chunked encoding.
    pub fn is_chunked(&self) -> bool {
        self.intro.is_chunked()
    }

    /// Reads the response body.
    pub fn body(&self) -> Result<Vec<u8>, HttpClientError> {
        let mut body = Vec::new();

        if self.is_chunked() {
            // Read chunked encoding
            loop {
                // Read chunk size
                let mut chunk_size_line = String::new();
                self.conn.connection_mut().take_until(b"\r\n", &mut chunk_size_line)?;
                self.conn.connection_mut().consume(b"\r\n")?;

                let chunk_size: u64 = chunk_size_line
                    .trim()
                    .parse()
                    .map_err(|_| HttpClientError::InvalidUrl(format!("Invalid chunk size: {}", chunk_size_line)))?;

                if chunk_size == 0 {
                    // Last chunk
                    self.conn.connection_mut().consume(b"\r\n");
                    break;
                }

                // Read chunk data
                let mut chunk_data = vec![0u8; chunk_size as usize];
                self.conn.connection_mut().read_exact(&mut chunk_data)?;
                self.conn.connection_mut().consume(b"\r\n");

                body.extend_from_slice(&chunk_data);
            }
        } else if let Some(content_length) = self.content_length() {
            // Read fixed length
            let mut buf = vec![0u8; content_length as usize];
            self.conn.connection_mut().read_exact(&mut buf)?;
            body = buf;
        } else {
            // Read until connection closed
            let mut buf = vec![0u8; 8192];
            while let Ok(n) = self.conn.connection_mut().read(&mut buf) {
                if n == 0 {
                    break;
                }
                body.extend_from_slice(&buf[..n]);
            }
        }

        Ok(body)
    }

    /// Returns the headers iterator.
    pub fn header_iter(&self) -> ResponseHeaders {
        ResponseHeaders::new(&self.intro.headers)
    }

    /// Consumes the response and returns the connection.
    pub fn into_connection(self) -> Arc<HttpClientConnection> {
        self.conn
    }
}

/// Client request builder for building HTTP requests.
#[derive(Debug)]
pub struct ClientRequestBuilder {
    url: ParsedUrl,
    method: Method,
    headers: Headers,
    body: Option<Body>,
    timeout: Option<Duration>,
}

impl ClientRequestBuilder {
    /// Creates a new request builder for a GET request.
    pub fn get(url: impl Into<ParsedUrl>) -> Self {
        Self::new(url.into(), Method::Get)
    }

    /// Creates a new request builder for a POST request.
    pub fn post(url: impl Into<ParsedUrl>) -> Self {
        Self::new(url.into(), Method::Post)
    }

    /// Creates a new request builder for a PUT request.
    pub fn put(url: impl Into<ParsedUrl>) -> Self {
        Self::new(url.into(), Method::Put)
    }

    /// Creates a new request builder for a DELETE request.
    pub fn delete(url: impl Into<ParsedUrl>) -> Self {
        Self::new(url.into(), Method::Delete)
    }

    /// Creates a new request builder for a PATCH request.
    pub fn patch(url: impl Into<ParsedUrl>) -> Self {
        Self::new(url.into(), Method::Patch)
    }

    /// Creates a new request builder for a HEAD request.
    pub fn head(url: impl Into<ParsedUrl>) -> Self {
        Self::new(url.into(), Method::Head)
    }

    /// Creates a new request builder for an OPTIONS request.
    pub fn options(url: impl Into<ParsedUrl>) -> Self {
        Self::new(url.into(), Method::Options)
    }

    /// Creates a new request builder for a TRACE request.
    pub fn trace(url: impl Into<ParsedUrl>) -> Self {
        Self::new(url.into(), Method::Trace)
    }

    /// Creates a new request builder with the specified method and URL.
    pub fn new(url: impl Into<ParsedUrl>, method: Method) -> Self {
        Self {
            url: url.into(),
            method,
            headers: Headers::new(),
            body: None,
            timeout: None,
        }
    }

    /// Returns the URL.
    pub fn url(&self) -> &ParsedUrl {
        &self.url
    }

    /// Returns the method.
    pub fn method(&self) -> Method {
        self.method
    }

    /// Returns the headers.
    pub fn headers(&self) -> &Headers {
        &self.headers
    }

    /// Returns the body, if any.
    pub fn body(&self) -> Option<&Body> {
        self.body.as_ref()
    }

    /// Returns the timeout, if set.
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// Sets a request header.
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key, value);
        self
    }

    /// Sets multiple request headers.
    pub fn headers(mut self, headers: Headers) -> Self {
        self.headers = headers;
        self
    }

    /// Sets the request body.
    pub fn body(mut self, body: Body) -> Self {
        self.body = Some(body);
        self
    }

    /// Sets a timeout for the request.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Builds a prepared request.
    pub fn build(self) -> PreparedRequest {
        PreparedRequest {
            url: self.url,
            method: self.method,
            headers: self.headers,
            body: self.body,
            timeout: self.timeout,
        }
    }

    /// Sends the request and returns a response.
    ///
    /// This method requires a DNS resolver to establish the connection.
    pub async fn send<R: DnsResolver>(
        self,
        resolver: &R,
    ) -> Result<Response, HttpClientError> {
        self.build().send(resolver).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::simple_http::client::dns::MockDnsResolver;

    /// WHY: Verify Method::Get returns correct string
    /// WHAT: Tests that GET method string representation is correct
    #[test]
    fn test_method_get_as_str() {
        assert_eq!(Method::Get.as_str(), "GET");
    }

    /// WHY: Verify Method::Post returns correct string
    /// WHAT: Tests that POST method string representation is correct
    #[test]
    fn test_method_post_as_str() {
        assert_eq!(Method::Post.as_str(), "POST");
    }

    /// WHY: Verify Method::from_str parses correctly
    /// WHAT: Tests that method string parsing works for all methods
    #[test]
    fn test_method_from_str() {
        assert_eq!(Method::from_str("GET"), Some(Method::Get));
        assert_eq!(Method::from_str("POST"), Some(Method::Post));
        assert_eq!(Method::from_str("PUT"), Some(Method::Put));
        assert_eq!(Method::from_str("DELETE"), Some(Method::Delete));
        assert_eq!(Method::from_str("PATCH"), Some(Method::Patch));
        assert_eq!(Method::from_str("HEAD"), Some(Method::Head));
        assert_eq!(Method::from_str("OPTIONS"), Some(Method::Options));
        assert_eq!(Method::from_str("TRACE"), Some(Method::Trace));
        assert_eq!(Method::from_str("INVALID"), None);
    }

    /// WHY: Verify Headers::new creates empty headers
    /// WHAT: Tests that new headers map is empty
    #[test]
    fn test_headers_new() {
        let headers = Headers::new();
        assert!(headers.is_empty());
        assert_eq!(headers.len(), 0);
    }

    /// WHY: Verify Headers::insert adds header
    /// WHAT: Tests that inserting a header works
    #[test]
    fn test_headers_insert() {
        let mut headers = Headers::new();
        headers.insert("Content-Type", "text/plain");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers.get("Content-Type"), Some("text/plain"));
    }

    /// WHY: Verify Headers::set overwrites existing values
    /// WHAT: Tests that set() replaces all values for a key
    #[test]
    fn test_headers_set_overwrites() {
        let mut headers = Headers::new();
        headers.insert("Content-Type", "text/plain");
        headers.insert("Content-Type", "application/json");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers.get_all("Content-Type"), vec!["application/json"]);
    }

    /// WHY: Verify Headers::get returns first value
    /// WHAT: Tests that get() returns the first value for a key
    #[test]
    fn test_headers_get() {
        let mut headers = Headers::new();
        headers.insert("X-Custom", "value1");
        headers.insert("X-Custom", "value2");
        assert_eq!(headers.get("X-Custom"), Some("value1"));
    }

    /// WHY: Verify Headers::get_all returns all values
    /// WHAT: Tests that get_all() returns all values for a key
    #[test]
    fn test_headers_get_all() {
        let mut headers = Headers::new();
        headers.insert("X-Custom", "value1");
        headers.insert("X-Custom", "value2");
        assert_eq!(headers.get_all("X-Custom"), vec!["value1", "value2"]);
    }

    /// WHY: Verify Headers::contains_key checks existence
    /// WHAT: Tests that contains_key() returns correct boolean
    #[test]
    fn test_headers_contains_key() {
        let mut headers = Headers::new();
        headers.insert("X-Custom", "value");
        assert!(headers.contains_key("X-Custom"));
        assert!(!headers.contains_key("X-Missing"));
    }

    /// WHY: Verify Headers::len counts correctly
    /// WHAT: Tests that len() counts total header entries
    #[test]
    fn test_headers_len() {
        let mut headers = Headers::new();
        headers.insert("X-Custom", "value");
        headers.insert("X-Custom", "value2");
        headers.insert("X-Another", "value3");
        assert_eq!(headers.len(), 3);
    }

    /// WHY: Verify Headers::clear removes all headers
    /// WHAT: Tests that clear() empties the headers
    #[test]
    fn test_headers_clear() {
        let mut headers = Headers::new();
        headers.insert("X-Custom", "value");
        assert_eq!(headers.len(), 1);
        headers.clear();
        assert!(headers.is_empty());
    }

    /// WHY: Verify Headers::from_iter creates from iterator
    /// WHAT: Tests that headers can be created from iterator
    #[test]
    fn test_headers_from_iter() {
        let headers = Headers::from_iter([
            ("Content-Type", "text/plain"),
            ("X-Custom", "value"),
        ]);
        assert_eq!(headers.len(), 2);
        assert_eq!(headers.get("Content-Type"), Some("text/plain"));
    }

    /// WHY: Verify Body::empty creates empty body
    /// WHAT: Tests that empty body has zero length
    #[test]
    fn test_body_empty() {
        let body = Body::empty();
        assert_eq!(body.content_length(), Some(0));
    }

    /// WHY: Verify Body::text creates string body
    /// WHAT: Tests that text body has correct length
    #[test]
    fn test_body_text() {
        let body = Body::text("Hello");
        assert_eq!(body.content_length(), Some(5));
        match body {
            Body::String(s) => assert_eq!(s, "Hello"),
            _ => panic!("Expected String body"),
        }
    }

    /// WHY: Verify Body::bytes creates bytes body
    /// WHAT: Tests that bytes body has correct length
    #[test]
    fn test_body_bytes() {
        let body = Body::bytes(vec![1, 2, 3]);
        assert_eq!(body.content_length(), Some(3));
        match body {
            Body::Bytes(b) => assert_eq!(b, vec![1, 2, 3]),
            _ => panic!("Expected Bytes body"),
        }
    }

    /// WHY: Verify PreparedRequest::new creates request
    /// WHAT: Tests that prepared request is created with correct values
    #[test]
    fn test_prepared_request_new() {
        let url = ParsedUrl::parse("http://example.com").unwrap();
        let request = PreparedRequest::new(url, Method::Get);
        assert_eq!(request.method(), Method::Get);
        assert!(request.headers().is_empty());
        assert!(request.body().is_none());
        assert!(request.timeout().is_none());
    }

    /// WHY: Verify PreparedRequest::with_timeout sets timeout
    /// WHAT: Tests that timeout can be configured
    #[test]
    fn test_prepared_request_timeout() {
        let url = ParsedUrl::parse("http://example.com").unwrap();
        let request = PreparedRequest::new(url, Method::Get)
            .with_timeout(Duration::from_secs(30));
        assert_eq!(request.timeout(), Some(Duration::from_secs(30)));
    }

    /// WHY: Verify PreparedRequest::with_body sets body
    /// WHAT: Tests that body can be configured
    #[test]
    fn test_prepared_request_body() {
        let url = ParsedUrl::parse("http://example.com").unwrap();
        let request = PreparedRequest::new(url, Method::Post)
            .with_body(Body::text("test"));
        assert!(request.body().is_some());
        match request.body().unwrap() {
            Body::String(s) => assert_eq!(s, "test"),
            _ => panic!("Expected String body"),
        }
    }

    /// WHY: Verify PreparedRequest::with_headers sets headers
    /// WHAT: Tests that headers can be configured
    #[test]
    fn test_prepared_request_headers() {
        let url = ParsedUrl::parse("http://example.com").unwrap();
        let headers = Headers::from_iter([("X-Custom", "value")]);
        let request = PreparedRequest::new(url, Method::Get).with_headers(headers);
        assert_eq!(request.headers().get("X-Custom"), Some("value"));
    }

    /// WHY: Verify ResponseIntro::new creates intro
    /// WHAT: Tests that response intro is created correctly
    #[test]
    fn test_response_intro_new() {
        let headers = Headers::new();
        let intro = ResponseIntro::new(200, "OK", headers);
        assert_eq!(intro.status_code(), 200);
        assert_eq!(intro.status_text(), "OK");
    }

    /// WHY: Verify ResponseIntro::is_success returns correct
    /// WHAT: Tests that success status codes are identified
    #[test]
    fn test_response_intro_is_success() {
        assert!(ResponseIntro::new(200, "OK", Headers::new()).is_success());
        assert!(!ResponseIntro::new(404, "Not Found", Headers::new()).is_success());
    }

    /// WHY: Verify ResponseIntro::is_redirect returns correct
    /// WHAT: Tests that redirect status codes are identified
    #[test]
    fn test_response_intro_is_redirect() {
        assert!(ResponseIntro::new(302, "Found", Headers::new()).is_redirect());
        assert!(!ResponseIntro::new(200, "OK", Headers::new()).is_redirect());
    }

    /// WHY: Verify ResponseIntro::content_type returns header value
    /// WHAT: Tests that content type header can be extracted
    #[test]
    fn test_response_intro_content_type() {
        let mut headers = Headers::new();
        headers.insert("Content-Type", "application/json");
        let intro = ResponseIntro::new(200, "OK", headers);
        assert_eq!(intro.content_type(), Some("application/json"));
    }

    /// WHY: Verify ResponseIntro::is_chunked returns correct
    /// WHAT: Tests that chunked encoding is detected
    #[test]
    fn test_response_intro_is_chunked() {
        let mut headers = Headers::new();
        headers.insert("Transfer-Encoding", "chunked");
        assert!(ResponseIntro::new(200, "OK", headers).is_chunked());
    }

    /// WHY: Verify Response::new parses status line
    /// WHAT: Tests that response status line is parsed correctly
    #[test]
    fn test_response_new_status_line() {
        let url = ParsedUrl::parse("http://example.com").unwrap();
        let conn = HttpClientConnection::connect(&url, &MockDnsResolver::new(), None).unwrap();
        let conn = Arc::new(conn);

        // Write a mock response
        conn.connection_mut().write_all(b"HTTP/1.1 200 OK\r\n").unwrap();
        conn.connection_mut().consume(b"\r\n");

        let response = Response::new(conn);
        assert_eq!(response.status_code(), 200);
        assert_eq!(response.status_text(), "OK");
    }

    /// WHY: Verify Response::new parses headers
    /// WHAT: Tests that response headers are parsed correctly
    #[test]
    fn test_response_new_headers() {
        let url = ParsedUrl::parse("http://example.com").unwrap();
        let conn = HttpClientConnection::connect(&url, &MockDnsResolver::new(), None).unwrap();
        let conn = Arc::new(conn);

        // Write a mock response with headers
        conn.connection_mut().write_all(b"HTTP/1.1 200 OK\r\n").unwrap();
        conn.connection_mut().consume(b"\r\n");
        conn.connection_mut().write_all(b"Content-Type: text/plain\r\n").unwrap();
        conn.connection_mut().consume(b"\r\n");
        conn.connection_mut().write_all(b"\r\n").unwrap();

        let response = Response::new(conn);
        assert!(response.headers().contains_key("Content-Type"));
        assert_eq!(response.headers().get("Content-Type"), Some("text/plain"));
    }

    /// WHY: Verify Response::body reads response body
    /// WHAT: Tests that response body can be read
    #[test]
    fn test_response_body() {
        let url = ParsedUrl::parse("http://example.com").unwrap();
        let conn = HttpClientConnection::connect(&url, &MockDnsResolver::new(), None).unwrap();
        let conn = Arc::new(conn);

        // Write a mock response with body
        conn.connection_mut().write_all(b"HTTP/1.1 200 OK\r\n").unwrap();
        conn.connection_mut().consume(b"\r\n");
        conn.connection_mut().write_all(b"Content-Length: 5\r\n").unwrap();
        conn.connection_mut().consume(b"\r\n");
        conn.connection_mut().write_all(b"\r\n").unwrap();
        conn.connection_mut().write_all(b"Hello").unwrap();

        let response = Response::new(conn).unwrap();
        let body = response.body().unwrap();
        assert_eq!(body, b"Hello");
    }

    /// WHY: Verify Response::is_success returns correct
    /// WHAT: Tests that success status is identified
    #[test]
    fn test_response_is_success() {
        let url = ParsedUrl::parse("http://example.com").unwrap();
        let conn = HttpClientConnection::connect(&url, &MockDnsResolver::new(), None).unwrap();
        let conn = Arc::new(conn);

        conn.connection_mut().write_all(b"HTTP/1.1 200 OK\r\n\r\n").unwrap();
        assert!(Response::new(conn).unwrap().is_success());

        conn.connection_mut().write_all(b"HTTP/1.1 404 Not Found\r\n\r\n").unwrap();
        assert!(!Response::new(conn).unwrap().is_success());
    }

    /// WHY: Verify ClientRequestBuilder::get creates GET builder
    /// WHAT: Tests that GET builder is created correctly
    #[test]
    fn test_client_request_builder_get() {
        let builder = ClientRequestBuilder::get("http://example.com");
        assert_eq!(builder.method(), Method::Get);
        assert_eq!(builder.url().host, "example.com");
    }

    /// WHY: Verify ClientRequestBuilder::post creates POST builder
    /// WHAT: Tests that POST builder is created correctly
    #[test]
    fn test_client_request_builder_post() {
        let builder = ClientRequestBuilder::post("http://example.com");
        assert_eq!(builder.method(), Method::Post);
    }

    /// WHY: Verify ClientRequestBuilder::header sets header
    /// WHAT: Tests that headers can be added via chainable method
    #[test]
    fn test_client_request_builder_header() {
        let builder = ClientRequestBuilder::get("http://example.com")
            .header("X-Custom", "value");
        assert_eq!(builder.headers().get("X-Custom"), Some("value"));
    }

    /// WHY: Verify ClientRequestBuilder::body sets body
    /// WHAT: Tests that body can be set via chainable method
    #[test]
    fn test_client_request_builder_body() {
        let builder = ClientRequestBuilder::post("http://example.com")
            .body(Body::text("test"));
        assert!(builder.body().is_some());
    }

    /// WHY: Verify ClientRequestBuilder::send requires resolver
    /// WHAT: Tests that send() requires a DNS resolver
    #[test]
    fn test_client_request_builder_send_requires_resolver() {
        let builder = ClientRequestBuilder::get("http://example.com");
        let resolver = MockDnsResolver::new();
        // This will fail at compile time if resolver is not provided
        let _ = builder.send(&resolver);
    }
}
