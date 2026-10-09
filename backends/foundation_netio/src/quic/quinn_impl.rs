//! `quinn-proto` implementations of the QUIC trait set (F33).
//!
//! WHY: the traits in [`super::traits`] are what HTTP/3 is written against. This
//! module is the one place that knows `quinn_proto` exists.
//!
//! WHAT: [`QuinnConnection`], [`QuinnSendStream`], [`QuinnRecvStream`],
//! [`QuinnBidiStream`].
//!
//! HOW: every handle carries a [`SharedConn`] and a [`StreamId`]. A call locks the
//! shared state, re-borrows the `quinn_proto::Connection` for the length of one
//! operation, and returns a single `Stream<..>` value. `quinn-proto` is sans-IO —
//! nothing under this lock performs a syscall, so the lock is never held across
//! blocking work. Actual datagram I/O happens in [`super::QuicDriver`].
//!
//! ## Mapping quinn's errors onto the trait vocabulary
//!
//! | quinn | trait |
//! |---|---|
//! | `WriteError::Blocked` / `ReadError::Blocked` | `Stream::Pending(())` — flow control, not failure |
//! | `WriteError::Stopped(code)` / `ReadError::Reset(code)` | `Next(Err(Terminated { code }))` |
//! | `WriteError::ClosedStream`, `ClosedStream` | `Next(Err(Terminated { code: 0 }))` |
//! | connection gone | `Next(Err(ConnClosed(..)))` |
//!
//! `Blocked` becoming `Pending` rather than an error is the whole reason h3's
//! `poll_ready` is unnecessary here: back-pressure *is* the return value.

use bytes::{Buf, Bytes};
use quinn_proto::{Dir, VarInt};

use foundation_core::valtron::Stream;

use super::state::{lock, SharedConn};
use super::traits::{
    QuicBidiStream, QuicConnError, QuicConnection, QuicRecvStream, QuicSendStream, QuicStreamError,
};
use super::StreamId;

/// Clone the recorded close reason, if the connection has ended.
fn closed_error(state: &SharedConn) -> Option<QuicStreamError> {
    let guard = lock(state).ok()?;
    guard.closed.as_ref().map(|e| {
        QuicStreamError::ConnClosed(match e {
            QuicConnError::ApplicationClose { code } => {
                QuicConnError::ApplicationClose { code: *code }
            }
            QuicConnError::Timeout => QuicConnError::Timeout,
            QuicConnError::Internal(m) => QuicConnError::Internal(m.clone()),
            QuicConnError::Other(e) => QuicConnError::Internal(e.to_string()),
        })
    })
}

/// A live QUIC connection over `quinn-proto`.
pub struct QuinnConnection {
    state: SharedConn,
}

impl std::fmt::Debug for QuinnConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuinnConnection").finish_non_exhaustive()
    }
}

impl QuinnConnection {
    pub(crate) fn new(state: SharedConn) -> Self {
        Self { state }
    }

    /// Build a [`ConnectionContext`] for this QUIC connection (F53).
    ///
    /// Populates the peer's network address and the ALPN protocol id (`h3`) from
    /// the QUIC handshake. The caller wraps this in `Arc` and passes it through
    /// `request_from_fields` so handlers can access `connection.peer_addr` etc.
    #[must_use]
    pub fn connection_context(&self) -> crate::shared::context::ConnectionContext {
        let peer_addr = lock(&self.state).ok().map(|g| g.peer);

        crate::shared::context::ConnectionContext {
            peer_addr: peer_addr.map(|a| core::net::SocketAddr::new(a.ip(), a.port())),
            alpn: Some(b"h3".to_vec()),
            ..Default::default()
        }
    }

    /// Accept the next inbound stream of `dir`, or report why we cannot.
    fn accept_dir<S>(
        &mut self,
        dir: Dir,
        make: impl FnOnce(SharedConn, StreamId) -> S,
    ) -> Stream<Result<S, QuicConnError>, ()> {
        let mut guard = match lock(&self.state) {
            Ok(g) => g,
            Err(e) => return Stream::Next(Err(e)),
        };

        if let Some(id) = guard.next_inbound(dir) {
            drop(guard);
            return Stream::Next(Ok(make(std::sync::Arc::clone(&self.state), id)));
        }

        // The driver records the close reason; report it rather than looking idle
        // forever.
        if let Some(err) = guard.closed.take() {
            guard.closed = Some(clone_conn_error(&err));
            return Stream::Next(Err(err));
        }

        Stream::Pending(())
    }

