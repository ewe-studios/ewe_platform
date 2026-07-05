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

use bytes::Bytes;
use foundation_core::extensions::result_ext::SendableBoxedError;
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
#[derive(Debug)]
pub enum TransportError {
    /// Dial / TLS / pool-checkout failure.
    Connect(SendableBoxedError),
    /// Connect or response-head deadline elapsed.
    Timeout,
    /// `RST_STREAM` / QUIC reset / connection closed mid-exchange.
    Reset,
    /// Malformed wire data below the RPC protocol layer.
    Protocol(String),
    /// Local cancellation (the call's `CancelSignal` fired).
    Canceled,
    /// An underlying I/O error.
    Io(std::io::Error),
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
