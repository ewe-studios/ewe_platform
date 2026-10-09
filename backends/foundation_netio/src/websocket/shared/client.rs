//! Cross-platform WebSocket client (F51 Stage 6).
//!
//! WHY: F51 unifies the connection-owning client across native and wasm. The
//! WebSocket client used to be `WebSocketClient<R: DnsResolver>` — native-only,
//! parameterised over a resolver, and wired directly to the native
//! `DrivenStreamIterator`. That baggage stopped it (and the transports built on
//! it) from ever compiling to wasm. This module hosts the **platform-erased**
//! client: no `R` generic, no native stream type in its signature.
//!
//! WHAT: [`WebSocketClient`] — a non-generic consumer over an already-spawned,
//! boxed stream of inbound messages plus a [`MessageDelivery`] for outbound
//! sends. Platform `connect` helpers (native: HTTP/1.1 Upgrade over the pool;
//! wasm: the browser `WebSocket`) each build the boxed stream and hand it here
//! via [`WebSocketClient::new`]. [`WebSocketEvent`] and the
//! [`WebSocketMessageIterator`] present the same simple iterator surface as
//! before.
//!
//! HOW: The inbound half is erased to `Box<dyn Iterator<Item =
//! WebSocketStreamItem> + Send>` — the pending/progress context is collapsed to
//! `()` because consumers only ever act on `Stream::Next`. The outbound half is
//! the cross-platform [`MessageDelivery`] (a `PipeSender`), unchanged.

use std::time::Duration;

use foundation_core::valtron::{Pipe, PipeReceiver, PipeSender, Stream};

use crate::shared::http::SimpleHeaders;

use super::error::WebSocketError;
use super::message::WebSocketMessage;

/// Cross-platform connection progress reported while the stream is pending.
///
/// WHY: The boxed stream needs a single, shared `Pending` type both targets can
/// produce. Native maps its `WsPending` (single-shot + reconnecting task
/// progress) into this; wasm maps the browser socket's `readyState`. Keeping a
/// real progress enum (rather than erasing to `()`) preserves the connecting /
/// handshaking / reconnecting signal a consumer can surface to a user.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WsProgress {
    /// Establishing the transport (DNS/TCP/TLS on native; browser dialing on wasm).
    Connecting,
    /// Performing the WebSocket upgrade handshake.
    Handshaking,
    /// Connection is open; reading/awaiting frames.
    Reading,
    /// A dropped connection is being re-established (native reconnecting task;
    /// best-effort/native-only — see [`Reconnect`]).
    Reconnecting,
}

/// One item from a platform-erased WebSocket stream: a decoded message result,
/// with progress reported as the shared [`WsProgress`].
pub type WebSocketStreamItem = Stream<Result<WebSocketMessage, WebSocketError>, WsProgress>;

/// A boxed, platform-erased stream of inbound WebSocket messages.
///
/// Native builds this from a `DrivenStreamIterator<WsTask<R>>`; wasm from a
/// browser-`WebSocket` bridge. Either way the concrete driver never appears in
/// [`WebSocketClient`]'s signature.
pub type BoxedWebSocketStream = Box<dyn Iterator<Item = WebSocketStreamItem> + Send>;

/// Whether to open a single-shot connection or a self-reconnecting one.
///
/// WHY: Shared so both platforms accept the same client configuration. Native
/// honours [`Reconnect::Yes`] with an exponential-backoff reconnecting task; the
/// browser `WebSocket` on wasm manages its own lifecycle, so wasm treats `Yes` as
/// a best-effort request and does not add a reconnect layer of its own (a
/// documented, native-only capability — the surface still accepts it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reconnect {
    No,
    Yes,
}

/// Cross-platform options for opening a WebSocket through a
/// [`WebSocketConnector`](super::connector::WebSocketConnector).
///
/// WHY: One configuration type both targets accept, so a caller writes identical
/// code. Native honours every field; wasm honours the ones the browser exposes
/// (`subprotocols`, and where possible timeouts) and treats [`Self::reconnect`]
/// as best-effort (`Reconnect::Yes` is a native-only capability — the browser
/// `WebSocket` owns its own lifecycle). Fields the platform can't honour are
/// documented no-ops rather than a compile-time fork.
#[derive(Clone, Debug)]
pub struct WebSocketConnectConfig {
    /// Optional comma-separated `Sec-WebSocket-Protocol` list.
    pub subprotocols: Option<String>,
    /// Extra request headers merged into the handshake (native) / request (wasm).
    pub extra_headers: SimpleHeaders,
    /// Whether to layer automatic reconnection (native only; see the type doc).
    pub reconnect: Reconnect,
    /// Per-read timeout for the single-shot native task.
    pub read_timeout: Duration,
    /// Idle poll interval for the native task.
    pub sleep_timeout: Duration,
}

