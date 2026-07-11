//! WebSocket connection and client APIs.
//!
//! WHY: Users need both a low-level `TaskIterator` API and a high-level blocking API
//! for WebSocket communication.
//!
//! WHAT: Provides `WebSocketConnection` (blocking send/recv API) and `WebSocketClient`
//! (consumer wrapper around `TaskIterator` using executor boundary with send capability).
//!
//! HOW: `WebSocketConnection` wraps the shared stream directly for blocking operations.
//! `WebSocketClient` uses `execute_stream()` to integrate with valtron executor and
//! provides `MessageDelivery` for sending messages via `ConcurrentQueue`.

use crate::netcap::RawStream;
use crate::simple_http::client::shared::DnsResolver;
use crate::simple_http::client::HttpConnectionPool;
use crate::simple_http::shared::SimpleHeader;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::{
    execute, BoxedSendExecutionAction, DrivenStreamIterator, Pipe, PipeReceiver, PipeSender,
    Stream, TaskIterator, TaskSpread, TaskStatus,
};
use std::sync::Arc;
use std::time::Duration;

use crate::websocket::native::reconnecting_task::{
    ReconnectingWebSocketProgress, ReconnectingWebSocketTask,
};
use crate::websocket::native::task::{WebSocketProgress, WebSocketTask};
use crate::websocket::shared::batch_writer::BatchFrameWriter;
use crate::websocket::shared::error::WebSocketError;
use crate::websocket::shared::frame::{generate_mask, Opcode, WebSocketFrame};

use crate::websocket::shared::message::WebSocketMessage;

/// WHY: Users need a simple blocking API for WebSocket communication.
///
/// WHAT: High-level WebSocket connection with send/recv/close methods.
///
/// HOW: Wraps the shared stream and manages connection state.
/// Uses `BatchFrameWriter` for efficient frame writing.
pub struct WebSocketConnection {
    stream: SharedByteBufferStream<RawStream>,
    writer: BatchFrameWriter<SharedByteBufferStream<RawStream>>,
    state: ConnectionState,
}

#[allow(dead_code)] // Closed state used in Phase 2
enum ConnectionState {
    Open,
    Closing {
        close_sent: bool,
        close_received: bool,
    },
    Closed,
}

impl WebSocketConnection {
    /// Create a new `WebSocketConnection` from an established stream.
    ///
    /// WHY: After successful handshake, user needs a connection object.
    /// WHAT: Wraps the stream for frame-based communication.
    #[must_use]
    pub fn new(stream: SharedByteBufferStream<RawStream>) -> Self {
        let writer = BatchFrameWriter::with_defaults(stream.clone());
        Self {
            stream,
            writer,
            state: ConnectionState::Open,
        }
    }

    /// Send a WebSocket message.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the message cannot be sent.
    pub fn send(&mut self, message: WebSocketMessage) -> Result<(), WebSocketError> {
        if matches!(self.state, ConnectionState::Closed) {
            return Err(WebSocketError::ConnectionClosed);
        }

        // RFC 6455 §5.3: a client MUST mask every frame it sends. (Lenient
        // servers like Chromium's CDP tolerate unmasked frames; strict ones like
        // Firefox's WebDriver BiDi close the connection — so this is mandatory.)
        let frame = match message {
            WebSocketMessage::ConnectionEstablished => {
                return Ok(()); // No frame to send
            }
            WebSocketMessage::Text(text) => WebSocketFrame {
                fin: true,
                opcode: Opcode::Text,
                mask: Some(generate_mask()),
                payload: text.into_bytes(),
            },
            WebSocketMessage::Binary(data) => WebSocketFrame {
                fin: true,
                opcode: Opcode::Binary,
                mask: Some(generate_mask()),
                payload: data,
            },
            WebSocketMessage::Ping(data) => WebSocketFrame {
                fin: true,
                opcode: Opcode::Ping,
                mask: Some(generate_mask()),
                payload: data,
            },
            WebSocketMessage::Pong(data) => WebSocketFrame {
                fin: true,
                opcode: Opcode::Pong,
                mask: Some(generate_mask()),
                payload: data,
            },
            WebSocketMessage::Close(code, reason) => {
                let mut payload = code.to_be_bytes().to_vec();
                payload.extend_from_slice(reason.as_bytes());
                WebSocketFrame {
                    fin: true,
                    opcode: Opcode::Close,
                    mask: Some(generate_mask()),
                    payload,
                }
            }
        };

        self.send_frame(frame)?;
        // Flush after sending a complete message to ensure timely delivery
        self.writer.flush()
    }

