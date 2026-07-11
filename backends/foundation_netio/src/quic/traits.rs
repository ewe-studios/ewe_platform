//! The backend-agnostic QUIC trait set (F33 — Decision 01 §"Our QUIC trait abstraction").
//!
//! WHY: HTTP/3 needs a QUIC backend, but it must not be welded to one. `h3` solves
//! this with a `quic.rs` trait module — and we take that as the *catalogue of
//! operations*, not as gospel. h3's traits are shaped by `Poll`/`Context`/`Waker`
//! because h3 is driven by a tokio reactor. We are driven by valtron, whose
//! [`Stream<D, P>`] already carries the readiness vocabulary (`Next` / `Pending` /
//! `Wait` / `Delayed`), and whose `TaskStatus::Depends(EventReadiness)` is how a
//! task actually *parks* on the native reactor. So the traits below are
//! progress-returning and synchronous: **no futures, no `Poll`, no `Waker`.**
//!
//! WHAT: [`QuicConnection`], [`QuicSendStream`], [`QuicRecvStream`],
//! [`QuicBidiStream`], plus the two protocol-meaningful error enums.
//!
//! HOW: every method that can make partial progress hands back **one
//! [`Stream`] value per call** — call it again to advance. `Next(v)` is progress,
//! `Pending`/`Wait` means "nothing yet, ask again", and the accept/read streams
//! end (`None` from the driving iterator) when the connection or stream closes.
//! The *driving task* is what maps those into `TaskStatus`; the traits themselves
//! stay ignorant of the executor.
//!
//! Fire-and-forget operations (`reset` / `stop_sending` / `close` / `id`) are
//! plain synchronous methods — there is nothing to wait for.
//!
//! ## Deliberate deviations from `h3` (Decision 01, normative)
//!
//! - **No `Poll`/`Context`/`Waker`.** The driver is a valtron task using the full
//!   `TaskStatus` vocabulary, including `Depends` to park on the reactor. This
//!   richer readiness signalling is the whole reason we do not use `h3` as-is.
//! - **`poll_ready` folded into [`QuicSendStream::send`].** Write back-pressure is
//!   communicated by the driving task's `TaskStatus` — `Depends(QueueVacancyReadiness)`
//!   when the send pipe is full, or `Depends` on socket-writable via the reactor. A
//!   separate readiness method would be redundant.
//! - **`Is0rtt` is not a stream trait.** 0-RTT is connection metadata: a flag on
//!   `ConnectionContext`, not an I/O method.
//! - **`SendStreamUnframed` dropped** until raw byte streaming (WebTransport-style)
//!   is actually needed.
//!
//! ## These traits are not `dyn`-compatible, on purpose
//!
//! [`QuicSendStream::send`] takes `&mut impl Buf` (an argument-position impl trait)
//! and [`QuicBidiStream::split`] returns `impl` types. Associated types plus generics
//! are the intended usage — static dispatch throughout. A backend is chosen at the
//! type level, not behind a `Box<dyn>`.

use bytes::Buf;
use bytes::Bytes;

use foundation_core::valtron::Stream;

use super::StreamId;

/// Why a QUIC connection ended.
///
/// Protocol-meaningful, not backend-specific: every QUIC implementation can
/// report these, and HTTP/3 has to distinguish them (an application close carries
/// an h3 error code; a timeout does not).
#[derive(Debug)]
pub enum QuicConnError {
    /// The peer (or we) closed the connection at the application layer.
    ApplicationClose {
        /// The application error code carried by the CONNECTION_CLOSE frame.
        code: u64,
    },
    /// The connection idled out or handshake timed out.
    Timeout,
    /// The backend hit an internal invariant.
    Internal(String),
    /// Anything else the backend wants to surface.
    Other(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl std::fmt::Display for QuicConnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApplicationClose { code } => {
                write!(f, "connection closed by application: code {code}")
            }
            Self::Timeout => write!(f, "connection timed out"),
            Self::Internal(msg) => write!(f, "internal QUIC error: {msg}"),
            Self::Other(e) => write!(f, "QUIC error: {e}"),
        }
    }
}

impl std::error::Error for QuicConnError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Other(e) => Some(&**e),
            _ => None,
        }
    }
}

/// Why a QUIC stream ended.
#[derive(Debug)]
pub enum QuicStreamError {
    /// The whole connection went away; the stream died with it.
    ConnClosed(QuicConnError),
    /// The peer reset this stream (`RESET_STREAM`) or told us to stop
    /// (`STOP_SENDING`), with an application error code.
    Terminated {
        /// The application error code carried by the frame.
        code: u64,
    },
    /// Anything else the backend wants to surface.
    Other(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl std::fmt::Display for QuicStreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnClosed(e) => write!(f, "stream ended: {e}"),
            Self::Terminated { code } => write!(f, "stream terminated by peer: code {code}"),
            Self::Other(e) => write!(f, "stream error: {e}"),
        }
    }
}

