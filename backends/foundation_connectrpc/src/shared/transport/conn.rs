//! The per-call seam: `HandlerConn` (server) / `ClientConn` (client) and their
//! independently-owned halves (Decision 11 §The seam).
//!
//! WHY: These are the byte-stream objects each protocol writes to without knowing
//! the transport. They are **split into independently-owned halves** so read and
//! write proceed concurrently on full-duplex transports (each half used by one
//! task at a time) — the connect-go contract that is impossible over one `&mut`
//! handle. The seam speaks only in **frames (bytes)**, headers, and trailers —
//! never typed messages or `dyn Any`.
//!
//! WHAT: the [`HandlerConn`]/[`ConnReceiver`]/[`ConnSender`] and
//! [`ClientConn`]/[`ClientSender`]/[`ClientReceiver`] traits, plus the concrete
//! [`PipeHandlerConn`]/[`PipeClientConn`] that bridge the reader/writer tasks over
//! [`FramePipe`]s.
//!
//! HOW: each direction is a bounded [`FramePipe`]; a parked receive/send is
//! composed with the call's [`CancelSignal`] via [`race`] so cancellation wakes it
//! (Decision 11 §Cancellation). Client end-of-stream semantics: a terminal
//! `EndStream.error` surfaces as `Err`, clean EOS as `Ok(None)`.

use std::sync::{Arc, Mutex};

use bytes::Bytes;
use foundation_core::valtron::{PipeReceiver, PipeSender};
use foundation_errstacks::ErrorTrace;
use foundation_netio::shared::http::SimpleHeaders;

use crate::shared::context::{CancelSignal, Peer, Spec};
use crate::shared::error::{ConnectError, ConnectResult};

use super::frame::{race, BoxFuture, Either, Frame, FramePipe, DEFAULT_PIPE_DEPTH};

fn canceled() -> ErrorTrace<ConnectError> {
    ConnectError::canceled("call canceled").into()
}

fn pipe_closed() -> ErrorTrace<ConnectError> {
    ConnectError::unavailable("peer closed the stream").into()
}

/// Await the next frame, waking on cancel as well as pipe readiness.
async fn next_frame(
    rx: &PipeReceiver<Frame>,
    cancel: &CancelSignal,
) -> ConnectResult<Option<Frame>> {
    if cancel.is_canceled() {
        return Err(canceled());
    }
    match race(rx.receive(), cancel.cancelled()).await {
        Either::Left(frame) => Ok(frame),
        Either::Right(()) => Err(canceled()),
    }
}

/// Push a frame, parking on pipe vacancy and waking on cancel.
async fn send_frame(
    tx: &PipeSender<Frame>,
    frame: Frame,
    cancel: &CancelSignal,
) -> ConnectResult<()> {
    if cancel.is_canceled() {
        return Err(canceled());
    }
    match race(tx.send(frame), cancel.cancelled()).await {
        Either::Left(Ok(())) => Ok(()),
        Either::Left(Err(_)) => Err(pipe_closed()),
        Either::Right(()) => Err(canceled()),
    }
}

// ============================================================================
// Server seam
// ============================================================================

/// Server-side per-call seam — speaks only in frames, headers, and trailers.
pub trait HandlerConn: Send {
    /// The procedure spec.
    fn spec(&self) -> &Spec;
    /// The remote peer.
    fn peer(&self) -> &Peer;
    /// Inbound request headers.
    fn request_headers(&self) -> &SimpleHeaders;
    /// Split into independently-owned halves (receiver feeds `MessageSource<Req>`,
    /// sender feeds `MessageSink<Res>`).
    fn split(self: Box<Self>) -> (Box<dyn ConnReceiver>, Box<dyn ConnSender>);
}

/// The receive half of a [`HandlerConn`].
pub trait ConnReceiver: Send {
    /// Next request frame (`None` at end of stream). Parks while the pipe is empty;
    /// cancellation also wakes it.
    fn receive(&mut self) -> BoxFuture<'_, ConnectResult<Option<Bytes>>>;
    /// Trailing metadata observed at end of stream.
    fn trailers(&self) -> &SimpleHeaders;
}

