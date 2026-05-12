//! Protocol upgrade helpers — WebSocket accept and SSE streaming.
//!
//! SSE client types are provided by `foundation_core::wire::event_source`.
//! Server-side SSE streaming is provided by `SseStream`.

use std::io::Write;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::netcap::RawStream;
use foundation_core::wire::event_source::{EventWriter, SseEvent};
use foundation_core::wire::simple_http::{
    Http11, RenderHttp, SimpleHeader, SimpleHeaders, SimpleIncomingRequest,
    SimpleOutgoingResponse, SendSafeBody, Status,
};

/// Error type for upgrade operations.
#[derive(Debug)]
pub enum UpgradeError {
    /// Failed to write response headers.
    WriteError(String),
    /// Invalid request or state for upgrade.
    InvalidRequest(String),
}

impl std::fmt::Display for UpgradeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpgradeError::WriteError(msg) => write!(f, "write error: {msg}"),
            UpgradeError::InvalidRequest(msg) => write!(f, "invalid request: {msg}"),
        }
    }
}

impl std::error::Error for UpgradeError {}

impl From<std::io::Error> for UpgradeError {
    fn from(err: std::io::Error) -> Self {
        UpgradeError::WriteError(err.to_string())
    }
}

/// Accept a WebSocket upgrade and return the raw connection stream.
///
/// Writes the 101 Switching Protocols response to the connection.
/// Returns `true` if the upgrade was accepted, `false` if the request
/// was not a valid WebSocket upgrade.
///
/// # Panics
///
/// Panics if the 101 response cannot be built (this should never happen
/// with valid input).
pub fn accept_websocket(
    conn: &mut SharedByteBufferStream<RawStream>,
    req: &SimpleIncomingRequest,
) -> bool {
    let upgrade = get_header_value(&req.headers, "upgrade");
    let connection = get_header_value(&req.headers, "connection");
    let ws_key = get_header_value(&req.headers, "sec-websocket-key");

    if !upgrade.eq_ignore_ascii_case("websocket")
        || !connection.to_lowercase().contains("upgrade")
        || ws_key.is_empty()
    {
        return false;
    }

    let accept_key = {
        use base64::{engine::general_purpose::STANDARD, Engine};
        use sha1::{Digest, Sha1};
        let mut h = Sha1::new();
        h.update(ws_key.as_bytes());
        h.update(b"258EAFA5-E914-47DA-95CA-5AB3977F1E39");
        let result = h.finalize();
        STANDARD.encode(result)
    };

    let response = SimpleOutgoingResponse::builder()
        .with_status(Status::SwitchingProtocols)
        .add_header(SimpleHeader::UPGRADE, "websocket")
        .add_header(SimpleHeader::CONNECTION, "Upgrade")
        .add_header(SimpleHeader::custom("Sec-WebSocket-Accept"), accept_key)
        .with_body(SendSafeBody::None)
        .build()
        .expect("valid 101 response");

    match Http11::response(response).http_render_to_writer(conn) {
        Ok(_) => true,
        Err(e) => {
            tracing::error!("Failed to write WebSocket upgrade response: {e}");
            false
        }
    }
}

fn get_header_value(headers: &SimpleHeaders, name: &str) -> String {
    for (key, values) in headers {
        let key_name = format!("{key}");
        if key_name.eq_ignore_ascii_case(name) {
            return values.first().cloned().unwrap_or_default();
        }
    }
    String::new()
}

/// Server-side SSE stream for writing events to clients.
///
/// WHY: SSE requires specific HTTP headers before streaming events.
/// WHAT: Writes SSE headers (200 OK, text/event-stream), then wraps the stream
/// in an EventWriter for sending events.
///
/// NOTE: The handler must return `ConnectionResult::Take` after creating
/// the SseStream to take ownership of the connection.
pub struct SseStream {
    writer: EventWriter<SharedByteBufferStream<RawStream>>,
}

impl SseStream {
    /// Create a new SSE stream by writing headers and wrapping the connection.
    ///
    /// WHY: SSE requires specific headers before streaming:
    /// - 200 OK status
    /// - Content-Type: text/event-stream
    /// - Cache-Control: no-cache
    /// - Connection: keep-alive
    ///
    /// WHAT: Writes headers to the stream, then creates EventWriter wrapper.
    ///
    /// # Errors
    ///
    /// Returns `UpgradeError` if header writing fails.
    pub fn new(
        conn: &mut SharedByteBufferStream<RawStream>,
    ) -> Result<Self, UpgradeError> {
        let response = SimpleOutgoingResponse::builder()
            .with_status(Status::OK)
            .add_header(SimpleHeader::CONTENT_TYPE, "text/event-stream")
            .add_header(SimpleHeader::CACHE_CONTROL, "no-cache")
            .add_header(SimpleHeader::CONNECTION, "keep-alive")
            .with_body(SendSafeBody::None)
            .build()
            .map_err(|e| UpgradeError::InvalidRequest(e.to_string()))?;

        Http11::response(response)
            .http_render_to_writer(conn)
            .map_err(|e| UpgradeError::WriteError(e.to_string()))?;

        // Flush headers immediately so client receives them
        conn.flush()
            .map_err(|e| UpgradeError::WriteError(e.to_string()))?;

        // Clone the stream for the EventWriter
        // EventWriter needs its own reference to write events
        let stream_clone = conn.clone();
        let writer = EventWriter::new(stream_clone);

        Ok(Self { writer })
    }

    /// Send an SSE event to the client.
    ///
    /// WHY: Primary method for transmitting events.
    /// WHAT: Formats and writes event according to SSE spec.
    ///
    /// # Errors
    ///
    /// Returns `UpgradeError` if writing fails.
    pub fn send(&mut self, event: &SseEvent) -> Result<(), UpgradeError> {
        self.writer
            .send(event)
            .map_err(|e| UpgradeError::WriteError(e.to_string()))
    }

    /// Send a simple message event (no event type).
    ///
    /// WHY: Convenience for simple SSE streaming.
    /// WHAT: Creates and sends a message event.
    ///
    /// # Errors
    ///
    /// Returns `UpgradeError` if writing fails.
    pub fn message(&mut self, data: impl Into<String>) -> Result<(), UpgradeError> {
        self.writer
            .message(data)
            .map_err(|e| UpgradeError::WriteError(e.to_string()))
    }

    /// Send a comment (keep-alive).
    ///
    /// WHY: Comments keep connection alive during idle periods.
    /// WHAT: Writes a comment line starting with ":".
    ///
    /// # Errors
    ///
    /// Returns `UpgradeError` if writing fails.
    pub fn comment(&mut self, comment: &str) -> Result<(), UpgradeError> {
        self.writer
            .comment(comment)
            .map_err(|e| UpgradeError::WriteError(e.to_string()))
    }

    /// Get a reference to the underlying EventWriter.
    ///
    /// WHY: May need direct access to EventWriter for advanced use.
    /// WHAT: Returns immutable reference to the writer.
    #[must_use]
    pub fn writer(&self) -> &EventWriter<SharedByteBufferStream<RawStream>> {
        &self.writer
    }

    /// Get a mutable reference to the underlying EventWriter.
    ///
    /// WHY: May need mutable access for custom operations.
    /// WHAT: Returns mutable reference to the writer.
    #[must_use]
    pub fn writer_mut(&mut self) -> &mut EventWriter<SharedByteBufferStream<RawStream>> {
        &mut self.writer
    }
}
