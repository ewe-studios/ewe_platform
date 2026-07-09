//! The streaming-capable client `Transport` (Decision 11 §Transport).
//!
//! WHY: One transport abstraction serves unary and streaming alike. It is
//! **byte-level and protocol-agnostic** — it moves wire body bytes and knows
//! nothing of envelopes, compression, or protocols (it could not produce
//! `Frame`s; normalization is protocol-specific). The protocol layer's
//! reader/writer tasks sit between the transport bytes and the `FramePipe`s.
//!
//! WHAT: the [`Transport`] trait, [`TransportStream`] (the live byte-level
//! exchange), [`ByteSink`]/[`ByteSource`], and [`TransportError`] with its
//! `ConnectError` mapping. Concrete transports (HTTP/1.1, HTTP/2, WASM Fetch, …)
//! implement this in their own features; this defines the contract they satisfy.
//!
//! ## Valton-native design (Decision 11 §Connection ownership)
//!
//! `open()` is synchronous — it spawns the byte pump on the valtron pool and
//! returns the caller-facing pipe halves immediately. The caller pushes request
//! bytes into `send_body` and receives responses via `recv_body`. The response
//! head (status + headers) arrives through a 1-slot `head` pipe. No `BoxFuture`,
//! no `futures_lite::block_on` — the pump task on the pool handles all the async
//! I/O, and the caller is already in a valtron context (or uses the pipe's own
//! `receive().await` / `try_recv()` + `readiness()` surface).

use std::pin::Pin;
use std::sync::Arc;

use bytes::Bytes;
use futures::{Stream, StreamExt};
use foundation_core::valtron::{PipeReceiver, PipeSender};
use foundation_errstacks::ErrorTrace;
use foundation_netio::simple_http::shared::{RequestDescriptor, SimpleHeaders, Status};

use crate::error::{Code, ConnectError};

use super::capabilities::TransportCapabilities;

/// The wire-body byte sink (request bytes out).
pub type ByteSink = PipeSender<Bytes>;
/// The wire-body byte source (response bytes in) — the pipe-backed variant still
/// used by server-side readers and pipe-fed test harnesses.
pub type ByteSource = PipeReceiver<Bytes>;
/// Response-head source (status + headers, at most one item) — pipe-backed
/// variant retained for pipe-fed test harnesses.
pub type HeadSource = PipeReceiver<(Status, SimpleHeaders)>;

/// The response head as an **awaitable stream** of header blocks or a transport
/// error (F45 Part D, design D1).
///
/// WHY a stream, not a one-shot future: HTTP/2 and HTTP/3 can deliver *interim*
/// heads (`100 Continue`, `103 Early Hints`) before the final response head, so a
/// header channel is inherently multi-item — the pre-D `PipeReceiver` head was
/// already repeatable. Modelling it as a `Stream` (symmetric with [`BodyStream`])
/// keeps that capability and avoids an API change when an h2/h3 transport starts
/// surfacing interim heads. RPC protocols emit exactly one head today, so their
/// consumers simply take the first item (`head.next().await`).
///
/// WHY erased: `TransportStream` must not leak *how* a transport produces its head
/// — h1 peels it with a split, WASM reads a Fetch response, a future h2 reads
/// header frames. Any `StreamIterator` (or pipe) bridges into this one boxed
/// `Stream`, so downstream code consumes uniformly. Each item is the head or a
/// pre-head transport failure as `Err`.
pub type HeadStream =
    Pin<Box<dyn Stream<Item = Result<(Status, SimpleHeaders), TransportError>> + Send>>;

/// The response body as an **awaitable stream** of byte chunks or a transport
/// error (F45 Part D, design D1).
///
/// WHY: same erasure as [`HeadFuture`] — the concrete producer (a split observer,
/// a Fetch body, a pipe) is hidden behind this boxed `Stream`, so a mid-body
/// failure rides the payload as `Err` instead of being silently dropped, and the
/// reader never learns what fed it. Each item is `Ok(bytes)` or `Err(transport)`.
pub type BodyStream =
    Pin<Box<dyn Stream<Item = Result<Bytes, TransportError>> + Send>>;

/// Bridge a pipe-backed [`ByteSource`] into a [`BodyStream`] (F45 Part D).
///
/// WHY: server-side readers and test harnesses feed body bytes through a valtron
/// `Pipe`, but the unified reader consumes a [`BodyStream`]. WHAT: wraps the pipe
/// so each received chunk surfaces as `Ok`; a pipe has no error lane, so this
/// bridge never yields `Err`. HOW: `futures::stream::unfold` over `receive()`.
///
/// # Panics
/// Never panics.
#[must_use]
pub fn body_stream_from_pipe(rx: ByteSource) -> BodyStream {
    Box::pin(futures::stream::unfold(rx, |rx| async move {
        match rx.receive().await {
            Some(bytes) => Some((Ok(bytes), rx)),
            None => None,
        }
    }))
}

/// Bridge a pipe-backed head receiver into a [`HeadStream`] (F45 Part D).
///
/// WHY: pipe-fed test harnesses supply the head through a `Pipe`; the [`HeadStream`]
/// contract needs a `Stream`. WHAT: each received head surfaces as `Ok`; a pipe has
/// no error lane, so this bridge never yields `Err`, and the stream ends when the
/// pipe closes. HOW: `futures::stream::unfold` over `receive()`.
///
/// # Panics
/// Never panics.
#[must_use]
pub fn head_stream_from_pipe(rx: HeadSource) -> HeadStream {
    Box::pin(futures::stream::unfold(rx, |rx| async move {
        match rx.receive().await {
            Some(head) => Some((Ok(head), rx)),
            None => None,
        }
    }))
}

