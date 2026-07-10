//! `Pipe<T>` — the bounded, two-sided waker-hooked seam pipe (Decision 00 L1b / 00-F4).
//!
//! WHY: Streaming RPC bodies are modeled as bounded pipes between cooperating
//! valtron tasks / async handlers (Decision 11). A bounded queue gives
//! backpressure, but on its own a full/empty pipe would degrade to bare
//! `Pending` re-polling — the busy-poll Decision 00 exists to remove. `Pipe<T>`
//! closes that: each end can **park** (async await, or a valtron-task `Depends`)
//! and is woken by the *opposite* end's `push`/`pop`.
//!
//! WHAT: A bounded `ConcurrentQueue<T>` plus two single-slot waker stashes
//! (producer / consumer). It hands out two independently-owned halves —
//! [`PipeSender<T>`] and [`PipeReceiver<T>`] — so read and write proceed
//! concurrently (each half used by one task at a time). Both the async
//! (`send`/`receive`) and the valtron-task (`try_send`/`try_recv` +
//! [`QueueReadiness`]/[`QueueVacancyReadiness`]) surfaces are provided; the two
//! interoperate (a task-path `pop` wakes an async-path `send`, and vice-versa).
//!
//! HOW: A successful `push` fires the stashed **consumer** waker; a successful
//! `pop` fires the stashed **producer** waker (Decision 00 L1b two-sided wake).
//! The async futures stash-then-re-check to close the wake-before-park race, so
//! there is **no `max_turns`/`park_duration` polling handoff** anywhere — a full
//! pipe parks the producer on [`QueueVacancyReadiness`], an empty pipe parks the
//! consumer on [`QueueReadiness`], and cancellation composes via
//! [`AnyReadiness`](crate::valtron::AnyReadiness) (task path) or by the canceller
//! firing the parked future's waker (async path, wired by the call's
//! `CancelSignal`).

use core::fmt;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use std::sync::Arc;

use concurrent_queue::{ConcurrentQueue, PopError, PushError};

use crate::compati::Mutex;
use crate::valtron::{QueueReadiness, QueueVacancyReadiness};

/// Shared state behind both pipe halves: the bounded queue plus the two waker
/// stashes fired by the opposite end.
pub(crate) struct PipeInner<T> {
    /// Bounded queue. Held behind its own `Arc` so the readiness accessors can
    /// hand out `QueueReadiness`/`QueueVacancyReadiness` over the same queue.
    pub(crate) queue: Arc<ConcurrentQueue<T>>,
    /// Waker of a consumer parked on an empty pipe; fired by a producer `push`.
    pub(crate) consumer_waker: Mutex<Option<Waker>>,
    /// Waker of a producer parked on a full pipe; fired by a consumer `pop`.
    pub(crate) producer_waker: Mutex<Option<Waker>>,
}

impl<T> PipeInner<T> {
    /// Wake a consumer parked on an empty pipe (called after a successful push).
    pub(crate) fn wake_consumer(&self) {
        if let Some(waker) = self.consumer_waker.lock().unwrap().take() {
            waker.wake();
        }
    }

    /// Wake a producer parked on a full pipe (called after a successful pop).
    pub(crate) fn wake_producer(&self) {
        if let Some(waker) = self.producer_waker.lock().unwrap().take() {
            waker.wake();
        }
    }
}

/// The bounded two-sided seam pipe primitive (Decision 00 L1b / 00-F4).
///
/// `Pipe<T>` is a namespace for the constructors; the actual endpoints are the
/// owned halves [`PipeSender<T>`] and [`PipeReceiver<T>`]. Instantiations named
/// by the design: `FramePipe = Pipe<Frame>` (the seam) and `Pipe<Bytes>` (the
/// transport byte pipes), both defined in the connectrpc crate.
///
/// # Panics
/// Never panics. (`with_depth` clamps a `0` depth up to `1`.)
pub struct Pipe<T>(core::marker::PhantomData<T>);

