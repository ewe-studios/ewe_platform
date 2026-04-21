//! Integration tests for EventSourceTask with real TCP connections.
//!
//! WHY: Tests that verify EventSourceTask behavior over real network connections
//! must use a local test server to avoid external dependencies.
//!
//! HOW: Uses TestHttpServer (or raw TcpListener) bound to localhost with
//! SSE-formatted responses.

use foundation_core::valtron::TaskIterator;
use foundation_core::valtron::TaskStatus;
use foundation_core::wire::event_source::{
    Event, EventSourceProgress, EventSourceTask, ParseResult,
};
use foundation_core::wire::simple_http::client::{MockDnsResolver, StaticSocketAddr};
use foundation_core::wire::simple_http::{SendSafeBody, SimpleMethod};
use foundation_testing::http::{HttpResponse, TestHttpServer};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

/// Parse the SocketAddr from a TestHttpServer's base_url (e.g. "http://127.0.0.1:PORT").
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

/// WHY: EventSourceTask should connect to a real server and receive SSE events.
/// WHAT: Verify full connect → read → event → close lifecycle.
#[test]
fn test_event_source_task_connects_to_server() {
    let server = TestHttpServer::with_response(|_req| sse_response(b"data: hello\n\n"));

    let addr = server_addr(&server);
    let resolver = StaticSocketAddr::new(addr);
    let url = server.url("/events");

    let mut task = EventSourceTask::connect(resolver, &url).unwrap();

    // First call: Init → Connecting, returns Pending(Connecting)
    let first = task.next_status();
    assert_eq!(
        first.as_ref().map(|s| match s {
            TaskStatus::Pending(p) => Some(*p),
            _ => None,
        }),
        Some(Some(EventSourceProgress::Connecting)),
        "First next() should return Pending(Connecting)"
    );

    // Second call: Connecting → Reading, returns Pending(Reading)
    let second = task.next_status();
    assert_eq!(
        second.as_ref().map(|s| match s {
            TaskStatus::Pending(p) => Some(*p),
            _ => None,
        }),
        Some(Some(EventSourceProgress::Reading)),
        "Second next() should return Pending(Reading) after successful connection"
    );

    // Third call: Reading → parse event → Ready(ParseResult)
    let third = task.next_status();
    match third {
        Some(TaskStatus::Ready(ParseResult {
            event: Event::Message { data, .. },
            ..
        })) => {
            assert_eq!(data, "hello", "Event data should be 'hello'");
        }
        other => panic!(
            "Expected Ready(ParseResult with Event::Message) with data 'hello', got {:?}",
            other.as_ref().map(|s| match s {
                TaskStatus::Ready(e) => format!("Ready({e:?})"),
                TaskStatus::Pending(p) => format!("Pending({p:?})"),
                _ => "Other".to_string(),
            })
        ),
    }

    // Fourth call: stream exhausted → None (Closed)
    let fourth = task.next_status();
    assert!(fourth.is_none(), "Expected None after stream exhausted");
}

/// WHY: EventSourceTask should resolve DNS and connect to a real server.
/// WHAT: Verify MockDnsResolver routes to the test server's address and events are received.
#[test]
fn test_event_source_task_dns_resolves_to_server() {
    let server = TestHttpServer::with_response(|_req| sse_response(b"data: resolved\n\n"));

    let addr = server_addr(&server);
    let resolver = MockDnsResolver::new().with_response("sse.test", vec![addr]);

    let mut task =
        EventSourceTask::connect(resolver, format!("http://sse.test:{}/events", addr.port()))
            .unwrap();

    // First call: Init → Connecting (pool handles DNS internally)
    let first = task.next_status();
    assert_eq!(
        first.as_ref().map(|s| match s {
            TaskStatus::Pending(p) => Some(*p),
            _ => None,
        }),
        Some(Some(EventSourceProgress::Connecting)),
        "First next() should return Pending(Connecting)"
    );

    // Second call: Connecting → Reading, returns Pending(Reading)
    let second = task.next_status();
    assert_eq!(
        second.as_ref().map(|s| match s {
            TaskStatus::Pending(p) => Some(*p),
            _ => None,
        }),
        Some(Some(EventSourceProgress::Reading)),
        "DNS should resolve to test server, returning Pending(Reading)"
    );

    // Third call: should yield the event
    let third = task.next_status();
    match third {
        Some(TaskStatus::Ready(ParseResult {
            event: Event::Message { data, .. },
            ..
        })) => {
            assert_eq!(data, "resolved");
        }
        other => {
            panic!("Expected Ready(ParseResult with Event::Message) with 'resolved', got {other:?}")
        }
    }
}

