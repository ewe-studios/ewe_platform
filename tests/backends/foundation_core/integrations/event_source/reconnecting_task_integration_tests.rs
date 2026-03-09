//! Integration tests for ReconnectingEventSourceTask with real TCP connections.
//!
//! WHY: Reconnection logic must be verified against real SSE streams
//! to confirm Last-Event-ID tracking, backoff behavior, and event delivery
//! across reconnection cycles.
//!
//! HOW: Uses TestHttpServer with SSE-formatted responses.

use foundation_core::valtron::TaskIterator;
use foundation_core::valtron::TaskStatus;
use foundation_core::wire::event_source::{
    Event, ParseResult, ReconnectingEventSourceTask,
};
use foundation_core::wire::simple_http::client::StaticSocketAddr;
use foundation_testing::http::{HttpResponse, TestHttpServer};
use std::net::SocketAddr;

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

/// WHY: ReconnectingEventSourceTask should receive events from a real server.
/// WHAT: Verify events are delivered through the reconnecting wrapper.
#[test]
fn test_reconnecting_task_receives_events() {
    let server = TestHttpServer::with_response(|_req| sse_response(b"data: hello\n\n"));

    let addr = server_addr(&server);
    let resolver = StaticSocketAddr::new(addr);
    let url = server.url("/events");

    let mut task = ReconnectingEventSourceTask::connect(resolver, &url)
        .unwrap()
        .with_max_retries(1);

    let mut got_event = false;
    let mut steps = 0;

    while let Some(status) = task.next() {
        match status {
            TaskStatus::Ready(ParseResult { event: Event::Message { ref data, .. }, .. }) => {
                assert_eq!(data, "hello");
                got_event = true;
                break;
            }
            TaskStatus::Pending(_) | TaskStatus::Delayed(_) => {}
            other => panic!("Unexpected status: {other:?}"),
        }
        steps += 1;
        assert!(steps < 50, "Did not receive event within 50 steps");
    }

    assert!(got_event, "Should have received the event");
}

/// WHY: ReconnectingEventSourceTask should track Last-Event-ID from events.
/// WHAT: Verify events with id field update internal tracking.
#[test]
fn test_reconnecting_task_tracks_event_id() {
    let server = TestHttpServer::with_response(|_req| {
        sse_response(b"id: 42\ndata: tracked\n\nid: 43\ndata: also tracked\n\n")
    });

    let addr = server_addr(&server);
    let resolver = StaticSocketAddr::new(addr);
    let url = server.url("/events");

    let mut task = ReconnectingEventSourceTask::connect(resolver, &url)
        .unwrap()
        .with_max_retries(0);

    let mut event_ids: Vec<String> = Vec::new();
    let mut steps = 0;

    while let Some(status) = task.next() {
        if let TaskStatus::Ready(ParseResult { event: Event::Message { id: Some(id), .. }, .. }) = status {
            event_ids.push(id);
        }
        steps += 1;
        if steps > 50 {
            break;
        }
    }

    assert_eq!(event_ids, vec!["42", "43"]);
}

/// WHY: ReconnectingEventSourceTask should handle connection refused with reconnection.
/// WHAT: Verify task attempts reconnection and eventually exhausts.
#[test]
fn test_reconnecting_task_connection_refused_retries() {
    // Bind and immediately drop to get a port that's guaranteed unused
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let resolver = StaticSocketAddr::new(addr);
    let url = format!("http://{}:{}/events", addr.ip(), addr.port());

    let mut task = ReconnectingEventSourceTask::connect(resolver, &url)
        .unwrap()
        .with_max_retries(2);

    let mut delayed_count = 0;
    let mut steps = 0;

    while let Some(status) = task.next() {
        if let TaskStatus::Delayed(_) = status {
            delayed_count += 1;
        }
        steps += 1;
        assert!(steps < 100, "Did not exhaust within 100 steps");
    }

    assert!(
        delayed_count >= 1,
        "Should have seen backoff delays before exhaustion, got {delayed_count}"
    );
}

/// WHY: ReconnectingEventSourceTask should deliver multiple events.
/// WHAT: Verify all events in a multi-event stream are received.
#[test]
fn test_reconnecting_task_multiple_events() {
    let server = TestHttpServer::with_response(|_req| {
        sse_response(b"data: first\n\ndata: second\n\ndata: third\n\n")
    });

    let addr = server_addr(&server);
    let resolver = StaticSocketAddr::new(addr);
    let url = server.url("/events");

    let mut task = ReconnectingEventSourceTask::connect(resolver, &url)
        .unwrap()
        .with_max_retries(0);

    let mut data_values: Vec<String> = Vec::new();
    let mut steps = 0;

    while let Some(status) = task.next() {
        if let TaskStatus::Ready(ParseResult { event: Event::Message { data, .. }, .. }) = status {
            data_values.push(data);
        }
        steps += 1;
        if steps > 50 {
            break;
        }
    }

    assert_eq!(data_values, vec!["first", "second", "third"]);
}

/// WHY: ReconnectingEventSourceTask should pass through comments.
/// WHAT: Verify Comment events are delivered through the reconnecting wrapper.
#[test]
fn test_reconnecting_task_passes_comments() {
    let server = TestHttpServer::with_response(|_req| {
        sse_response(b": keep-alive\ndata: after comment\n\n")
    });

    let addr = server_addr(&server);
    let resolver = StaticSocketAddr::new(addr);
    let url = server.url("/events");

    let mut task = ReconnectingEventSourceTask::connect(resolver, &url)
        .unwrap()
        .with_max_retries(0);

    let mut saw_comment = false;
    let mut saw_message = false;
    let mut steps = 0;

    while let Some(status) = task.next() {
        match status {
            TaskStatus::Ready(ParseResult { event: Event::Comment(c), .. }) => {
                assert_eq!(c, "keep-alive");
                saw_comment = true;
            }
            TaskStatus::Ready(ParseResult { event: Event::Message { ref data, .. }, .. }) => {
                assert_eq!(data, "after comment");
                saw_message = true;
            }
            _ => {}
        }
        steps += 1;
        if steps > 50 {
            break;
        }
    }

    assert!(saw_comment, "Should have received comment");
    assert!(saw_message, "Should have received message after comment");
}