impl<T> Pipe<T> {
    /// Default per-pipe depth (Decision 11 §Decided Details): 4 messages. Amortizes
    /// park/wake round-trips on bursty producers while keeping in-flight frames
    /// tightly bounded.
    pub const DEFAULT_DEPTH: usize = 4;

    /// Create a pipe of [`DEFAULT_DEPTH`](Self::DEFAULT_DEPTH) and return its halves.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn new() -> (PipeSender<T>, PipeReceiver<T>) {
        Self::with_depth(Self::DEFAULT_DEPTH)
    }

    /// Create a pipe with the given bounded depth (clamped to a minimum of 1) and
    /// return its owned sender/receiver halves.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn with_depth(depth: usize) -> (PipeSender<T>, PipeReceiver<T>) {
        let inner = Arc::new(PipeInner {
            queue: Arc::new(ConcurrentQueue::bounded(depth.max(1))),
            consumer_waker: Mutex::new(None),
            producer_waker: Mutex::new(None),
        });
        (
            PipeSender {
                inner: inner.clone(),
            },
            PipeReceiver { inner },
        )
    }
}

// ============================================================================
// Sender half
// ============================================================================

/// The producer half of a [`Pipe<T>`]. Not `Clone` — the seam model gives each
/// pipe exactly one sender and one receiver.
pub struct PipeSender<T> {
    pub(crate) inner: Arc<PipeInner<T>>,
}

impl<T> Clone for PipeSender<T> {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone() }
    }
}

impl<T> PipeSender<T> {
    /// Non-blocking push (valtron-task producer path). On success, wakes a parked
    /// consumer.
    ///
    /// WHY: A valtron task producing into the pipe does one step per poll and
    /// parks via `Depends`, rather than awaiting.
    ///
    /// # Errors
    /// [`TrySendError::Full`] if the pipe is at capacity (park on
    /// [`vacancy`](Self::vacancy)); [`TrySendError::Closed`] if the receiver is gone.
    ///
    /// # Panics
    /// Never panics.
    pub fn try_send(&self, item: T) -> Result<(), TrySendError<T>> {
        match self.inner.queue.push(item) {
            Ok(()) => {
                self.inner.wake_consumer();
                Ok(())
            }
            Err(PushError::Full(item)) => Err(TrySendError::Full(item)),
            Err(PushError::Closed(item)) => Err(TrySendError::Closed(item)),
        }
    }

    /// Async send: encode-free hand-off of one item. On a full pipe the future
    /// **parks** — its waker is stashed and fired by the consumer's `pop`
    /// (Decision 00 L1b) — never a bare-`Pending` re-poll, never an `Err(Full)`.
    ///
    /// # Errors
    /// [`SendError`] (carrying the un-sent item) if the pipe is closed because the
    /// receiver was dropped.
    ///
    /// # Panics
    /// Never panics.
    #[must_use = "futures do nothing unless awaited"]
    pub fn send(&self, item: T) -> SendFuture<'_, T> {
        SendFuture {
            sender: self,
            item: Some(item),
        }
    }

    /// Producer-side readiness for the valtron-task path: park with
    /// `Depends(sender.vacancy())` (or compose it into an
    /// [`AnyReadiness`](crate::valtron::AnyReadiness) with a cancel signal).
    #[must_use]
    pub fn vacancy(&self) -> QueueVacancyReadiness<T>
    where
        T: Send,
    {
        QueueVacancyReadiness::new(self.inner.queue.clone())
    }

    /// `true` if the pipe is currently at capacity.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.inner.queue.is_full()
    }

    /// `true` once either half has closed the pipe.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.inner.queue.is_closed()
    }

    /// Number of buffered items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.queue.len()
    }

    /// `true` if no items are buffered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.queue.is_empty()
    }

    /// Bounded capacity (depth) of the pipe.
    ///
    /// # Panics
    /// Never panics (a bounded `ConcurrentQueue` always reports a capacity).
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.inner
            .queue
            .capacity()
            .expect("Pipe is always constructed bounded")
    }

    /// Explicitly close the send direction (also done on drop). Wakes a parked
    /// consumer so it observes end-of-stream. Returns `true` if this call closed it.
    pub fn close(&self) -> bool {
        let closed = self.inner.queue.close();
        self.inner.wake_consumer();
        closed
    }
}

