//! `H2Serve` — the HTTP/2 equivalent of [`Serve`](super::Serve) (F47).
//!
//! WHY: `Serve` hands a handler the whole request plus the raw stream, because
//! HTTP/1.1 runs one exchange at a time on a connection. HTTP/2 does not: one
//! connection multiplexes many concurrent streams, so a handler must never be
//! given the connection. It gets a body pipe to read from and a response pipe
//! to write into; the connection's poll loop stays the only writer of `H2Conn`.
//!
//! WHAT: [`H2Serve`], returning a future the connection handler spawns on the
//! valtron pool — one task per h2 stream.
//!
//! HOW: The handler drains `body` until it closes, pushes [`H2Frame`]s into
//! `tx`, and drops `tx` when done. `H2ConnectionHandler` drains the paired
//! receiver each poll and serializes those frames onto the shared write buffer.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;

use foundation_core::valtron::{PipeReceiver, PipeSender};
use foundation_netio::http2::types::{H2Frame, H2IncomingFrame, SimpleIncomingRequestHeader};

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
