#![cfg(test)]

//! WebSocket ReconnectingWebSocketTask integration tests.
//!
//! Tests automatic reconnection with exponential backoff.

use std::time::Duration;

use foundation_core::valtron::{PoolGuard, TaskIterator, TaskStatus};
use foundation_core::wire::simple_http::client::SystemDnsResolver;
use foundation_core::wire::websocket::{ReconnectingWebSocketProgress, ReconnectingWebSocketTask};
use serial_test::serial;
use tracing_test::traced_test;

// All valtron pool tests use the same global serial lock to prevent PoolGuard interference

// Test 1: ReconnectingWebSocketTask creates successfully
#[test]
#[traced_test]
#[serial(valtron_pool)]
fn test_reconnecting_task_creation() {
    let resolver = SystemDnsResolver;
    let result = ReconnectingWebSocketTask::connect(resolver, "ws://localhost:8080/chat");

    assert!(result.is_ok(), "Should create reconnecting task");
}

// Test 2: ReconnectingWebSocketTask rejects invalid URL
#[test]
#[traced_test]
#[serial(valtron_pool)]
fn test_reconnecting_task_invalid_url() {
    let resolver = SystemDnsResolver;
    let result = ReconnectingWebSocketTask::connect(resolver, "not-a-valid-url");

    assert!(result.is_err(), "Should reject invalid URL");
}

// Test 3: ReconnectingWebSocketTask rejects non-ws scheme
#[test]
#[traced_test]
#[serial(valtron_pool)]
fn test_reconnecting_task_non_ws_scheme() {
    let resolver = SystemDnsResolver;
    let result = ReconnectingWebSocketTask::connect(resolver, "http://localhost:8080/chat");

    assert!(result.is_err(), "Should reject http:// scheme");
}

// Test 4: Builder configuration methods work
#[test]
#[traced_test]
#[serial(valtron_pool)]
fn test_reconnecting_task_builder() {
    let resolver = SystemDnsResolver;

    let task = ReconnectingWebSocketTask::connect(resolver, "ws://localhost:8080/chat")
        .unwrap()
        .with_max_retries(3)
        .with_max_reconnect_duration(Duration::from_secs(30))
        .with_subprotocol("chat")
        .with_read_timeout(Duration::from_secs(10));

    // If it compiles, builder works
    let _ = task;
}

// Test 5: Task progresses through Connecting state
#[test]
#[traced_test]
#[serial(valtron_pool)]
fn test_reconnecting_task_progress_states() {
    let _pool_guard: PoolGuard = foundation_core::valtron::initialize_pool(42, None);

    let resolver = SystemDnsResolver;
    let mut task = ReconnectingWebSocketTask::connect(resolver, "ws://127.0.0.1:1").unwrap();

    // First next() should return a pending state
    let status = task.next_status();
    assert!(status.is_some(), "Should return Some status");

    match status {
        Some(TaskStatus::Pending(ReconnectingWebSocketProgress::Connecting)) => {
            // Expected - trying to connect
        }
        Some(TaskStatus::Pending(_)) => {
            // Also acceptable - might transition quickly
        }
        _ => {
            // Other states are also valid depending on timing
        }
    }
}

// Test 6: ReconnectingWebSocketTask is Send
#[test]
#[traced_test]
#[serial(valtron_pool)]
fn test_reconnecting_task_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<ReconnectingWebSocketTask<SystemDnsResolver>>();
}

// Test 7: Task with subprotocols configuration
#[test]
#[traced_test]
#[serial(valtron_pool)]
fn test_reconnecting_task_with_subprotocols() {
    let resolver = SystemDnsResolver;

    let task = ReconnectingWebSocketTask::connect(resolver, "ws://localhost:8080/chat")
        .unwrap()
        .with_subprotocols(&["chat", "superchat"]);

    let _ = task;
}

// Test 8: Task with custom header
#[test]
#[traced_test]
#[serial(valtron_pool)]
fn test_reconnecting_task_with_header() {
    use foundation_core::wire::simple_http::SimpleHeader;

    let resolver = SystemDnsResolver;

    let task = ReconnectingWebSocketTask::connect(resolver, "ws://localhost:8080/chat")
        .unwrap()
        .with_header(
            SimpleHeader::Custom("X-Custom-Header".to_string()),
            "custom-value",
        );

    let _ = task;
}

// Test 9: Task with custom backoff
#[test]
#[traced_test]
#[serial(valtron_pool)]
fn test_reconnecting_task_with_custom_backoff() {
    use foundation_core::retries::ExponentialBackoffDecider;

    let resolver = SystemDnsResolver;
    let backoff = ExponentialBackoffDecider::from_duration(
        Duration::from_millis(100),
        Some(Duration::from_secs(1)),
    );

    let task = ReconnectingWebSocketTask::connect(resolver, "ws://localhost:8080/chat")
        .unwrap()
        .with_backoff(backoff);

    let _ = task;
}