    /// Receive a WebSocket message.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the message cannot be received.
    pub fn recv(&mut self) -> Result<WebSocketMessage, WebSocketError> {
        if matches!(self.state, ConnectionState::Closed) {
            return Err(WebSocketError::ConnectionClosed);
        }

        // Flush any pending frames before reading to ensure queued data is sent
        self.writer.flush()?;

        let frame = WebSocketFrame::decode(&mut self.stream)?;

        // Validate frame
        frame.validate()?;

        // Client expects unmasked frames from server
        if frame.mask.is_some() {
            return Err(WebSocketError::ProtocolError(
                "Received masked frame from server (protocol violation)".to_string(),
            ));
        }

        // Handle control frames immediately
        if frame.opcode.is_control() {
            return self.handle_control_frame(frame);
        }

        // Data frame - for now, only support non-fragmented messages
        if !frame.fin {
            return Err(WebSocketError::ProtocolError(
                "Fragmented messages not yet supported".to_string(),
            ));
        }

        if frame.opcode == Opcode::Continuation {
            return Err(WebSocketError::ProtocolError(
                "Unexpected continuation frame".to_string(),
            ));
        }

        // Convert frame to message
        match frame.opcode {
            Opcode::Text => {
                let text = String::from_utf8(frame.payload).map_err(WebSocketError::InvalidUtf8)?;
                Ok(WebSocketMessage::Text(text))
            }
            Opcode::Binary => Ok(WebSocketMessage::Binary(frame.payload)),
            _ => Err(WebSocketError::ProtocolError(
                "Unexpected data frame opcode".to_string(),
            )),
        }
    }

    fn handle_control_frame(
        &mut self,
        frame: WebSocketFrame,
    ) -> Result<WebSocketMessage, WebSocketError> {
        match frame.opcode {
            Opcode::Ping => {
                // Auto-respond with Pong (same payload). Client frames MUST mask.
                let pong_frame = WebSocketFrame {
                    fin: true,
                    opcode: Opcode::Pong,
                    mask: Some(generate_mask()),
                    payload: frame.payload.clone(),
                };
                self.send_frame(pong_frame)?;
                Ok(WebSocketMessage::Ping(frame.payload))
            }
            Opcode::Pong => Ok(WebSocketMessage::Pong(frame.payload)),
            Opcode::Close => {
                let (code, reason) = parse_close_payload(&frame.payload);

                // Send Close response if we haven't already
                if let ConnectionState::Open = self.state {
                    self.state = ConnectionState::Closing {
                        close_sent: false,
                        close_received: true,
                    };
                    self.close(code, &reason)?;
                }

                Ok(WebSocketMessage::Close(code, reason))
            }
            _ => Err(WebSocketError::ProtocolError(
                "Unknown control frame".to_string(),
            )),
        }
    }

    /// Close the WebSocket connection gracefully.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the close frame cannot be sent.
    pub fn close(&mut self, code: u16, reason: &str) -> Result<(), WebSocketError> {
        match self.state {
            ConnectionState::Closed => return Ok(()),
            ConnectionState::Closing {
                close_sent: true, ..
            } => return Ok(()),
            _ => {}
        }

        // Flush any pending frames before sending close
        self.writer.flush()?;

        let mut payload = code.to_be_bytes().to_vec();
        payload.extend_from_slice(reason.as_bytes());

        let frame = WebSocketFrame {
            fin: true,
            opcode: Opcode::Close,
            mask: Some(generate_mask()),
            payload,
        };
        self.send_frame(frame)?;

        self.state = match self.state {
            ConnectionState::Open => ConnectionState::Closing {
                close_sent: true,
                close_received: false,
            },
            ConnectionState::Closing { close_received, .. } => ConnectionState::Closing {
                close_sent: true,
                close_received,
            },
            ConnectionState::Closed => ConnectionState::Closed,
        };

        Ok(())
    }

