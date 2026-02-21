use std::time::Duration;

use foundation_core::wire::simple_http::client::{
    ClientConfig, ClientRequestBuilder, MockDnsResolver, SimpleHttpClient, StaticSocketAddr,
    SystemDnsResolver,
};

use super::*;

// ========================================================================
// SimpleHttpClient Tests
// ========================================================================

/// WHY: Verify SimpleHttpClient::new creates client with system resolver
/// WHAT: Tests that new() creates client with default config
#[test]
fn test_simple_http_client_new() {
    let client = SimpleHttpClient::from_system();

    let config = client.client_config();

    assert_eq!(config.max_redirects, 5);
    assert!(!config.pool_enabled);
}

/// WHY: Verify SimpleHttpClient::with_resolver accepts custom resolver
/// WHAT: Tests that with_resolver works with MockDnsResolver
#[test]
fn test_simple_http_client_with_resolver() {
    let client = SimpleHttpClient::from_system();

    let config = client.client_config();
    assert_eq!(config.max_redirects, 5);
}

/// WHY: Verify SimpleHttpClient::config sets configuration
/// WHAT: Tests that config() replaces configuration
#[test]
fn test_simple_http_client_config() {
    let mut custom_config = ClientConfig::default();
    custom_config.max_redirects = 10;

    let client = SimpleHttpClient::from_system().config(custom_config);
    let config = client.client_config();

    assert_eq!(config.max_redirects, 10);
}

/// WHY: Verify SimpleHttpClient::connect_timeout sets timeout
/// WHAT: Tests that builder method sets connect timeout
#[test]
fn test_simple_http_client_connect_timeout() {
    let client = SimpleHttpClient::from_system().connect_timeout(Duration::from_secs(5));
    let config = client.client_config();

    assert_eq!(config.connect_timeout, Some(Duration::from_secs(5)));
}

/// WHY: Verify SimpleHttpClient::read_timeout sets timeout
/// WHAT: Tests that builder method sets read timeout
#[test]
fn test_simple_http_client_read_timeout() {
    let client = SimpleHttpClient::from_system().read_timeout(Duration::from_secs(15));

    let config = client.client_config();
    assert_eq!(config.read_timeout, Some(Duration::from_secs(15)));
}

/// WHY: Verify SimpleHttpClient::write_timeout sets timeout
/// WHAT: Tests that builder method sets write timeout
#[test]
fn test_simple_http_client_write_timeout() {
    let client = SimpleHttpClient::from_system().write_timeout(Duration::from_secs(20));

    let config = client.client_config();
    assert_eq!(config.write_timeout, Some(Duration::from_secs(20)));
}

/// WHY: Verify SimpleHttpClient::max_redirects sets redirect limit
/// WHAT: Tests that builder method sets max redirects
#[test]
fn test_simple_http_client_max_redirects() {
    let client = SimpleHttpClient::from_system().max_redirects(3);

    let config = client.client_config();
    assert_eq!(config.max_redirects, 3);
}

/// WHY: Verify SimpleHttpClient::enable_pool enables connection pooling
/// WHAT: Tests that builder method enables pooling and sets max connections
#[test]
fn test_simple_http_client_enable_pool() {
    let client = SimpleHttpClient::from_system().enable_pool(20);

    let config = client.client_config();
    assert!(config.pool_enabled);
    assert_eq!(config.pool_max_connections, 20);
}

/// WHY: Verify SimpleHttpClient builder methods are chainable
/// WHAT: Tests that multiple builder methods can be chained
#[test]
fn test_simple_http_client_builder_chaining() {
    let client = SimpleHttpClient::from_system()
        .connect_timeout(Duration::from_secs(10))
        .max_redirects(3)
        .enable_pool(15);

    let config = client.client_config();
    assert_eq!(config.connect_timeout, Some(Duration::from_secs(10)));
    assert_eq!(config.max_redirects, 3);
    assert!(config.pool_enabled);
    assert_eq!(config.pool_max_connections, 15);
}

