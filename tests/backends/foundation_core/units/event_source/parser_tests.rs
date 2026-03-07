//! Unit tests for event_source parser module.
//!
//! Tests SseParser functionality for SSE protocol parsing.
//!
//! NOTE: Parser uses stateful accumulation - field lines return None,
//! only complete events (comments, dispatched messages) return Some(Event).
//! Tests use parse_next() directly or collect_complete_events() helper.

use foundation_core::io::ioutils::SharedBufferReadStream;
use foundation_core::wire::event_source::{Event, SseParser};
use foundation_testing::io::{SharedBuffer, SharedBufferWriter};
use std::io::Write;

/// Collect all events from parser by repeatedly calling parse_next() until buffer empty.
///
/// WHY: Iterator::collect() doesn't work because None means "no event yet" not "exhausted".
/// WHAT: Helper to gather all events for testing.
fn collect_complete_events(mut parser: SseParser<SharedBufferReadStream>) -> Vec<Event> {
    let mut events = Vec::new();

    // First drain any events already in the buffer
    while let Some(event) = parser.parse_next() {
        events.push(event);
    }

    events
}

/// WHY: Parser must yield single event from minimal input.
/// WHAT: Verify write + parse_next() pattern works.
#[test]
fn test_parser_single_event() {
    let (mut writer, reader): (SharedBufferWriter, _) = SharedBuffer::split();
    writer.write_all(b"data: hello\n\n").unwrap();

    // Debug: check what's in the buffer
    println!(
        "Buffer after write: {:?}",
        writer.clone_arc().lock().unwrap()
    );

    let parser = SseParser::new(reader.into_buffered_stream());

    let events = collect_complete_events(parser);
    println!("Events collected: {}", events.len());

    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::Message { data, .. } => assert_eq!(data, "hello"),
        _ => panic!("Expected Message event"),
    }
}

/// WHY: Parser must handle event type field.
/// WHAT: Verify event field is captured correctly.
#[test]
fn test_parser_event_type() {
    let (mut writer, reader): (SharedBufferWriter, _) = SharedBuffer::split();
    writer
        .write_all(b"event: user_joined\ndata: alice\n\n")
        .unwrap();
    let parser = SseParser::new(reader.into_buffered_stream());

    let events = collect_complete_events(parser);

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

/// WHY: Parser must handle multi-line data fields.
/// WHAT: Verify multiple data: lines are joined with newline.
#[test]
fn test_parser_multiline_data() {
    let (mut writer, reader): (SharedBufferWriter, _) = SharedBuffer::split();
    writer
        .write_all(b"data: Line 1\ndata: Line 2\ndata: Line 3\n\n")
        .unwrap();
    let parser = SseParser::new(reader.into_buffered_stream());

    let events = collect_complete_events(parser);

    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::Message { data, .. } => assert_eq!(data, "Line 1\nLine 2\nLine 3"),
        _ => panic!("Expected Message event"),
    }
}

/// WHY: Parser must handle id field.
/// WHAT: Verify id field is captured correctly.
#[test]
fn test_parser_event_id() {
    let (mut writer, reader): (SharedBufferWriter, _) = SharedBuffer::split();
    writer.write_all(b"id: 123\ndata: hello\n\n").unwrap();
    let parser = SseParser::new(reader.into_buffered_stream());

    let events = collect_complete_events(parser);

    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::Message { id, .. } => assert_eq!(id, &Some("123".to_string())),
        _ => panic!("Expected Message event"),
    }
}

/// WHY: Parser must handle retry field.
/// WHAT: Verify retry field is parsed as integer.
#[test]
fn test_parser_retry() {
    let (mut writer, reader): (SharedBufferWriter, _) = SharedBuffer::split();
    writer.write_all(b"retry: 5000\ndata: hello\n\n").unwrap();
    let parser = SseParser::new(reader.into_buffered_stream());

    let events = collect_complete_events(parser);

    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::Message { retry, .. } => assert_eq!(retry, &Some(5000)),
        _ => panic!("Expected Message event"),
    }
}

/// WHY: Parser must handle comment lines (keep-alive).
/// WHAT: Verify comment lines return Event::Comment immediately.
#[test]
fn test_parser_comment() {
    let (mut writer, reader): (SharedBufferWriter, _) = SharedBuffer::split();
    writer.write_all(b": This is a comment\n").unwrap();
    let parser = SseParser::new(reader.into_buffered_stream());

    let events = collect_complete_events(parser);

    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::Comment(comment) => assert_eq!(comment, "This is a comment"),
        _ => panic!("Expected Comment event"),
    }
}

/// WHY: Parser must yield multiple events from single chunk.
/// WHAT: Verify all events are produced in order.
#[test]
fn test_parser_multiple_events() {
    let (mut writer, reader): (SharedBufferWriter, _) = SharedBuffer::split();
    writer
        .write_all(b"data: first\n\nid: 2\ndata: second\n\n")
        .unwrap();
    let parser = SseParser::new(reader.into_buffered_stream());

    let events = collect_complete_events(parser);

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
