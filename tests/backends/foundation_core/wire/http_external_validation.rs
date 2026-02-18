//! HTTP client external validation tests against real HTTP servers.
//!
//! WHY: Validates HTTP client works with real-world servers, not just our test server.
//! Ensures HTTP implementation is compliant with actual server behavior.
//!
//! WHAT: Integration tests against public HTTP test servers (httpbin.org).
//!
//! HOW: Uses reqwest (existing dev-dependency) as reference client to verify
//! httpbin.org is accessible, then tests our client against same endpoints.
//!
//! NOTE: These tests are #[ignore] by default (require network).
//! Run with: cargo test -- --ignored

// Note: Uncomment these when HTTP client public API is ready
// use foundation_core::wire::simple_http::client::SimpleHttpClient;

// ========================================================================
// External HTTP Server Validation Tests
// ========================================================================

use foundation_core::wire::simple_http::client::SimpleHttpClient;

/// WHY: Verify HTTP client works with real external HTTP server
/// WHAT: Tests basic GET request against httpbin.org
#[test]
#[ignore] // Run with: cargo test test_external_httpbin_get -- --ignored
fn test_external_httpbin_get() {
    let client = SimpleHttpClient::from_system();
    let response = client.get("http://httpbin.org/get").execute().unwrap();
    assert_eq!(response.status(), 200);
    assert!(response
        .body_text()
        .unwrap()
        .contains("\"url\": \"http://httpbin.org/get\""));
}

/// WHY: Verify HTTP client follows redirects with real server
/// WHAT: Tests 302 redirect handling against httpbin.org
#[test]
#[ignore] // Requires network
fn test_external_httpbin_redirect() {
    // TODO: Uncomment when SimpleHttpClient has execute() method
    // let client = SimpleHttpClient::new();
    // // httpbin.org/redirect/1 returns 302 to /get
    // let response = client.get("http://httpbin.org/redirect/1").execute().unwrap();
    // assert_eq!(response.status(), 200); // After following redirect
    // assert!(response.body_text().unwrap().contains("/get"));
}

/// WHY: Verify HTTP client works with HTTPS and real certificates
/// WHAT: Tests TLS handshake against real server
#[test]
#[ignore] // Requires network
fn test_external_httpbin_https() {
    // TODO: Uncomment when SimpleHttpClient has execute() method
    // let client = SimpleHttpClient::new();
    // let response = client.get("https://httpbin.org/get").execute().unwrap();
    // assert_eq!(response.status(), 200);
}

/// WHY: Verify HTTP client handles various status codes correctly
/// WHAT: Tests 404, 500, and other non-200 responses from real server
#[test]
#[ignore] // Requires network
fn test_external_httpbin_status_codes() {
    // TODO: Uncomment when SimpleHttpClient has execute() method
    // let client = SimpleHttpClient::new();
    //
    // let response_404 = client.get("http://httpbin.org/status/404").execute().unwrap();
    // assert_eq!(response_404.status(), 404);
    //
    // let response_500 = client.get("http://httpbin.org/status/500").execute().unwrap();
    // assert_eq!(response_500.status(), 500);
    //
    // let response_201 = client.get("http://httpbin.org/status/201").execute().unwrap();
    // assert_eq!(response_201.status(), 201);
}

/// WHY: Verify HTTP client sends headers correctly to real server
/// WHAT: Tests httpbin.org/headers echoes our headers back
#[test]
#[ignore] // Requires network
fn test_external_httpbin_headers() {
    // TODO: Uncomment when SimpleHttpClient has execute() method and header support
    // use foundation_core::wire::simple_http::SimpleHeader;
    //
    // let client = SimpleHttpClient::new();
    // let response = client
    //     .get("http://httpbin.org/headers")
    //     .header(SimpleHeader::USER_AGENT, "foundation-http/1.0")
    //     .execute()
    //     .unwrap();
    //
    // assert_eq!(response.status(), 200);
    // let body = response.body_text().unwrap();
    // assert!(body.contains("foundation-http/1.0"));
}

// ========================================================================
// External Validation Setup Tests
// ========================================================================

/// WHY: Verify httpbin.org is accessible before running real tests
/// WHAT: Uses reqwest (existing dev-dep) to validate test server availability
#[test]
#[ignore] // Requires network
fn test_httpbin_availability() {
    // Use reqwest as reference to verify httpbin.org works
    let response = reqwest::blocking::get("http://httpbin.org/get");
    assert!(
        response.is_ok(),
        "httpbin.org unavailable - external validation tests will fail"
    );
    assert_eq!(response.unwrap().status(), 200);
}

/// WHY: Verify we can access HTTPS endpoints for TLS testing
/// WHAT: Validates HTTPS connectivity to httpbin.org
#[test]
#[ignore] // Requires network
fn test_httpbin_https_availability() {
    let response = reqwest::blocking::get("https://httpbin.org/get");
    assert!(
        response.is_ok(),
        "httpbin.org HTTPS unavailable - TLS validation tests will fail"
    );
    assert_eq!(response.unwrap().status(), 200);
}
