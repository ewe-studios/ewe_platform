//! `SseParser` field/line-handling tests (moved from `src/event_source/shared/sse.rs`).

use std::io::Cursor;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::event_source::{Event, SseParser};

fn create_parser(data: &str) -> SseParser<Cursor<Vec<u8>>> {
    let cursor = Cursor::new(data.as_bytes().to_vec());
    let buffer = SharedByteBufferStream::rwrite(cursor);
    SseParser::new(buffer)
}

#[test]
fn test_sse_parser_simple_message() {
    let data = "data: hello world\n\n";
    let mut parser = create_parser(data);

    let result = parser.parse_next().unwrap().unwrap();
    assert_eq!(
        result.event,
        Event::Message {
            id: None,
            event_type: None,
            data: "hello world".to_string(),
            retry: None,
        }
    );
}

#[test]
fn test_sse_parser_with_id() {
    let data = "id: 123\ndata: test message\n\n";
    let mut parser = create_parser(data);

    let result = parser.parse_next().unwrap().unwrap();
    assert_eq!(
        result.event,
        Event::Message {
            id: Some("123".to_string()),
            event_type: None,
            data: "test message".to_string(),
            retry: None,
        }
    );
    assert_eq!(parser.last_event_id(), Some("123"));
}

#[test]
fn test_sse_parser_with_event_type() {
    let data = "event: update\ndata: some data\n\n";
    let mut parser = create_parser(data);

    let result = parser.parse_next().unwrap().unwrap();
    assert_eq!(
        result.event,
        Event::Message {
            id: None,
            event_type: Some("update".to_string()),
            data: "some data".to_string(),
            retry: None,
        }
    );
}

#[test]
fn test_sse_parser_multiline_data() {
    let data = "data: line1\ndata: line2\ndata: line3\n\n";
    let mut parser = create_parser(data);

    let result = parser.parse_next().unwrap().unwrap();
    assert_eq!(
        result.event,
        Event::Message {
            id: None,
            event_type: None,
            data: "line1\nline2\nline3".to_string(),
            retry: None,
        }
    );
}

#[test]
fn test_sse_parser_comment() {
    let data = ":this is a comment\n";
    let mut parser = create_parser(data);

    let result = parser.parse_next().unwrap().unwrap();
    assert_eq!(
        result.event,
        Event::Comment("this is a comment".to_string())
    );
}

#[test]
fn test_sse_parser_empty_lines_skip() {
    // Empty lines with no accumulated data should be skipped
    let data = "\n\ndata: message\n\n";
    let mut parser = create_parser(data);

    let result = parser.parse_next().unwrap().unwrap();
    assert_eq!(
        result.event,
        Event::Message {
            id: None,
            event_type: None,
            data: "message".to_string(),
            retry: None,
        }
    );
}

#[test]
fn test_sse_parser_retry_field() {
    let data = "retry: 5000\ndata: reconnect info\n\n";
    let mut parser = create_parser(data);

    let result = parser.parse_next().unwrap().unwrap();
    assert_eq!(
        result.event,
        Event::Message {
            id: None,
            event_type: None,
            data: "reconnect info".to_string(),
            retry: Some(5000),
        }
    );
}

#[test]
fn test_sse_parser_iterator_interface() {
    let data = "data: first\n\ndata: second\n\n";
    let mut parser = create_parser(data);

    let results: Vec<_> = parser.by_ref().take(2).collect();
    assert_eq!(results.len(), 2);

    // First event
    assert_eq!(
        results[0].as_ref().unwrap().event,
        Event::Message {
            id: None,
            event_type: None,
            data: "first".to_string(),
            retry: None,
        }
    );

    // Second event
    assert_eq!(
        results[1].as_ref().unwrap().event,
        Event::Message {
            id: None,
            event_type: None,
            data: "second".to_string(),
            retry: None,
        }
    );
}

#[test]
fn test_sse_parser_eof_with_accumulated_data() {
    // Data without trailing empty line - should dispatch on EOF
    let data = "data: incomplete";
    let mut parser = create_parser(data);

    let result = parser.parse_next().unwrap().unwrap();
    assert_eq!(
        result.event,
        Event::Message {
            id: None,
            event_type: None,
            data: "incomplete".to_string(),
            retry: None,
        }
    );

    // Next call should return None (EOF)
    assert!(parser.parse_next().unwrap().is_none());
}

#[test]
fn test_sse_parser_id_with_null_ignored() {
    // ID containing null should be ignored per spec
    let data = "id: test\x00null\ndata: message\n\n";
    let mut parser = create_parser(data);

    // ID with null should be ignored, so no ID on event
    // But the line "id: test\x00null" has a null, so it's ignored
    // The event still parses the data
    let result = parser.parse_next().unwrap().unwrap();
    assert_eq!(
        result.event,
        Event::Message {
            id: None, // ID was ignored due to null
            event_type: None,
            data: "message".to_string(),
            retry: None,
        }
    );
}

#[test]
fn test_sse_parser_unknown_fields_ignored() {
    let data = "unknown: value\ndata: actual data\n\n";
    let mut parser = create_parser(data);

    let result = parser.parse_next().unwrap().unwrap();
    // Unknown fields are ignored, only data is used
    assert_eq!(
        result.event,
        Event::Message {
            id: None,
            event_type: None,
            data: "actual data".to_string(),
            retry: None,
        }
    );
}

#[test]
fn test_sse_parser_crlf_handling() {
    // Test CRLF line endings
    let data = "data: hello\r\n\r\n";
    let mut parser = create_parser(data);

    let result = parser.parse_next().unwrap().unwrap();
    assert_eq!(
        result.event,
        Event::Message {
            id: None,
            event_type: None,
            data: "hello".to_string(),
            retry: None,
        }
    );
}

#[test]
fn test_sse_parser_field_without_colon() {
    // Lines without colon are ignored per spec
    let data = "nocolon\ndata: message\n\n";
    let mut parser = create_parser(data);

    let result = parser.parse_next().unwrap().unwrap();
    assert_eq!(
        result.event,
        Event::Message {
            id: None,
            event_type: None,
            data: "message".to_string(),
            retry: None,
        }
    );
}
