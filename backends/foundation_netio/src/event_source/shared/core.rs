//! Core SSE types: [`Event`], [`SseEvent`], and [`ParseResult`].
//!
//! WHY: Server-Sent Events require standardized types for consuming and producing events.
//! WHAT: Defines the [`Event`] enum for received events, [`SseEvent`] builder for sending events,
//! and [`ParseResult`] which pairs parsed events with their last known event ID.
//!
//! Reference: W3C Server-Sent Events specification (<https://html.spec.whatwg.org/multipage/server-sent-events.html>)

/// `ParseResult` represents a parsed SSE event with explicit `last_known_id`.
///
/// WHY: Reconnection logic needs last known event ID. Instead of hidden
/// parser state, we return it explicitly with each event.
/// WHAT: Tuple-like struct with event and `last_known_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseResult {
    /// The parsed event.
    pub event: Event,
    /// Last known event ID after parsing this event.
    /// - `None` if no ID has ever been seen
    /// - `Some(id)` if the current or a previous event had an ID
    ///
    /// Updated when the parsed event contains an `id:` field.
    pub last_known_id: Option<String>,
}

impl ParseResult {
    /// Create a new `ParseResult`.
    ///
    /// # Arguments
    /// * `event` - The parsed SSE event
    /// * `last_known_id` - The last known event ID (updated if this event has an ID)
    #[must_use]
    pub fn new(event: Event, last_known_id: Option<String>) -> Self {
        Self {
            event,
            last_known_id,
        }
    }
}

/// Event represents a parsed SSE event received from the server.
///
/// WHY: SSE protocol defines specific event types (message, comment, reconnect) with fields.
/// WHAT: Enum capturing all possible SSE event types with their associated data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A message event with optional id, event type, data, and retry interval.
    Message {
        id: Option<String>,
        event_type: Option<String>,
        data: String,
        retry: Option<u64>,
    },
    /// A comment (keep-alive) - ignored by clients but useful for debugging.
    Comment(String),
    /// Reconnection signal - used internally to indicate reconnection needed.
    Reconnect,
}

impl Event {
    /// Get the event ID (if this is a Message event with an ID).
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Message { id, .. } => id.as_deref(),
            _ => None,
        }
    }

    /// Get the event type (if this is a Message event with a type).
    #[must_use]
    pub fn event_type(&self) -> Option<&str> {
        match self {
            Self::Message { event_type, .. } => event_type.as_deref(),
            _ => None,
        }
    }

    /// Get the event data (if this is a Message event).
    #[must_use]
    pub fn data(&self) -> Option<&str> {
        match self {
            Self::Message { data, .. } => Some(data.as_str()),
            _ => None,
        }
    }
}

/// [`SseEvent`] represents an SSE event to be sent to clients (server-side).
///
/// WHY: Server needs to format events according to SSE specification.
/// WHAT: Builder pattern for constructing SSE events with all fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    id: Option<String>,
    event_type: Option<String>,
    data: Vec<String>,
    retry: Option<u64>,
}

impl SseEvent {
    /// Create a simple message event with just data.
    ///
    /// WHY: Most common use case - send a simple message.
    /// WHAT: Convenience constructor for single-line data events.
    pub fn message(data: impl Into<String>) -> Self {
        Self {
            id: None,
            event_type: None,
            data: vec![data.into()],
            retry: None,
        }
    }

    /// Create a new [`SseEvent`] builder.
    ///
    /// WHY: Allow building events with all fields (id, event, data, retry).
    /// WHAT: Returns a builder for fluent construction.
    #[must_use]
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> SseEventBuilder {
        SseEventBuilder {
            id: None,
            event_type: None,
            data: Vec::new(),
        }
    }

    /// Create a retry event to set the reconnection interval.
    ///
    /// WHY: Server can control client reconnection timing.
    /// WHAT: Creates an event that sets the retry field on the client.
    #[must_use]
    pub fn retry(milliseconds: u64) -> Self {
        Self {
            id: None,
            event_type: None,
            data: Vec::new(),
            retry: Some(milliseconds),
        }
    }

    /// Get the event ID.
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// Get the event type.
    #[must_use]
    pub fn event_type(&self) -> Option<&str> {
        self.event_type.as_deref()
    }

    /// Get the data lines.
    #[must_use]
    pub fn data_lines(&self) -> &[String] {
        &self.data
    }

