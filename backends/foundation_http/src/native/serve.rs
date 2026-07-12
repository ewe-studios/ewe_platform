//! `H2Serve` — the HTTP/2 equivalent of [`Serve`](crate::shared::serve::Serve) (F47).
//!
//! Native-only: the HTTP/2 serve path drives `foundation_netio::http2` (the native
//! frame substrate), which does not exist on wasm. It lives under `native/` so the
//! `shared/` tree stays genuinely cross-platform.
//!
//! WHY: `Serve` hands a handler the whole request plus the raw stream, because
//! HTTP/1.1 runs one exchange at a time on a connection. HTTP/2 does not: one
//! connection multiplexes many concurrent streams, so a handler must never be
//! given the connection. It gets a body pipe to read from and a response pipe
//! to write into; the connection's poll loop stays the only writer of `H2Conn`.
//!
//! WHAT: [`H2Serve`], returning a future the connection handler spawns on the
//! valtron pool — one task per h2 stream. [`H3Serve`] (F35), the HTTP/3 analogue:
//! one task per QUIC bidi stream, driving the poll-based `H3Request` API.
//!
//! HOW: The handler drains `body` until it closes, pushes [`H2Frame`]s into
//! `tx`, and drops `tx` when done. `H2ConnectionHandler` drains the paired
//! receiver each poll and serializes those frames onto the shared write buffer.
//! For H3, the handler owns the `H3Request` and drives `poll_headers` /
//! `poll_body` / `poll_send_*` to completion.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;

use foundation_core::valtron::{PipeReceiver, PipeSender};
use foundation_netio::http2::types::{H2Frame, H2IncomingFrame, SimpleIncomingRequestHeader};
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use foundation_netio::http3::connection::H3Request;
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use foundation_netio::quic::QuinnBidiStream;

use crate::shared::context::ContextBag;

/// A boxed, sendable future — the unit of work the connection handler spawns.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Core HTTP/2 handler trait — one invocation per h2 stream.
///
/// Implementors **must not** block: the returned future is polled on the
/// valtron pool alongside every other stream on the same connection.
pub trait H2Serve: Send + Sync + 'static {
    /// Handle one h2 stream.
    ///
    /// `body` yields [`H2IncomingFrame::Data`] until the peer sets `END_STREAM`
    /// (the pipe then closes), or a single [`H2IncomingFrame::Reset`] if the
    /// peer cancelled. Response frames are pushed into `tx`; dropping `tx`
    /// signals the stream is complete.
    fn serve_h2(
        &self,
        bag: Arc<ContextBag>,
        header: SimpleIncomingRequestHeader,
        body: PipeReceiver<H2IncomingFrame>,
        tx: PipeSender<H2Frame>,
    ) -> BoxFuture<'static, io::Result<()>>;
}

// ── H3Serve (F35 server half) ────────────────────────────────────────────

/// Core HTTP/3 handler trait — one invocation per QUIC bidi stream (F35).
///
/// Like [`H2Serve`], implementors **must not** block: the returned future is
/// polled on the valtron pool alongside every other stream on the same QUIC
/// connection. One task per request — the connection's poll loop stays the only
/// driver of the QUIC endpoint, and the handler only touches its own stream.
///
/// The `H3Request` is handed to the handler by value; the handler owns it and
/// drives `poll_headers` / `poll_body` / `poll_send_*` to completion, then
/// drops it.
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
pub trait H3Serve: Send + Sync + 'static {
    /// Handle one HTTP/3 request stream.
    ///
    /// The handler drives the [`H3Request`] through its poll-based API (headers,
    /// body chunks, response headers/data, finish). When the returned future
    /// completes, the stream is done and the response has been fully flushed.
    fn serve_h3(
        &self,
        bag: Arc<ContextBag>,
        request: H3Request<QuinnBidiStream>,
    ) -> BoxFuture<'static, io::Result<()>>;
}
