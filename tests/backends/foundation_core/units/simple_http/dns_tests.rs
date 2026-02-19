//! Unit tests for simple_http DNS resolvers moved into the canonical units test tree.
//!
//! These tests exercise `DnsResolver` implementations used by the `foundation_core`
//! simple HTTP client: `StaticSocketAddr`, `SystemDnsResolver`, `MockDnsResolver`,
//! and `CachingDnsResolver`.
//!
//! These tests aim to be fast and deterministic where possible (use mock/static
//! resolvers). Time-based behavior for cache expiration uses short TTLs to keep
//! test runtime minimal.

use foundation_core::wire::simple_http::client::{
    CachingDnsResolver, DnsResolver, MockDnsResolver, StaticSocketAddr, SystemDnsResolver,
};
use foundation_core::wire::simple_http::client::{DnsError, HttpClientError};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use std::{io, thread};

#[test]
fn test_static_socket_addr_resolves_to_configured_address() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    let resolver = StaticSocketAddr::new(addr);
    let result = resolver.resolve("example.com", 80);

    assert!(result.is_ok(), "StaticSocketAddr should always resolve");
    let addrs = result.unwrap();
    assert_eq!(addrs, vec![addr]);
}

#[test]
fn test_mock_resolver_returns_configured_response() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    let resolver = MockDnsResolver::new().with_response("example.com", vec![addr]);

    let result = resolver.resolve("example.com", 80);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), vec![addr]);
}

#[test]
fn test_mock_resolver_returns_configured_error() {
    let resolver = MockDnsResolver::new().with_error(
        "error.com",
        DnsError::ResolutionFailed("test error".to_string()),
    );

    let result = resolver.resolve("error.com", 80);
    assert!(result.is_err());
    matches!(result.unwrap_err(), DnsError::ResolutionFailed(_));
}

#[test]
fn test_mock_resolver_returns_not_found_for_unconfigured_host() {
    let resolver = MockDnsResolver::new();
    let result = resolver.resolve("unconfigured.com", 80);

    assert!(result.is_err());
    matches!(result.unwrap_err(), DnsError::NoAddressesFound(_));
}

#[test]
fn test_caching_resolver_caches_results() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    let mock = MockDnsResolver::new().with_response("cache-test.com", vec![addr]);
    let resolver = CachingDnsResolver::new(mock, Duration::from_secs(60));

    // First resolution - cache miss
    assert_eq!(resolver.cache_size(), 0);
    let result1 = resolver.resolve("cache-test.com", 80);
    assert!(result1.is_ok());
    assert_eq!(resolver.cache_size(), 1);

    // Second resolution - cache hit
    let result2 = resolver.resolve("cache-test.com", 80);
    assert!(result2.is_ok());
    assert_eq!(result1.unwrap(), result2.unwrap());
    assert_eq!(resolver.cache_size(), 1);
}

#[test]
fn test_caching_resolver_expires_entries() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    let mock = MockDnsResolver::new().with_response("expire-test.com", vec![addr]);
    // Short TTL for deterministic expiration
    let resolver = CachingDnsResolver::new(mock, Duration::from_millis(100));

    // First resolution
    let result1 = resolver.resolve("expire-test.com", 80);
    assert!(result1.is_ok());
    assert_eq!(resolver.cache_size(), 1);

    // Wait for expiration
    thread::sleep(Duration::from_millis(150));

    // Second resolution after expiration
    let result2 = resolver.resolve("expire-test.com", 80);
    assert!(result2.is_ok());
    // Cache still has 1 entry (replaced the expired one)
    assert_eq!(resolver.cache_size(), 1);
}

#[test]
fn test_caching_resolver_clear_cache() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    let mock = MockDnsResolver::new().with_response("clear-test.com", vec![addr]);
    let resolver = CachingDnsResolver::new(mock, Duration::from_secs(60));

    // Populate cache
    resolver.resolve("clear-test.com", 80).unwrap();
    assert_eq!(resolver.cache_size(), 1);

    // Clear cache
    resolver.clear_cache();
    assert_eq!(resolver.cache_size(), 0);
}

#[test]
fn test_caching_resolver_differentiates_by_port() {
    let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 80);
    let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 443);
    let mock = MockDnsResolver::new()
        .with_response("port-test.com", vec![addr1])
        .with_response("port-test.com", vec![addr2]);

    let resolver = CachingDnsResolver::new(mock, Duration::from_secs(60));

    // Resolve with different ports (cache keys include port)
    resolver.resolve("port-test.com", 80).unwrap();
    resolver.resolve("port-test.com", 443).unwrap();

    // Should have 2 cache entries (host:port keys)
    assert_eq!(resolver.cache_size(), 2);
}

#[test]
fn test_dns_resolver_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SystemDnsResolver>();
    assert_send_sync::<MockDnsResolver>();
    assert_send_sync::<CachingDnsResolver<SystemDnsResolver>>();
}

