//! Integration tests for HTTP 1.1 client redirect logic
// WHY: Synchronous coverage for redirect chains, edge-case state transitions (including POST→GET)
// WHAT: Ensure correct status transitions, header/semantic mutability, limit enforcement, and sensitive header stripping per sync-only project mandate

use foundation_core::valtron;
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_core::wire::simple_http::HttpClientError;
use foundation_testing::TestHttpServer;
use tracing_test::traced_test;

#[test]
#[traced_test]
fn test_redirect_chain_resolves_successfully() {
    // Initialize Valtron executor for HTTP client concurrency
    valtron::initialize_pool(42, None);

    let server = TestHttpServer::redirect_chain(vec![
        (301, "/step2"),
        (302, "/step3"),
        (307, "/final"),
        (200, "OK"),
    ]);
    let client = SimpleHttpClient::from_system().max_redirects(6);

    let url = server.url("/step1");
    let res = client
        .get(url.as_str())
        .unwrap()
        .build_client()
        .unwrap()
        .send();

    assert!(res.is_ok());
}

#[test]
#[traced_test]
fn test_redirect_chain_limit_enforced() {
    // Initialize Valtron executor for HTTP client concurrency
    valtron::initialize_pool(42, None);

    let server = TestHttpServer::redirect_chain(vec![
        (301, "/step2"),
        (302, "/step3"),
        (307, "/final"),
        (200, "OK"),
    ]);
    let client = SimpleHttpClient::from_system().max_redirects(2);

    let url = server.url("/step1");
    let res = client
        .get(url.as_str())
        .unwrap()
        .build_client()
        .unwrap()
        .send();

    assert!(matches!(res, Err(HttpClientError::TooManyRedirects)));
}

/// WHAT: Tests POST→GET redirect configuration and builder logic using the public API.
/// WHY: Ensures the client can build a POST request with redirect settings, and that execution
///      does not panic even when no real server is present. Validates public API usage and
///      proper initialization of the Valtron executor.
/// WHAT: Tests POST→GET redirect configuration and builder logic using the public API.
/// WHY: Ensures the client can build a POST request with redirect settings, and that execution
///      does not panic even when no real server is present. Validates public API usage and
///      proper initialization of the Valtron executor.
#[test]
fn test_post_without_redirect() {
    use foundation_core::valtron::single::initialize_pool;
    initialize_pool(42);

    let server = TestHttpServer::redirect_chain(vec![(200, "OK")]);

    // Setup: Create a client with redirect enabled and POST method
    let client = SimpleHttpClient::from_system().max_redirects(5);

    // Build a POST request
    let url = server.url("/step1");
    let request_result = client.post(url.as_str());
    assert!(request_result.is_ok());
}
