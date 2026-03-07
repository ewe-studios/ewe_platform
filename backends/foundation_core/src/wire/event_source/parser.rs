//! SSE message parser following W3C specification.
//!
//! WHY: SSE protocol requires parsing incoming data according to specific rules.
//! WHAT: Streaming SSE parser that reads lines and accumulates state until complete events.
//!
//! NOTE: This parser uses internal state accumulation (`current_data`, `current_id`, etc.)
//! to build events across multiple lines - this is REQUIRED by SSE spec.
//!
//! Reference: W3C Server-Sent Events specification (<https://html.spec.whatwg.org/multipage/server-sent-events.html>)

use crate::io::ioutils::SharedByteBufferStream;
use crate::wire::event_source::Event;
use std::io::Read;

/// [`SseParser`] parses incoming SSE data according to W3C specification.
///
/// WHY: SSE protocol has specific parsing rules for fields, line endings, and multi-line data.
/// WHAT: Stateful parser wrapping a `Read`er, reading lines and accumulating state until complete events.
///
/// NOTE: Generic over any `Read` type wrapped in `SharedByteBufferStream`.
/// The parser uses `BufRead` to efficiently read lines from the buffered stream.
pub struct SseParser<R: Read> {
    buffer: SharedByteBufferStream<R>,
    current_id: Option<String>,
    current_event: Option<String>,
    current_data: Vec<String>,
    current_retry: Option<u64>,
}

impl<R: Read> SseParser<R> {
    /// Create a new SSE parser with a buffer.
    ///
    /// WHY: Parser needs a buffer to read from.
    /// WHAT: Returns a parser that reads from the provided `SharedByteBufferStream`.
    ///
    /// NOTE: External code writes to the buffer; this parser only reads from it.
    pub fn new(buffer: SharedByteBufferStream<R>) -> Self {
        Self {
            buffer,
            current_id: None,
            current_event: None,
            current_data: Vec::new(),
            current_retry: None,
        }
    }

    /// Parse a single line and update state.
    ///
    /// WHY: Each line in SSE has specific meaning (field, comment, empty).
    /// WHAT: Returns `Some(Event)` for comments or completed events,
    ///       `None` for field lines that need more data.
    fn parse_line(&mut self, line: &str) -> Option<Event> {
        // Empty line dispatches event
        if line.is_empty() {
            return self.dispatch_event();
        }

        // Comment (starts with :) - immediate single-line event
        if line.starts_with(':') {
            let comment = line.strip_prefix(':').unwrap_or("").trim_start();
            return Some(Event::Comment(comment.to_string()));
        }

        // Field line - accumulate state, event comes later
        if let Some(colon_pos) = line.find(':') {
            let field = &line[..colon_pos];
            let value = line.get(colon_pos + 1..).unwrap_or("");

            // Strip leading space if present (optional per spec)
            let value = value.strip_prefix(' ').unwrap_or(value);

            match field {
                "id" => {
                    // Ignore if value contains null byte
                    if !value.contains('\0') {
                        self.current_id = Some(value.to_string());
                    }
                }
                "event" => {
                    self.current_event = Some(value.to_string());
                }
                "data" => {
                    self.current_data.push(value.to_string());
                }
                "retry" => {
                    if let Ok(ms) = value.parse::<u64>() {
                        self.current_retry = Some(ms);
                    }
                }
                // Unknown fields are ignored
                _ => {}
            }
        }

        // Field line - no event yet, still accumulating
        None
    }

    /// Read and parse one line from the buffer.
    ///
    /// WHY: Each line in SSE has specific meaning (field, comment, empty).
    /// WHAT: Reads a line into a String using the higher-level `read_line` method,
    ///       parses it, returns event if complete.
    ///
    /// NOTE: Uses `read_line` which internally handles `next_until` and state management.
    /// After extracting the line, identifies event type and either:
    /// - Returns `Some(Event)` for comments
    /// - Accumulates state and returns `None` for field lines
    /// - Dispatches event for empty lines
    fn read_and_parse_line(&mut self) -> Option<Event> {
        let mut line = String::new();

        // Read line using higher-level read_line method
        match self.buffer.read_line(&mut line) {
            Ok(0) => return None, // EOF or empty line read
            Ok(_) => {}
            Err(_) => return None, // Read error, stop iteration
        }

        // Remove the newline byte from the end (read_line includes it)
        if line.ends_with('\n') {
            line.pop();
        }

        // Handle CRLF - strip trailing \r if present
        if line.ends_with('\r') {
            line.pop();
        }

        // Parse line and return event if complete
        self.parse_line(&line)
    }

    /// Dispatch the current accumulated data as an event.
    ///
    /// WHY: Empty line signals end of event block.
    /// WHAT: Creates Event from accumulated state and resets state.
    fn dispatch_event(&mut self) -> Option<Event> {
        // No event if no data accumulated
        if self.current_data.is_empty() {
            self.reset_state();
            return None;
        }

        // Join data lines with newline
        let data = self.current_data.join("\n");

        let event = Event::Message {
            id: self.current_id.take(),
            event_type: self.current_event.take(),
            data,
            retry: self.current_retry.take(),
        };

        self.reset_state();
        Some(event)
    }

    /// Reset parser state after dispatching event.
    ///
    /// WHY: Each event starts fresh.
    /// WHAT: Clears accumulated data while preserving last event ID for resume.
    fn reset_state(&mut self) {
        self.current_data = Vec::new();
        self.current_event = None;
        self.current_retry = None;
        // Note: We DON'T reset current_id here as it persists for Last-Event-ID
    }

    /// Get the last event ID seen.
    ///
    /// WHY: Client needs to track last event ID for reconnection resume.
    /// WHAT: Returns reference to current ID.
    #[must_use]
    pub fn last_event_id(&self) -> Option<&str> {
        self.current_id.as_deref()
    }

    /// Parse next available line and return event if complete.
    ///
    /// WHY: Combined method for single-step parsing.
    /// WHAT: Reads line, parses it, returns event or None.
    pub fn parse_next(&mut self) -> Option<Event> {
        self.read_and_parse_line()
    }
}

impl<R: Read> Iterator for SseParser<R> {
    type Item = Event;

    /// Get next event from parser.
    ///
    /// WHY: Provide standard Iterator interface for SSE event consumption.
    /// WHAT: Reads and parses one line, returns event if produced.
    ///
    /// NOTE: Returns `None` when:
    /// - Buffer is empty (no complete line to extract)
    /// - Line was a field (accumulates state, event comes later)
    ///
    /// This does NOT mean the iterator is exhausted - caller should
    /// `feed()` more data or wait for more data from the reader, then call `next()` again.
    fn next(&mut self) -> Option<Self::Item> {
        self.parse_next()
    }
}