impl Default for WebSocketConnectConfig {
    fn default() -> Self {
        Self {
            subprotocols: None,
            extra_headers: SimpleHeaders::new(),
            reconnect: Reconnect::No,
            read_timeout: Duration::from_secs(30),
            sleep_timeout: Duration::from_secs(1),
        }
    }
}

impl WebSocketConnectConfig {
    /// A default configuration (single-shot, no subprotocols, empty headers).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the request headers merged into the handshake.
    #[must_use]
    pub fn with_headers(mut self, headers: SimpleHeaders) -> Self {
        self.extra_headers = headers;
        self
    }

    /// Set the requested subprotocols (comma-separated).
    #[must_use]
    pub fn with_subprotocols(mut self, subprotocols: impl Into<String>) -> Self {
        self.subprotocols = Some(subprotocols.into());
        self
    }

    /// Request automatic reconnection (native only; best-effort on wasm).
    #[must_use]
    pub fn with_reconnect(mut self, reconnect: Reconnect) -> Self {
        self.reconnect = reconnect;
        self
    }
}

/// WHY: Users need a send-capable WebSocket handle that integrates with valtron.
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
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError::ConnectionClosed`] if the receiver end has been dropped.
    pub fn ping(&self, data: Vec<u8>) -> Result<(), WebSocketError> {
        self.send(WebSocketMessage::Ping(data))
    }

    /// Send a Pong message.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError::ConnectionClosed`] if the receiver end has been dropped.
    pub fn pong(&self, data: Vec<u8>) -> Result<(), WebSocketError> {
        self.send(WebSocketMessage::Pong(data))
    }

    /// Send a Close message.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError::ConnectionClosed`] if the receiver end has been dropped.
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

/// A cross-platform WebSocket client.
///
/// WHY: Users want to consume WebSocket messages without understanding the
/// per-platform task machinery. F51 erases the resolver generic and the native
/// stream type so the exact same client value works on native and wasm.
///
/// WHAT: Wraps an already-spawned, boxed inbound stream and a [`MessageDelivery`]
/// for outbound sends. Construct it with [`WebSocketClient::new`] from a platform
/// `connect` helper, or obtain one from
/// [`WebSocketConnector::open_websocket`](crate::websocket::shared::connector::WebSocketConnector).
pub struct WebSocketClient {
    inner: BoxedWebSocketStream,
    delivery: MessageDelivery,
}

impl WebSocketClient {
    /// Build a client from an already-spawned inbound stream and delivery handle.
    ///
    /// WHY: Platform `connect` helpers own the target-specific work of spawning
    /// the stream (native `execute()`; wasm browser `WebSocket` bridge); this
    /// constructor is the single, platform-agnostic join point.
    #[must_use]
    pub fn new(inner: BoxedWebSocketStream, delivery: MessageDelivery) -> Self {
        Self { inner, delivery }
    }

    /// Get the message delivery handle for sending messages.
    #[must_use]
    pub fn delivery(&self) -> MessageDelivery {
        self.delivery.clone()
    }

    /// Consume the client, returning the inner stream and delivery handle.
    #[must_use]
    pub fn into_parts(self) -> (BoxedWebSocketStream, MessageDelivery) {
        (self.inner, self.delivery)
    }

    /// Get an iterator over incoming messages.
    pub fn messages(&mut self) -> WebSocketMessageIterator<'_> {
        WebSocketMessageIterator { client: self }
    }
}

/// Iterator over messages from a [`WebSocketClient`].
pub struct WebSocketMessageIterator<'a> {
    client: &'a mut WebSocketClient,
}

impl Iterator for WebSocketMessageIterator<'_> {
    type Item = Result<WebSocketEvent, WebSocketError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.client.inner.next()? {
            Stream::Next(result) => {
                tracing::debug!("Stream got Next value");
                // Skip ConnectionEstablished message, return actual messages.
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