/// The send half of a [`HandlerConn`].
pub trait ConnSender: Send {
    /// Send response headers (idempotent until the first frame).
    fn send_headers(&mut self, headers: SimpleHeaders) -> BoxFuture<'_, ConnectResult<()>>;
    /// Hand one already-encoded response frame to the response pipe (the writer
    /// task envelopes/compresses). Parks while the pipe is full.
    fn send(&mut self, frame: Bytes) -> BoxFuture<'_, ConnectResult<()>>;
    /// Finish: emit end-of-stream then close. Consumes the box (`'static`).
    fn close(
        self: Box<Self>,
        error: Option<ErrorTrace<ConnectError>>,
        trailers: SimpleHeaders,
    ) -> BoxFuture<'static, ConnectResult<()>>;
}

/// The transport/reader-writer side of a [`PipeHandlerConn`]: the protocol's
/// reader task pushes request frames into `request_tx`, and its writer task drains
/// response frames from `response_rx` (reading `response_headers` before the first).
pub struct HandlerConnEnds {
    /// Reader task pushes de-enveloped request frames here.
    pub request_tx: PipeSender<Frame>,
    /// Writer task drains response frames here.
    pub response_rx: PipeReceiver<Frame>,
    /// Response headers set by [`ConnSender::send_headers`], read by the writer.
    pub response_headers: Arc<Mutex<Option<SimpleHeaders>>>,
}

/// A concrete pipe-backed [`HandlerConn`] (Decision 11 — the framework wires its
/// pipes to the reader/writer tasks).
pub struct PipeHandlerConn {
    spec: Spec,
    peer: Peer,
    request_headers: SimpleHeaders,
    req_rx: PipeReceiver<Frame>,
    res_tx: PipeSender<Frame>,
    response_headers: Arc<Mutex<Option<SimpleHeaders>>>,
    cancel: CancelSignal,
}

impl PipeHandlerConn {
    /// Build a conn and the transport-side ends, with the given pipe depth.
    #[must_use]
    pub fn new(
        spec: Spec,
        peer: Peer,
        request_headers: SimpleHeaders,
        cancel: CancelSignal,
        depth: usize,
    ) -> (Self, HandlerConnEnds) {
        let (req_tx, req_rx) = FramePipe::with_depth(depth);
        let (res_tx, res_rx) = FramePipe::with_depth(depth);
        let response_headers = Arc::new(Mutex::new(None));
        let conn = Self {
            spec,
            peer,
            request_headers,
            req_rx,
            res_tx,
            response_headers: Arc::clone(&response_headers),
            cancel,
        };
        let ends = HandlerConnEnds {
            request_tx: req_tx,
            response_rx: res_rx,
            response_headers,
        };
        (conn, ends)
    }

    /// Build with the default pipe depth (Decision 11 §Decided Details 1).
    #[must_use]
    pub fn with_defaults(
        spec: Spec,
        peer: Peer,
        request_headers: SimpleHeaders,
        cancel: CancelSignal,
    ) -> (Self, HandlerConnEnds) {
        Self::new(spec, peer, request_headers, cancel, DEFAULT_PIPE_DEPTH)
    }
}

impl HandlerConn for PipeHandlerConn {
    fn spec(&self) -> &Spec {
        &self.spec
    }
    fn peer(&self) -> &Peer {
        &self.peer
    }
    fn request_headers(&self) -> &SimpleHeaders {
        &self.request_headers
    }
    fn split(self: Box<Self>) -> (Box<dyn ConnReceiver>, Box<dyn ConnSender>) {
        let receiver = PipeConnReceiver {
            rx: self.req_rx,
            cancel: self.cancel.clone(),
            trailers: SimpleHeaders::new(),
        };
        let sender = PipeConnSender {
            tx: self.res_tx,
            response_headers: self.response_headers,
            cancel: self.cancel,
            headers_sent: false,
        };
        (Box::new(receiver), Box::new(sender))
    }
}

struct PipeConnReceiver {
    rx: PipeReceiver<Frame>,
    cancel: CancelSignal,
    trailers: SimpleHeaders,
}

impl ConnReceiver for PipeConnReceiver {
    fn receive(&mut self) -> BoxFuture<'_, ConnectResult<Option<Bytes>>> {
        Box::pin(async move {
            match next_frame(&self.rx, &self.cancel).await? {
                Some(Frame::Message(bytes)) => Ok(Some(bytes)),
                Some(Frame::EndStream { trailers, .. }) => {
                    self.trailers = trailers;
                    Ok(None)
                }
                None => Ok(None),
            }
        })
    }
    fn trailers(&self) -> &SimpleHeaders {
        &self.trailers
    }
}