    /// Open an outbound stream of `dir`. `Pending` while the peer's `MAX_STREAMS`
    /// limit is reached — QUIC flow control, not an error.
    fn open_dir<S>(
        &mut self,
        dir: Dir,
        make: impl FnOnce(SharedConn, StreamId) -> S,
    ) -> Stream<Result<S, QuicStreamError>, ()> {
        if let Some(err) = closed_error(&self.state) {
            return Stream::Next(Err(err));
        }

        let mut guard = match lock(&self.state) {
            Ok(g) => g,
            Err(e) => return Stream::Next(Err(QuicStreamError::ConnClosed(e))),
        };

        // Streams cannot be opened before the handshake completes.
        if !guard.is_established() {
            return Stream::Pending(());
        }

        match guard.conn.streams().open(dir) {
            Some(id) => {
                drop(guard);
                Stream::Next(Ok(make(std::sync::Arc::clone(&self.state), id.into())))
            }
            // Peer's stream limit reached; retry when it raises MAX_STREAMS.
            None => Stream::Pending(()),
        }
    }
}

/// `QuicConnError` is not `Clone` (it can carry a boxed error), but the close
/// reason has to be reportable more than once.
fn clone_conn_error(e: &QuicConnError) -> QuicConnError {
    match e {
        QuicConnError::ApplicationClose { code } => QuicConnError::ApplicationClose { code: *code },
        QuicConnError::Timeout => QuicConnError::Timeout,
        QuicConnError::Internal(m) => QuicConnError::Internal(m.clone()),
        QuicConnError::Other(e) => QuicConnError::Internal(e.to_string()),
    }
}

impl QuicConnection for QuinnConnection {
    type RecvStream = QuinnRecvStream;
    type SendStream = QuinnSendStream;
    type BidiStream = QuinnBidiStream;

    fn accept_recv(&mut self) -> Stream<Result<Self::RecvStream, QuicConnError>, ()> {
        self.accept_dir(Dir::Uni, QuinnRecvStream::new)
    }

    fn accept_bidi(&mut self) -> Stream<Result<Self::BidiStream, QuicConnError>, ()> {
        self.accept_dir(Dir::Bi, QuinnBidiStream::new)
    }

    fn open_bidi(&mut self) -> Stream<Result<Self::BidiStream, QuicStreamError>, ()> {
        self.open_dir(Dir::Bi, QuinnBidiStream::new)
    }

    fn open_send(&mut self) -> Stream<Result<Self::SendStream, QuicStreamError>, ()> {
        self.open_dir(Dir::Uni, QuinnSendStream::new)
    }

    fn close(&mut self, code: u64, reason: &[u8]) {
        let Ok(mut guard) = lock(&self.state) else {
            return;
        };
        let code = VarInt::from_u64(code).unwrap_or(VarInt::MAX);
        guard
            .conn
            .close(std::time::Instant::now(), code, reason.to_vec().into());
        guard.closed = Some(QuicConnError::ApplicationClose {
            code: code.into_inner(),
        });
    }

    fn send_datagram(&mut self, data: &[u8]) -> Stream<Result<(), QuicStreamError>, ()> {
        let Ok(mut guard) = lock(&self.state) else {
            return Stream::Next(Err(QuicStreamError::ConnClosed(
                QuicConnError::Internal("state poisoned".into()),
            )));
        };
        let max_size = guard.max_datagram_size;
        if let Some(max) = max_size {
            if data.len() > max {
                return Stream::Next(Err(QuicStreamError::Other(
                    format!("datagram too large: {} > max {}", data.len(), max).into(),
                )));
            }
            let dgram = bytes::Bytes::copy_from_slice(data);
            let result = guard.conn.datagrams().send(dgram, true);
            drop(guard);
            match result {
                Ok(()) => Stream::Next(Ok(())),
                Err(quinn_proto::SendDatagramError::Blocked(_)) => Stream::Pending(()),
                Err(quinn_proto::SendDatagramError::UnsupportedByPeer) => Stream::Next(Err(
                    QuicStreamError::Other("datagrams unsupported by peer".into()),
                )),
                Err(quinn_proto::SendDatagramError::TooLarge) => Stream::Next(Err(
                    QuicStreamError::Other("datagram too large for path MTU".into()),
                )),
                Err(quinn_proto::SendDatagramError::Disabled) => Stream::Next(Err(
                    QuicStreamError::Other("datagrams disabled at endpoint".into()),
                )),
            }
        } else {
            drop(guard);
            Stream::Next(Err(QuicStreamError::Other(
                "datagrams not negotiated".into(),
            )))
        }
    }

