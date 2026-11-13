//! Unit tests for `event_source` writer module.
//!
//! Tests `EventWriter` functionality for SSE event formatting.

use foundation_netio::event_source::{render_comment, EventWriter, SseEvent};

/// WHY: `EventWriter` must format simple messages correctly.
/// WHAT: Verify `message()` writes "data: <text>" followed by empty line.
#[test]
fn test_writer_simple_message() {
    let mut buffer = Vec::new();
    let mut writer = EventWriter::new(&mut buffer);

    writer.message("Hello").unwrap();

    let output = String::from_utf8(buffer).unwrap();
    assert_eq!(output, "data: Hello\n\n");
}

/// WHY: `EventWriter` must format full events with all fields.
/// WHAT: Verify `send()` writes fields in correct order.
#[test]
fn test_writer_full_event() {
    let mut buffer = Vec::new();
    let mut writer = EventWriter::new(&mut buffer);

    let event = SseEvent::new()
        .id("123")
        .event("user_joined")
        .data(r#"{"user": "alice"}"#)
        .build();

    writer.send(&event).unwrap();

    let output = String::from_utf8(buffer).unwrap();
    assert!(output.contains("event: user_joined\n"));
    assert!(output.contains("id: 123\n"));
    assert!(output.contains("data: {\"user\": \"alice\"}\n"));
    assert!(output.ends_with("\n\n"));
}

/// WHY: `EventWriter` must format multi-line data correctly.
/// WHAT: Verify each data line is prefixed with "data: ".
#[test]
fn test_writer_multiline_data() {
    let mut buffer = Vec::new();
    let mut writer = EventWriter::new(&mut buffer);

    let event = SseEvent::new()
        .data("Line 1")
        .data("Line 2")
        .data("Line 3")
        .build();

    writer.send(&event).unwrap();

    let output = String::from_utf8(buffer).unwrap();
    assert_eq!(output, "data: Line 1\ndata: Line 2\ndata: Line 3\n\n");
}

/// WHY: `EventWriter` must send comments for keep-alive.
/// WHAT: Verify `comment()` writes ": <text>" format.
#[test]
fn test_writer_comment() {
    let mut buffer = Vec::new();
    let mut writer = EventWriter::new(&mut buffer);

    writer.comment("keep-alive").unwrap();

    let output = String::from_utf8(buffer).unwrap();
    assert_eq!(output, ": keep-alive\n");
}

/// WHY: `EventWriter` must send retry events.
/// WHAT: Verify retry field is written correctly.
#[test]
fn test_writer_retry() {
    let mut buffer = Vec::new();
    let mut writer = EventWriter::new(&mut buffer);

    writer.send(&SseEvent::retry(5000)).unwrap();

    let output = String::from_utf8(buffer).unwrap();
    assert_eq!(output, "retry: 5000\n\n");
}

/// WHY: One renderer must produce the wire format for both the synchronous
/// writer and the async native writer.
/// WHAT: `SseEvent::render` and `EventWriter::send` write identical bytes for
/// every field combination.
#[test]
fn test_render_matches_event_writer_bytes() {
    let events = [
        SseEvent::message("Hello"),
        SseEvent::retry(0),
        SseEvent::retry(u64::MAX),
        SseEvent::new()
            .id("123")
            .event("user_joined")
            .data(r#"{"user": "alice"}"#)
            .build(),
        SseEvent::new().data("Line 1").data("Line 2").build(),
        SseEvent::default(),
    ];

    for event in events {
        let mut written = Vec::new();
        EventWriter::new(&mut written)
            .send(&event)
            .expect("in-memory writer");
        assert_eq!(
            event.rendered(),
            written,
            "rendered bytes differ for {event:?}"
        );

        let mut rendered = Vec::new();
        event.render(&mut rendered);
        assert_eq!(rendered, written, "render output differs for {event:?}");
    }
}

/// WHY: Comments are part of the wire format and the async writer renders them
/// with the same helper.
/// WHAT: `render_comment` and `EventWriter::comment` write identical bytes.
#[test]
fn test_render_comment_matches_event_writer_bytes() {
    let mut written = Vec::new();
    EventWriter::new(&mut written)
        .comment("keep-alive")
        .expect("in-memory writer");

    let mut rendered = Vec::new();
    render_comment("keep-alive", &mut rendered);

    assert_eq!(written, rendered);
    assert_eq!(written, b": keep-alive\n");
}