impl std::error::Error for QuicStreamError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConnClosed(e) => Some(e),
            Self::Other(e) => Some(&**e),
            Self::Terminated { .. } => None,
        }
    }
}

/// A live QUIC connection: accept inbound streams, open outbound ones, close.
///
/// Every stream-producing method returns one [`Stream`] value per call.
/// `Stream::Next(Ok(s))` yields a stream; `Stream::Pending(())` / `Stream::Wait`
/// mean "not yet, call again"; `Stream::Next(Err(e))` reports the failure.
pub trait QuicConnection: Send {
    /// A peer-initiated unidirectional stream.
    type RecvStream: QuicRecvStream;
    /// A locally-initiated unidirectional stream.
    type SendStream: QuicSendStream;
    /// A bidirectional stream, from either side.
    type BidiStream: QuicBidiStream;

    /// WHY: HTTP/3 control, QPACK encoder and QPACK decoder streams all arrive as
    /// peer-initiated unidirectional streams.
    ///
    /// WHAT: accept the next inbound unidirectional stream.
    ///
    /// HOW: `Next(Ok(s))` when one has arrived, `Pending`/`Wait` when none has yet.
    /// The caller stops when the connection closes.
    fn accept_recv(&mut self) -> Stream<Result<Self::RecvStream, QuicConnError>, ()>;

    /// WHAT: accept the next inbound bidirectional stream — an HTTP/3 request.
    ///
    /// HOW: as [`Self::accept_recv`].
    fn accept_bidi(&mut self) -> Stream<Result<Self::BidiStream, QuicConnError>, ()>;

    /// WHY: an HTTP/3 client opens one bidirectional stream per request.
    ///
    /// WHAT: open an outbound bidirectional stream.
    ///
    /// HOW: `Pending` while blocked on the peer's `MAX_STREAMS` limit — QUIC flow
    /// control, not an error. Call again once the peer raises the limit.
    fn open_bidi(&mut self) -> Stream<Result<Self::BidiStream, QuicStreamError>, ()>;

    /// WHAT: open an outbound unidirectional stream (control, QPACK).
    ///
    /// HOW: as [`Self::open_bidi`].
    fn open_send(&mut self) -> Stream<Result<Self::SendStream, QuicStreamError>, ()>;

    /// Close the connection with an application error code. Fire and forget.
    fn close(&mut self, code: u64, reason: &[u8]);
}

/// The write half of a QUIC stream.
pub trait QuicSendStream: Send {
    /// WHY: h3 splits this into `poll_ready` then `send_data`. We fold them: the
    /// return value *is* the readiness signal, and the driving task translates a
    /// `Pending` into `TaskStatus::Depends(..)`.
    ///
    /// WHAT: write as much of `buf` as the flow-control window allows.
    ///
    /// HOW: `Next(Ok(n))` advances `buf` by `n` bytes. `Pending(())` means the
    /// window is full — nothing was written; call again after the peer opens it.
    fn send(&mut self, buf: &mut impl Buf) -> Stream<Result<usize, QuicStreamError>, ()>;

    /// WHAT: finish the stream (send FIN) once everything queued has been flushed.
    ///
    /// HOW: `Pending` until the backend has accepted the FIN; `Next(Ok(()))` after.
    fn finish(&mut self) -> Stream<Result<(), QuicStreamError>, ()>;

    /// Abort the stream with an application error code. Fire and forget.
    fn reset(&mut self, code: u64);

    /// This stream's QUIC stream id.
    fn id(&self) -> StreamId;
}

/// The read half of a QUIC stream.
pub trait QuicRecvStream: Send {
    /// WHAT: read the next chunk.
    ///
    /// HOW: `Next(Ok(Some(bytes)))` per chunk, `Next(Ok(None))` at end of stream,
    /// `Pending`/`Wait` when nothing is buffered yet.
    fn read(&mut self) -> Stream<Result<Option<Bytes>, QuicStreamError>, ()>;

    /// Tell the peer to stop sending, with an application error code. Fire and forget.
    fn stop_sending(&mut self, code: u64);

    /// This stream's QUIC stream id.
    fn id(&self) -> StreamId;
}

/// A bidirectional QUIC stream — both halves, splittable.
pub trait QuicBidiStream: QuicSendStream + QuicRecvStream {
    /// WHY: HTTP/3 drives request body and response body independently; the halves
    /// often end up owned by different tasks.
    ///
    /// WHAT: split into the write half and the read half.
    ///
    /// HOW: static dispatch — these traits are not `dyn`-compatible, so `impl`
    /// return types are the intended shape.
    fn split(self) -> (impl QuicSendStream, impl QuicRecvStream)
    where
        Self: Sized;
}