    fn send_frame(&mut self, frame: WebSocketFrame) -> Result<(), WebSocketError> {
        // Control frames (Pong, Close) should be sent immediately
        // to avoid buffering delays for time-sensitive responses
        if frame.opcode.is_control() {
            self.writer.write_immediate(frame)
        } else {
            self.writer.queue_frame(frame)
        }
    }

    /// Get an iterator over incoming messages.
    pub fn messages(&mut self) -> ConnectionMessageIterator<'_> {
        ConnectionMessageIterator { conn: self }
    }

    /// Check if the connection is still open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        matches!(self.state, ConnectionState::Open)
    }

    /// Flush any pending frames in the batch writer.
    ///
    /// WHY: Ensures queued frames are immediately transmitted.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the flush fails.
    pub fn flush(&mut self) -> Result<(), WebSocketError> {
        self.writer.flush()
    }

    /// Get writer statistics (if batch writing is enabled).
    #[must_use]
    pub fn writer_stats(&self) -> crate::websocket::BatchWriterStats {
        self.writer.stats()
    }
}

/// Iterator over messages from a `WebSocketConnection`.
pub struct ConnectionMessageIterator<'a> {
    conn: &'a mut WebSocketConnection,
}

impl Iterator for ConnectionMessageIterator<'_> {
    type Item = Result<WebSocketMessage, WebSocketError>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.conn.is_open() {
            return None;
        }
        Some(self.conn.recv())
    }
}

/// Parse close frame payload into status code and reason.
fn parse_close_payload(payload: &[u8]) -> (u16, String) {
    if payload.is_empty() {
        return (1005, String::new()); // No status code present (1005 = No Status Received)
    }
    if payload.len() == 1 {
        return (1002, String::from("Invalid close payload")); // Protocol error
    }

    let code = u16::from_be_bytes([payload[0], payload[1]]);
    let reason = if payload.len() > 2 {
        String::from_utf8_lossy(&payload[2..]).to_string()
    } else {
        String::new()
    };

    (code, reason)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Unified task types: either WebSocketTask or ReconnectingWebSocketTask
// ═══════════════════════════════════════════════════════════════════════════════

/// Unified pending type — wraps the `Pending` associated types of both
/// `WebSocketTask` and `ReconnectingWebSocketTask`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WsPending {
    Task(WebSocketProgress),
    Reconnecting(ReconnectingWebSocketProgress),
}

impl From<WebSocketProgress> for WsPending {
    fn from(p: WebSocketProgress) -> Self {
        WsPending::Task(p)
    }
}

impl From<ReconnectingWebSocketProgress> for WsPending {
    fn from(p: ReconnectingWebSocketProgress) -> Self {
        WsPending::Reconnecting(p)
    }
}

/// Either a single-shot or reconnecting WS task — unified [`TaskIterator`].
pub enum WsTask<R: DnsResolver + Clone + Send + 'static> {
    Single(WebSocketTask<R>),
    Reconnecting(ReconnectingWebSocketTask<R>),
}

impl<R> TaskIterator for WsTask<R>
where
    R: DnsResolver + Clone + Send + 'static,
{
    type Ready = Result<WebSocketMessage, WebSocketError>;
    type Pending = WsPending;
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self {
            WsTask::Single(t) => t
                .next_status()
                .map(|s| remap_task_status(s, WsPending::Task)),
            WsTask::Reconnecting(t) => t
                .next_status()
                .map(|s| remap_task_status(s, WsPending::Reconnecting)),
        }
    }
}

/// Remap the `Pending` variant of a `TaskSpread`.
fn remap_spread<D, P1, P2>(sp: TaskSpread<D, P1>, f: fn(P1) -> P2) -> TaskSpread<D, P2> {
    match sp {
        TaskSpread::Pending(p) => TaskSpread::Pending(f(p)),
        TaskSpread::Ready(d) => TaskSpread::Ready(d),
    }
}

