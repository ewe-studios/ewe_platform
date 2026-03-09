//! Unit tests for event_source consumer module.
//!
//! Tests SseStream, ReconnectingSseStream, and SseStreamEvent.
//! These tests use MockDnsResolver and never make real network connections.

use foundation_core::wire::event_source::{Event, SseStreamEvent};

/// WHY: SseStreamEvent should wrap Event variants correctly.
/// WHAT: Verify From<ParseResult> implementation creates Event variant.
#[test]
fn test_sse_stream_event_from_parse_result() {
    let event = Event::Message {
        id: None,
        event_type: None,
        data: "test data".to_string(),
        retry: None,
    };
    let parse_result = foundation_core::wire::event_source::ParseResult {
        event,
        last_known_id: None,
    };

    let stream_event: SseStreamEvent = parse_result.into();

    match stream_event {
        SseStreamEvent::Event(Event::Message { data, .. }) => {
            assert_eq!(data, "test data");
        }
        SseStreamEvent::Event(_) => panic!("Expected Message event variant"),
        SseStreamEvent::Skip => panic!("Expected Event variant, got Skip"),
    }
}

/// WHY: SseStreamEvent should have Event variant for actual events.
/// WHAT: Verify Event variant can be constructed directly.
#[test]
fn test_sse_stream_event_event_variant() {
    let event = Event::Message {
        id: None,
        event_type: None,
        data: "hello".to_string(),
        retry: None,
    };
    let stream_event = SseStreamEvent::Event(event);

    match stream_event {
        SseStreamEvent::Event(Event::Message { data, .. }) => {
            assert_eq!(data, "hello");
        }
        SseStreamEvent::Event(_) => panic!("Expected Message event variant"),
        SseStreamEvent::Skip => panic!("Expected Event variant, got Skip"),
    }
}

/// WHY: SseStreamEvent should have Skip variant for pending/delayed states.
/// WHAT: Verify Skip variant can be constructed.
#[test]
fn test_sse_stream_event_skip_variant() {
    let stream_event = SseStreamEvent::Skip;

    match stream_event {
        SseStreamEvent::Skip => {
            // Expected behavior
        }
        SseStreamEvent::Event(_) => panic!("Expected Skip variant, got Event"),
    }
}

/// WHY: SseStreamEvent should derive Debug for logging.
/// WHAT: Verify Debug trait is implemented.
#[test]
fn test_sse_stream_event_debug() {
    let event = SseStreamEvent::Event(Event::Message {
        id: None,
        event_type: None,
        data: "test".to_string(),
        retry: None,
    });
    let debug_str = format!("{:?}", event);
    assert!(debug_str.contains("Event"), "Debug output should contain 'Event'");

    let skip = SseStreamEvent::Skip;
    let skip_debug = format!("{:?}", skip);
    assert!(skip_debug.contains("Skip"), "Debug output should contain 'Skip'");
}

/// WHY: SseStreamEvent should derive Clone.
/// WHAT: Verify Clone trait works correctly.
#[test]
fn test_sse_stream_event_clone() {
    let event = SseStreamEvent::Event(Event::Message {
        id: None,
        event_type: None,
        data: "clone test".to_string(),
        retry: None,
    });
    let cloned = event.clone();

    match (event, cloned) {
        (SseStreamEvent::Event(Event::Message { data: d1, .. }), SseStreamEvent::Event(Event::Message { data: d2, .. })) => {
            assert_eq!(d1, d2);
        }
        _ => panic!("Clone should preserve variant type"),
    }

    let skip = SseStreamEvent::Skip;
    let skip_cloned = skip.clone();
    assert!(matches!(skip_cloned, SseStreamEvent::Skip));
}
