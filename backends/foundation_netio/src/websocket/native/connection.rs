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

use crate::http::HttpConnectionPool;
use crate::netcap::RawStream;
use crate::shared::client::DnsResolver;
use crate::shared::http::SimpleHeaders;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::{
    execute, BoxedSendExecutionAction, DrivenStreamIterator, Stream, StreamSpread, TaskIterator,
    TaskSpread, TaskStatus,
};
use std::sync::Arc;
use std::time::Duration;

use crate::websocket::shared::client::{
    BoxedWebSocketStream, MessageDelivery, Reconnect, WebSocketClient, WebSocketConnectConfig,
    WsProgress,
};
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
        Self::Task(p)
    }
}

impl From<ReconnectingWebSocketProgress> for WsPending {
    fn from(p: ReconnectingWebSocketProgress) -> Self {
        Self::Reconnecting(p)
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
            Self::Single(t) => t
                .next_status()
                .map(|s| remap_task_status(s, WsPending::Task)),
            Self::Reconnecting(t) => t
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

// ═══════════════════════════════════════════════════════════════════════════════
// Boxing the native stream into the platform-erased WebSocketClient shape
// ═══════════════════════════════════════════════════════════════════════════════

/// Map the native `WsPending` progress into the shared [`WsProgress`].
pub(crate) fn native_progress(pending: WsPending) -> WsProgress {
    match pending {
        WsPending::Task(WebSocketProgress::Connecting)
        | WsPending::Reconnecting(ReconnectingWebSocketProgress::Connecting) => WsProgress::Connecting,
        WsPending::Task(WebSocketProgress::Handshaking)
        | WsPending::Reconnecting(ReconnectingWebSocketProgress::Handshaking) => {
            WsProgress::Handshaking
        }
        WsPending::Task(WebSocketProgress::Reading)
        | WsPending::Reconnecting(ReconnectingWebSocketProgress::Reading) => WsProgress::Reading,
        WsPending::Reconnecting(ReconnectingWebSocketProgress::Reconnecting) => {
            WsProgress::Reconnecting
        }
    }
}

/// Map the native `WsPending` progress context to the shared [`WsProgress`],
/// preserving every data-bearing variant (including `Spread` payloads).
fn map_progress<D>(item: Stream<D, WsPending>) -> Stream<D, WsProgress> {
    match item {
        Stream::Init => Stream::Init,
        Stream::Ignore => Stream::Ignore,
        Stream::Delayed(d) => Stream::Delayed(d),
        Stream::Pending(p) => Stream::Pending(native_progress(p)),
        Stream::Next(d) => Stream::Next(d),
        Stream::Wait => Stream::Wait,
        Stream::Spread(items) => Stream::Spread(
            items
                .into_iter()
                .map(|spread| match spread {
                    StreamSpread::Pending(p) => StreamSpread::Pending(native_progress(p)),
                    StreamSpread::Done(d) => StreamSpread::Done(d),
                })
                .collect(),
        ),
    }
}

/// Box a spawned native WebSocket stream into the shared [`BoxedWebSocketStream`]
/// the cross-platform client consumes.
fn box_ws_stream<R>(inner: DrivenStreamIterator<WsTask<R>>) -> BoxedWebSocketStream
where
    R: DnsResolver + Clone + Send + 'static,
{
    Box::new(inner.map(map_progress))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Native connect helpers on the cross-platform WebSocketClient
// ═══════════════════════════════════════════════════════════════════════════════
//
// WHY: `WebSocketClient` is now non-generic and shared. Its native constructors
// stay here because they name native types (`WsTask`, `HttpConnectionPool`,
// `execute`). They are generic over the resolver `R` but return the non-generic
// `WebSocketClient`, so callers write `WebSocketClient::connect(resolver, …)`
// exactly as before — the resolver is erased at construction.

impl WebSocketClient {
    /// Connect to a WebSocket endpoint (native: HTTP/1.1 Upgrade over the
    /// resolver's own dial/TLS).
    ///
    /// Returns both the client and a [`MessageDelivery`] handle for sending.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the URL is invalid or the executor fails to
    /// schedule the task.
    pub fn connect<R>(
        resolver: R,
        url: impl Into<String>,
        read_timeout: Duration,
        sleep_timeout: Duration,
    ) -> Result<(Self, MessageDelivery), WebSocketError>
    where
        R: DnsResolver + Clone + Send + 'static,
    {
        Self::with_options(resolver, url, None, SimpleHeaders::new(), read_timeout, sleep_timeout)
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
    /// * `sleep_timeout` - Idle poll interval
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the URL is invalid or the executor fails to
    /// schedule the task.
    #[tracing::instrument(name = "websocket_connect", skip(resolver, extra_headers), fields(url))]
    pub fn with_options<R>(
        resolver: R,
        url: impl Into<String>,
        subprotocols: Option<String>,
        extra_headers: SimpleHeaders,
        read_timeout: Duration,
        sleep_timeout: Duration,
    ) -> Result<(Self, MessageDelivery), WebSocketError>
    where
        R: DnsResolver + Clone + Send + 'static,
    {
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
        let client = Self::new(box_ws_stream(inner), delivery.clone());
        Ok((client, delivery))
    }

    /// Connect with optional reconnection.
    ///
    /// When [`Reconnect::Yes`], uses `ReconnectingWebSocketTask` which handles
    /// disconnect detection and exponential backoff transparently.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the URL is invalid or the executor fails to
    /// schedule the task.
    pub fn connect_with_reconnect<R>(
        resolver: R,
        url: impl Into<String>,
        reconnect: Reconnect,
        read_timeout: Duration,
        sleep_timeout: Duration,
    ) -> Result<(Self, MessageDelivery), WebSocketError>
    where
        R: DnsResolver + Clone + Send + 'static,
    {
        let (task, delivery) =
            Self::connect_parts(resolver, url, reconnect, read_timeout, sleep_timeout)?;
        let inner = execute(task, None)
            .map_err(|e| WebSocketError::ProtocolError(format!("Executor error: {e}")))?;
        Ok((Self::new(box_ws_stream(inner), delivery.clone()), delivery))
    }

    /// Connect, returning the raw native task and delivery without spawning it —
    /// for Transport use, where the caller drives the task on its own pool.
    ///
    /// Returns:
    /// - `WsTask<R>` — the un-spawned task. Use `execute()` to drive it.
    /// - `MessageDelivery` — push messages to send to the task.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the URL is invalid.
    pub fn connect_parts<R>(
        resolver: R,
        url: impl Into<String>,
        reconnect: Reconnect,
        read_timeout: Duration,
        sleep_timeout: Duration,
    ) -> Result<(WsTask<R>, MessageDelivery), WebSocketError>
    where
        R: DnsResolver + Clone + Send + 'static,
    {
        let url_str = url.into();
        let (delivery, msg_rx) = MessageDelivery::new();

        let task = match reconnect {
            Reconnect::No => WsTask::Single(WebSocketTask::connect_with_delivery(
                resolver,
                url_str,
                None,
                SimpleHeaders::new(),
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
    /// Returns [`WebSocketError`] if the URL is invalid or the executor fails to
    /// schedule the task.
    pub fn with_pool<R>(
        url: impl Into<String>,
        pool: Arc<HttpConnectionPool<R>>,
    ) -> Result<(Self, MessageDelivery), WebSocketError>
    where
        R: DnsResolver + Clone + Send + 'static,
    {
        Self::with_pool_and_options(
            url,
            pool,
            None,
            SimpleHeaders::new(),
            Duration::from_secs(3),
            Duration::from_secs(1),
        )
    }

    /// Connect using an existing connection pool with custom options.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the URL is invalid or the executor fails to
    /// schedule the task.
    pub fn with_pool_and_options<R>(
        url: impl Into<String>,
        pool: Arc<HttpConnectionPool<R>>,
        subprotocols: Option<String>,
        extra_headers: SimpleHeaders,
        read_timeout: Duration,
        sleep_timeout: Duration,
    ) -> Result<(Self, MessageDelivery), WebSocketError>
    where
        R: DnsResolver + Clone + Send + 'static,
    {
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
        let client = Self::new(box_ws_stream(inner), delivery.clone());
        Ok((client, delivery))
    }

    /// Connect honouring a full [`WebSocketConnectConfig`] — the native basis for
    /// [`WebSocketConnector::open_websocket`](crate::websocket::shared::connector::WebSocketConnector).
    ///
    /// Dispatches on [`WebSocketConnectConfig::reconnect`]: `No` builds a
    /// single-shot task honouring subprotocols, headers, and timeouts; `Yes`
    /// layers the reconnecting task (subprotocols/headers use its defaults —
    /// a documented limitation of the reconnect path).
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the URL is invalid or the executor fails to
    /// schedule the task.
    pub fn connect_with_config<R>(
        resolver: R,
        url: impl Into<String>,
        config: &WebSocketConnectConfig,
    ) -> Result<(Self, MessageDelivery), WebSocketError>
    where
        R: DnsResolver + Clone + Send + 'static,
    {
        match config.reconnect {
            Reconnect::No => Self::with_options(
                resolver,
                url,
                config.subprotocols.clone(),
                config.extra_headers.clone(),
                config.read_timeout,
                config.sleep_timeout,
            ),
            Reconnect::Yes => Self::connect_with_reconnect(
                resolver,
                url,
                Reconnect::Yes,
                config.read_timeout,
                config.sleep_timeout,
            ),
        }
    }
}
