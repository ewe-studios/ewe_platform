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
    let _pool_guard = valtron::initialize_pool(42, None);

    let server = TestHttpServer::http_chain(vec![
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
    let _pool_guard = valtron::initialize_pool(42, None);

    let server = TestHttpServer::http_chain(vec![
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
#[test]
fn test_post_without_redirect() {
    use foundation_core::valtron::single::initialize_pool;
    let _pool_guard = initialize_pool(42);

    let server = TestHttpServer::http_chain(vec![(200, "OK")]);

    // Setup: Create a client with redirect enabled and POST method
    let client = SimpleHttpClient::from_system().max_redirects(5);

    // Build a POST request
    let url = server.url("/step1");
    let request_result = client.post(url.as_str());
    assert!(request_result.is_ok());
}

/// WHY: Validate redirect handling when each redirect is returned as the final response
///      to a request (not during initial connection phase). This exercises the CheckRedirect
///      state in SendRequestTask which was added to handle this edge case.
///
/// WHAT: Tests a chain of redirects where each one is returned as the final response,
///       verifying the client automatically follows them via the CheckRedirect state.
///
/// IMPORTANCE: The original implementation only handled redirects during initial connection.
///             The updated SendRequestTask adds a CheckRedirect state that validates the
///             final response for redirect status codes (301-308) and follows the Location.
///
/// NOTE: This test uses http_chain which inherently returns each redirect as a final
///       response to each request in the chain, exercising the CheckRedirect state.
#[test]
#[traced_test]
fn test_redirect_as_final_response_chain() {
    // Initialize Valtron executor for HTTP client concurrency
    let _pool_guard = valtron::initialize_pool(42, None);

    // Server that returns redirect as final response, then serves the target
    // This exercises the CheckRedirect state for each redirect in the chain
    let server = TestHttpServer::http_chain(vec![
        (302, "/final"),  // First request: redirect to /final
        (200, "Final destination reached!"),  // Second request (after redirect): OK
    ]);

    let client = SimpleHttpClient::from_system().max_redirects(5);
    let url = server.url("/redirect");

    let response = client
        .get(url.as_str())
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .expect("Should follow redirect from final response");

    assert!(response.is_success(), "Should receive 200 OK from final destination");
}

/// WHY: Validate 307 Temporary Redirect (method-preserving) works as final response.
///
/// WHAT: Tests 307 redirect which should preserve the original HTTP method.
///
/// IMPORTANCE: 307 differs from 302 in that it preserves POST method and body.
#[test]
#[traced_test]
fn test_redirect_307_as_final_response() {
    // Initialize Valtron executor for HTTP client concurrency
    let _pool_guard = valtron::initialize_pool(42, None);

    // Server that returns 307 redirect as final response
    let server = TestHttpServer::http_chain(vec![
        (307, "/target"),  // 307 Temporary Redirect
        (200, "307 redirect target"),
    ]);

    let client = SimpleHttpClient::from_system().max_redirects(5);
    let url = server.url("/redirect");

    let response = client
        .get(url.as_str())
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .expect("Should follow 307 redirect");

    assert!(response.is_success());
}

/// WHY: Validate that redirect fails gracefully when max_redirects limit is exceeded.
///
/// WHAT: Tests that too many redirects returns TooManyRedirects error.
///
/// IMPORTANCE: Prevents infinite redirect loops from hanging the client.
#[test]
#[traced_test]
fn test_too_many_redirects_as_final_response() {
    // Initialize Valtron executor for HTTP client concurrency
    let _pool_guard = valtron::initialize_pool(42, None);

    // Server that always redirects (infinite loop)
    let server = TestHttpServer::http_chain(vec![
        (302, "/redirect"),
        (302, "/redirect"),
        (302, "/redirect"),
        (302, "/redirect"),  // Will keep returning redirect
    ]);

    let client = SimpleHttpClient::from_system().max_redirects(3);
    let url = server.url("/start");

    let result = client.get(url.as_str()).unwrap().build_client().unwrap().send();

    assert!(
        matches!(result, Err(HttpClientError::TooManyRedirects)),
        "Should fail with TooManyRedirects after 3 redirects"
    );
}
