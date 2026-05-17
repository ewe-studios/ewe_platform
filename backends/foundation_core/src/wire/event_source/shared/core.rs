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
}

impl Default for SseEvent {
    fn default() -> Self {
        SseEvent::new().build()
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