    fn recv_datagram(&mut self) -> Stream<Result<Option<Bytes>, QuicConnError>, ()> {
        let Ok(mut guard) = lock(&self.state) else {
            return Stream::Next(Err(QuicConnError::Internal("state poisoned".into())));
        };
        let closed = guard.closed.as_ref().map(|e| clone_conn_error(e));
        if let Some(err) = closed {
            return Stream::Next(Err(err));
        }
        if let Some(data) = guard.recv_datagrams.pop_front() {
            Stream::Next(Ok(Some(data)))
        } else {
            Stream::Pending(())
        }
    }

    fn max_datagram_size(&self) -> Option<usize> {
        let Ok(guard) = lock(&self.state) else {
            return None;
        };
        guard.max_datagram_size
    }
}

/// The write half of a `quinn-proto` stream.
pub struct QuinnSendStream {
    state: SharedConn,
    id: StreamId,
}

impl std::fmt::Debug for QuinnSendStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuinnSendStream")
            .field("id", &self.id)
            .finish()
    }
}

impl QuinnSendStream {
    pub(crate) fn new(state: SharedConn, id: StreamId) -> Self {
        Self { state, id }
    }
}

impl QuicSendStream for QuinnSendStream {
    fn send(&mut self, buf: &mut impl Buf) -> Stream<Result<usize, QuicStreamError>, ()> {
        if !buf.has_remaining() {
            return Stream::Next(Ok(0));
        }
        if let Some(err) = closed_error(&self.state) {
            return Stream::Next(Err(err));
        }

        let mut guard = match lock(&self.state) {
            Ok(g) => g,
            Err(e) => return Stream::Next(Err(QuicStreamError::ConnClosed(e))),
        };
        let Ok(qid) = quinn_proto::StreamId::try_from(self.id) else {
            return Stream::Next(Err(QuicStreamError::Terminated { code: 0 }));
        };

        // `Buf::chunk` gives the next contiguous run; a partial write is normal.
        let chunk_len = buf.chunk().len();
        let result = guard.conn.send_stream(qid).write(buf.chunk());
        drop(guard);

        match result {
            Ok(n) => {
                buf.advance(n);
                debug_assert!(n <= chunk_len);
                Stream::Next(Ok(n))
            }
            // Flow-control window full: this is the back-pressure signal, not an
            // error. h3 would have surfaced it through `poll_ready`.
            Err(quinn_proto::WriteError::Blocked) => Stream::Pending(()),
            Err(quinn_proto::WriteError::Stopped(code)) => {
                Stream::Next(Err(QuicStreamError::Terminated {
                    code: code.into_inner(),
                }))
            }
            Err(quinn_proto::WriteError::ClosedStream) => {
                Stream::Next(Err(QuicStreamError::Terminated { code: 0 }))
            }
        }
    }

    fn finish(&mut self) -> Stream<Result<(), QuicStreamError>, ()> {
        let mut guard = match lock(&self.state) {
            Ok(g) => g,
            Err(e) => return Stream::Next(Err(QuicStreamError::ConnClosed(e))),
        };
        let Ok(qid) = quinn_proto::StreamId::try_from(self.id) else {
            return Stream::Next(Err(QuicStreamError::Terminated { code: 0 }));
        };

        match guard.conn.send_stream(qid).finish() {
            Ok(()) => Stream::Next(Ok(())),
            // Already finished: the FIN is on the wire, which is what the caller
            // asked for.
            Err(quinn_proto::FinishError::ClosedStream) => Stream::Next(Ok(())),
            Err(quinn_proto::FinishError::Stopped(code)) => {
                Stream::Next(Err(QuicStreamError::Terminated {
                    code: code.into_inner(),
                }))
            }
        }
    }

    fn reset(&mut self, code: u64) {
        let Ok(mut guard) = lock(&self.state) else {
            return;
        };
        let Ok(qid) = quinn_proto::StreamId::try_from(self.id) else {
            return;
        };
        let code = VarInt::from_u64(code).unwrap_or(VarInt::MAX);
        let _ = guard.conn.send_stream(qid).reset(code);
    }

    fn id(&self) -> StreamId {
        self.id
    }
}

/// The read half of a `quinn-proto` stream.
pub struct QuinnRecvStream {
    state: SharedConn,
    id: StreamId,
    /// Set once the peer's FIN has been observed, so subsequent reads keep
    /// reporting end-of-stream instead of `Pending`.
    finished: bool,
}

impl std::fmt::Debug for QuinnRecvStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuinnRecvStream")
            .field("id", &self.id)
            .field("finished", &self.finished)
            .finish()
    }
}

impl QuinnRecvStream {
    pub(crate) fn new(state: SharedConn, id: StreamId) -> Self {
        Self {
            state,
            id,
            finished: false,
        }
    }
}