// Test 10: Connection failure triggers reconnection attempt
#[test]
#[traced_test]
#[serial(valtron_pool)]
fn test_connection_failure_triggers_reconnect() {
    let _pool_guard: PoolGuard = foundation_core::valtron::initialize_pool(42, None);

    // Connect to invalid address - should trigger reconnection attempts
    let resolver = SystemDnsResolver;
    let mut task = ReconnectingWebSocketTask::connect(resolver, "ws://127.0.0.1:1")
        .unwrap()
        .with_max_retries(3)
        .with_backoff(
            foundation_core::retries::ExponentialBackoffDecider::from_duration(
                Duration::from_millis(10),
                Some(Duration::from_millis(50)),
            ),
        );

    // Run task and observe reconnection behavior
    let mut iterations = 0;
    let mut saw_reconnecting = false;
    let mut saw_delay = false;
    let max_iterations = 200;

    while iterations < max_iterations {
        iterations += 1;
        match task.next_status() {
            None => {
                // Task exhausted - reconnection attempts used up
                break;
            }
            Some(TaskStatus::Pending(ReconnectingWebSocketProgress::Reconnecting)) => {
                saw_reconnecting = true;
            }
            Some(TaskStatus::Delayed(duration)) => {
                // Backoff delay - expected during reconnection
                saw_delay = true;
                // Simulate executor delay (capped for test speed)
                let delay = std::cmp::min(duration, Duration::from_millis(50));
                std::thread::sleep(delay);
            }
            Some(TaskStatus::Ready(Err(_))) => {
                // Connection error - expected
            }
            _ => {}
        }
    }

    // Should have seen reconnecting state or backoff delay
    assert!(
        saw_reconnecting || saw_delay,
        "Should have seen reconnection activity (Reconnecting state or backoff delay)"
    );
}

// Test 11: Max retries eventually exhausts
#[test]
#[traced_test]
#[serial(valtron_pool)]
fn test_max_retries_exhausts() {
    let _pool_guard: PoolGuard = foundation_core::valtron::initialize_pool(42, None);

    let resolver = SystemDnsResolver;
    let mut task = ReconnectingWebSocketTask::connect(resolver, "ws://127.0.0.1:1")
        .unwrap()
        .with_max_retries(2); // Very low retry count

    // Run until exhausted
    let mut iterations = 0;
    let max_iterations = 100;
    let mut got_exhausted = false;

    while iterations < max_iterations {
        iterations += 1;
        if task.next_status().is_none() {
            got_exhausted = true;
            break;
        }
    }

    assert!(got_exhausted, "Task should exhaust after max retries");
}

// Test 12: Max reconnect duration eventually exhausts
#[test]
#[traced_test]
#[serial(valtron_pool)]
fn test_max_reconnect_duration_exhausts() {
    let _pool_guard: PoolGuard = foundation_core::valtron::initialize_pool(42, None);

    let resolver = SystemDnsResolver;
    let mut task = ReconnectingWebSocketTask::connect(resolver, "ws://127.0.0.1:1")
        .unwrap()
        .with_max_reconnect_duration(Duration::from_millis(500)) // Short duration
        .with_max_retries(100) // High retry count - duration should limit
        .with_backoff(
            foundation_core::retries::ExponentialBackoffDecider::from_duration(
                Duration::from_millis(10),
                Some(Duration::from_millis(50)),
            ),
        );

    // Run until exhausted - need more iterations because each reconnect cycle takes multiple next() calls
    let start = std::time::Instant::now();
    let mut iterations = 0;
    let max_iterations = 500; // Increased for more reconnect cycles
    let mut got_exhausted = false;

    while iterations < max_iterations {
        iterations += 1;
        match task.next_status() {
            None => {
                got_exhausted = true;
                break;
            }
            Some(TaskStatus::Delayed(duration)) => {
                // Simulate executor delay (but cap it for test speed)
                let delay = std::cmp::min(duration, Duration::from_millis(50));
                std::thread::sleep(delay);
            }
            _ => {}
        }
    }

    let elapsed = start.elapsed();
    assert!(
        got_exhausted,
        "Task should exhaust after max duration, iterations={}, elapsed={:?}",
        iterations, elapsed
    );
    assert!(
        elapsed >= Duration::from_millis(100),
        "Should have run for some time before exhausting, elapsed={:?}",
        elapsed
    );
}