/// WHY: EventSourceTask should preserve query parameters when connecting.
/// WHAT: Verify URL query string is sent to the test server and events are received.
#[test]
fn test_event_source_task_url_with_query() {
    let server = TestHttpServer::with_response(|_req| sse_response(b"data: with-query\n\n"));

    let addr = server_addr(&server);
    let resolver = StaticSocketAddr::new(addr);
    let url = format!("{}{}", server.base_url(), "/events?filter=active&limit=10");

    let mut task = EventSourceTask::connect(resolver, &url).unwrap();

    // First call: Init → Connecting
    let first = task.next_status();
    assert_eq!(
        first.as_ref().map(|s| match s {
            TaskStatus::Pending(p) => Some(*p),
            _ => None,
        }),
        Some(Some(EventSourceProgress::Connecting)),
        "Should transition to Connecting first"
    );

    // Second call: Connecting → Reading
    let second = task.next_status();
    assert_eq!(
        second.as_ref().map(|s| match s {
            TaskStatus::Pending(p) => Some(*p),
            _ => None,
        }),
        Some(Some(EventSourceProgress::Reading)),
        "Should connect and return Pending(Reading)"
    );

    // Third call: should receive the event
    let third = task.next_status();
    match third {
        Some(TaskStatus::Ready(ParseResult {
            event: Event::Message { data, .. },
            ..
        })) => {
            assert_eq!(data, "with-query");
        }
        other => panic!(
            "Expected Ready(ParseResult with Event::Message) with 'with-query', got {other:?}"
        ),
    }
}

/// WHY: EventSourceTask should work with localhost URLs.
/// WHAT: Verify localhost URL connects, receives event, then closes.
#[test]
fn test_event_source_task_localhost_url() {
    let server = TestHttpServer::with_response(|_req| sse_response(b"data: localhost\n\n"));

    let addr = server_addr(&server);
    let resolver = StaticSocketAddr::new(addr);
    let url = server.url("/events");

    let mut task = EventSourceTask::connect(resolver, &url).unwrap();

    // First call: Init → Connecting
    let first = task.next_status();
    assert_eq!(
        first.as_ref().map(|s| match s {
            TaskStatus::Pending(p) => Some(*p),
            _ => None,
        }),
        Some(Some(EventSourceProgress::Connecting)),
        "Should transition to Connecting first"
    );

    // Second call: Connecting → Reading
    let second = task.next_status();
    assert_eq!(
        second.as_ref().map(|s| match s {
            TaskStatus::Pending(p) => Some(*p),
            _ => None,
        }),
        Some(Some(EventSourceProgress::Reading)),
        "Should connect to localhost test server"
    );

    // Third call: should receive the event
    let third = task.next_status();
    match third {
        Some(TaskStatus::Ready(ParseResult {
            event: Event::Message { data, .. },
            ..
        })) => {
            assert_eq!(data, "localhost");
        }
        other => panic!(
            "Expected Ready(ParseResult with Event::Message) with 'localhost', got {other:?}"
        ),
    }
}

/// WHY: EventSourceTask should handle connection refused gracefully.
/// WHAT: Verify task returns Pending(Connecting) then None when no server is listening.
#[test]
fn test_event_source_task_connection_refused() {
    // Bind and immediately drop to get a port that's guaranteed unused
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let resolver = StaticSocketAddr::new(addr);

    let mut task = EventSourceTask::connect(
        resolver,
        format!("http://{}:{}/events", addr.ip(), addr.port()),
    )
    .unwrap();

    // First call: Init → Connecting, returns Pending(Connecting)
    let first = task.next_status();
    assert_eq!(
        first.as_ref().map(|s| match s {
            TaskStatus::Pending(p) => Some(*p),
            _ => None,
        }),
        Some(Some(EventSourceProgress::Connecting)),
        "First next() should return Pending(Connecting)"
    );

    // Second call: Connection fails → Closed → None
    let second = task.next_status();
    assert!(
        second.is_none(),
        "Connection refused should return None (Closed)"
    );
}