impl QuicRecvStream for QuinnRecvStream {
    fn read(&mut self) -> Stream<Result<Option<Bytes>, QuicStreamError>, ()> {
        if self.finished {
            return Stream::Next(Ok(None));
        }

        let mut guard = match lock(&self.state) {
            Ok(g) => g,
            Err(e) => return Stream::Next(Err(QuicStreamError::ConnClosed(e))),
        };
        let Ok(qid) = quinn_proto::StreamId::try_from(self.id) else {
            return Stream::Next(Err(QuicStreamError::Terminated { code: 0 }));
        };

        let mut recv = guard.conn.recv_stream(qid);
        let mut chunks = match recv.read(true) {
            Ok(c) => c,
            Err(quinn_proto::ReadableError::ClosedStream) => {
                self.finished = true;
                return Stream::Next(Ok(None));
            }
            Err(quinn_proto::ReadableError::IllegalOrderedRead) => {
                return Stream::Next(Err(QuicStreamError::Other(
                    "illegal ordered read on QUIC stream".into(),
                )))
            }
        };

        // One chunk per call — the trait's contract is one `Stream` value per call.
        let outcome = chunks.next(usize::MAX);

        // `finalize` reports whether quinn wants a transmit (flow-control update).
        // It must run before the connection borrow ends, and the driver will pick
        // up any resulting transmit on its next pass.
        let _ = chunks.finalize();
        drop(guard);

        match outcome {
            Ok(Some(chunk)) => Stream::Next(Ok(Some(chunk.bytes))),
            // End of stream: the peer sent FIN and we have drained everything.
            Ok(None) => {
                self.finished = true;
                Stream::Next(Ok(None))
            }
            // Nothing buffered yet. Not an error — ask again.
            Err(quinn_proto::ReadError::Blocked) => Stream::Pending(()),
            Err(quinn_proto::ReadError::Reset(code)) => {
                self.finished = true;
                Stream::Next(Err(QuicStreamError::Terminated {
                    code: code.into_inner(),
                }))
            }
        }
    }

    fn stop_sending(&mut self, code: u64) {
        let Ok(mut guard) = lock(&self.state) else {
            return;
        };
        let Ok(qid) = quinn_proto::StreamId::try_from(self.id) else {
            return;
        };
        let code = VarInt::from_u64(code).unwrap_or(VarInt::MAX);
        let _ = guard.conn.recv_stream(qid).stop(code);
    }

    fn id(&self) -> StreamId {
        self.id
    }
}

/// A bidirectional `quinn-proto` stream — both halves over one id.
pub struct QuinnBidiStream {
    send: QuinnSendStream,
    recv: QuinnRecvStream,
}

impl std::fmt::Debug for QuinnBidiStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuinnBidiStream")
            .field("id", &self.send.id)
            .finish()
    }
}

impl QuinnBidiStream {
    /// WHY: `QuicBidiStream` inherits `id()` from **both** `QuicSendStream` and
    /// `QuicRecvStream`, so `stream.id()` on a concrete bidi stream is ambiguous
    /// (E0034). Both halves carry the same id, so an inherent method — which wins
    /// over trait methods in resolution — is the fix. Generic code that only knows
    /// the traits still disambiguates with `QuicSendStream::id(&s)`.
    ///
    /// WHAT: this stream's id.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn id(&self) -> StreamId {
        self.send.id
    }

    pub(crate) fn new(state: SharedConn, id: StreamId) -> Self {
        Self {
            send: QuinnSendStream::new(std::sync::Arc::clone(&state), id),
            recv: QuinnRecvStream::new(state, id),
        }
    }
}

impl QuicSendStream for QuinnBidiStream {
    fn send(&mut self, buf: &mut impl Buf) -> Stream<Result<usize, QuicStreamError>, ()> {
        self.send.send(buf)
    }
    fn finish(&mut self) -> Stream<Result<(), QuicStreamError>, ()> {
        self.send.finish()
    }
    fn reset(&mut self, code: u64) {
        self.send.reset(code);
    }
    fn id(&self) -> StreamId {
        self.send.id()
    }
}

impl QuicRecvStream for QuinnBidiStream {
    fn read(&mut self) -> Stream<Result<Option<Bytes>, QuicStreamError>, ()> {
        self.recv.read()
    }
    fn stop_sending(&mut self, code: u64) {
        self.recv.stop_sending(code);
    }
    fn id(&self) -> StreamId {
        self.recv.id()
    }
}

impl QuicBidiStream for QuinnBidiStream {
    fn split(self) -> (impl QuicSendStream, impl QuicRecvStream) {
        (self.send, self.recv)
    }
}