impl<T> Drop for PipeSender<T> {
    fn drop(&mut self) {
        // Closing on sender drop is the pipe's EOF: a parked consumer wakes and
        // its next `pop` drains any backlog then reports end-of-stream.
        self.inner.queue.close();
        self.inner.wake_consumer();
    }
}

/// Future returned by [`PipeSender::send`]; parks on a full pipe.
pub struct SendFuture<'a, T> {
    sender: &'a PipeSender<T>,
    item: Option<T>,
}

// Not self-referential: `item` is only ever moved in/out, never pin-projected.
impl<T> Unpin for SendFuture<'_, T> {}

impl<T> Future for SendFuture<'_, T> {
    type Output = Result<(), SendError<T>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let item = self
            .item
            .take()
            .expect("SendFuture polled after completion");
        match self.sender.inner.queue.push(item) {
            Ok(()) => {
                self.sender.inner.wake_consumer();
                Poll::Ready(Ok(()))
            }
            Err(PushError::Closed(item)) => Poll::Ready(Err(SendError(item))),
            Err(PushError::Full(item)) => {
                // Stash our waker, THEN re-check: a `pop` between the failed push
                // and the stash would otherwise be a lost wakeup.
                *self.sender.inner.producer_waker.lock().unwrap() = Some(cx.waker().clone());
                match self.sender.inner.queue.push(item) {
                    Ok(()) => {
                        self.sender.inner.wake_consumer();
                        Poll::Ready(Ok(()))
                    }
                    Err(PushError::Closed(item)) => Poll::Ready(Err(SendError(item))),
                    Err(PushError::Full(item)) => {
                        self.item = Some(item);
                        Poll::Pending
                    }
                }
            }
        }
    }
}

// ============================================================================
// Receiver half
// ============================================================================

/// The consumer half of a [`Pipe<T>`]. Not `Clone` — one receiver per pipe.
pub struct PipeReceiver<T> {
    pub(crate) inner: Arc<PipeInner<T>>,
}

impl<T> Clone for PipeReceiver<T> {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone() }
    }
}

impl<T> PipeReceiver<T> {
    /// Non-blocking pop (valtron-task consumer path). On success, wakes a parked
    /// producer.
    ///
    /// # Errors
    /// [`TryRecvError::Empty`] if no item is buffered (park on
    /// [`readiness`](Self::readiness)); [`TryRecvError::Closed`] if the pipe is
    /// closed and drained (end-of-stream).
    ///
    /// # Panics
    /// Never panics.
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        match self.inner.queue.pop() {
            Ok(item) => {
                self.inner.wake_producer();
                Ok(item)
            }
            Err(PopError::Empty) => Err(TryRecvError::Empty),
            Err(PopError::Closed) => Err(TryRecvError::Closed),
        }
    }

    /// Async receive: next item, or `None` at end-of-stream (pipe closed and
    /// drained). On an empty (open) pipe the future **parks** — its waker is
    /// stashed and fired by the producer's `push` (Decision 00 L1b).
    ///
    /// # Panics
    /// Never panics.
    #[must_use = "futures do nothing unless awaited"]
    pub fn receive(&self) -> RecvFuture<'_, T> {
        RecvFuture { receiver: self }
    }

    /// Consumer-side readiness for the valtron-task path: park with
    /// `Depends(receiver.readiness())` (or compose it into an
    /// [`AnyReadiness`](crate::valtron::AnyReadiness) with a cancel signal).
    #[must_use]
    pub fn readiness(&self) -> QueueReadiness<T>
    where
        T: Send,
    {
        QueueReadiness::new(self.inner.queue.clone())
    }

    /// `true` if no items are currently buffered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.queue.is_empty()
    }

    /// `true` once either half has closed the pipe.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.inner.queue.is_closed()
    }

    /// Number of buffered items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.queue.len()
    }

    /// Explicitly close the receive direction (also done on drop). Wakes a parked
    /// producer so it observes the close. Returns `true` if this call closed it.
    pub fn close(&self) -> bool {
        let closed = self.inner.queue.close();
        self.inner.wake_producer();
        closed
    }
}