/// Remap the `Pending` type of a `TaskStatus`.
fn remap_task_status<D, P1, P2>(
    s: TaskStatus<D, P1, BoxedSendExecutionAction>,
    f: fn(P1) -> P2,
) -> TaskStatus<D, P2, BoxedSendExecutionAction> {
    match s {
        TaskStatus::Ready(r) => TaskStatus::Ready(r),
        TaskStatus::Pending(p) => TaskStatus::Pending(f(p)),
        TaskStatus::Delayed(d) => TaskStatus::Delayed(d),
        TaskStatus::Init => TaskStatus::Init,
        TaskStatus::Ignore => TaskStatus::Ignore,
        TaskStatus::Wait => TaskStatus::Wait,
        TaskStatus::Spawn(a) => TaskStatus::Spawn(a),
        TaskStatus::Spread(items) => {
            TaskStatus::Spread(items.into_iter().map(|sp| remap_spread(sp, f)).collect())
        }
        TaskStatus::Depends(r) => TaskStatus::Depends(r),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════

/// Whether to use reconnecting or single-shot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reconnect {
    No,
    Yes,
}

// ============== WebSocketClient (Executor-based) ==============

/// WHY: Users need a send-capable WebSocket client that integrates with valtron executor.
///
/// WHAT: `MessageDelivery` provides message sending via `PipeSender` — the
/// bounded, waker-hooked replacement for `ConcurrentQueue`. Same public API,
/// now with executor-integrated wake and backpressure.
///
/// HOW: Wraps `PipeSender<WebSocketMessage>` — cloned for cheap sharing.
#[derive(Clone)]
pub struct MessageDelivery {
    tx: PipeSender<WebSocketMessage>,
}

impl MessageDelivery {
    /// Create a new `MessageDelivery` with a bounded pipe (depth 64).
    ///
    /// Returns both the delivery handle and the receiver half the task drains.
    #[must_use]
    pub fn new() -> (Self, PipeReceiver<WebSocketMessage>) {
        let (tx, rx) = Pipe::with_depth(64);
        (Self { tx }, rx)
    }

    /// Wrap an existing `PipeSender`. The caller already holds the matching receiver.
    #[must_use]
    pub fn from_pipe(tx: PipeSender<WebSocketMessage>) -> Self {
        Self { tx }
    }

    /// Send a WebSocket message (non-blocking — returns `Full` if the pipe is at capacity).
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError::ConnectionClosed`] if the receiver end has been dropped.
    pub fn send(&self, message: WebSocketMessage) -> Result<(), WebSocketError> {
        self.tx
            .try_send(message)
            .map_err(|_| WebSocketError::ConnectionClosed)?;
        Ok(())
    }

    /// Send a Ping message.
    pub fn ping(&self, data: Vec<u8>) -> Result<(), WebSocketError> {
        self.send(WebSocketMessage::Ping(data))
    }

    /// Send a Pong message.
    pub fn pong(&self, data: Vec<u8>) -> Result<(), WebSocketError> {
        self.send(WebSocketMessage::Pong(data))
    }

    /// Send a Close message.
    pub fn close(&self, code: u16, reason: &str) -> Result<(), WebSocketError> {
        self.send(WebSocketMessage::Close(code, reason.to_string()))
    }

    /// Get the underlying `PipeSender` for Transport bridging.
    #[must_use]
    pub fn pipe(&self) -> &PipeSender<WebSocketMessage> {
        &self.tx
    }

    /// Consume and return the underlying `PipeSender`.
    #[must_use]
    pub fn into_pipe(self) -> PipeSender<WebSocketMessage> {
        self.tx
    }

    /// Deprecated — kept for transition. Returns a reference to the pipe sender
    /// (use `pipe()` for the same access).
    #[must_use]
    #[deprecated(note = "use pipe() instead")]
    pub fn queue(&self) -> &PipeSender<WebSocketMessage> {
        &self.tx
    }
}

/// WebSocket event for the client API.
///
/// WHY: Users need to know when the stream is still working vs. when an actual
/// event is available.
///
/// WHAT: Enum with `Message` variant containing the actual message and `Skip` variant
/// for pending/delayed states.
#[derive(Debug, Clone)]
pub enum WebSocketEvent {
    /// A WebSocket message is available.
    Message(WebSocketMessage),
    /// Stream is still working, no message yet. User should call `next()` again.
    Skip,
}

/// A WebSocket client that uses the valtron executor.
///
/// WHY: Users want to consume WebSocket messages without understanding `TaskIterator` internals.
///
/// WHAT: Wraps the executor's stream and presents a simple iterator interface.
/// Includes `MessageDelivery` for sending messages. Can optionally use
/// [`ReconnectingWebSocketTask`] via [`Reconnect::Yes`].
pub struct WebSocketClient<R: DnsResolver + Clone + Send + 'static> {
    inner: DrivenStreamIterator<WsTask<R>>,
    delivery: MessageDelivery,
    #[allow(dead_code)]
    read_timeout: Duration,
}

impl<R: DnsResolver + Clone + Send + 'static> WebSocketClient<R> {
    /// Connect to a WebSocket endpoint.
    ///
    /// Returns both the client and a `MessageDelivery` handle for sending messages.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if:
    /// - URL is invalid
    /// - Executor fails to schedule the task
    pub fn connect(
        resolver: R,
        url: impl Into<String>,
        read_timeout: Duration,
        sleep_timeout: Duration,
    ) -> Result<(Self, MessageDelivery), WebSocketError> {
        Self::with_options(resolver, url, None, Vec::new(), read_timeout, sleep_timeout)
    }

    /// Connect to a WebSocket endpoint with custom options.
    ///
    /// # Arguments
    ///
    /// * `resolver` - DNS resolver for hostname resolution
    /// * `url` - WebSocket URL (ws:// or wss://)
    /// * `subprotocols` - Optional comma-separated list of subprotocols
    /// * `extra_headers` - Additional HTTP headers for handshake
    /// * `read_timeout` - Timeout for read operations
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if:
    /// - URL is invalid
    /// - Executor fails to schedule the task
    #[tracing::instrument(name = "websocket_connect", skip(resolver, extra_headers), fields(url))]
    pub fn with_options(
        resolver: R,
        url: impl Into<String>,
        subprotocols: Option<String>,
        extra_headers: Vec<(SimpleHeader, String)>,
        read_timeout: Duration,
        sleep_timeout: Duration,
    ) -> Result<(Self, MessageDelivery), WebSocketError> {
        let url_str = url.into();
        let (delivery, rx) = MessageDelivery::new();
        let task = WebSocketTask::connect_with_delivery(
            resolver,
            url_str,
            subprotocols,
            extra_headers,
            rx,
            read_timeout,
            sleep_timeout,
        )?;
        let inner = execute(WsTask::Single(task), None)
            .map_err(|e| WebSocketError::ProtocolError(format!("Executor error: {e}")))?;
        let client = Self {
            inner,
            delivery: delivery.clone(),
            read_timeout,
        };
        Ok((client, delivery))
    }

    /// Connect with optional reconnection.
    ///
    /// When `Reconnect::Yes`, uses `ReconnectingWebSocketTask` which handles
    /// disconnect detection and exponential backoff transparently.
    pub fn connect_with_reconnect(
        resolver: R,
        url: impl Into<String>,
        reconnect: Reconnect,
        read_timeout: Duration,
        sleep_timeout: Duration,
    ) -> Result<(Self, MessageDelivery), WebSocketError> {
        let url_str = url.into();
        let (delivery, rx) = MessageDelivery::new();

        let task = match reconnect {
            Reconnect::No => WsTask::Single(WebSocketTask::connect_with_delivery(
                resolver.clone(),
                url_str.clone(),
                None,
                Vec::new(),
                rx,
                read_timeout,
                sleep_timeout,
            )?),
            Reconnect::Yes => {
                WsTask::Reconnecting(ReconnectingWebSocketTask::connect(resolver, &url_str)?)
            }
        };

        let inner = execute(task, None)
            .map_err(|e| WebSocketError::ProtocolError(format!("Executor error: {e}")))?;
        Ok((
            Self {
                inner,
                delivery: delivery.clone(),
                read_timeout,
            },
            delivery,
        ))
    }

    /// Connect with byte pipes — for Transport use.
    ///
    /// Returns the three parts the caller needs:
    /// - `WsTask<R>` — spawned on the valtron pool. Use `execute()` to drive it.
    /// - `MessageDelivery` — push `Binary(bytes)` to send data to the task.
    /// - `PipeReceiver<Bytes>` — the task receives inbound bytes here.
    ///
    /// The caller spawns bridge tasks: one that wraps `Bytes → Binary → delivery`,
    /// one that drains the task stream → `Binary → Bytes → body_tx`.
    /// See `WsTransport::open()` for the canonical wiring.
    pub fn connect_parts(
        resolver: R,
        url: impl Into<String>,
        reconnect: Reconnect,
        read_timeout: Duration,
        sleep_timeout: Duration,
    ) -> Result<(WsTask<R>, MessageDelivery), WebSocketError> {
        let url_str = url.into();
        let (delivery, msg_rx) = MessageDelivery::new();

        let task = match reconnect {
            Reconnect::No => WsTask::Single(WebSocketTask::connect_with_delivery(
                resolver.clone(),
                url_str.clone(),
                None,
                Vec::new(),
                msg_rx,
                read_timeout,
                sleep_timeout,
            )?),
            Reconnect::Yes => {
                WsTask::Reconnecting(ReconnectingWebSocketTask::connect(resolver, &url_str)?)
            }
        };

        Ok((task, delivery))
    }

    /// Connect using an existing connection pool.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if:
    /// - URL is invalid
    /// - Executor fails to schedule the task
    pub fn with_pool(
        url: impl Into<String>,
        pool: Arc<HttpConnectionPool<R>>,
    ) -> Result<(Self, MessageDelivery), WebSocketError> {
        Self::with_pool_and_options(
            url,
            pool,
            None,
            Vec::new(),
            Duration::from_secs(3),
            Duration::from_secs(1),
        )
    }

    /// Connect using an existing connection pool with custom options.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if:
    /// - URL is invalid
    /// - Executor fails to schedule the task
    pub fn with_pool_and_options(
        url: impl Into<String>,
        pool: Arc<HttpConnectionPool<R>>,
        subprotocols: Option<String>,
        extra_headers: Vec<(SimpleHeader, String)>,
        read_timeout: Duration,
        sleep_timeout: Duration,
    ) -> Result<(Self, MessageDelivery), WebSocketError> {
        let url_str = url.into();
        let (delivery, rx) = MessageDelivery::new();
        let task = WebSocketTask::connect_with_pool_and_delivery(
            url_str,
            pool,
            subprotocols,
            extra_headers,
            rx,
            read_timeout,
            sleep_timeout,
        )?;
        let inner = execute(WsTask::Single(task), None)
            .map_err(|e| WebSocketError::ProtocolError(format!("Executor error: {e}")))?;
        let client = Self {
            inner,
            delivery: delivery.clone(),
            read_timeout,
        };
        Ok((client, delivery))
    }

    /// Get the message delivery handle for sending messages.
    #[must_use]
    pub fn delivery(&self) -> MessageDelivery {
        self.delivery.clone()
    }

    /// Consume the client, returning the inner stream for direct iteration.
    #[must_use]
    pub fn into_parts(self) -> (DrivenStreamIterator<WsTask<R>>, MessageDelivery) {
        (self.inner, self.delivery)
    }

    /// Get an iterator over incoming messages.
    pub fn messages(&mut self) -> WebSocketMessageIterator<'_, R> {
        WebSocketMessageIterator { client: self }
    }
}

/// Iterator over messages from a `WebSocketClient`.
// `+ Clone` is explicit now that `DnsResolver` no longer requires it (F51 Stage 1);
// `WebSocketClient<R>` clones the resolver. Folded into the unified client in Stage 6.
pub struct WebSocketMessageIterator<'a, R: DnsResolver + Clone + Send + 'static> {
    client: &'a mut WebSocketClient<R>,
}

impl<R: DnsResolver + Clone + Send + 'static> Iterator for WebSocketMessageIterator<'_, R> {
    type Item = Result<WebSocketEvent, WebSocketError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.client.inner.next()? {
            Stream::Next(result) => {
                tracing::debug!("Stream got Next value");
                // Skip ConnectionEstablished message, return actual messages
                match result {
                    Ok(WebSocketMessage::ConnectionEstablished) => Some(Ok(WebSocketEvent::Skip)),
                    other => Some(other.map(WebSocketEvent::Message)),
                }
            }
            Stream::Init
            | Stream::Ignore
            | Stream::Pending(_)
            | Stream::Delayed(_)
            | Stream::Wait
            | Stream::Spread(_) => {
                tracing::debug!("Stream got Init/Ignore/Pending/Delayed/Wait/Spread to be skipped");
                Some(Ok(WebSocketEvent::Skip))
            }
        }
    }
}