struct PipeConnSender {
    tx: PipeSender<Frame>,
    response_headers: Arc<Mutex<Option<SimpleHeaders>>>,
    cancel: CancelSignal,
    headers_sent: bool,
}

impl ConnSender for PipeConnSender {
    fn send_headers(&mut self, headers: SimpleHeaders) -> BoxFuture<'_, ConnectResult<()>> {
        Box::pin(async move {
            if !self.headers_sent {
                *self.response_headers.lock().unwrap_or_else(|e| e.into_inner()) = Some(headers);
                self.headers_sent = true;
            }
            Ok(())
        })
    }
    fn send(&mut self, frame: Bytes) -> BoxFuture<'_, ConnectResult<()>> {
        self.headers_sent = true;
        Box::pin(async move { send_frame(&self.tx, Frame::Message(frame), &self.cancel).await })
    }
    fn close(
        self: Box<Self>,
        error: Option<ErrorTrace<ConnectError>>,
        trailers: SimpleHeaders,
    ) -> BoxFuture<'static, ConnectResult<()>> {
        Box::pin(async move {
            send_frame(&self.tx, Frame::EndStream { error, trailers }, &self.cancel).await?;
            self.tx.close();
            Ok(())
        })
    }
}

// ============================================================================
// Client seam
// ============================================================================

/// Client-side per-call seam.
pub trait ClientConn: Send {
    /// The procedure spec.
    fn spec(&self) -> &Spec;
    /// Outbound request headers (before split / first send).
    fn request_headers_mut(&mut self) -> &mut SimpleHeaders;
    /// Split into independently-owned halves.
    fn split(self: Box<Self>) -> (Box<dyn ClientSender>, Box<dyn ClientReceiver>);
}

/// The send half of a [`ClientConn`].
pub trait ClientSender: Send {
    /// Outbound request headers, valid until the first `send`/`flush_headers`.
    fn request_headers_mut(&mut self) -> &mut SimpleHeaders;
    /// Header-only send: render the head now, before any body frame.
    fn flush_headers(&mut self) -> BoxFuture<'_, ConnectResult<()>>;
    /// Send one already-encoded request frame; parks on request-pipe vacancy.
    fn send(&mut self, frame: Bytes) -> BoxFuture<'_, ConnectResult<()>>;
    /// Half-close the request direction; the receiver half stays live.
    fn close_send(self: Box<Self>) -> BoxFuture<'static, ConnectResult<()>>;
}

/// The receive half of a [`ClientConn`].
pub trait ClientReceiver: Send {
    /// Next response frame. A terminal `EndStream.error` (Connect
    /// `EndStreamResponse.error` / `grpc-status != 0`) returns `Err`; clean EOS
    /// returns `Ok(None)`. Trailers are available after either outcome.
    fn receive(&mut self) -> BoxFuture<'_, ConnectResult<Option<Bytes>>>;
    /// Response headers (may wait on the network in a real transport).
    fn response_headers(&mut self) -> BoxFuture<'_, ConnectResult<&SimpleHeaders>>;
    /// Trailing metadata (available after end of stream).
    fn response_trailers(&mut self) -> BoxFuture<'_, ConnectResult<&SimpleHeaders>>;
}

/// The transport/reader-writer side of a [`PipeClientConn`].
pub struct ClientConnEnds {
    /// Writer task drains request frames here.
    pub request_rx: PipeReceiver<Frame>,
    /// Reader task pushes response frames here.
    pub response_tx: PipeSender<Frame>,
    /// Response headers the reader sets once the head arrives.
    pub response_headers: Arc<Mutex<Option<SimpleHeaders>>>,
}

/// A concrete pipe-backed [`ClientConn`].
pub struct PipeClientConn {
    spec: Spec,
    request_headers: SimpleHeaders,
    req_tx: PipeSender<Frame>,
    res_rx: PipeReceiver<Frame>,
    response_headers: Arc<Mutex<Option<SimpleHeaders>>>,
    cancel: CancelSignal,
}

