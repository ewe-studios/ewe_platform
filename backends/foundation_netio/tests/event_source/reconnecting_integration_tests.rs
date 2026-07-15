//! Integration tests for `ReconnectingEventSourceTask` with real HTTP servers.
//!
//! Tests are split between:
//! - `TestHttpServer` (request-response) — verifies initial connection sends body/headers
//! - `SseTestServer` (streaming) — verifies reconnection behavior with controlled close

use foundation_core::valtron::{TaskIterator, TaskStatus};
use foundation_netio::event_source::native::ReconnectingEventSourceTask;
use foundation_netio::shared::client::StaticSocketAddr;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader, SimpleMethod};
use foundation_testing::http::{
    HttpResponse, SseConnectionResult, SseStreamWriter, SseTestServer, TestHttpServer,
};
use serial_test::serial;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tracing_test::traced_test;

// -- TestHttpServer-based tests (initial connection verification)

fn server_addr(server: &TestHttpServer) -> SocketAddr {
    server
        .base_url()
        .strip_prefix("http://")
        .unwrap()
        .parse()
        .unwrap()
}

/// WHY: Verify that `ReconnectingEventSourceTask` sends POST with body and
/// headers on the **initial** connection, not just reconnections.
///
/// Regression test for: `connect()` eagerly creating the inner `EventSourceTask`
/// with GET/no body, so `.with_body()` only applied to reconnections.
#[test]
#[serial(valtron_pool)]
#[traced_test]
fn test_reconnecting_task_initial_connection_sends_post_with_body() {
    #[derive(Clone, Default)]
    struct CapturedRequest {
        method: Arc<std::sync::Mutex<String>>,
        body: Arc<std::sync::Mutex<String>>,
        auth_header: Arc<std::sync::Mutex<String>>,
    }

    let _pool_guard = foundation_core::valtron::initialize_pool(42, None);
    let captured = CapturedRequest::default();
    let captured_clone = captured.clone();

    let server =
        TestHttpServer::with_response(move |req: &foundation_testing::http::HttpRequest| {
            {
                let mut m = captured.method.lock().unwrap();
                *m = match &req.method {
                    SimpleMethod::GET => "GET".to_string(),
                    SimpleMethod::POST => "POST".to_string(),
                    SimpleMethod::PUT => "PUT".to_string(),
                    SimpleMethod::DELETE => "DELETE".to_string(),
                    SimpleMethod::PATCH => "PATCH".to_string(),
                    SimpleMethod::HEAD => "HEAD".to_string(),
                    SimpleMethod::OPTIONS => "OPTIONS".to_string(),
                    SimpleMethod::CONNECT => "CONNECT".to_string(),
                    SimpleMethod::TRACE => "TRACE".to_string(),
                    SimpleMethod::Custom(s) => s.clone(),
                };
            }
            {
                let mut b = captured.body.lock().unwrap();
                let bytes = match &req.body {
                    foundation_netio::shared::http::SendSafeBody::Bytes(b) => b.clone(),
                    _ => Vec::new(),
                };
                *b = String::from_utf8_lossy(&bytes).to_string();
            }
            {
                let mut a = captured.auth_header.lock().unwrap();
                if let Some(values) = req.headers.get(&SimpleHeader::AUTHORIZATION) {
                    if let Some(v) = values.first() {
                        *a = v.clone();
                    }
                }
            }

            HttpResponse {
                status: 200,
                status_text: "OK".to_string(),
                headers: vec![
                    ("Content-Type".to_string(), "text/event-stream".to_string()),
                    ("Connection".to_string(), "close".to_string()),
                ],
                body: b"data: {\"test\": true}\n\n".to_vec(),
            }
        });

    let addr = server_addr(&server);
    let resolver = StaticSocketAddr::new(addr);
    let base_url = server.base_url().to_string();

    let mut task = ReconnectingEventSourceTask::connect(resolver, base_url)
        .unwrap()
        .with_body(SendSafeBody::Text(r#"{"model":"qwen2.5"}"#.into()))
        .with_header(SimpleHeader::AUTHORIZATION, "Bearer test-key-123")
        .with_max_retries(0);

    let mut events = Vec::new();
    while let Some(status) = task.next_status() {
        if let TaskStatus::Ready(parse_result) = status {
            events.push(parse_result);
        }
    }

    assert!(
        !events.is_empty(),
        "Should have received at least one SSE event"
    );

    assert_eq!(
        captured_clone.method.lock().unwrap().as_str(),
        "POST",
        "Initial connection should use POST, got: {}",
        captured_clone.method.lock().unwrap()
    );

    assert!(
        captured_clone
            .body
            .lock()
            .unwrap()
            .contains(r#""model":"qwen2.5""#),
        "Initial connection should include JSON body, got: {}",
        captured_clone.body.lock().unwrap()
    );

    assert!(
        captured_clone
            .auth_header
            .lock()
            .unwrap()
            .contains("Bearer test-key-123"),
        "Initial connection should include Authorization header, got: {}",
        captured_clone.auth_header.lock().unwrap()
    );
}

// -- SseTestServer-based tests (reconnection behavior)

/// WHY: Verify that when an SSE connection drops abruptly (simulating network
/// failure), the reconnecting task actually reconnects and re-sends headers
/// but NOT the body (which is `take()`n on first connection).
///
/// WHAT: Uses `SseTestServer` with `abrupt_drop` close behavior to trigger
/// a reconnection, then verifies:
/// 1. Reconnection occurs (>= 2 connections seen)
/// 2. Both connections have POST method
/// 3. Only the first connection has the body
/// 4. Both connections have the Authorization header
#[test]
#[serial(valtron_pool)]
#[traced_test]
fn test_reconnecting_task_reconnects_with_headers_but_not_body() {
    #[derive(Clone, Default)]
    struct CapturedRequests {
        requests: Arc<std::sync::Mutex<Vec<(String, String, String)>>>, // (method, body, auth)
    }

    let _pool_guard = foundation_core::valtron::initialize_pool(42, None);
    let captured = CapturedRequests::default();
    let captured_clone = captured.clone();
    let connect_count = Arc::new(AtomicUsize::new(0));
    let count_for_assert = connect_count.clone();

    let server = SseTestServer::with_handler(
        move |req: &foundation_testing::http::HttpRequest,
              writer: &mut SseStreamWriter,
              conn_num: usize| {
            // Capture request details
            let method = match &req.method {
                SimpleMethod::GET => "GET".to_string(),
                SimpleMethod::POST => "POST".to_string(),
                SimpleMethod::PUT => "PUT".to_string(),
                SimpleMethod::DELETE => "DELETE".to_string(),
                SimpleMethod::PATCH => "PATCH".to_string(),
                SimpleMethod::HEAD => "HEAD".to_string(),
                SimpleMethod::OPTIONS => "OPTIONS".to_string(),
                SimpleMethod::CONNECT => "CONNECT".to_string(),
                SimpleMethod::TRACE => "TRACE".to_string(),
                SimpleMethod::Custom(s) => s.clone(),
            };

            let body = {
                let bytes = match &req.body {
                    foundation_netio::shared::http::SendSafeBody::Bytes(b) => b.clone(),
                    _ => Vec::new(),
                };
                String::from_utf8_lossy(&bytes).to_string()
            };

            let auth = req
                .headers
                .get(&SimpleHeader::AUTHORIZATION)
                .and_then(|v| v.first())
                .cloned()
                .unwrap_or_default();

            captured.requests.lock().unwrap().push((method, body, auth));

            // First connection: send one event then drop (triggers reconnect)
            if conn_num == 0 {
                let _ = writer.event(r#"{"seq": 0}"#);
                connect_count.fetch_add(1, Ordering::SeqCst);
                SseConnectionResult::abrupt_drop()
            } else {
                // Reconnection: send another event then clean close
                connect_count.fetch_add(1, Ordering::SeqCst);
                let _ = writer.event(r#"{"seq": 1}"#);
                SseConnectionResult::clean_close()
            }
        },
    );

    let addr: SocketAddr = server
        .base_url()
        .strip_prefix("http://")
        .unwrap()
        .parse()
        .unwrap();
    let resolver = StaticSocketAddr::new(addr);
    let base_url = server.base_url().to_string();

    let mut task = ReconnectingEventSourceTask::connect(resolver, base_url)
        .unwrap()
        .with_body(SendSafeBody::Text(r#"{"model":"qwen2.5"}"#.into()))
        .with_header(SimpleHeader::AUTHORIZATION, "Bearer test-key-123")
        .with_max_retries(3);

    let mut events = Vec::new();
    let mut steps = 0;
    while let Some(status) = task.next_status() {
        if let TaskStatus::Ready(parse_result) = status {
            events.push(parse_result);
        }
        steps += 1;
        assert!(steps < 500, "Too many steps, possible infinite loop");
    }

    // Verify we saw at least 2 connections (initial + reconnect)
    assert!(
        count_for_assert.load(Ordering::SeqCst) >= 2,
        "Should have reconnected at least once, got {} connections",
        count_for_assert.load(Ordering::SeqCst)
    );

    // Verify request details across connections
    let requests = captured_clone.requests.lock().unwrap();
    assert!(
        requests.len() >= 2,
        "Should have captured at least 2 requests, got {}",
        requests.len()
    );

    // First connection: POST + body + auth
    let (method, body, auth) = &requests[0];
    assert_eq!(method, "POST", "First connection should use POST");
    assert!(
        body.contains(r#""model":"qwen2.5""#),
        "First connection should include JSON body, got: {body}"
    );
    assert!(
        auth.contains("Bearer test-key-123"),
        "First connection should include Authorization header, got: {auth}"
    );

    // Second connection (reconnection): POST + NO body + auth
    let (method, body, auth) = &requests[1];
    assert_eq!(
        method, "POST",
        "Reconnection should use POST, got: {method}"
    );
    assert!(
        body.is_empty(),
        "Reconnection should NOT include body (body is take()n), got: {body}"
    );
    assert!(
        auth.contains("Bearer test-key-123"),
        "Reconnection should include Authorization header, got: {auth}"
    );
}
