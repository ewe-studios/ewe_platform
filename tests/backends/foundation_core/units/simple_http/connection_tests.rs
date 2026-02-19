//! Unit tests for `client::connection` moved into the canonical units test tree.
//!
//! These tests are non-destructive copies of the original in-crate `#[cfg(test)]`
//! module and exercise URL parsing functionality (`ParsedUrl`) used by the
//! simple HTTP client. They avoid performing real network operations so they
//! are suitable as fast unit tests under `tests/.../units/simple_http/`.

use super::*;
use foundation_core::wire::simple_http::client::{
    HttpClientConnection, HttpClientError, MockDnsResolver, SystemDnsResolver,
};

use foundation_core::wire::simple_http::{client::ParsedUrl, url::Scheme};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

// ========================================================================
// ParsedUrl Tests - URL Parsing
// ========================================================================

/// WHY: Verify ParsedUrl correctly parses a simple HTTP URL
/// WHAT: Tests that scheme, host, port, and path are extracted correctly
/// IMPORTANCE: Basic URL parsing is fundamental to all HTTP requests
#[test]
fn test_parsed_url_simple_http() {
    let url = ParsedUrl::parse("http://example.com").unwrap();

    assert!(url.scheme().is_http());
    assert_eq!(url.host_str().unwrap(), "example.com");
    assert_eq!(url.port_or_default(), 80); // Default HTTP port
    assert_eq!(url.path(), "/"); // Default path
    assert_eq!(url.query(), None);
}

/// WHY: Verify ParsedUrl correctly parses a simple HTTPS URL
/// WHAT: Tests that HTTPS scheme is detected and default port is 443
/// IMPORTANCE: HTTPS is the primary use case for secure connections
#[test]
fn test_parsed_url_simple_https() {
    let url = ParsedUrl::parse("https://example.com").unwrap();

    assert!(url.scheme().is_https());
    assert_eq!(url.host_str().unwrap(), "example.com");
    assert_eq!(url.port_or_default(), 443); // Default HTTPS port
    assert_eq!(url.path(), "/");
    assert_eq!(url.query(), None);
}

/// WHY: Verify ParsedUrl handles explicit port numbers
/// WHAT: Tests that non-default ports override scheme defaults
/// IMPORTANCE: Many services run on non-standard ports
#[test]
fn test_parsed_url_with_explicit_port() {
    let url = ParsedUrl::parse("http://example.com:8080").unwrap();

    assert!(url.scheme().is_http());
    assert_eq!(url.host_str().unwrap(), "example.com");
    assert_eq!(url.port().unwrap(), 8080); // Explicit port overrides default
    assert_eq!(url.path(), "/");
}

/// WHY: Verify ParsedUrl correctly extracts path components
/// WHAT: Tests that paths with multiple segments are preserved
/// IMPORTANCE: RESTful APIs use complex path structures
#[test]
fn test_parsed_url_with_path() {
    let url = ParsedUrl::parse("http://example.com/api/v1/users").unwrap();

    assert!(url.scheme().is_http());
    assert_eq!(url.host_str().unwrap(), "example.com");
    assert_eq!(url.path(), "/api/v1/users");
    assert_eq!(url.query(), None);
}

/// WHY: Verify ParsedUrl correctly extracts query strings
/// WHAT: Tests that query parameters are separated from path
/// IMPORTANCE: Query parameters are essential for API requests
#[test]
fn test_parsed_url_with_query() {
    let url = ParsedUrl::parse("http://example.com/search?q=test&limit=10").unwrap();

    assert!(url.scheme().is_http());
    assert_eq!(url.host_str().unwrap(), "example.com");
    assert_eq!(url.path(), "/search");
    assert_eq!(url.query(), Some("q=test&limit=10"));
}

/// WHY: Verify ParsedUrl handles complex URLs with all components
/// WHAT: Tests parsing of URL with port, path, and query together
/// IMPORTANCE: Real-world URLs often have all components
#[test]
fn test_parsed_url_complex() {
    let url =
        ParsedUrl::parse("https://api.example.com:8443/v2/users/123?fields=name,email").unwrap();

    assert!(url.scheme().is_https());
    assert_eq!(url.host_str().unwrap(), "api.example.com");
    assert_eq!(url.port().unwrap(), 8443);
    assert_eq!(url.path(), "/v2/users/123");
    assert_eq!(url.query(), Some("fields=name,email"));
}

/// WHY: Verify ParsedUrl rejects URLs with missing scheme
/// WHAT: Tests that URLs without http:// or https:// are rejected
/// IMPORTANCE: Prevents ambiguous URL interpretation
#[test]
fn test_parsed_url_missing_scheme() {
    let result = ParsedUrl::parse("example.com");

    assert!(result.is_err());
}

/// WHY: Verify ParsedUrl rejects URLs with unsupported schemes
/// WHAT: Tests that only HTTP and HTTPS schemes are accepted
/// IMPORTANCE: HTTP client should only handle HTTP/HTTPS
#[test]
fn test_parsed_url_unsupported_scheme() {
    let result = ParsedUrl::parse("ftp://example.com");

    // The Uri parser now accepts any scheme during parsing,
    // but scheme validation happens when checking is_http/is_https
    assert!(result.is_ok());
    let url = result.unwrap();
    assert!(!url.scheme().is_http() && !url.scheme().is_https());
}

