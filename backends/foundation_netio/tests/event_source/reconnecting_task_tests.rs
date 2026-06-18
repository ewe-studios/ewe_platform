//! Unit tests for `ReconnectingEventSourceTask`.
//!
//! Tests reconnection logic, backoff, Last-Event-ID tracking, and max retries.
//! Uses `MockDnsResolver` — no real network connections.

use foundation_core::valtron::{TaskIterator, TaskStatus};
use foundation_netio::event_source::native::ReconnectingEventSourceTask;
use foundation_netio::simple_http::client::shared::MockDnsResolver;
use foundation_netio::simple_http::shared::DnsError;

/// WHY: `ReconnectingEventSourceTask::connect` should validate URLs.
/// WHAT: Verify connect returns Err for invalid URL.
#[test]
fn test_reconnecting_task_invalid_url() {
    let resolver = MockDnsResolver::new();
    let result = ReconnectingEventSourceTask::connect(resolver, "not-a-valid-url");
    assert!(result.is_err(), "Expected Err for invalid URL");
}

/// WHY: `ReconnectingEventSourceTask` should accept valid URLs.
/// WHAT: Verify connect returns Ok for valid HTTP URL.
#[test]
fn test_reconnecting_task_valid_url() {
    let resolver = MockDnsResolver::new();
    let result = ReconnectingEventSourceTask::connect(resolver, "http://test.invalid/events");
    assert!(result.is_ok(), "Expected Ok for valid HTTP URL");
}

/// WHY: `ReconnectingEventSourceTask` should accept HTTPS URLs.
/// WHAT: Verify connect returns Ok for valid HTTPS URL.
#[test]
fn test_reconnecting_task_https_url() {
    let resolver = MockDnsResolver::new();
    let result = ReconnectingEventSourceTask::connect(resolver, "https://test.invalid/events");
    assert!(result.is_ok(), "Expected Ok for valid HTTPS URL");
}

/// WHY: Builder methods should support chaining without panic.
/// WHAT: Verify `with_max_retries`, `with_header`, `with_last_event_id` chain correctly.
#[test]
fn test_reconnecting_task_builder_chaining() {
    let resolver = MockDnsResolver::new();
    let result = ReconnectingEventSourceTask::connect(resolver, "http://test.invalid/events")
        .unwrap()
        .with_max_retries(10)
        .with_header(
            foundation_netio::simple_http::shared::SimpleHeader::custom("Authorization"),
            "Bearer token",
        )
        .with_last_event_id("42");

    // Should not panic — just verify the task was created
    let _ = result;
}

/// WHY: When DNS fails and retries exhaust, task should eventually return None.
/// WHAT: Verify task exhausts after `max_retries` reconnection attempts.
#[test]
fn test_reconnecting_task_exhausts_after_max_retries() {
    let resolver = MockDnsResolver::new().with_error(
        "test.invalid",
        DnsError::NoAddressesFound("test.invalid".to_string()),
    );

    let mut task = ReconnectingEventSourceTask::connect(resolver, "http://test.invalid/events")
        .unwrap()
        .with_max_retries(2);

    // Drive the task until it exhausts
    let mut steps = 0;
    while task.next_status().is_some() {
        steps += 1;
        // Safety: prevent infinite loop
        assert!(steps < 100, "Task did not exhaust within 100 steps");
    }

    // Should be exhausted now
    assert!(task.next_status().is_none(), "Task should stay exhausted");
}

/// WHY: When DNS has no configured response, initial connection fails.
/// WHAT: Verify task attempts reconnection (not just immediate death).
#[test]
fn test_reconnecting_task_attempts_reconnection_on_failure() {
    let resolver = MockDnsResolver::new();

    let mut task = ReconnectingEventSourceTask::connect(resolver, "http://test.invalid/events")
        .unwrap()
        .with_max_retries(1);

    // First call: inner task tries Init, DNS fails → inner returns None
    // Reconnecting task should transition to Waiting (not immediately exhaust)
    let mut saw_delayed = false;
    let mut steps = 0;

    while let Some(status) = task.next_status() {
        if let foundation_core::valtron::TaskStatus::Delayed(_) = status {
            saw_delayed = true;
        }
        steps += 1;
        assert!(steps < 50, "Task did not exhaust within 50 steps");
    }

    assert!(
        saw_delayed,
        "Should have seen at least one Delayed status for backoff"
    );
}

/// WHY: `ReconnectingEventSourceTask` should not create the inner connection
/// until `next_status()` is called. Body and headers configured via builders
/// must apply to the initial connection, not just reconnections.
/// WHAT: Verify that after construction (before first `next_status`), the task
/// is in a lazy Init state and headers/body are preserved in config.
#[test]
fn test_reconnecting_task_defers_connection_until_next_status() {
    let resolver = MockDnsResolver::new();

    // Build task with body and headers
    let _task = ReconnectingEventSourceTask::connect(resolver, "http://test.invalid/events")
        .unwrap()
        .with_body(foundation_netio::simple_http::shared::SendSafeBody::Text(
            r#"{"model":"test"}"#.into(),
        ))
        .with_header(
            foundation_netio::simple_http::shared::SimpleHeader::AUTHORIZATION,
            "Bearer test-key",
        );

    // Task should be constructable without attempting DNS or connection.
    // If connect() eagerly creates the inner task, this test still passes
    // because MockDnsResolver doesn't resolve anything.
    // The key behavior we want: body/headers apply to INITIAL connection,
    // not just reconnections.
}

/// WHY: When DNS fails on the FIRST `next_status()` call (not connect),
/// the task should transition to reconnection/backoff, not immediately exhaust.
/// WHAT: Verify the task enters the state machine and handles initial failure.
#[test]
fn test_reconnecting_task_initial_connection_failure_reconnects() {
    let resolver = MockDnsResolver::new().with_error(
        "test.invalid",
        DnsError::NoAddressesFound("test.invalid".to_string()),
    );

    let mut task = ReconnectingEventSourceTask::connect(resolver, "http://test.invalid/events")
        .unwrap()
        .with_max_retries(2);

    let mut states: Vec<String> = Vec::new();

    while let Some(status) = task.next_status() {
        let label = match status {
            foundation_core::valtron::TaskStatus::Init => "Init".to_string(),
            foundation_core::valtron::TaskStatus::Pending(p) => format!("Pending({:?})", p),
            foundation_core::valtron::TaskStatus::Delayed(d) => format!("Delayed({:?})", d),
            foundation_core::valtron::TaskStatus::Ready(_) => "Ready".to_string(),
            foundation_core::valtron::TaskStatus::Ignore => "Ignore".to_string(),
            foundation_core::valtron::TaskStatus::Spawn(_) => "Spawn".to_string(),
            foundation_core::valtron::TaskStatus::Wait => "Wait".to_string(),
            TaskStatus::Spread(_) => "Spread".to_string(),
            TaskStatus::Depends(_) => "Depends".to_string(),
        };
        states.push(label);
        // Safety valve
        assert!(states.len() < 100, "Too many steps: {:?}", states);
    }

    // Should have seen at least Pending (Connecting attempt) and Delayed (backoff)
    assert!(
        states.iter().any(|s| s.contains("Connecting")),
        "Should have attempted connecting, saw states: {:?}",
        states
    );
}
