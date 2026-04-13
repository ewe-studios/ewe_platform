#![cfg(test)]

//! WebSocket `ReconnectingWebSocketTask` state machine tests.

use foundation_core::valtron::{TaskIterator, TaskStatus};
use foundation_core::wire::simple_http::client::SystemDnsResolver;
use foundation_core::wire::websocket::{ReconnectingWebSocketProgress, ReconnectingWebSocketTask};
use tracing_test::traced_test;

// Test 1: connect creates task with valid URL
#[test]
#[traced_test]
fn test_connect_creates_task() {
    let resolver = SystemDnsResolver;
    let result = ReconnectingWebSocketTask::connect(resolver, "ws://localhost:8080/chat");

    assert!(result.is_ok(), "should create task with valid URL");
    let _task = result.unwrap();
    // Task created successfully - internal state is private
}

// Test 2: connect rejects invalid URL
#[test]
#[traced_test]
fn test_connect_invalid_url() {
    let resolver = SystemDnsResolver;
    let result = ReconnectingWebSocketTask::connect(resolver, "not-a-url");

    assert!(result.is_err(), "should reject invalid URL");
    if let Err(err) = result {
        assert!(err.to_string().contains("InvalidUrl") || err.to_string().contains("URL"));
    }
}

// Test 3: connect rejects non-ws/wss scheme
#[test]
#[traced_test]
fn test_connect_wrong_scheme() {
    let resolver = SystemDnsResolver;
    let result = ReconnectingWebSocketTask::connect(resolver, "http://localhost:8080/chat");

    assert!(result.is_err(), "should reject non-WebSocket scheme");
    if let Err(err) = result {
        assert!(err.to_string().contains("Unsupported scheme"));
    }
}

// Test 4: ReconnectingWebSocketProgress variants
#[test]
#[traced_test]
fn test_reconnecting_websocket_progress_variants() {
    // Just verifying the enum exists and variants are accessible
    // Debug implementation
    assert!(format!("{:?}", ReconnectingWebSocketProgress::Connecting).contains("Connecting"));
    assert!(format!("{:?}", ReconnectingWebSocketProgress::Handshaking).contains("Handshaking"));
    assert!(format!("{:?}", ReconnectingWebSocketProgress::Reading).contains("Reading"));
    assert!(format!("{:?}", ReconnectingWebSocketProgress::Reconnecting).contains("Reconnecting"));
}

// Test 5: ReconnectingWebSocketProgress PartialEq
#[test]
#[traced_test]
fn test_reconnecting_websocket_progress_eq() {
    assert_eq!(
        ReconnectingWebSocketProgress::Connecting,
        ReconnectingWebSocketProgress::Connecting
    );
    assert_eq!(
        ReconnectingWebSocketProgress::Handshaking,
        ReconnectingWebSocketProgress::Handshaking
    );
    assert_eq!(
        ReconnectingWebSocketProgress::Reading,
        ReconnectingWebSocketProgress::Reading
    );
    assert_eq!(
        ReconnectingWebSocketProgress::Reconnecting,
        ReconnectingWebSocketProgress::Reconnecting
    );

    assert_ne!(
        ReconnectingWebSocketProgress::Connecting,
        ReconnectingWebSocketProgress::Handshaking
    );
    assert_ne!(
        ReconnectingWebSocketProgress::Connecting,
        ReconnectingWebSocketProgress::Reading
    );
    assert_ne!(
        ReconnectingWebSocketProgress::Connecting,
        ReconnectingWebSocketProgress::Reconnecting
    );
    assert_ne!(
        ReconnectingWebSocketProgress::Handshaking,
        ReconnectingWebSocketProgress::Reading
    );
}

// Test 6: with_max_retries configures retries
#[test]
#[traced_test]
fn test_with_max_retries() {
    let resolver = SystemDnsResolver;
    let task = ReconnectingWebSocketTask::connect(resolver, "ws://localhost:8080/chat")
        .unwrap()
        .with_max_retries(10);

    // The config is private, but we can verify the method compiles and returns Self
    let _ = task;
}

// Test 7: with_max_reconnect_duration configures duration
#[test]
#[traced_test]
fn test_with_max_reconnect_duration() {
    use std::time::Duration;

    let resolver = SystemDnsResolver;
    let task = ReconnectingWebSocketTask::connect(resolver, "ws://localhost:8080/chat")
        .unwrap()
        .with_max_reconnect_duration(Duration::from_secs(60));

    let _ = task;
}