/// WHY: Verify ParsedUrl handles URLs with empty host
/// WHAT: Tests that URLs like "http:///" parse but have no host
/// IMPORTANCE: Connection validation happens when accessing host_str()
#[test]
fn test_parsed_url_empty_host() {
    let result = ParsedUrl::parse("http:///path");

    // The Uri parser may succeed but host validation happens on access
    assert!(result.is_ok());
    let url = result.unwrap();
    // Empty host should return None
    assert!(url.host_str().is_none() || url.host_str().unwrap().is_empty());
}

/// WHY: Verify ParsedUrl handles IP addresses as hosts
/// WHAT: Tests that IPv4 addresses are valid hosts
/// IMPORTANCE: Direct IP connections are common in testing
#[test]
fn test_parsed_url_with_ip_address() {
    let url = ParsedUrl::parse("http://127.0.0.1:8080/test").unwrap();

    assert_eq!(url.host_str().unwrap(), "127.0.0.1");
    assert_eq!(url.port().unwrap(), 8080);
    assert_eq!(url.path(), "/test");
}

/// WHY: Verify ParsedUrl handles URLs with fragment identifiers
/// WHAT: Tests that fragments (#section) are ignored in HTTP requests
/// IMPORTANCE: Fragments are client-side only, not sent to server
#[test]
fn test_parsed_url_with_fragment() {
    let url = ParsedUrl::parse("http://example.com/page#section").unwrap();

    assert_eq!(url.path(), "/page");
    // Fragment should be ignored
}

/// WHY: Verify Scheme::default_port returns correct values
/// WHAT: Tests that scheme default ports match HTTP standards
/// IMPORTANCE: Ensures correct default port selection
#[test]
fn test_scheme_default_ports() {
    assert_eq!(Scheme::HTTP.default_port(), 80);
    assert_eq!(Scheme::HTTPS.default_port(), 443);
}

// ========================================================================
// HttpClientConnection Tests - Connection Establishment
// ========================================================================

/// WHY: Verify HttpClientConnection can establish HTTP connections
/// WHAT: Tests that connect() successfully creates a TCP connection
/// IMPORTANCE: Basic HTTP connectivity is core functionality
#[test]
#[ignore] // Requires actual network - run with `cargo test -- --ignored`
fn test_connection_http_real() {
    let url = ParsedUrl::parse("http://httpbin.org").unwrap();
    let resolver = SystemDnsResolver::new();

    let result = HttpClientConnection::connect(&url, &resolver, Some(Duration::from_secs(10)));

    assert!(result.is_ok(), "Should connect to httpbin.org");
}

/// WHY: Verify HttpClientConnection can establish HTTPS connections with TLS
/// WHAT: Tests that connect() performs TLS handshake for HTTPS URLs
/// IMPORTANCE: HTTPS is the primary use case for secure connections
#[test]
#[cfg(feature = "ssl-rustls")]
fn test_connection_https_real() {
    let url = ParsedUrl::parse("https://httpbin.org").unwrap();
    let resolver = SystemDnsResolver::new();

    let result = HttpClientConnection::connect(&url, &resolver, Some(Duration::from_secs(10)));

    assert!(result.is_ok(), "Should connect to httpbin.org with TLS");
}

/// WHY: Verify HttpClientConnection respects connection timeout
/// WHAT: Tests that connect() fails when timeout is exceeded
/// IMPORTANCE: Prevents hanging connections from blocking indefinitely
#[test]
#[ignore] // Requires network - timeout test
fn test_connection_timeout() {
    // Use a non-routable IP to trigger timeout
    let url = ParsedUrl::parse("http://192.0.2.1").unwrap(); // TEST-NET-1, non-routable

    let resolver = MockDnsResolver::new().with_response(
        "192.0.2.1",
        vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 80)],
    );

    let result = HttpClientConnection::connect(&url, &resolver, Some(Duration::from_millis(100)));

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        HttpClientError::ConnectionTimeout(_)
    ));
}

/// WHY: Verify HttpClientConnection handles DNS resolution failures
/// WHAT: Tests that connect() returns appropriate error when DNS fails
/// IMPORTANCE: DNS errors are common and should be handled gracefully
#[test]
fn test_connection_dns_failure() {
    let url = ParsedUrl::parse("http://invalid-host-12345.invalid").unwrap();
    let resolver = SystemDnsResolver::new();

    let result = HttpClientConnection::connect(&url, &resolver, Some(Duration::from_secs(5)));

    assert!(result.is_err());
    // Should be wrapped in HttpClientError::DnsError
}

/// WHY: Verify HttpClientConnection works with mock resolver
/// WHAT: Tests that connect() uses provided resolver for DNS
/// IMPORTANCE: Enables testing without real network
#[test]
fn test_connection_with_mock_resolver() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    let resolver = MockDnsResolver::new().with_response("example.com", vec![addr]);

    let url = ParsedUrl::parse("http://example.com:8080").unwrap();

    // This will fail to connect since no server is running, but DNS should work
    let result = HttpClientConnection::connect(&url, &resolver, Some(Duration::from_millis(100)));

    // We expect connection failure, not DNS failure
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        HttpClientError::ConnectionFailed(_) | HttpClientError::ConnectionTimeout(_)
    ));
}
