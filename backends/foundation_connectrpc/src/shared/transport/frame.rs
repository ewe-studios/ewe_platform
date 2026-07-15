//! The frame carried by the streaming seam + shared async plumbing (Decision 11).
//!
//! WHY: The seam's bounded pipes carry **codec-level payloads plus a normalized
//! end-of-stream** — never raw envelope bytes (bare `Bytes` could not carry the
//! envelope flags, so Connect `0x02` / gRPC-Web `0x80` / h2 trailing HEADERS
//! would be indistinguishable from messages). A `FramePipe` is the `Pipe<Frame>`
//! instantiation of the bounded pipe primitive (feature 00-F4 / F02).
//!
//! WHAT: [`Frame`], [`FramePipe`], the [`BoxFuture`] alias the dyn-safe async seam
//! returns, and a [`race`] combinator used to compose a parked pipe op with the
//! call's cancel signal (Decision 11 §Cancellation).

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use foundation_core::valtron::Pipe;
use foundation_errstacks::ErrorTrace;
use foundation_netio::shared::http::SimpleHeaders;

use crate::shared::error::ConnectError;

/// Default frames-in-flight per pipe (Decision 11 §Decided Details 1).
pub const DEFAULT_PIPE_DEPTH: usize = 4;

/// A dyn-safe boxed future — the return type of every frame-moving seam method.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// What a seam pipe carries: codec-encoded payloads plus a normalized
/// end-of-stream (Decision 11).
pub enum Frame {
    /// One codec-encoded message (de-enveloped + decompressed on receive; not yet
    /// enveloped on send).
    Message(Bytes),
    /// The protocol's end-of-stream, normalized across Connect / gRPC / gRPC-Web.
    EndStream {
        /// A terminal error, if the stream ended in failure.
        error: Option<ErrorTrace<ConnectError>>,
        /// Trailing metadata.
        trailers: SimpleHeaders,
    },
}

impl core::fmt::Debug for Frame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Frame::Message(b) => f.debug_tuple("Message").field(&b.len()).finish(),
            Frame::EndStream { error, trailers } => f
                .debug_struct("EndStream")
                .field("error", &error.is_some())
                .field("trailers", &trailers.len())
                .finish(),
        }
    }
}

/// The seam's bounded frame pipe (`Pipe<Frame>`).
pub type FramePipe = Pipe<Frame>;

/// The winner of a [`race`] between two futures.
pub enum Either<A, B> {
    /// The first future completed.
    Left(A),
    /// The second future completed.
    Right(B),
}

/// Poll two futures concurrently, resolving as soon as **either** completes
/// (favoring `a` on a tie). Both wakers are registered, so a parked task wakes on
/// either source — this is how a pipe recv/send is composed with the call's
/// cancel signal (Decision 11 §Cancellation).
pub fn race<A: Future, B: Future>(a: A, b: B) -> Race<A, B> {
    Race { a, b }
}

/// Future returned by [`race`].
pub struct Race<A, B> {
    a: A,
    b: B,
}

impl<A: Future, B: Future> Future for Race<A, B> {
    type Output = Either<A::Output, B::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: we never move `a`/`b` out of `self`; we only project pinned
        // references to poll them in place.
        let this = unsafe { self.get_unchecked_mut() };
        let a = unsafe { Pin::new_unchecked(&mut this.a) };
        if let Poll::Ready(v) = a.poll(cx) {
            return Poll::Ready(Either::Left(v));
        }
        let b = unsafe { Pin::new_unchecked(&mut this.b) };
        if let Poll::Ready(v) = b.poll(cx) {
            return Poll::Ready(Either::Right(v));
        }
        Poll::Pending
    }
}