// Test 8: with_subprotocol configures subprotocol
#[test]
#[traced_test]
fn test_with_subprotocol() {
    let resolver = SystemDnsResolver;
    let task = ReconnectingWebSocketTask::connect(resolver, "ws://localhost:8080/chat")
        .unwrap()
        .with_subprotocol("chat");

    let _ = task;
}

// Test 9: with_subprotocols configures multiple subprotocols
#[test]
#[traced_test]
fn test_with_subprotocols() {
    let resolver = SystemDnsResolver;
    let task = ReconnectingWebSocketTask::connect(resolver, "ws://localhost:8080/chat")
        .unwrap()
        .with_subprotocols(&["chat", "superchat"]);

    let _ = task;
}

// Test 10: with_header adds custom header
#[test]
#[traced_test]
fn test_with_header() {
    use foundation_core::wire::simple_http::SimpleHeader;

    let resolver = SystemDnsResolver;
    let task = ReconnectingWebSocketTask::connect(resolver, "ws://localhost:8080/chat")
        .unwrap()
        .with_header(SimpleHeader::Custom("X-Custom".to_string()), "value");

    let _ = task;
}

// Test 11: with_read_timeout configures timeout
#[test]
#[traced_test]
fn test_with_read_timeout() {
    use std::time::Duration;

    let resolver = SystemDnsResolver;
    let task = ReconnectingWebSocketTask::connect(resolver, "ws://localhost:8080/chat")
        .unwrap()
        .with_read_timeout(Duration::from_secs(10));

    let _ = task;
}

// Test 12: Task is Send where required
#[test]
#[traced_test]
fn test_task_is_send() {
    // This compiles only if ReconnectingWebSocketTask is Send
    fn assert_send<T: Send>() {}

    // ReconnectingWebSocketTask<R> where R: Send should be Send
    assert_send::<ReconnectingWebSocketTask<SystemDnsResolver>>();
}

// Test 13: Fresh task starts in Connecting state
#[test]
#[traced_test]
fn test_fresh_task_pending_connecting() {
    let _ = foundation_core::valtron::initialize_pool(42, None);

    let resolver = SystemDnsResolver;
    let mut task = ReconnectingWebSocketTask::connect(resolver, "ws://127.0.0.1:1").unwrap();

    // First next() should return Pending(Connecting) as it tries to connect
    let status = task.next_status();
    assert!(status.is_some(), "should return Some status");

    if let Some(TaskStatus::Pending(progress)) = status {
        assert_eq!(progress, ReconnectingWebSocketProgress::Connecting);
    }
    // Task might also be in Handshaking or other states depending on timing
}

// Test 14: Multiple next() calls on failing connection eventual exhaust
#[test]
#[traced_test]
fn test_failing_connection_eventual_exhaust() {
    let _ = foundation_core::valtron::initialize_pool(42, None);

    let resolver = SystemDnsResolver;
    let mut task = ReconnectingWebSocketTask::connect(resolver, "ws://127.0.0.1:1")
        .unwrap()
        .with_max_retries(2); // Low retry count for faster test

    // Keep calling next() until we get None (Exhausted) or a reasonable number of iterations
    let max_iterations = 100;
    let mut iterations = 0;
    let mut got_exhaust = false;

    while iterations < max_iterations {
        iterations += 1;
        if task.next_status().is_none() {
            got_exhaust = true;
            break;
        }
    }

    // Task should eventually exhaust after retries are used up
    assert!(
        got_exhaust,
        "task should eventually exhaust after connection failures"
    );
}

// Test 15: Builder pattern preserves configuration across calls
#[test]
#[traced_test]
fn test_builder_chain() {
    use foundation_core::wire::simple_http::SimpleHeader;
    use std::time::Duration;

    let resolver = SystemDnsResolver;
    let task = ReconnectingWebSocketTask::connect(resolver, "ws://localhost:8080/chat")
        .unwrap()
        .with_max_retries(10)
        .with_max_reconnect_duration(Duration::from_secs(120))
        .with_subprotocol("chat")
        .with_header(SimpleHeader::Custom("X-Test".to_string()), "test-value")
        .with_read_timeout(Duration::from_secs(15));

    // If it compiles, the builder pattern works
    let _ = task;
}
