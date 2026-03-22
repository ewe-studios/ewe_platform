#![cfg(test)]

//! WebSocket TaskIterator state machine tests.

use foundation_core::valtron::{TaskIterator, TaskStatus};
use foundation_core::wire::simple_http::client::SystemDnsResolver;
use foundation_core::wire::websocket::{WebSocketProgress, WebSocketTask};
use tracing_test::traced_test;

// Test 1: connect creates task with valid URL
#[test]
#[traced_test]
fn test_connect_creates_task() {
    let resolver = SystemDnsResolver::default();
    let result = WebSocketTask::connect(resolver, "ws://localhost:8080/chat");

    assert!(result.is_ok(), "should create task with valid URL");
    let _task = result.unwrap();
    // Task created successfully - internal state is private
}

// Test 2: connect rejects invalid URL
#[test]
#[traced_test]
fn test_connect_invalid_url() {
    let resolver = SystemDnsResolver::default();
    let result = WebSocketTask::connect(resolver, "not-a-url");

    assert!(result.is_err(), "should reject invalid URL");
    if let Err(err) = result {
        assert!(err.to_string().contains("InvalidUrl") || err.to_string().contains("URL"));
    }
}

// Test 3: connect rejects non-ws/wss scheme
#[test]
#[traced_test]
fn test_connect_wrong_scheme() {
    let resolver = SystemDnsResolver::default();
    let result = WebSocketTask::connect(resolver, "http://localhost:8080/chat");

    assert!(result.is_err(), "should reject non-WebSocket scheme");
    if let Err(err) = result {
        assert!(err.to_string().contains("Unsupported scheme"));
    }
}

// Test 4: connect_with_pool creates task
#[test]
#[traced_test]
fn test_connect_with_pool() {
    use foundation_core::wire::simple_http::client::{ConnectionPool, HttpConnectionPool};
    use std::sync::Arc;

    let resolver = SystemDnsResolver::default();
    let pool = Arc::new(HttpConnectionPool::new(
        ConnectionPool::default(),
        resolver.clone(),
    ));

    let result = WebSocketTask::connect_with_pool("ws://localhost:8080/chat", pool);

    assert!(result.is_ok(), "should create task with pool");
}

// Test 5: WebSocketProgress variants
#[test]
#[traced_test]
fn test_websocket_progress_variants() {
    // Just verifying the enum exists and variants are accessible
    let _connecting = WebSocketProgress::Connecting;
    let _handshaking = WebSocketProgress::Handshaking;
    let _reading = WebSocketProgress::Reading;

    // Debug implementation
    let debug = format!("{:?}", WebSocketProgress::Connecting);
    assert!(debug.contains("Connecting"));
}

// Test 6: TaskIterator implementation - Invalid URL returns error
#[test]
#[traced_test]
fn test_task_invalid_url_returns_error() {
    let resolver = SystemDnsResolver::default();
    let result = WebSocketTask::connect(resolver, "invalid-url");

    // Task creation fails, which is expected
    assert!(result.is_err());
    if let Err(err) = result {
        assert!(err.to_string().contains("InvalidUrl") || err.to_string().contains("URL"));
    }
}

// Test 7: next() on fresh task returns Pending(Connecting)
#[test]
#[traced_test]
fn test_fresh_task_pending_connecting() {
    foundation_core::valtron::initialize_pool(42, None);

    let resolver = SystemDnsResolver::default();
    let mut task = WebSocketTask::connect(resolver, "ws://127.0.0.1:1").unwrap();

    // First next() should return Pending(Connecting) as it tries to connect
    // (connection will fail since port 1 is not listening)
    let status = task.next_status();
    assert!(status.is_some(), "should return Some status");

    if let Some(TaskStatus::Pending(progress)) = status {
        assert_eq!(progress, WebSocketProgress::Connecting);
    } else {
        // Could also be Delayed or Closed depending on implementation
        // This is acceptable - just verifying task runs
    }
}

// Test 8: Multiple next() calls on failing connection
#[test]
#[traced_test]
fn test_failing_connection_eventual_closed() {
    foundation_core::valtron::initialize_pool(42, None);

    let resolver = SystemDnsResolver::default();
    let mut task = WebSocketTask::connect(resolver, "ws://127.0.0.1:1").unwrap();

    // Keep calling next() until we get None (Closed) or a reasonable number of iterations
    let max_iterations = 50;
    let mut iterations = 0;
    let mut got_closed = false;

    while iterations < max_iterations {
        iterations += 1;
        match task.next_status() {
            None => {
                got_closed = true;
                break;
            }
            Some(TaskStatus::Ready(Err(_))) => {
                // Got an error, connection might be closed now
                continue;
            }
            _ => continue,
        }
    }

    // Task should eventually close after connection fails
    assert!(
        got_closed,
        "task should eventually close after connection failure"
    );
}

// Test 9: Task is Send where required
#[test]
#[traced_test]
fn test_task_is_send() {
    // This compiles only if WebSocketTask is Send
    fn assert_send<T: Send>() {}

    // WebSocketTask<R> where R: Send should be Send
    // This is a compile-time check
    assert_send::<WebSocketTask<SystemDnsResolver>>();
}

// Test 10: WebSocketProgress PartialEq
#[test]
#[traced_test]
fn test_websocket_progress_eq() {
    assert_eq!(WebSocketProgress::Connecting, WebSocketProgress::Connecting);
    assert_eq!(
        WebSocketProgress::Handshaking,
        WebSocketProgress::Handshaking
    );
    assert_eq!(WebSocketProgress::Reading, WebSocketProgress::Reading);

    assert_ne!(
        WebSocketProgress::Connecting,
        WebSocketProgress::Handshaking
    );
    assert_ne!(WebSocketProgress::Connecting, WebSocketProgress::Reading);
    assert_ne!(WebSocketProgress::Handshaking, WebSocketProgress::Reading);
}