/// HTTP transport abstraction — one implementation per transport.
pub trait Transport: Send + Sync + 'static {
    /// What this transport can do (for capability matching before sending).
    fn capabilities(&self) -> TransportCapabilities;

    /// Open a streaming exchange. Spawns the byte pump on the valtron pool and
    /// returns the caller-facing pipe halves synchronously. Unary is the
    /// degenerate case (send one, close, receive one).
    ///
    /// # Errors
    ///
    /// Returns `TransportError` if the pump task cannot be spawned (e.g. pool not
    /// initialised) or if building the request fails synchronously.
    fn open(&self, request: RequestDescriptor) -> Result<TransportStream, TransportError>;
}

/// A live transport exchange (F45 Part D, design D1). Four caller-facing halves.
pub struct TransportStream {
    /// Wire request-body bytes out (the one surviving `Pipe`).
    pub send_body: ByteSink,
    /// The response head(s). RPC protocols emit exactly one, so consumers take the
    /// first item (`head.next().await`); an h2/h3 transport may emit interim heads
    /// (`1xx`) before the final one. Each item is `Ok(head)` or `Err(TransportError)`
    /// (a pre-head failure); the stream closes once the head(s) are delivered.
    pub head: HeadStream,
    /// Wire response-body bytes in. Each `.next().await` yields `Ok(chunk)` or
    /// `Err(TransportError)` (a mid-body failure), then `None` at end of stream.
    pub recv_body: BodyStream,
    /// HTTP trailers (h2 trailing HEADERS). At most one item — `None` means the
    /// transport does not support trailers (e.g. HTTP/1.1 Connect) or the peer
    /// sent none. Consumers call `trailers.receive().await` after draining `recv_body`.
    pub trailers: PipeReceiver<SimpleHeaders>,
}

/// A transport-layer failure — a failure where no RPC response exists at all
/// (Decision 11). The client maps it into [`ConnectError`] via `From`.
///
/// WHY `Clone` (F45 Resolution 7): a transport failure rides the split payload as
/// `Result<_, TransportError>` and a pre-head `HttpExchange::Failed` must clone
/// losslessly into **both** split branches (head observer + body continuation).
/// The two non-`Clone` payloads are therefore `Arc`-wrapped rather than boxed.
#[derive(Debug, Clone)]
pub enum TransportError {
    /// Dial / TLS / pool-checkout failure.
    Connect(Arc<dyn std::error::Error + Send + Sync + 'static>),
    /// Connect or response-head deadline elapsed.
    Timeout,
    /// `RST_STREAM` / QUIC reset / connection closed mid-exchange.
    Reset,
    /// Malformed wire data below the RPC protocol layer.
    Protocol(String),
    /// Local cancellation (the call's `CancelSignal` fired).
    Canceled,
    /// An underlying I/O error.
    Io(Arc<std::io::Error>),
}

impl core::fmt::Display for TransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TransportError::Connect(e) => write!(f, "transport connect failure: {e}"),
            TransportError::Timeout => write!(f, "transport timeout"),
            TransportError::Reset => write!(f, "transport reset mid-exchange"),
            TransportError::Protocol(m) => write!(f, "transport protocol error: {m}"),
            TransportError::Canceled => write!(f, "transport canceled"),
            TransportError::Io(e) => write!(f, "transport i/o error: {e}"),
        }
    }
}

impl std::error::Error for TransportError {}

impl TransportError {
    /// The RPC code this transport failure maps to (Decision 11 mapping).
    #[must_use]
    pub fn code(&self) -> Code {
        match self {
            TransportError::Connect(_) | TransportError::Reset | TransportError::Io(_) => {
                Code::Unavailable
            }
            TransportError::Timeout => Code::DeadlineExceeded,
            TransportError::Protocol(_) => Code::Internal,
            TransportError::Canceled => Code::Canceled,
        }
    }
}

impl From<TransportError> for ConnectError {
    fn from(err: TransportError) -> Self {
        ConnectError::new(err.code(), err.to_string())
    }
}

impl From<TransportError> for ErrorTrace<ConnectError> {
    fn from(err: TransportError) -> Self {
        let connect = ConnectError::new(err.code(), err.to_string());
        ErrorTrace::new(err).change_context(connect)
    }
}

/// Convenience: open a transport stream, push the complete request body, close
/// the send side, await the response head, and collect all response body bytes.
///
/// This is the **sync** (valtron-native) equivalent of a unary RPC — useful for
/// testing and for callers that already hold the complete request message.
///
/// The caller **must be in a valtron executor context** — the awaits park via
/// `Depends`; outside a valtron pool they will hang forever.
///
/// # Errors
/// Returns `TransportError` if the exchange fails before the head (`head.await`),
/// or on a mid-body failure (a `recv_body` chunk resolves to `Err`).
pub async fn round_trip(
    transport: &impl Transport,
    request: RequestDescriptor,
    body: Bytes,
) -> Result<(Status, SimpleHeaders, Bytes), TransportError> {
    let mut stream = transport.open(request)?;
    if !body.is_empty() {
        stream
            .send_body
            .try_send(body)
            .map_err(|e| {
                TransportError::Io(Arc::new(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    format!("{e:?}"),
                )))
            })?;
    }
    stream.send_body.close();
    // RPC has exactly one response head — take the first item off the head stream.
    let (status, headers) = stream
        .head
        .next()
        .await
        .ok_or(TransportError::Reset)??;
    let mut body_bytes = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        body_bytes.extend_from_slice(&chunk?);
    }
    Ok((status, headers, Bytes::from(body_bytes)))
}
