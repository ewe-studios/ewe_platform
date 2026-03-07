//! Unit tests for event_source parser module.
//!
//! Tests SseParser functionality for SSE protocol parsing.

use foundation_core::wire::event_source::{Event, SseParser};

#[test]
fn test_parser_single_event() {
    let mut parser = SseParser::new();
    parser.feed("data: hello\n\n");

    let events: Vec<Event> = parser.collect();

    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::Message { data, .. } => assert_eq!(data, "hello"),
        _ => panic!("Expected Message event"),
    }
}

#[test]
fn test_parser_event_type() {
    let mut parser = SseParser::new();
    parser.feed("event: user_joined\ndata: alice\n\n");

    let events: Vec<Event> = parser.collect();

    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::Message {
            event_type, data, ..
        } => {
            assert_eq!(event_type, &Some("user_joined".to_string()));
            assert_eq!(data, "alice");
        }
        _ => panic!("Expected Message event"),
    }
}

#[test]
fn test_parser_multiline_data() {
    let mut parser = SseParser::new();
    parser.feed("data: Line 1\ndata: Line 2\ndata: Line 3\n\n");

    let events: Vec<Event> = parser.collect();

    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::Message { data, .. } => assert_eq!(data, "Line 1\nLine 2\nLine 3"),
        _ => panic!("Expected Message event"),
    }
}

#[test]
fn test_parser_event_id() {
    let mut parser = SseParser::new();
    parser.feed("id: 123\ndata: hello\n\n");

    let events: Vec<Event> = parser.collect();

    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::Message { id, .. } => assert_eq!(id, &Some("123".to_string())),
        _ => panic!("Expected Message event"),
    }
}

#[test]
fn test_parser_retry() {
    let mut parser = SseParser::new();
    parser.feed("retry: 5000\ndata: hello\n\n");

    let events: Vec<Event> = parser.collect();

    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::Message { retry, .. } => assert_eq!(retry, &Some(5000)),
        _ => panic!("Expected Message event"),
    }
}

#[test]
fn test_parser_comment() {
    let mut parser = SseParser::new();
    parser.feed(": This is a comment\n");

    let events: Vec<Event> = parser.collect();

    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::Comment(comment) => assert_eq!(comment, "This is a comment"),
        _ => panic!("Expected Comment event"),
    }
}

#[test]
fn test_parser_multiple_events() {
    let mut parser = SseParser::new();
    parser.feed("data: first\n\nid: 2\ndata: second\n\n");

    let events: Vec<Event> = parser.collect();

    assert_eq!(events.len(), 2);
    match &events[0] {
        Event::Message { data, .. } => assert_eq!(data, "first"),
        _ => panic!("Expected Message event"),
    }
    match &events[1] {
        Event::Message { id, data, .. } => {
            assert_eq!(id, &Some("2".to_string()));
            assert_eq!(data, "second");
        }
        _ => panic!("Expected Message event"),
    }
}
