use foundation_core::wire::simple_http::client::{DnsError, HttpClientError};
use std::io;

#[test]
fn test_dns_error_resolution_failed_display() {
    let error = DnsError::ResolutionFailed("example.com".to_string());
    let display = format!("{}", error);
    assert!(display.contains("DNS resolution failed"));
    assert!(display.contains("example.com"));
}

#[test]
fn test_dns_error_invalid_host_display() {
    let error = DnsError::InvalidHost("".to_string());
    let display = format!("{}", error);
    assert!(display.contains("Invalid hostname"));
}

#[test]
fn test_dns_error_no_addresses_display() {
    let error = DnsError::NoAddressesFound("localhost".to_string());
    let display = format!("{}", error);
    assert!(display.contains("No addresses found"));
    assert!(display.contains("localhost"));
}

#[test]
fn test_dns_error_io_error_conversion() {
    let io_error = io::Error::new(io::ErrorKind::TimedOut, "timeout");
    let dns_error = DnsError::from(io_error);
    let display = format!("{}", dns_error);
    assert!(display.contains("I/O error"));
}

#[test]
fn test_http_client_error_from_dns_error() {
    let dns_error = DnsError::ResolutionFailed("test.com".to_string());
    let http_error = HttpClientError::from(dns_error);
    let display = format!("{}", http_error);
    assert!(display.contains("DNS error"));
    assert!(display.contains("test.com"));
}

#[test]
fn test_http_client_error_connection_failed() {
    let error = HttpClientError::ConnectionFailed("connection reset".to_string());
    let display = format!("{}", error);
    assert!(display.contains("Connection failed"));
    assert!(display.contains("connection reset"));
}

#[test]
fn test_http_client_error_invalid_url() {
    let error = HttpClientError::InvalidUrl("not a url".to_string());
    let display = format!("{}", error);
    assert!(display.contains("Invalid URL"));
    assert!(display.contains("not a url"));
}

#[test]
fn test_errors_implement_std_error() {
    let dns_error: &dyn std::error::Error = &DnsError::ResolutionFailed("test".to_string());
    let http_error: &dyn std::error::Error = &HttpClientError::InvalidUrl("test".to_string());

    // These should compile and execute without issues
    assert!(!dns_error.to_string().is_empty());
    assert!(!http_error.to_string().is_empty());
}
