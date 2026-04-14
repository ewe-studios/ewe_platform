//! Integration tests for SseStream and ReconnectingSseStream consumer APIs.
//!
//! WHY: Consumer wrappers should be tested with real TCP connections to verify
//! they correctly translate TaskIterator states into SseStreamEvent variants.
//!
//! HOW: Uses TestHttpServer bound to localhost with SSE-formatted responses.

use foundation_core::valtron::PoolGuard;
use foundation_core::wire::event_source::{Event, SseStream, SseStreamEvent};
use foundation_core::wire::simple_http::client::StaticSocketAddr;
use foundation_testing::http::{HttpResponse, TestHttpServer};
use std::net::SocketAddr;
use tracing_test::traced_test;

/// Parse the SocketAddr from a TestHttpServer's base_url.
fn server_addr(server: &TestHttpServer) -> SocketAddr {
    server
        .base_url()
        .strip_prefix("http://")
        .unwrap()
        .parse()
        .unwrap()
}

/// Create an SSE-formatted HTTP response body.
fn sse_response(body: &[u8]) -> HttpResponse {
    HttpResponse {
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "text/event-stream".to_string()),
            ("Cache-Control".to_string(), "no-cache".to_string()),
            ("Content-Length".to_string(), body.len().to_string()),
        ],
        body: body.to_vec(),
    }
}

/// WHY: SseStream should connect to a real server and receive SSE events.
/// WHAT: Verify SseStream wraps EventSourceTask correctly and yields Event variant.
#[test]
#[traced_test]
fn test_sse_stream_connects_and_receives_event() {
    let _pool_guard: PoolGuard = foundation_core::valtron::initialize_pool(42, None);

    let server = TestHttpServer::with_response(|_req| sse_response(b"data: hello\n\n"));

    let addr = server_addr(&server);
    let resolver = StaticSocketAddr::new(addr);
    let url = server.url("/events");

    tracing::info!("Connecting to SSE stream at {}", url);
    let stream = SseStream::connect(resolver, &url).unwrap();

    tracing::info!("Iterating over stream");
    let mut event_count = 0;
    let mut skip_count = 0;
    for (i, result) in stream.enumerate() {
        tracing::info!("Iteration {}: result = {:?}", i, result);
        match result.expect("Expected Ok result") {
            SseStreamEvent::Event(Event::Message { ref data, .. }) => {
                tracing::info!("Received event with data: {}", data);
                event_count += 1;
                assert_eq!(data, "hello");
            }
            SseStreamEvent::Skip => {
                skip_count += 1;
                tracing::info!("Skip {} - stream still working", skip_count);
            }
            SseStreamEvent::Event(e) => {
                tracing::info!("Received other event: {:?}", e);
            }
        }
        // Limit iterations to prevent hanging
        if i > 20 {
            tracing::warn!("Breaking after 20 iterations");
            break;
        }
    }

    tracing::info!(
        "Stream exhausted: {} events, {} skips",
        event_count,
        skip_count
    );
    assert!(event_count >= 1, "Should receive at least one event");
}

/// WHY: SseStream should handle multiple SSE events from server.
/// WHAT: Verify SseStream yields all events before exhaustion.
#[test]
#[traced_test]
fn test_sse_stream_multiple_events() {
    let _pool_guard: PoolGuard = foundation_core::valtron::initialize_pool(42, None);

    let server = TestHttpServer::with_response(|_req| {
        sse_response(b"data: first\n\ndata: second\n\ndata: third\n\n")
    });

    let addr = server_addr(&server);
    let resolver = StaticSocketAddr::new(addr);
    let url = server.url("/events");

    tracing::info!("Connecting to SSE stream at {}", url);
    let stream = SseStream::connect(resolver, &url).unwrap();

    let mut event_count = 0;
    let mut received_events = Vec::new();
    for (i, result) in stream.enumerate() {
        tracing::info!("Iteration {}: {:?}", i, result);
        match result.expect("Expected Ok result") {
            SseStreamEvent::Event(Event::Message { ref data, .. }) => {
                tracing::info!("Event: {}", data);
                received_events.push(data.clone());
                event_count += 1;
            }
            SseStreamEvent::Skip => {}
            SseStreamEvent::Event(_) => {}
        }
        if i > 30 {
            break;
        }
    }

    tracing::info!("Received {} events: {:?}", event_count, received_events);
    assert_eq!(event_count, 3, "Should receive exactly 3 events");
}

/// WHY: SseStream should preserve event data correctly.
/// WHAT: Verify event data matches what server sent.
#[test]
#[traced_test]
fn test_sse_stream_preserves_event_data() {
    let _pool_guard: PoolGuard = foundation_core::valtron::initialize_pool(42, None);

    let expected_data = "test-payload-123";
    let server = TestHttpServer::with_response(move |_req| {
        sse_response(format!("data: {}\n\n", expected_data).as_bytes())
    });

    let addr = server_addr(&server);
    let resolver = StaticSocketAddr::new(addr);
    let url = server.url("/events");

    tracing::info!("Connecting to {}", url);
    let stream = SseStream::connect(resolver, &url).unwrap();

    let mut received_data: Option<String> = None;
    for (i, result) in stream.enumerate() {
        tracing::info!("Iteration {}: {:?}", i, result);
        if let Ok(SseStreamEvent::Event(Event::Message { data, .. })) = result {
            tracing::info!("Got data: {}", data);
            received_data = Some(data);
            break;
        }
        if i > 20 {
            break;
        }
    }

    assert_eq!(
        received_data,
        Some(expected_data.to_string()),
        "Event data should match"
    );
}
