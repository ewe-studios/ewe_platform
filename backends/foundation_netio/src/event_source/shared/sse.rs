//! SSE (Server-Sent Events) parser and types.
//!
//! WHY: SSE is an HTTP protocol extension - HTTP responses with `Content-Type: text/event-stream`
//! have bodies that should be parsed as SSE events. This module provides the SSE parser that
//! operates on streams positioned at the body (after HTTP headers have been parsed).
//!
//! WHAT: SSE parser and event types moved from `wire::event_source` to the HTTP layer
//! where they belong. This fixes the issue where HTTP response headers were being
//! incorrectly parsed as SSE events.
//!
//! HOW: Use `SseParser` with a stream positioned at the body after HTTP response headers
//! have been parsed by `HttpResponseReader`. The parser reads lines and yields complete
//! SSE events according to the W3C specification.
//!
//! Reference: W3C Server-Sent Events specification (<https://html.spec.whatwg.org/multipage/server-sent-events.html>)

use crate::event_source::{Event, EventSourceError, ParseResult};
use foundation_core::io::ioutils::SharedByteBufferStream;
use std::io::Read;

/// Accumulator for building a single SSE event from parsed lines.
struct EventBuilder {
    id: Option<String>,
    event_type: Option<String>,
    data: Vec<String>,
    retry: Option<u64>,
}

impl EventBuilder {
    fn new() -> Self {
        Self {
            id: None,
            event_type: None,
            data: Vec::new(),
            retry: None,
        }
    }

    fn process_field(&mut self, field: &str, value: &str) {
        match field {
            "id" if !value.contains('\0') => {
                self.id = Some(value.to_string());
            }
            "event" => {
                self.event_type = Some(value.to_string());
            }
            "data" => {
                self.data.push(value.to_string());
            }
            "retry" => {
                if let Ok(ms) = value.parse::<u64>() {
                    self.retry = Some(ms);
                }
            }
            // Unknown fields are ignored
            _ => {}
        }
    }

    fn build(&self) -> Option<Event> {
        if self.data.is_empty() {
            return None;
        }

        Some(Event::Message {
            id: self.id.clone(),
            event_type: self.event_type.clone(),
            data: self.data.join("\n"),
            retry: self.retry,
        })
    }

    fn reset(&mut self) {
        self.id = None;
        self.event_type = None;
        self.data = Vec::new();
        self.retry = None;
    }
}

/// [`SseParser`] parses incoming SSE data according to W3C specification.
///
/// WHY: SSE protocol has specific parsing rules for fields, line endings, and multi-line data.
/// WHAT: Parser wrapping a `Read`er, reading lines and yielding complete events.
///
/// NOTE: Generic over any `Read` type wrapped in `SharedByteBufferStream`.
/// Accumulation happens locally in `parse_next`, only `last_event_id` persists.
pub struct SseParser<R: Read> {
    buffer: SharedByteBufferStream<R>,
    last_event_id: Option<String>,
}

impl<R: Read> SseParser<R> {
    /// Create a new SSE parser with a buffer.
    ///
    /// WHY: Parser needs a buffer to read from.
    /// WHAT: Returns a parser that reads from the provided `SharedByteBufferStream`.
    ///
    /// NOTE: External code writes to the buffer; this parser only reads from it.
    /// The stream should be positioned at the body (after HTTP headers).
    #[must_use]
    pub fn new(buffer: SharedByteBufferStream<R>) -> Self {
        Self {
            buffer,
            last_event_id: None,
        }
    }

    /// Get the last event ID seen.
    ///
    /// WHY: Client needs to track last event ID for reconnection resume.
    /// WHAT: Returns reference to current ID.
    #[must_use]
    pub fn last_event_id(&self) -> Option<&str> {
        self.last_event_id.as_deref()
    }

    /// Clone the underlying stream.
    ///
    /// WHY: Allows creating a new parser from the same stream position.
    /// WHAT: Returns a new `SseParser` with a cloned stream.
    ///
    /// NOTE: The new parser starts with the same stream position but fresh state.
    #[must_use]
    pub fn clone_stream(&self) -> Self {
        Self {
            buffer: self.buffer.clone(),
            last_event_id: self.last_event_id.clone(),
        }
    }

    /// Parse next complete event from the stream, returning `ParseResult`.
    ///
    /// WHY: SSE events span multiple lines - need to accumulate until empty line or comment.
    /// WHAT: Reads lines in a loop, accumulating into an `EventBuilder`, returns `ParseResult`.
    ///
    /// Returns:
    /// - `Ok(Some(ParseResult))` when a complete event is parsed (includes `last_known_id`).
    /// - `Ok(None)` when EOF is reached with no more data.
    /// - `Err(EventSourceError)` on I/O read failure.
    ///
    /// NOTE: Field lines accumulate locally; empty lines dispatch; comments return immediately.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn parse_next(&mut self) -> Result<Option<ParseResult>, EventSourceError> {
        let mut builder = EventBuilder::new();

        loop {
            let mut line = String::new();

            tracing::trace!("Pulling next set of bytes from reader");
            let bytes_read = self.buffer.read_line(&mut line)?;
            tracing::trace!("Read total bytes {} from reader: {:?}", bytes_read, &line);

            if bytes_read == 0 {
                // EOF - dispatch any accumulated data before returning
                if let Some(event) = builder.build() {
                    let parse_result = self.build_parse_result(event);
                    return Ok(Some(parse_result));
                }
                return Ok(None);
            }

            // Remove the newline byte from the end (read_line includes it)
            if line.ends_with('\n') {
                line.pop();
            }

            // Handle CRLF - strip trailing \r if present
            if line.ends_with('\r') {
                line.pop();
            }

            // Empty line - dispatch accumulated event
            if line.is_empty() {
                if let Some(event) = builder.build() {
                    let parse_result = self.build_parse_result(event);
                    builder.reset();
                    return Ok(Some(parse_result));
                }
                builder.reset();
                continue;
            }

            // Comment line - return immediately
            if line.starts_with(':') {
                let comment = line.strip_prefix(':').unwrap_or("").trim_start();
                return Ok(Some(ParseResult::new(
                    Event::Comment(comment.to_string()),
                    self.last_event_id.clone(),
                )));
            }

            // Field line - accumulate
            if let Some(colon_pos) = line.find(':') {
                let field = &line[..colon_pos];
                let value = line.get(colon_pos + 1..).unwrap_or("");

                // Strip leading space if present (optional per spec)
                let value = value.strip_prefix(' ').unwrap_or(value);

                builder.process_field(field, value);
            }
            // Lines without colon are ignored per spec
        }
    }

    /// Build a `ParseResult` from an event, updating `last_event_id` if needed.
    fn build_parse_result(&mut self, event: Event) -> ParseResult {
        // Update last_event_id if this event has an ID
        if let Event::Message { id: Some(id), .. } = &event {
            self.last_event_id = Some(id.clone());
        }

        ParseResult::new(event, self.last_event_id.clone())
    }
}

impl<R: Read> Iterator for SseParser<R> {
    type Item = Result<ParseResult, EventSourceError>;

    /// Get next event from parser.
    ///
    /// WHY: Provide standard Iterator interface for SSE event consumption.
    /// WHAT: Parses and returns complete `ParseResult` with `last_known_id`.
    ///
    /// NOTE: Returns `None` only when EOF is reached.
    /// Returns `Some(Err(...))` on I/O or parse errors.
    fn next(&mut self) -> Option<Self::Item> {
        match self.parse_next() {
            Ok(Some(result)) => Some(Ok(result)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

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
}
