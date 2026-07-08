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

use std::sync::Arc;

use bytes::Bytes;
use foundation_core::valtron::{PipeReceiver, PipeSender};
use foundation_errstacks::ErrorTrace;
use foundation_netio::simple_http::shared::{RequestDescriptor, SimpleHeaders, Status};

use crate::error::{Code, ConnectError};

use super::capabilities::TransportCapabilities;

/// The wire-body byte sink (request bytes out).
pub type ByteSink = PipeSender<Bytes>;
/// The wire-body byte source (response bytes in).
pub type ByteSource = PipeReceiver<Bytes>;
/// Response-head source (status + headers, at most one item).
pub type HeadSource = PipeReceiver<(Status, SimpleHeaders)>;

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

/// A live transport exchange — **byte-level**. The three caller-facing halves:
/// push request bytes, await the response head, drain response body bytes.
///
/// All three are valtron [`Pipe`](foundation_core::valtron::Pipe) halves owned
/// by the caller. The transport's pump task (spawned by `open()`) holds the
/// socket-facing halves.
pub struct TransportStream {
    /// Wire request-body bytes out.
    pub send_body: ByteSink,
    /// The response head (status + headers) — at most one item. Once the head
    /// arrives, `receive().await` yields it; the pipe then closes so the next
    /// receive returns `None`.
    pub head: HeadSource,
    /// Wire response-body bytes in. Drained by the caller; the pump task pushes
    /// response-body chunks.
    pub recv_body: ByteSource,
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
/// The caller **must be in a valtron executor context** — `receive().await`
/// parks via `Depends`; outside a valtron pool it will hang forever.
pub async fn round_trip(
    transport: &impl Transport,
    request: RequestDescriptor,
    body: Bytes,
) -> Result<(Status, SimpleHeaders, Bytes), TransportError> {
    let stream = transport.open(request)?;
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
    let (status, headers) = stream.head.receive().await.ok_or_else(|| {
        TransportError::Io(Arc::new(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "response head pipe closed before yielding",
        )))
    })?;
    let mut body_bytes = Vec::new();
    while let Some(chunk) = stream.recv_body.receive().await {
        body_bytes.extend_from_slice(&chunk);
    }
    Ok((status, headers, Bytes::from(body_bytes)))
}
