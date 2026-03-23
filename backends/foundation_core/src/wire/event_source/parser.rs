//! SSE message parser following W3C specification.
//!
//! WHY: SSE protocol requires parsing incoming data according to specific rules.
//! WHAT: Streaming SSE parser that reads lines and yields complete events.
//!
//! NOTE: Accumulation happens locally within `parse_next` - no state is stored
//! in the parser struct except for `last_event_id` which persists across events.
//!
//! Reference: W3C Server-Sent Events specification (<https://html.spec.whatwg.org/multipage/server-sent-events.html>)

use crate::io::ioutils::SharedByteBufferStream;
use crate::wire::event_source::{Event, EventSourceError, ParseResult};
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
            "id" => {
                // Ignore if value contains null byte
                if !value.contains('\0') {
                    self.id = Some(value.to_string());
                }
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

    /// Parse next complete event from the stream, returning `ParseResult`.
    ///
    /// WHY: SSE events span multiple lines - need to accumulate until empty line or comment.
    /// WHAT: Reads lines in a loop, accumulating into an [`EventBuilder`], returns `ParseResult`.
    ///
    /// Returns:
    /// - `Ok(Some(ParseResult))` when a complete event is parsed (includes `last_known_id`).
    /// - `Ok(None)` when EOF is reached with no more data.
    /// - `Err(EventSourceError)` on I/O read failure.
    ///
    /// NOTE: Field lines accumulate locally; empty lines dispatch; comments return immediately.
    pub fn parse_next(&mut self) -> Result<Option<ParseResult>, EventSourceError> {
        let mut builder = EventBuilder::new();

        loop {
            let mut line = String::new();

            let bytes_read = self.buffer.read_line(&mut line)?;

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
