//! Unit tests for URL scheme parsing and behavior.
//!
//! These tests were copied into the canonical `tests/.../units/simple_http` tree
//! and exercise scheme-related behaviors of the URL/URI parsing utilities used
//! by the simple_http client. They are fast, deterministic unit tests that
//! avoid network IO.

use foundation_core::wire::simple_http::{client::ParsedUrl, url::Scheme};

#[test]
fn test_parsed_url_http_has_default_port_80() {
    let url = ParsedUrl::parse("http://example.com").expect("should parse http URL");

    // Default scheme detection
    assert!(url.scheme().is_http(), "scheme should be detected as HTTP");

    // Default port for HTTP
    assert_eq!(url.port_or_default(), 80);
    assert_eq!(url.host_str().unwrap(), "example.com");
}

#[test]
fn test_parsed_url_https_has_default_port_443() {
    let url = ParsedUrl::parse("https://example.com").expect("should parse https URL");

    // Default scheme detection
    assert!(
        url.scheme().is_https(),
        "scheme should be detected as HTTPS"
    );

    // Default port for HTTPS
    assert_eq!(url.port_or_default(), 443);
    assert_eq!(url.host_str().unwrap(), "example.com");
}

#[test]
fn test_parsed_url_explicit_port_preserved() {
    let url =
        ParsedUrl::parse("http://example.com:8080/path").expect("should parse http URL with port");

    assert!(url.scheme().is_http());
    assert_eq!(url.port().unwrap(), 8080);
    assert_eq!(url.port_or_default(), 8080);
    assert_eq!(url.path(), "/path");
}

#[test]
fn test_parsed_url_scheme_case_insensitive() {
    let url =
        ParsedUrl::parse("HTTP://Example.COM").expect("should parse case-insensitive scheme/host");

    // Scheme detection should be case-insensitive
    assert!(url.scheme().is_http());
    // Host name normalization may preserve case; we check lowercase host_str for robustness
    assert_eq!(
        url.host_str().map(|s| s.to_ascii_lowercase()).unwrap(),
        "example.com"
    );
}

#[test]
fn test_parsed_url_supports_custom_protocols() {
    // Unsupported/unknown schemes should produce a parse error
    let result = ParsedUrl::parse("ftp://example.com/resource");
    assert!(
        result.is_ok(),
        "unknown scheme should return be a Custom protocol"
    );
}

#[test]
fn test_scheme_constants() {
    assert_eq!(Scheme::HTTP.as_str(), "http");
    assert_eq!(Scheme::HTTPS.as_str(), "https");
    assert_eq!(Scheme::HTTP.default_port(), 80);
    assert_eq!(Scheme::HTTPS.default_port(), 443);
}

#[test]
fn test_scheme_parse_http() {
    let (scheme, rest) = Scheme::parse_from_uri("http://example.com").unwrap();
    assert_eq!(scheme.as_str(), "http");
    assert_eq!(rest, "//example.com");
}

#[test]
fn test_scheme_parse_https() {
    let (scheme, rest) = Scheme::parse_from_uri("https://example.com").unwrap();
    assert_eq!(scheme.as_str(), "https");
    assert_eq!(rest, "//example.com");
}

#[test]
fn test_scheme_parse_custom() {
    let (scheme, rest) = Scheme::parse_from_uri("ftp://example.com").unwrap();
    assert_eq!(scheme.as_str(), "ftp");
    assert_eq!(rest, "//example.com");
}

#[test]
fn test_scheme_case_insensitive() {
    let (scheme, _) = Scheme::parse_from_uri("HTTP://example.com").unwrap();
    assert_eq!(scheme.as_str(), "http");

    let (scheme, _) = Scheme::parse_from_uri("HtTpS://example.com").unwrap();
    assert_eq!(scheme.as_str(), "https");
}

#[test]
fn test_scheme_validation() {
    assert!(Scheme::parse_from_uri("example.com").is_err()); // Missing scheme
    assert!(Scheme::parse_from_uri(":///example.com").is_err()); // Empty scheme
    assert!(Scheme::parse_from_uri("123://example.com").is_err()); // Starts with digit
    assert!(Scheme::parse_from_uri("ht@tp://example.com").is_err()); // Invalid char
}

#[test]
fn test_scheme_valid_custom() {
    assert!(Scheme::parse_from_uri("http+unix://socket").is_ok());
    assert!(Scheme::parse_from_uri("custom-scheme://host").is_ok());
    assert!(Scheme::parse_from_uri("scheme.v2://host").is_ok());
}

#[test]
fn test_scheme_is_http_https() {
    assert!(Scheme::HTTP.is_http());
    assert!(!Scheme::HTTP.is_https());
    assert!(Scheme::HTTPS.is_https());
    assert!(!Scheme::HTTPS.is_http());
}
