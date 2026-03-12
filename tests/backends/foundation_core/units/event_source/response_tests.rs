//! Unit tests for event_source response module.
//!
//! Tests SseResponse builder for SSE HTTP responses.

use foundation_core::wire::event_source::SseResponse;
use foundation_core::wire::simple_http::{SimpleHeader, Status};

/// WHY: SseResponse must have correct default headers for SSE.
/// WHAT: Verify Content-Type, Cache-Control, and Connection headers are set.
#[test]
fn test_sse_response_default_headers() {
    let sse = SseResponse::new();
    assert_eq!(sse.headers().len(), 3);
    assert!(sse.headers().contains_key(&SimpleHeader::CONTENT_TYPE));
    assert!(sse.headers().contains_key(&SimpleHeader::CACHE_CONTROL));
    assert!(sse.headers().contains_key(&SimpleHeader::CONNECTION));
}

/// WHY: SseResponse must support custom headers.
/// WHAT: Verify with_header() adds headers correctly.
#[test]
fn test_sse_response_custom_header() {
    let sse = SseResponse::new().with_header(SimpleHeader::custom("X-Custom"), "value");
    assert_eq!(sse.headers().len(), 4);
}

/// WHY: SseResponse build() must produce correct HTTP response.
/// WHAT: Verify built response has all required headers.
#[test]
fn test_sse_response_build_headers() {
    let response = SseResponse::new().build();
    let headers = &response.headers;
    assert!(headers.contains_key(&SimpleHeader::CONTENT_TYPE));
    assert!(headers.contains_key(&SimpleHeader::CACHE_CONTROL));
    assert!(headers.contains_key(&SimpleHeader::CONNECTION));
    let content_type = headers.get(&SimpleHeader::CONTENT_TYPE).unwrap();
    assert!(content_type.contains(&"text/event-stream".to_string()));
}

/// WHY: SseResponse build() must include custom headers.
/// WHAT: Verify custom headers are present in built response.
#[test]
fn test_sse_response_build_custom_header() {
    let response = SseResponse::new()
        .with_header(SimpleHeader::custom("X-Custom"), "value")
        .build();
    let headers = &response.headers;
    assert!(headers.contains_key(&SimpleHeader::custom("X-Custom")));
    let custom = headers.get(&SimpleHeader::custom("X-Custom")).unwrap();
    assert!(custom.contains(&"value".to_string()));
}

/// WHY: SseResponse must support custom status codes.
/// WHAT: Verify with_status() sets the response status.
#[test]
fn test_sse_response_build_status() {
    let response = SseResponse::new().with_status(Status::Created).build();
    assert_eq!(response.status, Status::Created);
}
