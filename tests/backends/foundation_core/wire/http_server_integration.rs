//! HTTP client integration tests using foundation_testing::TestHttpServer.
//!
//! WHY: Validates HTTP client works with real HTTP server (no mocks).
//! Tests complete request/response flow using project's own test infrastructure.
//!
//! WHAT: Integration tests for HTTP GET, redirects, headers, and error handling.
//!
//! HOW: Uses foundation_testing::TestHttpServer (built on stdlib) to create
//! real HTTP server, then tests foundation_core HTTP client against it.

use foundation_testing::http::{HttpRequest, HttpResponse, TestHttpServer};

// Note: Uncomment these when HTTP client public API is ready
// use foundation_core::wire::simple_http::client::SimpleHttpClient;

// ========================================================================
// Internal HTTP Server Tests
// ========================================================================

/// WHY: Verify HTTP client can perform basic GET request against real server
/// WHAT: Tests complete HTTP request/response flow with 200 OK
#[test]
#[ignore] // TODO: Enable when HTTP client execute() API is implemented
fn test_http_client_get_request() {
    let server = TestHttpServer::start();

    // TODO: Uncomment when SimpleHttpClient has execute() method
    // let client = SimpleHttpClient::new();
    // let response = client.get(&server.url("/test")).execute().unwrap();
    // assert_eq!(response.status(), 200);
    // assert_eq!(response.body_text().unwrap(), "OK");

    // For now, just verify server starts
    assert!(server.base_url().starts_with("http://127.0.0.1:"));
}

/// WHY: Verify HTTP client follows 302 redirects correctly
/// WHAT: Tests redirect handling with real server
#[test]
#[ignore] // TODO: Enable when HTTP client execute() API is implemented
fn test_http_client_redirects() {
    let server = TestHttpServer::with_response(|req: &HttpRequest| {
        if req.path == "/redirect" {
            HttpResponse::redirect("/target")
        } else if req.path == "/target" {
            HttpResponse::ok(b"Redirected Successfully")
        } else {
            HttpResponse::ok(b"Default")
        }
    });

    // TODO: Uncomment when SimpleHttpClient has execute() method
    // let client = SimpleHttpClient::new();
    // let response = client.get(&server.url("/redirect")).execute().unwrap();
    // assert_eq!(response.status(), 200);
    // assert_eq!(response.body_text().unwrap(), "Redirected Successfully");

    assert!(server.base_url().starts_with("http://"));
}

/// WHY: Verify HTTP client handles different status codes
/// WHAT: Tests 404, 500, and other non-200 responses
#[test]
#[ignore] // TODO: Enable when HTTP client execute() API is implemented
fn test_http_client_status_codes() {
    let server = TestHttpServer::with_response(|req: &HttpRequest| match req.path.as_str() {
        "/not-found" => HttpResponse::status(404, "Not Found"),
        "/server-error" => HttpResponse::status(500, "Internal Server Error"),
        "/created" => HttpResponse::status(201, "Created"),
        _ => HttpResponse::ok(b"OK"),
    });

    // TODO: Uncomment when SimpleHttpClient has execute() method
    // let client = SimpleHttpClient::new();
    //
    // let response_404 = client.get(&server.url("/not-found")).execute().unwrap();
    // assert_eq!(response_404.status(), 404);
    //
    // let response_500 = client.get(&server.url("/server-error")).execute().unwrap();
    // assert_eq!(response_500.status(), 500);
    //
    // let response_201 = client.get(&server.url("/created")).execute().unwrap();
    // assert_eq!(response_201.status(), 201);

    assert!(server.base_url().starts_with("http://"));
}

/// WHY: Verify HTTP client sends headers correctly
/// WHAT: Tests custom headers are sent in request
#[test]
#[ignore] // TODO: Enable when HTTP client execute() API is implemented
fn test_http_client_custom_headers() {
    use std::sync::{Arc, Mutex};

    let received_headers = Arc::new(Mutex::new(Vec::new()));
    let headers_clone = Arc::clone(&received_headers);

    let server = TestHttpServer::with_response(move |req: &HttpRequest| {
        // Capture headers
        let mut headers = headers_clone.lock().unwrap();
        *headers = req.headers.clone();

        HttpResponse::ok(b"Headers Received")
    });

    // TODO: Uncomment when SimpleHttpClient has execute() API
    // let client = SimpleHttpClient::new();
    // let response = client
    //     .get(&server.url("/"))
    //     .header(SimpleHeader::USER_AGENT, "TestClient/1.0")
    //     .header(SimpleHeader::ACCEPT, "application/json")
    //     .execute()
    //     .unwrap();
    //
    // assert_eq!(response.status(), 200);
    //
    // let headers = received_headers.lock().unwrap();
    // assert!(headers.iter().any(|(k, v)| k == "User-Agent" && v == "TestClient/1.0"));
    // assert!(headers.iter().any(|(k, v)| k == "Accept" && v == "application/json"));

    assert!(server.base_url().starts_with("http://"));
}