impl PipeClientConn {
    /// Build a client conn and the transport-side ends, with the given pipe depth.
    #[must_use]
    pub fn new(
        spec: Spec,
        request_headers: SimpleHeaders,
        cancel: CancelSignal,
        depth: usize,
    ) -> (Self, ClientConnEnds) {
        let (req_tx, req_rx) = FramePipe::with_depth(depth);
        let (res_tx, res_rx) = FramePipe::with_depth(depth);
        let response_headers = Arc::new(Mutex::new(None));
        let conn = Self {
            spec,
            request_headers,
            req_tx,
            res_rx,
            response_headers: Arc::clone(&response_headers),
            cancel,
        };
        let ends = ClientConnEnds {
            request_rx: req_rx,
            response_tx: res_tx,
            response_headers,
        };
        (conn, ends)
    }

    /// Build with the default pipe depth.
    #[must_use]
    pub fn with_defaults(
        spec: Spec,
        request_headers: SimpleHeaders,
        cancel: CancelSignal,
    ) -> (Self, ClientConnEnds) {
        Self::new(spec, request_headers, cancel, DEFAULT_PIPE_DEPTH)
    }
}

impl ClientConn for PipeClientConn {
    fn spec(&self) -> &Spec {
        &self.spec
    }
    fn request_headers_mut(&mut self) -> &mut SimpleHeaders {
        &mut self.request_headers
    }
    fn split(self: Box<Self>) -> (Box<dyn ClientSender>, Box<dyn ClientReceiver>) {
        let sender = PipeClientSender {
            tx: self.req_tx,
            request_headers: self.request_headers,
            cancel: self.cancel.clone(),
            flushed: false,
        };
        let receiver = PipeClientReceiver {
            rx: self.res_rx,
            response_headers: self.response_headers,
            headers_cache: None,
            trailers: SimpleHeaders::new(),
            cancel: self.cancel,
        };
        (Box::new(sender), Box::new(receiver))
    }
}

struct PipeClientSender {
    tx: PipeSender<Frame>,
    request_headers: SimpleHeaders,
    cancel: CancelSignal,
    flushed: bool,
}

impl ClientSender for PipeClientSender {
    fn request_headers_mut(&mut self) -> &mut SimpleHeaders {
        &mut self.request_headers
    }
    fn flush_headers(&mut self) -> BoxFuture<'_, ConnectResult<()>> {
        Box::pin(async move {
            self.flushed = true;
            Ok(())
        })
    }
    fn send(&mut self, frame: Bytes) -> BoxFuture<'_, ConnectResult<()>> {
        self.flushed = true;
        Box::pin(async move { send_frame(&self.tx, Frame::Message(frame), &self.cancel).await })
    }
    fn close_send(self: Box<Self>) -> BoxFuture<'static, ConnectResult<()>> {
        Box::pin(async move {
            self.tx.close();
            Ok(())
        })
    }
}

struct PipeClientReceiver {
    rx: PipeReceiver<Frame>,
    response_headers: Arc<Mutex<Option<SimpleHeaders>>>,
    headers_cache: Option<SimpleHeaders>,
    trailers: SimpleHeaders,
    cancel: CancelSignal,
}

impl ClientReceiver for PipeClientReceiver {
    fn receive(&mut self) -> BoxFuture<'_, ConnectResult<Option<Bytes>>> {
        Box::pin(async move {
            match next_frame(&self.rx, &self.cancel).await? {
                Some(Frame::Message(bytes)) => Ok(Some(bytes)),
                Some(Frame::EndStream { error, trailers }) => {
                    self.trailers = trailers;
                    match error {
                        Some(e) => Err(e),
                        None => Ok(None),
                    }
                }
                None => Ok(None),
            }
        })
    }
    fn response_headers(&mut self) -> BoxFuture<'_, ConnectResult<&SimpleHeaders>> {
        Box::pin(async move {
            if self.headers_cache.is_none() {
                let guard = self.response_headers.lock().unwrap_or_else(|e| e.into_inner());
                self.headers_cache = Some(guard.clone().unwrap_or_default());
            }
            Ok(self.headers_cache.as_ref().expect("cached above"))
        })
    }
    fn response_trailers(&mut self) -> BoxFuture<'_, ConnectResult<&SimpleHeaders>> {
        Box::pin(async move { Ok(&self.trailers) })
    }
}
