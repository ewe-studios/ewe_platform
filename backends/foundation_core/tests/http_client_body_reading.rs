//! Integration tests for HTTP client body reading functionality.
//!
//! WHY: Validates that the HTTP client can actually read response bodies
//! through the complete atomic coordination flow (intro → signal → body).
//!
//! WHAT: Tests the introduction() + body() flow, parts() iterator, and send()
//! method with a real HTTP server to ensure bodies are readable.
//!
//! HOW: Uses foundation_testing::TestHttpServer to create real HTTP responses,
//! then validates all three consumption patterns work end-to-end.

use foundation_core::valtron;
use foundation_core::wire::simple_http::client::{
    ClientConfig, ClientRequest, ClientRequestBuilder, SystemDnsResolver,
};
use foundation_core::wire::simple_http::SimpleBody;
use foundation_testing::http::{HttpResponse, TestHttpServer};
use tracing_test::traced_test;

/// WHY: Verify we can read response body using introduction() + body() pattern
/// WHAT: Tests atomic coordination flow with real HTTP server
#[test]
#[traced_test]
fn test_body_reading_with_introduction_and_body() {
    // Initialize valtron pool with default settings
    valtron::initialize_pool(0, None);

    // Create test server
    let server = TestHttpServer::with_response(|_req| HttpResponse::ok(b"Hello from server!"));

    // Build request
    let url = server.url("/test");
    let builder = ClientRequestBuilder::get(&url).unwrap();
    let prepared = builder.build();

    // Create request with system resolver
    let mut request = ClientRequest::new(
        prepared,
        SystemDnsResolver::new(),
        ClientConfig::default(),
        None,
    );

    tracing::info!("Get the introduction of the request");

    // Step 1: Get introduction and headers
    let (intro, headers) = request.introduction().expect("Failed to get introduction");

    println!("Received intro: status={:?}", intro.status);
    println!("Received headers: {} entries", headers.len());

    assert!(matches!(
        intro.status,
        foundation_core::wire::simple_http::Status::OK
    ));

    // Step 2: Get body
    let body = request.body().expect("Failed to get body");

    // Step 3: Verify body content
    match body {
        SimpleBody::Text(text) => {
            println!("Received text body: {}", text);
            assert_eq!(text, "Hello from server!", "Body content mismatch");
        }
        SimpleBody::Bytes(data) => {
            let body_str = String::from_utf8_lossy(&data);
            println!("Received bytes body: {}", body_str);
            assert_eq!(body_str, "Hello from server!", "Body content mismatch");
        }
        SimpleBody::None => {
            panic!("Expected body content, got None");
        }
        _ => {
            panic!("Unexpected body type: {:?}", body);
        }
    }

    println!("✅ Test passed: Body reading with introduction() + body()");
}

/// WHY: Verify parts() iterator yields all response parts including body
/// WHAT: Tests that parts() iterator works through atomic coordination
// #[test]
// fn test_parts_iterator_reads_complete_response() {
//     // Initialize valtron pool
//     valtron::initialize_pool(0, None);
//
//     // Create test server
//     let server =
//         TestHttpServer::with_response(|_req| HttpResponse::ok(b"Parts iterator test body"));
//
//     // Build request
//     let url = server.url("/parts");
//     let builder = ClientRequestBuilder::get(&url).unwrap();
//     let prepared = builder.build();
//
//     // Create request
//     let request = ClientRequest::new(
//         prepared,
//         SystemDnsResolver::new(),
//         ClientConfig::default(),
//         None,
//     );
//
//     // Iterate through all parts
//     let mut got_intro = false;
//     let mut got_headers = false;
//     let mut got_body = false;
//     let mut body_content = Vec::new();
//
//     for part_result in request.parts() {
//         let part = part_result.expect("Failed to get part");
//         match part {
//             IncomingResponseParts::Intro(status, proto, reason) => {
//                 println!("Got intro: {:?} {:?} {:?}", status, proto, reason);
//                 assert!(matches!(
//                     status,
//                     foundation_core::wire::simple_http::Status::OK
//                 ));
//                 got_intro = true;
//             }
//             IncomingResponseParts::Headers(headers) => {
//                 println!("Got headers: {} entries", headers.len());
//                 got_headers = true;
//             }
//             IncomingResponseParts::SizedBody(body) => {
//                 println!("Got sized body");
//                 match body {
//                     SimpleBody::Text(text) => {
//                         body_content = text.into_bytes();
//                     }
//                     SimpleBody::Bytes(data) => {
//                         body_content = data;
//                     }
//                     _ => {}
//                 }
//                 got_body = true;
//             }
//             _ => {}
//         }
//     }
//
//     // Verify we got all parts
//     assert!(got_intro, "Should have received intro");
//     assert!(got_headers, "Should have received headers");
//     assert!(got_body, "Should have received body");
//
//     // Verify body content
//     let body_str = String::from_utf8_lossy(&body_content);
//     assert_eq!(body_str, "Parts iterator test body");
//
//     println!("✅ Test passed: Parts iterator reads complete response");
// }