/// WHY: Verify DnsError::ResolutionFailed creates correct error message
/// WHAT: Tests that the error message includes the hostname
#[test]
fn test_dns_error_resolution_failed_display() {
    let error = DnsError::ResolutionFailed("example.com".to_string());
    let display = format!("{}", error);
    assert!(display.contains("DNS resolution failed"));
    assert!(display.contains("example.com"));
}

/// WHY: Verify DnsError::InvalidHost creates correct error message
/// WHAT: Tests that invalid hostname errors are clearly communicated
#[test]
fn test_dns_error_invalid_host_display() {
    let error = DnsError::InvalidHost("".to_string());
    let display = format!("{}", error);
    assert!(display.contains("Invalid hostname"));
}

/// WHY: Verify DnsError::NoAddressesFound creates correct error message
/// WHAT: Tests that no addresses error includes the hostname
#[test]
fn test_dns_error_no_addresses_display() {
    let error = DnsError::NoAddressesFound("localhost".to_string());
    let display = format!("{}", error);
    assert!(display.contains("No addresses found"));
    assert!(display.contains("localhost"));
}

/// WHY: Verify DnsError::IoError wraps std::io::Error correctly
/// WHAT: Tests that I/O errors are properly converted and displayed
#[test]
fn test_dns_error_io_error_conversion() {
    let io_error = io::Error::new(io::ErrorKind::TimedOut, "timeout");
    let dns_error = DnsError::from(io_error);
    let display = format!("{}", dns_error);
    assert!(display.contains("I/O error"));
}

/// WHY: Verify HttpClientError::DnsError wraps DnsError correctly
/// WHAT: Tests that DNS errors are properly converted to HTTP client errors
#[test]
fn test_http_client_error_from_dns_error() {
    let dns_error = DnsError::ResolutionFailed("test.com".to_string());
    let http_error = HttpClientError::from(dns_error);
    let display = format!("{}", http_error);
    assert!(display.contains("DNS error"));
    assert!(display.contains("test.com"));
}

/// WHY: Verify HttpClientError::ConnectionFailed creates correct message
/// WHAT: Tests that connection failures are clearly described
#[test]
fn test_http_client_error_connection_failed() {
    let error = HttpClientError::ConnectionFailed("connection reset".to_string());
    let display = format!("{}", error);
    assert!(display.contains("Connection failed"));
    assert!(display.contains("connection reset"));
}

/// WHY: Verify HttpClientError::InvalidUrl creates correct message
/// WHAT: Tests that invalid URL errors include the URL
#[test]
fn test_http_client_error_invalid_url() {
    let error = HttpClientError::InvalidUrl("not a url".to_string());
    let display = format!("{}", error);
    assert!(display.contains("Invalid URL"));
    assert!(display.contains("not a url"));
}

/// WHY: Verify error types implement std::error::Error trait
/// WHAT: Tests that errors can be used with error handling infrastructure
#[test]
fn test_errors_implement_std_error() {
    let dns_error: &dyn std::error::Error = &DnsError::ResolutionFailed("test".to_string());
    let http_error: &dyn std::error::Error = &HttpClientError::InvalidUrl("test".to_string());

    // These should compile and execute without issues
    assert!(!dns_error.to_string().is_empty());
    assert!(!http_error.to_string().is_empty());
}

/// WHY: Verify SystemDnsResolver can resolve localhost
/// WHAT: Tests basic DNS resolution functionality with a known hostname
#[test]
fn test_system_resolver_resolves_localhost() {
    let resolver = SystemDnsResolver::new();
    let result = resolver.resolve("localhost", 8080);

    assert!(result.is_ok(), "localhost should resolve successfully");
    let addrs = result.unwrap();
    assert!(
        !addrs.is_empty(),
        "localhost should have at least one address"
    );
}

/// WHY: Verify SystemDnsResolver rejects empty hostnames
/// WHAT: Tests that invalid input is properly rejected
#[test]
fn test_system_resolver_rejects_empty_host() {
    let resolver = SystemDnsResolver::new();
    let result = resolver.resolve("", 8080);

    assert!(result.is_err(), "empty hostname should fail");
    matches!(result.unwrap_err(), DnsError::InvalidHost(_));
}

/// WHY: Verify SystemDnsResolver handles invalid hostnames
/// WHAT: Tests error handling for hostnames that don't resolve
#[test]
fn test_system_resolver_handles_invalid_host() {
    let resolver = SystemDnsResolver::new();
    let result = resolver.resolve("this-hostname-should-not-exist-12345.invalid", 8080);

    assert!(result.is_err(), "invalid hostname should fail to resolve");
}

/// WHY: Verify CachingDnsResolver propagates errors from inner resolver
/// WHAT: Tests that DNS resolution errors are not cached and properly propagated
#[test]
fn test_caching_resolver_propagates_errors() {
    let mock = MockDnsResolver::new()
        .with_error("error.com", DnsError::ResolutionFailed("test".to_string()));
    let resolver = CachingDnsResolver::new(mock, Duration::from_secs(60));

    let result = resolver.resolve("error.com", 80);
    assert!(result.is_err());

    // Error should not be cached
    assert_eq!(resolver.cache_size(), 0);
}
