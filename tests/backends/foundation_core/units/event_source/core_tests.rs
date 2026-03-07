//! Unit tests for event_source core module.
//!
//! Tests SseEvent and SseEventBuilder functionality.

use foundation_core::wire::event_source::{Event, SseEvent};

#[test]
fn test_sse_event_message_creates_simple_message() {
    let event = SseEvent::message("Hello, World!");

    assert_eq!(event.id(), None);
    assert_eq!(event.event_type(), None);
    assert_eq!(event.retry_ms(), None);
    assert_eq!(event.data_lines(), &["Hello, World!"]);
}

#[test]
fn test_sse_event_retry_creates_retry_event() {
    let event = SseEvent::retry(5000);

    assert_eq!(event.id(), None);
    assert_eq!(event.event_type(), None);
    assert_eq!(event.retry_ms(), Some(5000));
    assert_eq!(event.data_lines(), &[] as &[String]);
}

#[test]
fn test_sse_event_builder_creates_full_event() {
    let event = SseEvent::new()
        .id("123")
        .event("user_joined")
        .data(r#"{"user": "alice"}"#)
        .build();

    assert_eq!(event.id(), Some("123"));
    assert_eq!(event.event_type(), Some("user_joined"));
    assert_eq!(event.retry_ms(), None);
    assert_eq!(event.data_lines(), &[r#"{"user": "alice"}"#]);
}

#[test]
fn test_sse_event_builder_multiline_data() {
    let event = SseEvent::new()
        .data("Line 1")
        .data("Line 2")
        .data("Line 3")
        .build();

    assert_eq!(event.data_lines(), &["Line 1", "Line 2", "Line 3"]);
}