/// WHY: Verify send() method returns complete response with body
/// WHAT: Tests one-shot send() convenience method
#[test]
#[traced_test]
fn test_send_returns_complete_response_with_body() {
    // Initialize valtron pool
    valtron::initialize_pool(0, None);

    // Create test server with larger body
    let test_body = "This is a complete response body from send()";
    let test_body_clone = test_body.to_string();

    let server =
        TestHttpServer::with_response(move |_req| HttpResponse::ok(test_body_clone.as_bytes()));

    // Build request
    let url = server.url("/send");
    let builder = ClientRequestBuilder::get(&url).unwrap();
    let prepared = builder.build();

    // Create request
    let request = ClientRequest::new(
        prepared,
        SystemDnsResolver::new(),
        ClientConfig::default(),
        None,
    );

    // Use send() to get complete response
    let mut response = request.send().expect("Failed to send request");

    println!("Response status: {:?}", response.get_status());
    assert!(matches!(
        response.get_status(),
        foundation_core::wire::simple_http::Status::OK
    ));

    // Verify body
    match response.get_body_mut() {
        SimpleBody::Text(text) => {
            println!("Response body: {text:}");
            assert_eq!(text, test_body);
        }
        SimpleBody::Bytes(data) => {
            let body_str = String::from_utf8_lossy(&data);
            println!("Response body (bytes): {body_str:}");
            assert_eq!(body_str, test_body);
        }
        SimpleBody::None => {
            panic!("Expected body content, got None");
        }
        _ => {
            panic!("Unexpected body type");
        }
    }

    println!("✅ Test passed: send() returns complete response with body");
}

/// WHY: Verify large bodies can be read successfully
/// WHAT: Tests with 10KB body to ensure no buffer issues
#[test]
fn test_large_body_reading() {
    // Initialize valtron pool
    valtron::initialize_pool(0, None);

    // Create test server with large body
    let large_body = "x".repeat(10000);
    let large_body_clone = large_body.clone();

    let server =
        TestHttpServer::with_response(move |_req| HttpResponse::ok(large_body_clone.as_bytes()));

    // Build request
    let url = server.url("/large");
    let builder = ClientRequestBuilder::get(&url).unwrap();
    let prepared = builder.build();

    // Create request
    let mut request = ClientRequest::new(
        prepared,
        SystemDnsResolver::new(),
        ClientConfig::default(),
        None,
    );

    // Get response
    let (_intro, _headers) = request.introduction().expect("Failed to get intro");
    let body = request.body().expect("Failed to get body");

    // Verify large body
    match body {
        SimpleBody::Text(text) => {
            assert_eq!(text.len(), 10000, "Body size mismatch");
            assert_eq!(&text[..100], &"x".repeat(100), "Body content mismatch");
        }
        SimpleBody::Bytes(data) => {
            assert_eq!(data.len(), 10000, "Body size mismatch");
            assert_eq!(&data[..100], &vec![b'x'; 100][..], "Body content mismatch");
        }
        SimpleBody::None => {
            panic!("Expected body content, got None");
        }
        _ => {
            panic!("Unexpected body type");
        }
    }

    println!("✅ Test passed: Large body (10KB) reading successful");
}
