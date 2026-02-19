//! Integration tests for HTTP client task-iterator machinery.
//!
//! WHY: Verifies that the task-iterator components (actions, tasks, executor)
//! work together correctly in realistic scenarios. Unit tests verify individual
//! components, but integration tests verify the complete system.
//!
//! WHAT: Tests HTTP client components work together with public APIs.
//!
//! HOW: Creates realistic HTTP request scenarios using public client APIs.

use foundation_core::wire::simple_http::client::{
    ClientRequestBuilder, DnsResolver, MockDnsResolver, SimpleHttpClient, SystemDnsResolver,
};
use std::net::SocketAddr;

// ========================================================================
// Public API Integration Tests
// ========================================================================

/// WHY: Verify SimpleHttpClient can be constructed with default resolver
/// WHAT: Tests that SimpleHttpClient::new() works and creates GET requests
#[test]
fn test_simple_http_client_construction() {
    let client = SimpleHttpClient::new();
    let builder = client.get("http://example.com").unwrap();
    let request = builder.build();

    assert_eq!(request.url.host_str().unwrap(), "example.com");
}

/// WHY: Verify SimpleHttpClient can be constructed with custom resolver
/// WHAT: Tests that SimpleHttpClient::with_resolver() accepts custom resolvers
#[test]
fn test_simple_http_client_with_custom_resolver() {
    let resolver = SystemDnsResolver::new();
    let client = SimpleHttpClient::with_resolver(resolver);
    let builder = client.get("http://example.com").unwrap();
    let request = builder.build();

    assert_eq!(request.url.host_str().unwrap(), "example.com");
}

/// WHY: Verify MockDnsResolver can be used for testing
/// WHAT: Tests that MockDnsResolver integrates with SimpleHttpClient
#[test]
fn test_simple_http_client_with_mock_resolver() {
    let addr = SocketAddr::new("127.0.0.1".parse().unwrap(), 8080);
    let resolver = MockDnsResolver::new().with_response("example.com", vec![addr]);

    let client = SimpleHttpClient::with_resolver(resolver.clone());
    let builder = client.get("http://example.com").unwrap();
    let request = builder.build();

    // Verify resolver works independently
    let resolved = resolver.resolve("example.com", 80);
    assert!(resolved.is_ok());

    assert_eq!(request.url.host_str().unwrap(), "example.com");
}

/// WHY: Verify ClientRequestBuilder can create various request types
/// WHAT: Tests that all HTTP method builders work
#[test]
fn test_client_request_builder_methods() {
    let get = ClientRequestBuilder::get("http://example.com")
        .unwrap()
        .build();
    assert_eq!(get.url.scheme().as_str(), "http");

    let post = ClientRequestBuilder::post("http://example.com")
        .unwrap()
        .build();
    assert_eq!(post.url.scheme().as_str(), "http");

    let put = ClientRequestBuilder::put("http://example.com")
        .unwrap()
        .build();
    assert_eq!(put.url.scheme().as_str(), "http");

    let delete = ClientRequestBuilder::delete("http://example.com")
        .unwrap()
        .build();
    assert_eq!(delete.url.scheme().as_str(), "http");

    let head = ClientRequestBuilder::head("http://example.com")
        .unwrap()
        .build();
    assert_eq!(head.url.scheme().as_str(), "http");

    let options = ClientRequestBuilder::options("http://example.com")
        .unwrap()
        .build();
    assert_eq!(options.url.scheme().as_str(), "http");

    let patch = ClientRequestBuilder::patch("http://example.com")
        .unwrap()
        .build();
    assert_eq!(patch.url.scheme().as_str(), "http");
}

/// WHY: Verify SimpleHttpClient has all HTTP method helpers
/// WHAT: Tests that client provides convenient methods for all HTTP verbs
#[test]
fn test_simple_http_client_all_methods() {
    let client = SimpleHttpClient::new();

    let _get = client.get("http://example.com").unwrap();
    let _post = client.post("http://example.com").unwrap();
    let _put = client.put("http://example.com").unwrap();
    let _delete = client.delete("http://example.com").unwrap();
    let _head = client.head("http://example.com").unwrap();
    let _options = client.options("http://example.com").unwrap();
    let _patch = client.patch("http://example.com").unwrap();
}

