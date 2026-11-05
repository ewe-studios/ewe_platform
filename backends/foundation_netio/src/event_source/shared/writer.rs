//! Server-side SSE event writer.
//!
//! WHY: Servers need to format SSE events according to W3C specification.
//! WHAT: [`EventWriter`] formats and writes SSE events to output streams.
//!
//! NOTE: [`EventWriter`] is synchronous (`W: Write`). It is for in-memory
//! writers and tests. A non-blocking socket must use
//! `foundation_http::native::upgrade::SseWriter`, which writes the same bytes
//! through `ResponseWriter` and parks on write readiness instead of truncating
//! an event with `WouldBlock`.
//!
//! Reference: W3C Server-Sent Events specification (<https://html.spec.whatwg.org/multipage/server-sent-events.html>)

use crate::event_source::SseEvent;
use std::io::Write;

/// Append a comment line (`: <text>\n`) to `out`.
///
/// WHY: Comments (keep-alive) are part of the SSE wire format too. The
/// synchronous writer and the async native writer must produce the same line.
/// WHAT: Appends `": "`, the text, and a newline.
pub fn render_comment(text: &str, out: &mut Vec<u8>) {
    out.extend_from_slice(b": ");
    out.extend_from_slice(text.as_bytes());
    out.push(b'\n');
}

/// [`EventWriter`] formats and writes SSE events to an output stream.
///
/// WHY: Server needs to send properly formatted SSE events to clients.
/// WHAT: Wraps a Write stream and provides methods to send events.
///
/// NOTE: Synchronous. Do not wrap a non-blocking socket in this writer; use the
/// async `SseWriter` in `foundation_http` for that.
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
    /// WHAT: Renders the event with [`SseEvent::render`] and writes the bytes
    /// to the stream, then flushes.
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
        let mut bytes = Vec::new();
        event.render(&mut bytes);
        self.writer.write_all(&bytes)?;
        // Flush to ensure immediate delivery.
        self.writer.flush()
    }

    /// Send a comment (keep-alive) to the client.
    ///
    /// WHY: Comments are used for keep-alive and debugging.
    /// WHAT: Renders the comment with [`render_comment`] and writes it.
    ///
    /// # Errors
    ///
    /// Returns [`std::io::Error`] if writing to the stream fails.
    pub fn comment(&mut self, comment: &str) -> Result<(), std::io::Error> {
        let mut bytes = Vec::new();
        render_comment(comment, &mut bytes);
        self.writer.write_all(&bytes)?;
        self.writer.flush()
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
