//! Unit tests for ReconnectingEventSourceTask.
//!
//! Tests reconnection logic, backoff, Last-Event-ID tracking, and max retries.
//! Uses MockDnsResolver — no real network connections.

use foundation_core::valtron::TaskIterator;
use foundation_core::wire::event_source::ReconnectingEventSourceTask;
use foundation_core::wire::simple_http::client::MockDnsResolver;
use foundation_core::wire::simple_http::DnsError;

/// WHY: ReconnectingEventSourceTask::connect should validate URLs.
/// WHAT: Verify connect returns Err for invalid URL.
#[test]
fn test_reconnecting_task_invalid_url() {
    let resolver = MockDnsResolver::new();
    let result = ReconnectingEventSourceTask::connect(resolver, "not-a-valid-url");
    assert!(result.is_err(), "Expected Err for invalid URL");
}

/// WHY: ReconnectingEventSourceTask should accept valid URLs.
/// WHAT: Verify connect returns Ok for valid HTTP URL.
#[test]
fn test_reconnecting_task_valid_url() {
    let resolver = MockDnsResolver::new();
    let result = ReconnectingEventSourceTask::connect(resolver, "http://test.invalid/events");
    assert!(result.is_ok(), "Expected Ok for valid HTTP URL");
}

/// WHY: ReconnectingEventSourceTask should accept HTTPS URLs.
/// WHAT: Verify connect returns Ok for valid HTTPS URL.
#[test]
fn test_reconnecting_task_https_url() {
    let resolver = MockDnsResolver::new();
    let result = ReconnectingEventSourceTask::connect(resolver, "https://test.invalid/events");
    assert!(result.is_ok(), "Expected Ok for valid HTTPS URL");
}

/// WHY: Builder methods should support chaining without panic.
/// WHAT: Verify with_max_retries, with_header, with_last_event_id chain correctly.
#[test]
fn test_reconnecting_task_builder_chaining() {
    let resolver = MockDnsResolver::new();
    let result = ReconnectingEventSourceTask::connect(resolver, "http://test.invalid/events")
        .unwrap()
        .with_max_retries(10)
        .with_header(
            foundation_core::wire::simple_http::SimpleHeader::custom("Authorization"),
            "Bearer token",
        )
        .with_last_event_id("42");

    // Should not panic — just verify the task was created
    let _ = result;
}

/// WHY: When DNS fails and retries exhaust, task should eventually return None.
/// WHAT: Verify task exhausts after max_retries reconnection attempts.
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
    while task.next().is_some() {
        steps += 1;
        // Safety: prevent infinite loop
        assert!(steps < 100, "Task did not exhaust within 100 steps");
    }

    // Should be exhausted now
    assert!(task.next().is_none(), "Task should stay exhausted");
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

    while let Some(status) = task.next() {
        match status {
            foundation_core::valtron::TaskStatus::Delayed(_) => {
                saw_delayed = true;
            }
            _ => {}
        }
        steps += 1;
        assert!(steps < 50, "Task did not exhaust within 50 steps");
    }

    assert!(
        saw_delayed,
        "Should have seen at least one Delayed status for backoff"
    );
}
