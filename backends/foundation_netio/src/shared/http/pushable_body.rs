//! Pushable client request body over the 00-F4 `Pipe<Bytes>` (Decision 12 §7).
//!
//! WHY: A client-streaming / bidi RPC must **push** request-body bytes *after* the
//! request has started sending (connect-go `io.Pipe` parity) without buffering the
//! whole request. B6 in the review.
//!
//! WHAT: A factory that pairs a [`PushableRequestBody`] producer handle with a
//! `SendSafeBody::Stream` backed by a bounded, waker-hooked [`Pipe<Bytes>`]. The
//! producer half is held by the client-stream/bidi task; the consumer half is
//! drained by the request renderer.
//!
//! HOW: Bytes flow through the bounded pipe (backpressure, no whole-request
//! buffering). This uses the 00-F4 `Pipe` primitive — **not**
//! `ConcurrentQueueStreamIterator`, whose internal `max_turns`/`park_duration`
//! polling handoff 00-F4 exists to remove: a full pipe parks the async producer on
//! its vacancy waker, and an empty pipe lets the consuming task park on the pipe's
//! `QueueReadiness` (feature 23's writer task). The sync `SendSafeBody::Stream`
//! adapter surfaces a transient empty pipe as `Data::Retry` (the existing
//! renderer's "no data yet" signal), never an internal spin.

use bytes::Bytes;

use foundation_core::extensions::result_ext::BoxedError;
use foundation_core::io::readers::Data;
use foundation_core::valtron::{
    BoxedSendableDataIterator, Pipe, PipeReceiver, PipeSender, SendError, TryRecvError,
    TrySendError,
};

use crate::simple_http::shared::impls::SendSafeBody;

/// Default depth of a pushable request-body pipe (matches the seam pipe default).
pub const DEFAULT_PUSHABLE_DEPTH: usize = 4;

/// Producer handle for a pushable request body.
///
/// The client-stream / bidi task holds this and pushes request-body chunks after
/// the request head has been sent. Dropping it (or calling [`close`](Self::close))
/// ends the body so the renderer emits the final chunk.
pub struct PushableRequestBody {
    sender: PipeSender<Bytes>,
}

impl PushableRequestBody {
    /// Non-blocking push of one body chunk. On success a parked consumer wakes.
    ///
    /// # Errors
    /// [`TrySendError::Full`] if the bounded pipe is at capacity (push later or use
    /// [`push`](Self::push) to await vacancy); [`TrySendError::Closed`] if the body
    /// was already ended / the renderer dropped.
    pub fn try_push(&self, chunk: impl Into<Bytes>) -> Result<(), TrySendError<Bytes>> {
        self.sender.try_send(chunk.into())
    }

    /// Async push of one body chunk. On a full pipe the future **parks** (vacancy
    /// waker, Decision 00 L1b), woken by the renderer draining a chunk.
    ///
    /// # Errors
    /// [`SendError`] (carrying the chunk) if the body/pipe is closed.
    ///
    /// # Panics
    /// Never panics.
    pub async fn push(&self, chunk: impl Into<Bytes>) -> Result<(), SendError<Bytes>> {
        self.sender.send(chunk.into()).await
    }

    /// End the request body — the renderer then emits its final chunk and finishes.
    /// Idempotent; also happens automatically on drop.
    pub fn close(&self) {
        self.sender.close();
    }

    /// Whether the body has been ended (by `close`, drop, or the renderer going away).
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }

    /// Consume the producer and return its raw [`PipeSender<Bytes>`].
    ///
    /// WHY: a byte-level transport seam (spec 41 Decision 11 §Connection ownership) wants the
    /// pushable body's producer pipe to *be* its `send_body: PipeSender<Bytes>` directly — the
    /// caller pushes wire request bytes into `send_body`, and the connection-owner pump drains the
    /// paired receiver (this body's `SendSafeBody::Stream`) with no intermediate bridge task or
    /// copy. `push`/`try_push`/`close` are exactly `PipeSender::send`/`try_send`/`close`, so
    /// handing out the sender loses nothing.
    #[must_use]
    pub fn into_sender(self) -> PipeSender<Bytes> {
        self.sender
    }
}

/// Consumer iterator that drains the pipe into `Data` batches for the request
/// renderer (`SendSafeBody::Stream`).
struct PushableBodyReader {
    receiver: PipeReceiver<Bytes>,
}

impl Iterator for PushableBodyReader {
    type Item = Result<Data, BoxedError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.receiver.try_recv() {
            // A pushed chunk — hand it to the renderer (wakes a parked producer).
            Ok(bytes) => Some(Ok(Data::Bytes(bytes.to_vec()))),
            // Open but empty: transient "no data yet" — the driving task parks on
            // the pipe readiness rather than spinning here.
            Err(TryRecvError::Empty) => Some(Ok(Data::Retry)),
            // Closed and drained: end of the request body.
            Err(TryRecvError::Closed) => None,
        }
    }
}

/// Create a pushable request body with the [`DEFAULT_PUSHABLE_DEPTH`].
///
/// Returns the producer handle and the `SendSafeBody::Stream` to attach to the
/// outgoing request (e.g. via `with_body_stream`).
#[must_use]
pub fn pushable_request_body() -> (PushableRequestBody, SendSafeBody) {
    pushable_request_body_with_depth(DEFAULT_PUSHABLE_DEPTH)
}

/// Create a pushable request body with an explicit pipe depth (clamped to ≥ 1).
///
/// # Panics
/// Never panics.
#[must_use]
pub fn pushable_request_body_with_depth(depth: usize) -> (PushableRequestBody, SendSafeBody) {
    let (sender, receiver) = Pipe::<Bytes>::with_depth(depth);
    let reader: BoxedSendableDataIterator<BoxedError> = Box::new(PushableBodyReader { receiver });
    (
        PushableRequestBody { sender },
        SendSafeBody::Stream(Some(reader)),
    )
}