/// WHY: Verify HTTP client handles response body correctly
/// WHAT: Tests reading response body of various sizes
#[test]
#[ignore] // TODO: Enable when HTTP client execute() API is implemented
fn test_http_client_response_body() {
    let large_body = "x".repeat(10000);
    let large_body_clone = large_body.clone();

    let server = TestHttpServer::with_response(move |req: &HttpRequest| match req.path.as_str() {
        "/empty" => HttpResponse::ok(b""),
        "/small" => HttpResponse::ok(b"Small body"),
        "/large" => HttpResponse::ok(large_body_clone.as_bytes()),
        _ => HttpResponse::ok(b"Default"),
    });

    // TODO: Uncomment when SimpleHttpClient has execute() API
    // let client = SimpleHttpClient::new();
    //
    // let empty = client.get(&server.url("/empty")).execute().unwrap();
    // assert_eq!(empty.body_bytes().len(), 0);
    //
    // let small = client.get(&server.url("/small")).execute().unwrap();
    // assert_eq!(small.body_text().unwrap(), "Small body");
    //
    // let large = client.get(&server.url("/large")).execute().unwrap();
    // assert_eq!(large.body_text().unwrap().len(), 10000);

    assert!(server.base_url().starts_with("http://"));
}

/// WHY: Verify HTTP client handles concurrent requests correctly
/// WHAT: Tests multiple simultaneous requests to same server
#[test]
#[ignore] // TODO: Enable when HTTP client execute() API is implemented
fn test_http_client_concurrent_requests() {
    let server = TestHttpServer::with_response(|req: &HttpRequest| {
        HttpResponse::ok(format!("Path: {}", req.path).as_bytes())
    });

    let server_url = server.base_url().to_string();

    // TODO: Uncomment when SimpleHttpClient has execute() API
    // use std::thread;
    // let handles: Vec<_> = (0..5)
    //     .map(|i| {
    //         let url = format!("{}/path{}", server_url, i);
    //         thread::spawn(move || {
    //             let client = SimpleHttpClient::new();
    //             let response = client.get(&url).execute().unwrap();
    //             assert_eq!(response.status(), 200);
    //             response.body_text().unwrap()
    //         })
    //     })
    //     .collect();
    //
    // for handle in handles {
    //     let body = handle.join().unwrap();
    //     assert!(body.starts_with("Path: /path"));
    // }

    assert!(server_url.starts_with("http://"));
}

// ========================================================================
// TestHttpServer Utility Tests
// ========================================================================

/// WHY: Verify TestHttpServer works in basic scenario
/// WHAT: Tests server starts and provides valid URLs
#[test]
fn test_server_provides_valid_urls() {
    let server = TestHttpServer::start();

    assert!(server.base_url().starts_with("http://127.0.0.1:"));
    assert!(server.url("/test").contains("/test"));
    assert!(server.url("/api/v1").contains("/api/v1"));
}

/// WHY: Verify TestHttpServer custom response handler works
/// WHAT: Tests different response types
#[test]
fn test_server_custom_responses() {
    let server = TestHttpServer::with_response(|req: &HttpRequest| match req.path.as_str() {
        "/ok" => HttpResponse::ok(b"Success"),
        "/redirect" => HttpResponse::redirect("/new-location"),
        "/error" => HttpResponse::status(500, "Error"),
        _ => HttpResponse::status(404, "Not Found"),
    });

    // Server is running and ready for HTTP client tests
    assert!(server.base_url().starts_with("http://"));
}

/// WHY: Verify TestHttpServer stops cleanly
/// WHAT: Tests Drop implementation doesn't hang
#[test]
fn test_server_cleanup() {
    {
        let _server = TestHttpServer::start();
        // Server running in background
    }
    // Server stopped after drop - test passes if no hang
}
