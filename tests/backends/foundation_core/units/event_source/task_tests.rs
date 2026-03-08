//! Unit tests for event_source task module.
//!
//! Tests EventSourceTask TaskIterator implementation.
//! These tests use MockDnsResolver and never make real network connections.

use foundation_core::valtron::TaskIterator;
use foundation_core::wire::event_source::EventSourceTask;
use foundation_core::wire::simple_http::client::MockDnsResolver;
use foundation_core::wire::simple_http::{DnsError, SimpleHeader};

/// WHY: EventSourceTask::connect should create task in Init state.
/// WHAT: Verify connect returns Ok and transitions through Resolving → Connecting → None when DNS has no response.
#[test]
fn test_event_source_task_connect_creates_task() {
    let resolver = MockDnsResolver::new();
    let mut task =
        EventSourceTask::connect(resolver.clone(), "http://test.invalid/events").unwrap();

    // First call: Init → Resolving
    let first = task.next();
    assert!(
        matches!(first, Some(foundation_core::valtron::TaskStatus::Pending(foundation_core::wire::event_source::EventSourceProgress::Resolving))),
        "Expected Pending(Resolving) on first call"
    );

    // Second call: DNS resolution fails → transitions to Connecting for observability
    let second = task.next();
    assert!(
        matches!(second, Some(foundation_core::valtron::TaskStatus::Pending(foundation_core::wire::event_source::EventSourceProgress::Connecting))),
        "Expected Pending(Connecting) after DNS failure for observability"
    );

    // Third call: Connecting → Closed → None
    let result = task.next();
    assert!(result.is_none(), "Expected None when DNS resolver has no configured response");
}

/// WHY: EventSourceTask should handle DNS resolution failure gracefully.
/// WHAT: Verify task returns Pending(Resolving) → Pending(Connecting) then None on explicit DNS error.
#[test]
fn test_event_source_task_dns_failure() {
    let resolver = MockDnsResolver::new().with_error(
        "test.invalid",
        DnsError::NoAddressesFound("test.invalid".to_string()),
    );

    let mut task =
        EventSourceTask::connect(resolver, "http://test.invalid/events").unwrap();

    // First call: Init → Resolving
    let first = task.next();
    assert!(
        matches!(first, Some(foundation_core::valtron::TaskStatus::Pending(foundation_core::wire::event_source::EventSourceProgress::Resolving))),
        "Expected Pending(Resolving) on first call"
    );

    // Second call: DNS resolution fails → transitions to Connecting for observability
    let second = task.next();
    assert!(
        matches!(second, Some(foundation_core::valtron::TaskStatus::Pending(foundation_core::wire::event_source::EventSourceProgress::Connecting))),
        "Expected Pending(Connecting) after DNS failure for observability"
    );

    // Third call: Connecting → Closed → None
    let result = task.next();
    assert!(result.is_none(), "Expected None after DNS failure transitions to Closed");

    // Subsequent calls should also return None (Closed state is terminal)
    assert!(task.next().is_none(), "Expected None on repeated calls after close");
}

/// WHY: EventSourceTask should handle invalid URLs gracefully.
/// WHAT: Verify connect returns Err for invalid URL.
#[test]
fn test_event_source_task_invalid_url() {
    let resolver = MockDnsResolver::new();
    let result = EventSourceTask::connect(resolver, "not-a-valid-url");
    assert!(result.is_err(), "Expected Err for invalid URL");
}

/// WHY: Builder methods should not panic and should return Self for chaining.
/// WHAT: Verify with_header and with_last_event_id can be chained without panic.
/// NOTE: These are smoke tests only — actual header/last_event_id delivery
/// is verified in integration tests with a real server.
#[test]
fn test_event_source_task_builder_chaining() {
    let resolver = MockDnsResolver::new();
    let mut task = EventSourceTask::connect(resolver.clone(), "http://test.invalid/events")
        .unwrap()
        .with_header(SimpleHeader::custom("Authorization"), "Bearer token123")
        .with_header(SimpleHeader::custom("X-Custom-Header"), "custom-value")
        .with_header(SimpleHeader::custom("X-Empty"), "")
        .with_last_event_id("last-event-42");

    // First call: Init → Resolving
    let first = task.next();
    assert!(
        matches!(first, Some(foundation_core::valtron::TaskStatus::Pending(foundation_core::wire::event_source::EventSourceProgress::Resolving))),
        "Expected Pending(Resolving) on first call"
    );

    // Second call: DNS fails → transitions to Connecting for observability
    let second = task.next();
    assert!(
        matches!(second, Some(foundation_core::valtron::TaskStatus::Pending(foundation_core::wire::event_source::EventSourceProgress::Connecting))),
        "Expected Pending(Connecting) after DNS failure for observability"
    );

    // Third call: Connecting → Closed → None
    let result = task.next();
    assert!(result.is_none(), "Expected None when DNS resolver has no configured response");
}

/// WHY: EventSourceTask should handle HTTPS URLs.
/// WHAT: Verify HTTPS URL is accepted at parse time.
#[test]
fn test_event_source_task_https_url() {
    let resolver = MockDnsResolver::new();
    let result = EventSourceTask::connect(resolver, "https://test.invalid/events");
    assert!(result.is_ok(), "Expected Ok for valid HTTPS URL");
}