/// WHY: EventSourceTask should fully exhaust after server closes connection.
/// WHAT: Verify task receives at least one event, then returns None.
#[test]
fn test_event_source_task_stream_exhaust() {
    let server = TestHttpServer::with_response(|_req| sse_response(b"data: done\n\n"));

    let addr = server_addr(&server);
    let resolver = StaticSocketAddr::new(addr);
    let url = server.url("/events");

    let mut task = EventSourceTask::connect(resolver, &url).unwrap();

    let mut got_pending_connecting = false;
    let mut got_pending_reading = false;
    let mut got_event = false;

    while let Some(status) = task.next_status() {
        match status {
            TaskStatus::Pending(EventSourceProgress::Connecting) => {
                got_pending_connecting = true;
            }
            TaskStatus::Pending(EventSourceProgress::Reading) => {
                got_pending_reading = true;
            }
            TaskStatus::Ready(ParseResult {
                event: Event::Message { ref data, .. },
                ..
            }) => {
                assert_eq!(data, "done", "Event data should be 'done'");
                got_event = true;
            }
            other => panic!("Unexpected status: {other:?}"),
        }
    }

    assert!(
        got_pending_connecting,
        "Should have seen Pending(Connecting) during lifecycle"
    );
    assert!(
        got_pending_reading,
        "Should have seen Pending(Reading) during lifecycle"
    );
    assert!(
        got_event,
        "Should have received at least one event before exhaustion"
    );

    // Task should be exhausted (Closed state is terminal)
    assert!(
        task.next_status().is_none(),
        "Exhausted task should keep returning None"
    );
}

/// WHY: `with_body` should switch method from GET to POST.
/// WHAT: Verify the server receives a POST request when a body is provided.
#[test]
fn test_event_source_task_with_body_uses_post() {
    let captured_method = Arc::new(Mutex::new(None));
    let captured_body = Arc::new(Mutex::new(None));
    let method_clone = captured_method.clone();
    let body_clone = captured_body.clone();

    let server = TestHttpServer::with_response(move |req| {
        *method_clone.lock().unwrap() = Some(req.method.clone());
        let text = match &req.body {
            SendSafeBody::Text(t) => Some(t.clone()),
            SendSafeBody::Bytes(b) => Some(String::from_utf8_lossy(b).to_string()),
            _ => None,
        };
        *body_clone.lock().unwrap() = text;
        sse_response(b"data: post-ok\n\n")
    });

    let addr = server_addr(&server);
    let resolver = StaticSocketAddr::new(addr);
    let url = server.url("/v1/chat/completions");

    let json_body =
        r#"{"model":"gpt-4","messages":[{"role":"user","content":"hello"}],"stream":true}"#;
    let mut task = EventSourceTask::connect(resolver, &url)
        .unwrap()
        .with_body(SendSafeBody::Text(json_body.to_string()));

    let mut got_event = false;
    while let Some(status) = task.next_status() {
        if let TaskStatus::Ready(ParseResult {
            event: Event::Message { ref data, .. },
            ..
        }) = status
        {
            assert_eq!(data, "post-ok");
            got_event = true;
            break;
        }
    }

    assert!(got_event, "Should have received the event");

    let method = captured_method.lock().unwrap();
    assert_eq!(
        *method,
        Some(SimpleMethod::POST),
        "with_body should switch method to POST"
    );

    let body = captured_body.lock().unwrap();
    assert_eq!(
        body.as_deref(),
        Some(json_body),
        "Server should receive the JSON body"
    );
}

/// WHY: Without `with_body`, the default method should be GET.
/// WHAT: Verify the server receives a GET request when no body is set.
#[test]
fn test_event_source_task_default_method_is_get() {
    let captured_method = Arc::new(Mutex::new(None));
    let method_clone = captured_method.clone();

    let server = TestHttpServer::with_response(move |req| {
        *method_clone.lock().unwrap() = Some(req.method.clone());
        sse_response(b"data: get-ok\n\n")
    });

    let addr = server_addr(&server);
    let resolver = StaticSocketAddr::new(addr);
    let url = server.url("/events");

    let mut task = EventSourceTask::connect(resolver, &url).unwrap();

    let mut got_event = false;
    while let Some(status) = task.next_status() {
        if let TaskStatus::Ready(ParseResult {
            event: Event::Message { ref data, .. },
            ..
        }) = status
        {
            assert_eq!(data, "get-ok");
            got_event = true;
            break;
        }
    }

    assert!(got_event, "Should have received the event");

    let method = captured_method.lock().unwrap();
    assert_eq!(
        *method,
        Some(SimpleMethod::GET),
        "Default method should be GET"
    );
}