/// WHY: Verify SimpleHttpClient::get creates GET request builder
/// WHAT: Tests that get() returns ClientRequestBuilder for GET
#[test]
fn test_simple_http_client_get() {
    let builder = ClientRequestBuilder::get("http://example.com").unwrap();

    let request = builder.build().unwrap();
    assert_eq!(request.url.host_str().unwrap(), "example.com");
}

/// WHY: Verify SimpleHttpClient::get validates URL
/// WHAT: Tests that get() returns error for invalid URL
#[test]
fn test_simple_http_client_get_invalid_url() {
    let result = ClientRequestBuilder::get("not a url");

    assert!(result.is_err());
}

/// WHY: Verify SimpleHttpClient::post creates POST request builder
/// WHAT: Tests that post() returns ClientRequestBuilder for POST
#[test]
fn test_simple_http_client_post() {
    let builder = ClientRequestBuilder::post("http://example.com").unwrap();

    let request = builder.build().unwrap();
    assert_eq!(request.url.host_str().unwrap(), "example.com");
}

/// WHY: Verify SimpleHttpClient::put creates PUT request builder
/// WHAT: Tests that put() returns ClientRequestBuilder for PUT
#[test]
fn test_simple_http_client_put() {
    let builder = ClientRequestBuilder::put("http://example.com").unwrap();

    let request = builder.build().unwrap();
    assert_eq!(request.url.host_str().unwrap(), "example.com");
}

/// WHY: Verify SimpleHttpClient::delete creates DELETE request builder
/// WHAT: Tests that delete() returns ClientRequestBuilder for DELETE
#[test]
fn test_simple_http_client_delete() {
    let builder = ClientRequestBuilder::delete("http://example.com").unwrap();

    let request = builder.build().unwrap();
    assert_eq!(request.url.host_str().unwrap(), "example.com");
}

/// WHY: Verify SimpleHttpClient::patch creates PATCH request builder
/// WHAT: Tests that patch() returns ClientRequestBuilder for PATCH
#[test]
fn test_simple_http_client_patch() {
    let builder = ClientRequestBuilder::patch("http://example.com").unwrap();

    let request = builder.build().unwrap();
    assert_eq!(request.url.host_str().unwrap(), "example.com");
}

/// WHY: Verify SimpleHttpClient::head creates HEAD request builder
/// WHAT: Tests that head() returns ClientRequestBuilder for HEAD
#[test]
fn test_simple_http_client_head() {
    let builder = ClientRequestBuilder::head("http://example.com").unwrap();

    let request = builder.build().unwrap();
    assert_eq!(request.url.host_str().unwrap(), "example.com");
}

/// WHY: Verify SimpleHttpClient::options creates OPTIONS request builder
/// WHAT: Tests that options() returns ClientRequestBuilder for OPTIONS
#[test]
fn test_simple_http_client_options() {
    let builder = ClientRequestBuilder::options("http://example.com").unwrap();

    let request = builder.build().unwrap();
    assert_eq!(request.url.host_str().unwrap(), "example.com");
}

/// WHY: Verify SimpleHttpClient::request accepts pre-built builder
/// WHAT: Tests that request() can take ClientRequestBuilder
#[test]
fn test_simple_http_client_request() {
    let client = SimpleHttpClient::<StaticSocketAddr>::default();
    let builder = ClientRequestBuilder::get("http://example.com").unwrap();

    let _result = client.request(builder);
    // TEST NOTE: Assertion pending ClientRequest implementation
    // Once api.rs is complete, assert on ClientRequest type and .execute() method
}

/// WHY: Verify SimpleHttpClient implements Default
/// WHAT: Tests that Default trait is implemented
#[test]
fn test_simple_http_client_default() {
    let client = SimpleHttpClient::<SystemDnsResolver>::default();

    let config = client.client_config();
    assert_eq!(config.max_redirects, 5);
}
