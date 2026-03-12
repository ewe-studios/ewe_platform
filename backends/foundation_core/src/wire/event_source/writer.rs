//! Server-side SSE event writer.
//!
//! WHY: Servers need to format SSE events according to W3C specification.
//! WHAT: [`EventWriter`] formats and writes SSE events to output streams.
//!
//! Reference: W3C Server-Sent Events specification (<https://html.spec.whatwg.org/multipage/server-sent-events.html>)

use crate::wire::event_source::SseEvent;
use std::io::Write;

/// [`EventWriter`] formats and writes SSE events to an output stream.
///
/// WHY: Server needs to send properly formatted SSE events to clients.
/// WHAT: Wraps a Write stream and provides methods to send events.
pub struct EventWriter<W>
where
    W: Write,
{
    writer: W,
}

impl<W> EventWriter<W>
where
    W: Write,
{
    /// Create a new [`EventWriter`] wrapping the given writer.
    ///
    /// WHY: Need to wrap any Write stream for SSE event output.
    /// WHAT: Takes ownership of writer and returns [`EventWriter`].
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Send an SSE event to the client.
    ///
    /// WHY: Primary method for transmitting events.
    /// WHAT: Formats event according to SSE spec and writes to stream.
    ///
    /// # Format
    /// ```text
    /// event: <event_type>\n
    /// id: <id>\n
    /// data: <data_line_1>\n
    /// data: <data_line_2>\n
    /// retry: <retry>\n
    /// \n
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`std::io::Error`] if writing to the stream fails.
    pub fn send(&mut self, event: &SseEvent) -> Result<(), std::io::Error> {
        // Write event type if present
        if let Some(event_type) = event.event_type() {
            writeln!(self.writer, "event: {event_type}")?;
        }

        // Write ID if present
        if let Some(id) = event.id() {
            writeln!(self.writer, "id: {id}")?;
        }

        // Write retry if present
        if let Some(retry) = event.retry_ms() {
            writeln!(self.writer, "retry: {retry}")?;
        }

        // Write data lines (each prefixed with "data: ")
        for data_line in event.data_lines() {
            writeln!(self.writer, "data: {data_line}")?;
        }

        // Empty line to dispatch event
        writeln!(self.writer)?;

        // Flush to ensure immediate delivery
        self.writer.flush()?;

        Ok(())
    }

    /// Send a comment (keep-alive) to the client.
    ///
    /// WHY: Comments are used for keep-alive and debugging.
    /// WHAT: Writes a comment line starting with ":".
    ///
    /// # Errors
    ///
    /// Returns [`std::io::Error`] if writing to the stream fails.
    pub fn comment(&mut self, comment: &str) -> Result<(), std::io::Error> {
        writeln!(self.writer, ": {comment}")?;
        self.writer.flush()?;
        Ok(())
    }

    /// Send a simple message without event type.
    ///
    /// WHY: Convenience method for simple messages.
    /// WHAT: Creates and sends [`SseEvent::message()`].
    ///
    /// # Errors
    ///
    /// Returns [`std::io::Error`] if writing to the stream fails.
    pub fn message(&mut self, data: impl Into<String>) -> Result<(), std::io::Error> {
        self.send(&SseEvent::message(data))
    }

    /// Unwrap the [`EventWriter`] and return the underlying writer.
    ///
    /// WHY: Caller may need to access the underlying stream.
    /// WHAT: Consumes self and returns the wrapped writer.
    #[must_use]
    pub fn into_inner(self) -> W {
        self.writer
    }

    /// Get a reference to the underlying writer.
    ///
    /// WHY: May need to inspect or use the writer directly.
    /// WHAT: Returns immutable reference to wrapped writer.
    #[must_use]
    pub fn get_ref(&self) -> &W {
        &self.writer
    }

    /// Get a mutable reference to the underlying writer.
    ///
    /// WHY: May need to modify or use the writer directly.
    /// WHAT: Returns mutable reference to wrapped writer.
    #[must_use]
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.writer
    }
}
