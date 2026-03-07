//! SSE message parser following W3C specification.
//!
//! WHY: SSE protocol requires parsing incoming data according to specific rules.
//! WHAT: Implements streaming SSE parser as an Iterator that handles field types, multi-line data, and line endings.
//!
//! Reference: W3C Server-Sent Events specification (<https://html.spec.whatwg.org/multipage/server-sent-events.html>)

use crate::wire::event_source::Event;

/// [`SseParser`] parses incoming SSE data according to W3C specification.
///
/// WHY: SSE protocol has specific parsing rules for fields, line endings, and multi-line data.
/// WHAT: Stateful iterator that accumulates data and yields complete events one at a time.
pub struct SseParser {
    buffer: String,
    current_id: Option<String>,
    current_event: Option<String>,
    current_data: Vec<String>,
    current_retry: Option<u64>,
    event_queue: Vec<Event>,
}

impl SseParser {
    /// Create a new SSE parser.
    ///
    /// WHY: Parser needs to start with empty state.
    /// WHAT: Returns a parser with default empty state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            current_id: None,
            current_event: None,
            current_data: Vec::new(),
            current_retry: None,
            event_queue: Vec::new(),
        }
    }

    /// Feed incoming chunk to the parser.
    ///
    /// WHY: SSE streams arrive in chunks; parser accumulates data until complete events are formed.
    /// WHAT: Processes chunk line-by-line, queues complete events for iteration.
    pub fn feed(&mut self, chunk: &str) {
        self.buffer.push_str(chunk);

        // Process complete lines
        loop {
            // Find next line ending (LF or CRLF)
            let line_end = self.buffer.find('\n').or_else(|| self.buffer.find("\r\n"));

            match line_end {
                None => break, // No complete line yet
                Some(end) => {
                    let line = self.buffer[..end].to_string();
                    self.buffer.drain(..=end);

                    // Strip carriage return if present (\r\n)
                    let line = line.strip_suffix('\r').unwrap_or(&line);

                    // Parse line and queue event if complete
                    self.parse_line(line);
                }
            }
        }
    }

    /// Parse a single line and update state.
    ///
    /// WHY: Each line in SSE has specific meaning (field, comment, empty).
    /// WHAT: Updates internal state or queues complete events.
    fn parse_line(&mut self, line: &str) {
        // Empty line dispatches event
        if line.is_empty() {
            if let Some(event) = self.dispatch_event() {
                self.event_queue.push(event);
            }
            return;
        }

        // Comment (starts with :)
        if line.starts_with(':') {
            let comment = line.strip_prefix(':').unwrap_or("").trim_start();
            self.event_queue.push(Event::Comment(comment.to_string()));
            return;
        }

        // Field line
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

    /// Check if there are more events to iterate.
    ///
    /// WHY: Caller may want to check if parser has buffered events.
    /// WHAT: Returns true if event queue has events.
    #[must_use]
    pub fn has_events(&self) -> bool {
        !self.event_queue.is_empty()
    }
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Iterator for SseParser {
    type Item = Event;

    fn next(&mut self) -> Option<Self::Item> {
        if self.event_queue.is_empty() {
            None
        } else {
            Some(self.event_queue.remove(0))
        }
    }
}
