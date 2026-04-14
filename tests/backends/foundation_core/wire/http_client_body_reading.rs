//! Integration tests for HTTP client body reading functionality.
//!
//! WHY: Validates that the HTTP client can actually read response bodies
//! through the complete atomic coordination flow.
//!
//! WHAT: Tests the send() method with a real HTTP server to ensure bodies are readable.
//!
//! HOW: Uses foundation_testing::TestHttpServer to create real HTTP responses,
//! then validates the send() consumption pattern works end-to-end.

use foundation_core::valtron;
use foundation_core::valtron::PoolGuard;
use foundation_core::wire::simple_http::client::{ClientRequestBuilder, SystemDnsResolver};
use foundation_core::wire::simple_http::SendSafeBody;
use foundation_testing::http::{HttpResponse, TestHttpServer};
use serial_test::serial;
use tracing_test::traced_test;

// All valtron pool tests use the same global serial lock to prevent PoolGuard interference

/// WHY: Verify send() method returns complete response with body
/// WHAT: Tests one-shot send() convenience method
#[test]
#[traced_test]
#[serial(valtron_pool)]
fn test_send_returns_complete_response_with_body() {
    // Initialize valtron pool - keep guard alive for test duration
    let _pool_guard: PoolGuard = valtron::initialize_pool(0, None);

    // Create test server with larger body
    let test_body = "This is a complete response body from send()";
    let test_body_clone = test_body.to_string();

    let server =
        TestHttpServer::with_response(move |_req| HttpResponse::ok(test_body_clone.as_bytes()));

    // Build request
    let url = server.url("/send");
    let request = ClientRequestBuilder::<SystemDnsResolver>::get(&url)
        .unwrap()
        .build_client()
        .unwrap();

    // Use send() to get complete response
    let mut response = request.send().expect("Failed to send request");

    println!("Response status: {:?}", response.get_status());
    assert!(matches!(
        response.get_status(),
        foundation_core::wire::simple_http::Status::OK
    ));

    // Verify body
    match response.get_body_mut() {
        SendSafeBody::Text(text) => {
            println!("Response body: {text:}");
            assert_eq!(text, test_body);
        }
        SendSafeBody::Bytes(data) => {
            let body_str = String::from_utf8_lossy(data);
            println!("Response body (bytes): {body_str:}");
            assert_eq!(body_str, test_body);
        }
        SendSafeBody::None => {
            panic!("Expected body content, got None");
        }
        _ => {
            panic!("Unexpected body type");
        }
    }

    println!("✅ Test passed: send() returns complete response with body");
}