    /// Get the retry interval in milliseconds.
    #[must_use]
    pub fn retry_ms(&self) -> Option<u64> {
        self.retry
    }

    /// Append this event's wire bytes to `out`.
    ///
    /// WHY: There is one wire format, and every writer must produce it. The
    /// synchronous [`EventWriter`](crate::event_source::EventWriter) and the
    /// native `foundation_http::native::upgrade::SseWriter` both call this, so
    /// the two paths cannot drift apart.
    ///
    /// WHAT: Appends the optional `event:`, `id:` and `retry:` lines, one
    /// `data:` line per data line, then the empty dispatch line. It writes
    /// exactly the bytes the original `EventWriter::send` wrote.
    ///
    /// NOTE: In-memory writers and tests call this directly. A non-blocking
    /// socket must not be written with this buffer through a synchronous
    /// `Write`: use `foundation_http::native::upgrade::SseWriter`, which parks
    /// on write readiness and resumes a partial write.
    ///
    /// Reference: W3C Server-Sent Events specification
    /// (<https://html.spec.whatwg.org/multipage/server-sent-events.html>).
    pub fn render(&self, out: &mut Vec<u8>) {
        if let Some(event_type) = self.event_type() {
            push_line(out, "event: ", event_type);
        }
        if let Some(id) = self.id() {
            push_line(out, "id: ", id);
        }
        if let Some(retry) = self.retry_ms() {
            // Render the number without allocating an intermediate String.
            out.extend_from_slice(b"retry: ");
            push_u64(out, retry);
            out.push(b'\n');
        }
        for data_line in self.data_lines() {
            push_line(out, "data: ", data_line);
        }
        // Empty line to dispatch the event.
        out.push(b'\n');
    }

    /// Render this event's wire bytes.
    ///
    /// WHY: Convenience for in-memory writers and the async SSE writer, which
    /// needs the whole event before it writes it.
    /// WHAT: Returns the bytes [`Self::render`] appends.
    #[must_use]
    pub fn rendered(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.render(&mut out);
        out
    }
}

/// Append `prefix`, then `value`, then a newline to `out`.
fn push_line(out: &mut Vec<u8>, prefix: &str, value: &str) {
    out.extend_from_slice(prefix.as_bytes());
    out.extend_from_slice(value.as_bytes());
    out.push(b'\n');
}

/// Render a `u64` in decimal without an intermediate allocation.
fn push_u64(out: &mut Vec<u8>, mut value: u64) {
    if value == 0 {
        out.push(b'0');
        return;
    }
    let start = out.len();
    while value > 0 {
        out.push(b'0' + (value % 10) as u8);
        value /= 10;
    }
    out[start..].reverse();
}

impl Default for SseEvent {
    fn default() -> Self {
        Self::new().build()
    }
}

/// Builder for [`SseEvent`].
///
/// WHY: Fluent API for constructing SSE events with multiple fields.
/// WHAT: Allows chaining field setters and building the final event.
pub struct SseEventBuilder {
    id: Option<String>,
    event_type: Option<String>,
    data: Vec<String>,
}

impl SseEventBuilder {
    /// Set the event ID.
    ///
    /// WHY: Clients track event IDs for reconnection resume.
    /// WHAT: Sets the id field that becomes `Last-Event-ID` header on reconnect.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the event type.
    ///
    /// WHY: Allows clients to filter events by type.
    /// WHAT: Sets the event field (default is "message" if not specified).
    #[must_use]
    pub fn event(mut self, event_type: impl Into<String>) -> Self {
        self.event_type = Some(event_type.into());
        self
    }

    /// Add a data line.
    ///
    /// WHY: SSE supports multi-line data (each line prefixed with "data: ").
    /// WHAT: Adds a line to the data field. Multiple calls create multi-line events.
    #[must_use]
    pub fn data(mut self, data: impl Into<String>) -> Self {
        self.data.push(data.into());
        self
    }

    /// Build the [`SseEvent`].
    ///
    /// WHY: Finalize the builder and create the event.
    /// WHAT: Returns the constructed [`SseEvent`] ready for sending.
    #[must_use]
    pub fn build(self) -> SseEvent {
        SseEvent {
            id: self.id,
            event_type: self.event_type,
            data: self.data,
            retry: None,
        }
    }
}
