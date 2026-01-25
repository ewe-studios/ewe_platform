//! Response intro wrapper for initial response parsing.
//!
//! This module provides `ResponseIntro` wrapper that handles the initial
//! response parsing (status line and headers) before reading the body.

use crate::wire::simple_http::client::connection::{HttpClientConnection, ParsedUrl};
use crate::wire::simple_http::client::errors::HttpClientError;
use derive_more::From;
use std::fmt;
use std::io::Read;

/// HTTP response intro (status line and headers).
///
/// This wrapper handles parsing the initial response from the server.
#[derive(Debug, Clone)]
pub struct ResponseIntro {
    status_code: u16,
    status_text: String,
    headers: crate::wire::simple_http::client::request::Headers,
}

impl ResponseIntro {
    /// Creates a new response intro.
    pub fn new(status_code: u16, status_text: impl Into<String>, headers: crate::wire::simple_http::client::request::Headers) -> Self {
        Self {
            status_code,
            status_text: status_text.into(),
            headers,
        }
    }

    /// Parses a response intro from a connection.
    ///
    /// # Arguments
    ///
    /// * `conn` - The HTTP client connection
    ///
    /// # Returns
    ///
    /// A `ResponseIntro` with status line and headers.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if parsing fails.
    pub fn from_connection(conn: &mut HttpClientConnection) -> Result<Self, HttpClientError> {
        // Read status line: "HTTP/1.1 200 OK"
        let mut status_line = String::new();
        conn.take_until(b"\r\n", &mut status_line)?;
        conn.consume(b"\r\n")?;

        // Parse status line
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
        let mut headers = crate::wire::simple_http::client::request::Headers::new();
        loop {
            let mut header_line = String::new();
            conn.take_until(b"\r\n", &mut header_line)?;
            conn.consume(b"\r\n")?;

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
            status_code,
            status_text,
            headers,
        })
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
    pub fn headers(&self) -> &crate::wire::simple_http::client::request::Headers {
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

impl fmt::Display for ResponseIntro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.status_code, self.status_text, self.headers)
    }
}

/// Response intro iterator for iterating over headers.
pub struct ResponseIntroHeaders<'a> {
    intro: &'a ResponseIntro,
    index: usize,
}

impl<'a> ResponseIntroHeaders<'a> {
    /// Creates a new headers iterator.
    pub fn new(intro: &'a ResponseIntro) -> Self {
        Self {
            intro,
            index: 0,
        }
    }
}

impl<'a> Iterator for ResponseIntroHeaders<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        let (key, values) = self.intro.headers.iter().nth(self.index)?;
        self.index += 1;
        Some((key, values.first()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::simple_http::client::dns::MockDnsResolver;

    /// WHY: Verify ResponseIntro::new creates intro
    /// WHAT: Tests that response intro is created correctly
    #[test]
    fn test_response_intro_new() {
        let headers = crate::wire::simple_http::client::request::Headers::new();
        let intro = ResponseIntro::new(200, "OK", headers);
        assert_eq!(intro.status_code(), 200);
        assert_eq!(intro.status_text(), "OK");
    }

    /// WHY: Verify ResponseIntro::is_success returns correct
    /// WHAT: Tests that success status codes are identified
    #[test]
    fn test_response_intro_is_success() {
        assert!(ResponseIntro::new(200, "OK", crate::wire::simple_http::client::request::Headers::new()).is_success());
        assert!(!ResponseIntro::new(404, "Not Found", crate::wire::simple_http::client::request::Headers::new()).is_success());
    }

    /// WHY: Verify ResponseIntro::is_redirect returns correct
    /// WHAT: Tests that redirect status codes are identified
    #[test]
    fn test_response_intro_is_redirect() {
        assert!(ResponseIntro::new(302, "Found", crate::wire::simple_http::client::request::Headers::new()).is_redirect());
        assert!(!ResponseIntro::new(200, "OK", crate::wire::simple_http::client::request::Headers::new()).is_redirect());
    }

    /// WHY: Verify ResponseIntro::content_type returns header value
    /// WHAT: Tests that content type header can be extracted
    #[test]
    fn test_response_intro_content_type() {
        let mut headers = crate::wire::simple_http::client::request::Headers::new();
        headers.insert("Content-Type", "application/json");
        let intro = ResponseIntro::new(200, "OK", headers);
        assert_eq!(intro.content_type(), Some("application/json"));
    }

    /// WHY: Verify ResponseIntro::is_chunked returns correct
    /// WHAT: Tests that chunked encoding is detected
    #[test]
    fn test_response_intro_is_chunked() {
        let mut headers = crate::wire::simple_http::client::request::Headers::new();
        headers.insert("Transfer-Encoding", "chunked");
        assert!(ResponseIntro::new(200, "OK", headers).is_chunked());
    }

    /// WHY: Verify ResponseIntro::from_connection parses status line
    /// WHAT: Tests that status line is parsed from connection
    #[test]
    fn test_response_intro_from_connection_status_line() {
        let url = ParsedUrl::parse("http://example.com").unwrap();
        let mut conn = HttpClientConnection::connect(&url, &MockDnsResolver::new(), None).unwrap();

        // Write a mock response
        conn.write_all(b"HTTP/1.1 200 OK\r\n").unwrap();
        conn.consume(b"\r\n");

        let intro = ResponseIntro::from_connection(&mut conn).unwrap();
        assert_eq!(intro.status_code(), 200);
        assert_eq!(intro.status_text(), "OK");
    }

    /// WHY: Verify ResponseIntro::from_connection parses headers
    /// WHAT: Tests that headers are parsed from connection
    #[test]
    fn test_response_intro_from_connection_headers() {
        let url = ParsedUrl::parse("http://example.com").unwrap();
        let mut conn = HttpClientConnection::connect(&url, &MockDnsResolver::new(), None).unwrap();

        // Write a mock response with headers
        conn.write_all(b"HTTP/1.1 200 OK\r\n").unwrap();
        conn.consume(b"\r\n");
        conn.write_all(b"Content-Type: text/plain\r\n").unwrap();
        conn.consume(b"\r\n");
        conn.write_all(b"\r\n").unwrap();

        let intro = ResponseIntro::from_connection(&mut conn).unwrap();
        assert!(intro.headers().contains_key("Content-Type"));
        assert_eq!(intro.headers().get("Content-Type"), Some("text/plain"));
    }

    /// WHY: Verify ResponseIntroHeaders iterator works
    /// WHAT: Tests that headers can be iterated
    #[test]
    fn test_response_intro_headers_iter() {
        let mut headers = crate::wire::simple_http::client::request::Headers::new();
        headers.insert("X-Custom", "value");
        headers.insert("X-Another", "value2");
        let intro = ResponseIntro::new(200, "OK", headers);

        let mut iter = ResponseIntroHeaders::new(&intro);
        assert_eq!(iter.next(), Some(("X-Custom", "value")));
        assert_eq!(iter.next(), Some(("X-Another", "value2")));
        assert_eq!(iter.next(), None);
    }

    /// WHY: Verify ResponseIntro::display formats correctly
    /// WHAT: Tests that Display trait works
    #[test]
    fn test_response_intro_display() {
        let mut headers = crate::wire::simple_http::client::request::Headers::new();
        headers.insert("Content-Type", "text/plain");
        let intro = ResponseIntro::new(200, "OK", headers);
        let display = format!("{}", intro);
        assert!(display.contains("200"));
        assert!(display.contains("OK"));
    }
}
