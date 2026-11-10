//! Protocol upgrade helpers — WebSocket accept and SSE streaming.
//!
//! SSE client types are provided by `foundation_netio::event_source`.
//! Server-side SSE streaming is provided by `SseStream`.

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_netio::event_source::{render_comment, SseEvent};
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::{
    SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleOutgoingResponse,
    Status,
};

use crate::native::writer::{render_message, ResponseWriter, WriterConfig};

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

/// Accept a WebSocket upgrade and return `true` when the `101` is written.
///
/// The `101 Switching Protocols` response goes out through the async
/// [`ResponseWriter`], so it cannot truncate on a non-blocking socket. The
/// caller keeps its own clone of `conn` for the frames that follow.
pub async fn accept_websocket(
    conn: &SharedByteBufferStream<RawStream>,
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

    let response = match SimpleOutgoingResponse::builder()
        .with_status(Status::SwitchingProtocols)
        .add_header(SimpleHeader::UPGRADE, "websocket")
        .add_header(SimpleHeader::CONNECTION, "Upgrade")
        .add_header(SimpleHeader::custom("Sec-WebSocket-Accept"), accept_key)
        .with_body(SendSafeBody::None)
        .build()
    {
        Ok(response) => response,
        Err(e) => {
            tracing::error!("Failed to build the WebSocket upgrade response: {e}");
            return false;
        }
    };

    let head = match render_message(response) {
        Ok(head) => head,
        Err(e) => {
            tracing::error!("Failed to render WebSocket upgrade response: {e}");
            return false;
        }
    };
    let mut writer = ResponseWriter::new(
        conn.clone(),
        WriterConfig::default(),
        foundation_core::valtron::fork_ctx(),
    );
    let result = async {
        writer.write_all(&head).await?;
        writer.flush().await
    }
    .await;
    match result {
        Ok(()) => true,
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

/// Server-side SSE event writer over the async [`ResponseWriter`].
///
/// WHY: An SSE event is formatted bytes. The connection socket is
/// non-blocking, so a synchronous `Write` can stop in the middle of an event,
/// return `WouldBlock`, and lose the rest of it ([`crate::native::upgrade::SseStream`]
/// had that bug). `ResponseWriter` parks on write readiness and resumes at the
/// byte offset the socket accepted.
/// WHAT: Renders each event with [`SseEvent::render`] (the one wire format),
/// writes the bytes with [`ResponseWriter::write_all`], then flushes.
/// HOW: [`SseStream`] shares one `ResponseWriter` between the response head and
/// every later event, so a parked head write and an event write use the same
/// readiness state.
pub struct SseWriter {
    writer: ResponseWriter,
}

impl SseWriter {
    /// A writer for `conn` with the given limits.
    #[must_use]
    pub fn new(conn: SharedByteBufferStream<RawStream>, config: WriterConfig) -> Self {
        Self {
            writer: ResponseWriter::new(conn, config, foundation_core::valtron::fork_ctx()),
        }
    }

    /// A writer over an existing [`ResponseWriter`].
    ///
    /// WHY: [`SseStream::new`] writes the response head first; the event
    /// writes must keep that writer so the idle timer and readiness state are
    /// shared.
    /// WHAT: Wraps `writer` without touching the connection.
    #[must_use]
    pub fn from_writer(writer: ResponseWriter) -> Self {
        Self { writer }
    }

    /// Send an SSE event to the client.
    ///
    /// # Errors
    ///
    /// Returns [`UpgradeError::WriteError`] when the connection fails or the
    /// client takes no byte within the writer's `send_timeout`.
    pub async fn send(&mut self, event: &SseEvent) -> Result<(), UpgradeError> {
        self.write_bytes(&event.rendered()).await
    }

    /// Send a comment (keep-alive) to the client.
    ///
    /// # Errors
    ///
    /// As [`Self::send`].
    pub async fn comment(&mut self, text: &str) -> Result<(), UpgradeError> {
        let mut bytes = Vec::new();
        render_comment(text, &mut bytes);
        self.write_bytes(&bytes).await
    }

    /// Send a simple message event (no event type).
    ///
    /// # Errors
    ///
    /// As [`Self::send`].
    pub async fn message(&mut self, data: impl Into<String>) -> Result<(), UpgradeError> {
        self.send(&SseEvent::message(data)).await
    }

    /// Get a reference to the underlying [`ResponseWriter`].
    #[must_use]
    pub fn writer(&self) -> &ResponseWriter {
        &self.writer
    }

    /// Get a mutable reference to the underlying [`ResponseWriter`].
    #[must_use]
    pub fn writer_mut(&mut self) -> &mut ResponseWriter {
        &mut self.writer
    }

    /// Write `bytes` whole, then flush.
    async fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), UpgradeError> {
        self.writer
            .write_all(bytes)
            .await
            .map_err(|e| UpgradeError::WriteError(e.to_string()))?;
        self.writer
            .flush()
            .await
            .map_err(|e| UpgradeError::WriteError(e.to_string()))
    }
}

/// Server-side SSE stream for writing events to clients.
///
/// WHY: SSE requires specific HTTP headers before streaming events.
/// WHAT: Writes SSE headers (200 OK, text/event-stream), then keeps the
/// [`ResponseWriter`] in an [`SseWriter`] for the events.
///
/// NOTE: The handler that creates the stream must keep ownership of the
/// connection (`ConnectionResult::Take`) or drive an async continuation, as
/// `foundation_browser`'s `StreamHandler` does.
pub struct SseStream {
    writer: SseWriter,
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
    /// WHAT: Writes the head through the async [`ResponseWriter`], then keeps
    /// that writer in an [`SseWriter`] for the events.
    ///
    /// # Errors
    ///
    /// Returns `UpgradeError` if header writing fails.
    pub async fn new(conn: &SharedByteBufferStream<RawStream>) -> Result<Self, UpgradeError> {
        let response = SimpleOutgoingResponse::builder()
            .with_status(Status::OK)
            .add_header(SimpleHeader::CONTENT_TYPE, "text/event-stream")
            .add_header(SimpleHeader::CACHE_CONTROL, "no-cache")
            .add_header(SimpleHeader::CONNECTION, "keep-alive")
            .with_body(SendSafeBody::None)
            .build()
            .map_err(|e| UpgradeError::InvalidRequest(e.to_string()))?;

        let head = render_message(response).map_err(UpgradeError::WriteError)?;

        let mut writer = ResponseWriter::new(
            conn.clone(),
            WriterConfig::default(),
            foundation_core::valtron::fork_ctx(),
        );
        writer
            .write_all(&head)
            .await
            .map_err(|e| UpgradeError::WriteError(e.to_string()))?;
        // Flush headers immediately so the client receives them.
        writer
            .flush()
            .await
            .map_err(|e| UpgradeError::WriteError(e.to_string()))?;

        // Keep the same writer: the head and the events share its idle timer
        // and its write-readiness registration.
        Ok(Self {
            writer: SseWriter::from_writer(writer),
        })
    }

    /// Send an SSE event to the client.
    ///
    /// WHY: Primary method for transmitting events.
    /// WHAT: Renders and writes the event through [`SseWriter`].
    ///
    /// # Errors
    ///
    /// Returns `UpgradeError` if writing fails.
    pub async fn send(&mut self, event: &SseEvent) -> Result<(), UpgradeError> {
        self.writer.send(event).await
    }

    /// Send a simple message event (no event type).
    ///
    /// WHY: Convenience for simple SSE streaming.
    /// WHAT: Creates and sends a message event.
    ///
    /// # Errors
    ///
    /// Returns `UpgradeError` if writing fails.
    pub async fn message(&mut self, data: impl Into<String>) -> Result<(), UpgradeError> {
        self.writer.message(data).await
    }

    /// Send a BINARY frame as an SSE message — base64-encoded.
    ///
    /// WHY: SSE `data:` lines are text (UTF-8, line-delimited), so raw binary
    /// protocol frames (NUL bytes, embedded newlines) can't ride them directly.
    /// This is the SERVER half of the binary-over-SSE contract: the browser
    /// decodes with `atob`. The frame stays self-describing — an envelope
    /// `[protocol][version][length][..]` carries its own type, so no SSE
    /// `event:`/headers are needed; the receiver reads the envelope header.
    /// WHAT: base64-encodes `frame` and writes it as a `data:` message event.
    ///
    /// # Errors
    ///
    /// Returns `UpgradeError` if writing fails.
    pub async fn binary(&mut self, frame: &[u8]) -> Result<(), UpgradeError> {
        use base64::Engine as _;
        let encoded = base64::engine::general_purpose::STANDARD.encode(frame);
        self.message(encoded).await
    }

    /// Send a comment (keep-alive).
    ///
    /// WHY: Comments keep connection alive during idle periods.
    /// WHAT: Writes a comment line starting with ":".
    ///
    /// # Errors
    ///
    /// Returns `UpgradeError` if writing fails.
    pub async fn comment(&mut self, comment: &str) -> Result<(), UpgradeError> {
        self.writer.comment(comment).await
    }

    /// Get a reference to the underlying [`SseWriter`].
    ///
    /// WHY: May need direct access to the writer for advanced use.
    /// WHAT: Returns immutable reference to the writer.
    #[must_use]
    pub fn writer(&self) -> &SseWriter {
        &self.writer
    }

    /// Get a mutable reference to the underlying [`SseWriter`].
    ///
    /// WHY: May need mutable access for custom operations.
    /// WHAT: Returns mutable reference to the writer.
    #[must_use]
    pub fn writer_mut(&mut self) -> &mut SseWriter {
        &mut self.writer
    }
}