impl<T> Drop for PipeReceiver<T> {
    fn drop(&mut self) {
        // A dropped receiver closes the pipe so a producer parked on a full pipe
        // wakes and observes `Closed` on its next send rather than hanging.
        self.inner.queue.close();
        self.inner.wake_producer();
    }
}

/// Future returned by [`PipeReceiver::receive`]; parks on an empty pipe.
pub struct RecvFuture<'a, T> {
    receiver: &'a PipeReceiver<T>,
}

// Holds no `T` by value — trivially movable.
impl<T> Unpin for RecvFuture<'_, T> {}

impl<T> Future for RecvFuture<'_, T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.receiver.inner.queue.pop() {
            Ok(item) => {
                self.receiver.inner.wake_producer();
                Poll::Ready(Some(item))
            }
            Err(PopError::Closed) => Poll::Ready(None),
            Err(PopError::Empty) => {
                // Stash our waker, THEN re-check: a `push` between the failed pop
                // and the stash would otherwise be a lost wakeup.
                *self.receiver.inner.consumer_waker.lock().unwrap() = Some(cx.waker().clone());
                match self.receiver.inner.queue.pop() {
                    Ok(item) => {
                        self.receiver.inner.wake_producer();
                        Poll::Ready(Some(item))
                    }
                    Err(PopError::Closed) => Poll::Ready(None),
                    Err(PopError::Empty) => Poll::Pending,
                }
            }
        }
    }
}

// ============================================================================
// Error types
// ============================================================================

/// Error from [`PipeSender::try_send`].
///
/// `Debug`/`Display` intentionally do not print the carried item, so the error is
/// printable even when `T` is not (channel-error ergonomics).
pub enum TrySendError<T> {
    /// The pipe is at capacity; the item is returned. Park on
    /// [`PipeSender::vacancy`] and retry.
    Full(T),
    /// The pipe is closed (receiver dropped); the item is returned.
    Closed(T),
}

impl<T> TrySendError<T> {
    /// Recover the item that could not be sent.
    pub fn into_inner(self) -> T {
        match self {
            TrySendError::Full(item) | TrySendError::Closed(item) => item,
        }
    }

    /// `true` if the failure was a full pipe (retryable once vacancy opens).
    #[must_use]
    pub fn is_full(&self) -> bool {
        matches!(self, TrySendError::Full(_))
    }
}

impl<T> fmt::Debug for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrySendError::Full(_) => f.write_str("TrySendError::Full(..)"),
            TrySendError::Closed(_) => f.write_str("TrySendError::Closed(..)"),
        }
    }
}

impl<T> fmt::Display for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrySendError::Full(_) => f.write_str("sending into a full pipe"),
            TrySendError::Closed(_) => f.write_str("sending into a closed pipe"),
        }
    }
}

#[cfg(feature = "std")]
impl<T> std::error::Error for TrySendError<T> {}

/// Error from [`PipeReceiver::try_recv`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryRecvError {
    /// No item buffered; park on [`PipeReceiver::readiness`] and retry.
    Empty,
    /// The pipe is closed and drained — end-of-stream.
    Closed,
}

impl fmt::Display for TryRecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TryRecvError::Empty => f.write_str("receiving from an empty pipe"),
            TryRecvError::Closed => f.write_str("receiving from a closed, drained pipe"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TryRecvError {}

/// Error from [`PipeSender::send`]: the pipe was closed (receiver dropped). Carries
/// the un-sent item so the caller can recover it.
pub struct SendError<T>(pub T);

impl<T> SendError<T> {
    /// Recover the item that could not be sent.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> fmt::Debug for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SendError(..)")
    }
}

impl<T> fmt::Display for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("sending into a closed pipe")
    }
}

#[cfg(feature = "std")]
impl<T> std::error::Error for SendError<T> {}