/// WHY: Verify ClientRequestBuilder can add headers
/// WHAT: Tests that header() method works with SimpleHeader enum
#[test]
fn test_client_request_builder_with_headers() {
    use foundation_core::wire::simple_http::SimpleHeader;

    let builder = ClientRequestBuilder::get("http://example.com")
        .unwrap()
        .header(SimpleHeader::USER_AGENT, "TestClient/1.0")
        .header(SimpleHeader::ACCEPT, "application/json");

    let request = builder.build();
    assert_eq!(request.url.host_str().unwrap(), "example.com");
}

/// WHY: Verify SimpleHttpClient configuration is flexible
/// WHAT: Tests that client can be constructed with SystemDnsResolver
#[test]
fn test_simple_http_client_configuration() {
    let resolver = SystemDnsResolver::new();
    let client = SimpleHttpClient::with_resolver(resolver);

    let builder = client.get("http://example.com").unwrap();
    let request = builder.build();

    assert_eq!(request.url.host_str().unwrap(), "example.com");
}

/// WHY: Verify SimpleHttpClient::default() provides sensible defaults
/// WHAT: Tests that Default trait implementation works
#[test]
fn test_simple_http_client_default() {
    let client = SimpleHttpClient::default();

    // Verify default client works
    let builder = client.get("http://example.com").unwrap();
    let request = builder.build();
    assert_eq!(request.url.host_str().unwrap(), "example.com");
}

/// WHY: Verify ClientRequestBuilder can be passed to client.request()
/// WHAT: Tests the advanced request() API
#[test]
fn test_simple_http_client_request_method() {
    let client = SimpleHttpClient::new();
    let builder = ClientRequestBuilder::get("http://example.com").unwrap();

    let _result = client.request(builder);
    // Note: Currently returns ClientRequestBuilder until api.rs is complete
}

// ========================================================================
// Clone Trait Integration Tests
// ========================================================================

/// WHY: Verify DnsResolver trait includes Clone bound
/// WHAT: Tests that resolvers can be cloned (required for RedirectAction)
#[test]
fn test_dns_resolver_clone() {
    let resolver1 = SystemDnsResolver::new();
    let resolver2 = resolver1.clone();

    // Both resolvers should work
    assert!(resolver1.resolve("localhost", 80).is_ok());
    assert!(resolver2.resolve("localhost", 80).is_ok());
}

/// WHY: Verify MockDnsResolver cloning preserves configuration
/// WHAT: Tests that cloned mock resolvers maintain their responses
#[test]
fn test_mock_dns_resolver_clone_preserves_config() {
    let addr = SocketAddr::new("10.0.0.1".parse().unwrap(), 443);
    let resolver1 = MockDnsResolver::new().with_response("secure.example.com", vec![addr]);
    let resolver2 = resolver1.clone();

    // Both should return the same configured response
    let result1 = resolver1.resolve("secure.example.com", 443);
    let result2 = resolver2.resolve("secure.example.com", 443);

    assert!(result1.is_ok());
    assert!(result2.is_ok());
    assert_eq!(result1.unwrap(), result2.unwrap());
}

/// WHY: Verify SimpleHttpClient can use cloned resolvers
/// WHAT: Tests that client works with cloned resolver instances
#[test]
fn test_simple_http_client_with_cloned_resolver() {
    let addr = SocketAddr::new("172.16.0.1".parse().unwrap(), 8080);
    let resolver1 = MockDnsResolver::new().with_response("api.example.com", vec![addr]);
    let resolver2 = resolver1.clone();

    let client1 = SimpleHttpClient::with_resolver(resolver1);
    let client2 = SimpleHttpClient::with_resolver(resolver2);

    // Both clients should work
    assert!(client1.get("http://api.example.com").is_ok());
    assert!(client2.get("http://api.example.com").is_ok());
}
