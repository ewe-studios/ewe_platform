//! Future-to-TaskIterator adapter for seamless async integration.
//!
//! This module provides adapters that allow standard Rust `Future`s and `Stream`s
//! to be executed through the valtron executor system without requiring a full
//! async runtime.

#[cfg(any(feature = "std", feature = "alloc"))]
use core::future::Future;
#[cfg(any(feature = "std", feature = "alloc"))]
use core::marker::PhantomData;
#[cfg(any(feature = "std", feature = "alloc"))]
use core::task::{RawWaker, RawWakerVTable, Waker};

#[cfg(any(feature = "std", feature = "alloc"))]
use crate::valtron::{NoAction, QueueReadiness, TaskIterator, TaskStatus};
#[cfg(any(feature = "std", feature = "alloc"))]
use concurrent_queue::ConcurrentQueue;
#[cfg(any(feature = "std", feature = "alloc"))]
use core::pin::Pin;
#[cfg(any(feature = "std", feature = "alloc"))]
use core::task::{Context, Poll};

#[cfg(feature = "std")]
use std::boxed::Box;
#[cfg(feature = "std")]
use std::sync::Arc;

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::boxed::Box;
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::sync::Arc;

#[cfg(all(feature = "std", feature = "multi"))]
use crate::synca::mpp::{self, SenderError};

use crate::valtron::EventReadinessPtr;

// ============================================================================
// No-Op Waker (no_std compatible)
// ============================================================================

/// Creates a no-op waker for use with Future polling.
///
/// WHY: Valtron executor drives polling loop, no actual waking needed
/// WHAT: Returns a Waker that does nothing when `wake()` is called
#[cfg(any(feature = "std", feature = "alloc"))]
fn create_noop_waker() -> Waker {
    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        |_| RAW_WAKER, // clone
        |_| {},        // wake
        |_| {},        // wake_by_ref
        |_| {},        // drop
    );
    const RAW_WAKER: RawWaker = RawWaker::new(core::ptr::null(), &VTABLE);

    // SAFETY: The vtable functions are all no-ops and handle null pointers correctly
    unsafe { Waker::from_raw(RAW_WAKER) }
}

/// Get a no-op waker (cached with thread-local on std, created fresh on `no_std`).
///
/// WHY: Reduce allocations on std by caching waker; `no_std` must create each time
/// WHAT: Returns cached waker on std, new waker on `no_std`
// Only the single-threaded `FutureIterator` still drives a future with a no-op
// waker (it re-polls inline via `Iterator::next`); the parking `FutureTask` /
// `StreamTask` paths use `queue_waker` instead. Gate to non-`multi` so the helper
// isn't dead code under `multi`.
#[cfg(all(feature = "std", not(feature = "multi")))]
fn get_noop_waker() -> Waker {
    thread_local! {
        static NOOP_WAKER: Waker = create_noop_waker();
    }
    NOOP_WAKER.with(std::clone::Clone::clone)
}

#[cfg(all(feature = "alloc", not(feature = "std"), not(feature = "multi")))]
fn get_noop_waker() -> Waker {
    create_noop_waker()
}

// ============================================================================
// Waker → QueueReadiness bridge (Decision 00, Level 1 / 00-F1)
// ============================================================================

/// Token pushed onto a future's wake queue when its `Waker` fires.
///
/// WHY: Bridges Rust's callback wake model (`Waker::wake()`) into valtron's
/// poll-the-readiness model — a fired waker becomes "queue non-empty", which the
/// executor already knows how to park/unpark on via [`QueueReadiness`].
///
/// WHAT: A `Copy` marker carrying an `id` that distinguishes wakers in
/// multi-future combinators (default `0` for a single bridged future).
///
/// HOW: [`queue_waker`] builds a `Waker` whose `wake()`/`wake_by_ref()` push one
/// of these onto the shared wake queue; the payload lets a woken task decide
/// *which* sub-signal fired when several share a queue.
///
/// # Panics
/// Never panics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WakeToken {
    /// Distinguishes wakers when several feed the same queue (default `0`).
    pub id: u64,
}

/// Internal `Waker` state: the shared wake queue plus this waker's token id.
///
/// Held behind an `Arc` as the `RawWaker` data pointer so cloning the `Waker`
/// only bumps the strong count and `wake()` can recover both the queue and id.
#[cfg(any(feature = "std", feature = "alloc"))]
struct WakerState {
    queue: Arc<ConcurrentQueue<WakeToken>>,
    id: u64,
}

#[cfg(any(feature = "std", feature = "alloc"))]
static QUEUE_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    queue_waker_clone,
    queue_waker_wake,
    queue_waker_wake_by_ref,
    queue_waker_drop,
);

/// Clone: bump the `Arc<WakerState>` strong count, reuse the same data pointer.
///
/// # Safety
/// `data` originates from `Arc::into_raw` of an `Arc<WakerState>` and is still
/// live (the owning `Waker` holds a strong reference across this call).
#[cfg(any(feature = "std", feature = "alloc"))]
unsafe fn queue_waker_clone(data: *const ()) -> RawWaker {
    Arc::increment_strong_count(data.cast::<WakerState>());
    RawWaker::new(data, &QUEUE_WAKER_VTABLE)
}

/// Wake (by value): push this waker's token, then drop the owned strong ref.
///
/// # Safety
/// `data` originates from `Arc::into_raw` of an `Arc<WakerState>`; `wake` consumes
/// the `Waker`, so reclaiming ownership here balances the original `into_raw`.
#[cfg(any(feature = "std", feature = "alloc"))]
unsafe fn queue_waker_wake(data: *const ()) {
    let state = Arc::from_raw(data.cast::<WakerState>());
    // Unbounded queue: push only fails if closed, in which case the woken task
    // has already gone away and dropping the token is the correct outcome.
    let _ = state.queue.push(WakeToken { id: state.id });
}

/// Wake by reference: push this waker's token without consuming the strong ref.
///
/// # Safety
/// `data` originates from `Arc::into_raw` of an `Arc<WakerState>` and is still
/// live; `ManuallyDrop` prevents this borrowed reconstruction from decrementing
/// the count owned by the caller's `Waker`.
#[cfg(any(feature = "std", feature = "alloc"))]
unsafe fn queue_waker_wake_by_ref(data: *const ()) {
    let state = core::mem::ManuallyDrop::new(Arc::from_raw(data.cast::<WakerState>()));
    let _ = state.queue.push(WakeToken { id: state.id });
}

/// Drop: reclaim and release one `Arc<WakerState>` strong reference.
///
/// # Safety
/// `data` originates from `Arc::into_raw` of an `Arc<WakerState>` and this call
/// balances exactly one outstanding strong reference.
#[cfg(any(feature = "std", feature = "alloc"))]
unsafe fn queue_waker_drop(data: *const ()) {
    drop(Arc::from_raw(data.cast::<WakerState>()));
}

/// Build a `Waker` whose `wake()` pushes a [`WakeToken`] onto `queue`.
///
/// WHY: Lets a bridged `Future`/`Stream` park via [`QueueReadiness`] instead of
/// busy-polling with a no-op waker (Decision 00 C1 fix).
///
/// WHAT: Returns a `Waker` backed by `Arc<`[`WakerState`]`>`; every `wake()` /
/// `wake_by_ref()` enqueues `WakeToken { id }`, flipping the queue to non-empty.
///
/// HOW: Uses `RawWaker`/`RawWakerVTable` with the `Arc` pointer as the data slot
/// — std/alloc only, no new dependency, mirroring [`create_noop_waker`].
///
/// # Panics
/// Never panics.
#[cfg(any(feature = "std", feature = "alloc"))]
#[must_use]
pub fn queue_waker(queue: Arc<ConcurrentQueue<WakeToken>>, id: u64) -> Waker {
    let state = Arc::new(WakerState { queue, id });
    let raw = RawWaker::new(Arc::into_raw(state).cast::<()>(), &QUEUE_WAKER_VTABLE);
    // SAFETY: `raw` is built from a fresh `Arc<WakerState>` and the vtable
    // functions uphold the `Arc` clone/drop contract described on each.
    unsafe { Waker::from_raw(raw) }
}

// ============================================================================
// FutureTask - Wrap Future as TaskIterator (requires Box)
// ============================================================================

/// State reported while future is being polled.
///
/// WHY: `TaskIterator` needs a Pending type to communicate progress
/// WHAT: Simple enum indicating future is still pending
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuturePollState {
    /// Future returned `Poll::Pending`
    Pending,
}

/// Wraps a Future and implements `TaskIterator` to poll it.
///
/// WHY: Enables executing async code through valtron without async runtime
/// WHAT: Polls the future on each `next()` call until Ready or exhausted
///
/// Requires `std` or `alloc` feature for `Box<Future>`.
#[cfg(any(feature = "std", feature = "alloc"))]
pub struct FutureTask<F>
where
    F: Future,
{
    future: Pin<Box<F>>,
    completed: bool,
    /// Wake queue watched via [`QueueReadiness`]; the future's `Waker` pushes a
    /// [`WakeToken`] here on `wake()`, so `Pending` parks instead of busy-polling.
    wake_queue: Arc<ConcurrentQueue<WakeToken>>,
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<F> FutureTask<F>
where
    F: Future,
{
    pub fn new(future: F) -> Self {
        Self {
            future: Box::pin(future),
            completed: false,
            wake_queue: Arc::new(ConcurrentQueue::unbounded()),
        }
    }

    #[must_use]
    pub fn from_pinned(future: Pin<Box<F>>) -> Self {
        Self {
            future,
            completed: false,
            wake_queue: Arc::new(ConcurrentQueue::unbounded()),
        }
    }
}

// WASM: FutureTask TaskIterator impl without Send bounds — for `from_future_non_send`.
// This impl works with !Send futures (e.g. JsFuture) on the single-threaded wasm32 target.
#[cfg(all(not(feature = "multi"), any(feature = "std", feature = "alloc")))]
impl<F> TaskIterator for FutureTask<F>
where
    F: Future + 'static,
    F::Output: 'static,
{
    type Ready = F::Output;
    type Pending = FuturePollState;
    type Spawner = NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.completed {
            return None;
        }

        // Drain stale tokens BEFORE polling so a prior turn's wake isn't mistaken
        // for a fresh one — a wake that lands *after* this drain (the
        // wake-before-park race) stays in the queue and keeps the readiness ready.
        while self.wake_queue.pop().is_ok() {}

        let waker = queue_waker(self.wake_queue.clone(), 0);
        let mut cx = Context::from_waker(&waker);

        match self.future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => {
                self.completed = true;
                Some(TaskStatus::Ready(output))
            }
            Poll::Pending => {
                // Two distinct Pending semantics (Decision 00-F1):
                //
                // 1. **Self-wake**: the future called `cx.waker().wake_by_ref()` during
                //    poll, pushing a token onto this queue. This is the stream-future
                //    pattern — the underlying `StreamIterator` advanced one step but
                //    isn't ready yet; the future wants another poll after other tasks
                //    run. → return `Pending(None)` to requeue without parking.
                //
                // 2. **External-wake**: no token was pushed. The future stashed the waker
                //    (e.g. `RecvFuture` waiting on a pipe, `SendFuture` waiting on
                //    capacity) and an *external* entity will fire it when data/capacity
                //    is ready. → return `Depends(queue)` to park until that wake.
                //
                // Draining before poll guarantees only a self-wake token (produced
                // *during* this poll) sits in the queue right now. The wake-before-park
                // race (token arriving between this check and the executor's
                // `is_ready()`) is harmless — it just means `Depends` is already ready,
                // the executor requeues, and we drain it on the next poll.
                if self.wake_queue.is_empty() {
                    // No self-wake: genuine park.
                    Some(TaskStatus::Depends(Arc::new(QueueReadiness::new(
                        self.wake_queue.clone(),
                    ))))
                } else {
                    // Self-wake: requeue without parking.
                    Some(TaskStatus::Pending(FuturePollState::Pending))
                }
            }
        }
    }
}

// Native implementation with Send bounds
#[cfg(feature = "multi")]
impl<F> TaskIterator for FutureTask<F>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    type Ready = F::Output;
    type Pending = FuturePollState;
    type Spawner = NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.completed {
            return None;
        }

        // Drain stale tokens BEFORE polling — see the non-multi impl above for
        // the full rationale.
        while self.wake_queue.pop().is_ok() {}

        let waker = queue_waker(self.wake_queue.clone(), 0);
        let mut cx = Context::from_waker(&waker);

        match self.future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => {
                self.completed = true;
                Some(TaskStatus::Ready(output))
            }
            Poll::Pending => {
                if self.wake_queue.is_empty() {
                    Some(TaskStatus::Depends(Arc::new(QueueReadiness::new(
                        self.wake_queue.clone(),
                    ))))
                } else {
                    Some(TaskStatus::Pending(FuturePollState::Pending))
                }
            }
        }
    }
}

// ============================================================================
// StreamTask - Wrap Stream as TaskIterator
// ============================================================================

/// State for stream polling.
///
/// WHY: `TaskIterator` needs a Pending type for streams
/// WHAT: Simple enum indicating stream is pending more items
#[cfg(any(feature = "std", feature = "alloc"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamPollState {
    /// Stream returned `Poll::Pending`
    Pending,
}

/// Wraps an async Stream and yields values through `TaskIterator`.
///
/// WHY: Enables processing async streams through valtron
/// WHAT: Polls stream on each `next()`, yields Some(item) or None when exhausted
///
/// Requires `std` or `alloc` feature for `Box<Stream>`.
#[cfg(any(feature = "std", feature = "alloc"))]
pub struct StreamTask<S>
where
    S: futures_core::Stream,
{
    stream: Pin<Box<S>>,
    exhausted: bool,
    /// Wake queue watched via [`QueueReadiness`]; the stream's `Waker` pushes a
    /// [`WakeToken`] here on `wake()`, so `Pending` parks instead of busy-polling.
    wake_queue: Arc<ConcurrentQueue<WakeToken>>,
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<S> StreamTask<S>
where
    S: futures_core::Stream,
{
    pub fn new(stream: S) -> Self {
        Self {
            stream: Box::pin(stream),
            exhausted: false,
            wake_queue: Arc::new(ConcurrentQueue::unbounded()),
        }
    }
}

// Native implementation with Send bounds
#[cfg(feature = "multi")]
impl<S> TaskIterator for StreamTask<S>
where
    S: futures_core::Stream + Send + 'static,
    S::Item: Send + 'static,
{
    type Ready = Option<S::Item>;
    type Pending = StreamPollState;
    type Spawner = NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.exhausted {
            return None;
        }

        // Drain stale tokens BEFORE polling (see `FutureTask::next_status`): a
        // wake that arrives after this drain survives to keep the readiness ready.
        while self.wake_queue.pop().is_ok() {}

        let waker = queue_waker(self.wake_queue.clone(), 0);
        let mut cx = Context::from_waker(&waker);

        match self.stream.as_mut().poll_next(&mut cx) {
            Poll::Ready(Some(item)) => Some(TaskStatus::Ready(Some(item))),
            Poll::Ready(None) => {
                self.exhausted = true;
                Some(TaskStatus::Ready(None))
            }
            Poll::Pending => {
                if self.wake_queue.is_empty() {
                    Some(TaskStatus::Depends(Arc::new(QueueReadiness::new(
                        self.wake_queue.clone(),
                    ))))
                } else {
                    Some(TaskStatus::Pending(StreamPollState::Pending))
                }
            }
        }
    }
}

// WASM: StreamTask TaskIterator impl without Send bounds — for `from_stream_non_send`.
// This impl works with !Send streams on the single-threaded wasm32 target.
#[cfg(all(not(feature = "multi"), any(feature = "std", feature = "alloc")))]
impl<S> TaskIterator for StreamTask<S>
where
    S: futures_core::Stream + 'static,
    S::Item: 'static,
{
    type Ready = Option<S::Item>;
    type Pending = StreamPollState;
    type Spawner = NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.exhausted {
            return None;
        }

        // Drain stale tokens BEFORE polling (see `FutureTask::next_status`): a
        // wake that arrives after this drain survives to keep the readiness ready.
        while self.wake_queue.pop().is_ok() {}

        let waker = queue_waker(self.wake_queue.clone(), 0);
        let mut cx = Context::from_waker(&waker);

        match self.stream.as_mut().poll_next(&mut cx) {
            Poll::Ready(Some(item)) => Some(TaskStatus::Ready(Some(item))),
            Poll::Ready(None) => {
                self.exhausted = true;
                Some(TaskStatus::Ready(None))
            }
            Poll::Pending => {
                if self.wake_queue.is_empty() {
                    Some(TaskStatus::Depends(Arc::new(QueueReadiness::new(
                        self.wake_queue.clone(),
                    ))))
                } else {
                    Some(TaskStatus::Pending(StreamPollState::Pending))
                }
            }
        }
    }
}

// ============================================================================
// CancellableFutureTask - FutureTask with external cancel signal
// ============================================================================

/// Outcome of a cancelled future.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelOutcome {
    Cancelled,
}

/// Readiness that fires when the inner signal is ready **or** the cancel flag is
/// set (Decision 00 L1b composition: "inner ready OR cancelled").
///
/// WHY: A parked [`CancellableFutureTask`] must be re-run when its cancel flag
/// flips, otherwise a future that never wakes (e.g. an always-pending one) would
/// park forever and the cancellation would never be observed.
///
/// WHAT: An [`EventReadiness`] that ORs the wrapped future's park signal with the
/// `Arc<AtomicBool>` cancel flag.
///
/// HOW: `is_ready` short-circuits on the cancel flag before delegating to the
/// inner signal, so setting cancel unparks the task immediately.
///
/// # Panics
/// Never panics.
#[cfg(any(feature = "std", feature = "alloc"))]
struct CancelOrReadiness {
    inner: EventReadinessPtr,
    cancel: Arc<core::sync::atomic::AtomicBool>,
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl crate::valtron::EventReadiness for CancelOrReadiness {
    fn is_ready(&self, dur: Option<core::time::Duration>) -> bool {
        self.cancel.load(core::sync::atomic::Ordering::Acquire) || self.inner.is_ready(dur)
    }
}

/// Wraps a [`FutureTask`] with an `Arc<AtomicBool>` cancel signal.
///
/// On each `next_status` poll the flag is checked first — if set, the
/// inner future is dropped and `Ready(Err(CancelOutcome::Cancelled))` is
/// returned immediately. This lets the agent loop cancel in-flight tool
/// executions when a steering interrupt arrives.
#[cfg(any(feature = "std", feature = "alloc"))]
pub struct CancellableFutureTask<F>
where
    F: Future,
{
    inner: FutureTask<F>,
    cancel: Arc<core::sync::atomic::AtomicBool>,
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<F: Future> CancellableFutureTask<F> {
    pub fn new(future: F, cancel: Arc<core::sync::atomic::AtomicBool>) -> Self {
        Self {
            inner: FutureTask::new(future),
            cancel,
        }
    }

    pub fn request_cancel(&self) {
        self.cancel
            .store(true, core::sync::atomic::Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(core::sync::atomic::Ordering::Acquire)
    }

    #[must_use]
    pub fn cancel_signal(&self) -> Arc<core::sync::atomic::AtomicBool> {
        self.cancel.clone()
    }
}

#[cfg(feature = "multi")]
impl<F> TaskIterator for CancellableFutureTask<F>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    type Ready = Result<F::Output, CancelOutcome>;
    type Pending = FuturePollState;
    type Spawner = NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.is_cancelled() {
            return Some(TaskStatus::Ready(Err(CancelOutcome::Cancelled)));
        }
        match self.inner.next_status() {
            Some(TaskStatus::Ready(v)) => Some(TaskStatus::Ready(Ok(v))),
            Some(TaskStatus::Pending(p)) => Some(TaskStatus::Pending(p)),
            Some(TaskStatus::Init) => Some(TaskStatus::Init),
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            Some(TaskStatus::Wait) => Some(TaskStatus::Wait),
            Some(TaskStatus::Delayed(d)) => Some(TaskStatus::Delayed(d)),
            Some(TaskStatus::Spawn(s)) => Some(TaskStatus::Spawn(s)),
            // Compose the cancel flag into the park signal so a set cancel
            // unparks the task even when the inner future never wakes.
            Some(TaskStatus::Depends(r)) => {
                Some(TaskStatus::Depends(Arc::new(CancelOrReadiness {
                    inner: r,
                    cancel: self.cancel.clone(),
                })))
            }
            Some(TaskStatus::Spread(items)) => {
                use crate::valtron::TaskSpread;
                Some(TaskStatus::Spread(
                    items
                        .into_iter()
                        .map(|item| match item {
                            TaskSpread::Ready(v) => TaskSpread::Ready(Ok(v)),
                            TaskSpread::Pending(p) => TaskSpread::Pending(p),
                        })
                        .collect(),
                ))
            }
            None => None,
        }
    }
}

#[cfg(all(not(feature = "multi"), any(feature = "std", feature = "alloc")))]
impl<F> TaskIterator for CancellableFutureTask<F>
where
    F: Future + 'static,
    F::Output: 'static,
{
    type Ready = Result<F::Output, CancelOutcome>;
    type Pending = FuturePollState;
    type Spawner = NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.is_cancelled() {
            return Some(TaskStatus::Ready(Err(CancelOutcome::Cancelled)));
        }
        match self.inner.next_status() {
            Some(TaskStatus::Ready(v)) => Some(TaskStatus::Ready(Ok(v))),
            Some(TaskStatus::Pending(p)) => Some(TaskStatus::Pending(p)),
            Some(TaskStatus::Init) => Some(TaskStatus::Init),
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            Some(TaskStatus::Wait) => Some(TaskStatus::Wait),
            Some(TaskStatus::Delayed(d)) => Some(TaskStatus::Delayed(d)),
            Some(TaskStatus::Spawn(s)) => Some(TaskStatus::Spawn(s)),
            // Compose the cancel flag into the park signal so a set cancel
            // unparks the task even when the inner future never wakes.
            Some(TaskStatus::Depends(r)) => {
                Some(TaskStatus::Depends(Arc::new(CancelOrReadiness {
                    inner: r,
                    cancel: self.cancel.clone(),
                })))
            }
            Some(TaskStatus::Spread(items)) => {
                use crate::valtron::TaskSpread;
                Some(TaskStatus::Spread(
                    items
                        .into_iter()
                        .map(|item| match item {
                            TaskSpread::Ready(v) => TaskSpread::Ready(Ok(v)),
                            TaskSpread::Pending(p) => TaskSpread::Pending(p),
                        })
                        .collect(),
                ))
            }
            None => None,
        }
    }
}

// ============================================================================
// ThreadedValue - Common result type (unconditional)
// ============================================================================

/// Result value sent from ThreadedIterFuture worker or iterator.
#[derive(Debug)]
pub enum ThreadedValue<T, E> {
    Value(Result<T, E>),
    Waiting,
}

// ============================================================================
// Multi-threaded implementation (std + multi)
// ============================================================================

/// Iterator wrapper around Receiver for ThreadedIterFuture.
///
/// Uses `Receiver.into_recv_iter()` which already provides the Iterator
/// implementation with proper blocking/consumption logic.
#[cfg(feature = "multi")]
pub struct FutureIterator<T, E> {
    iter: mpp::RecvIterator<ThreadedValue<T, E>>,
}

#[cfg(feature = "multi")]
impl<T, E> Iterator for FutureIterator<T, E> {
    type Item = ThreadedValue<T, E>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// A future executor that runs !Send operations on a background worker thread.
///
/// In multi-threaded mode, this submits work to the BackgroundJobRegistry.
/// In single-threaded mode, this polls the future inline on each next() call.
#[cfg(feature = "multi")]
pub struct ThreadedIterFuture<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<I, E>> + 'static,
    I: Iterator<Item = Result<T, E>> + 'static,
    T: Send + 'static,
    E: Send + 'static,
{
    future_fn: F,
    queue_size: usize,
    backpressure_sleep: Option<std::time::Duration>,
    _phantom: PhantomData<(Fut, I, T, E)>,
}

#[cfg(feature = "multi")]
impl<F, Fut, I, T, E> ThreadedIterFuture<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<I, E>> + 'static,
    I: Iterator<Item = Result<T, E>> + 'static,
    T: Send + 'static,
    E: Send + 'static,
{
    /// Create a new ThreadedIterFuture with default settings.
    pub fn new(future_fn: F) -> Self {
        Self {
            future_fn,
            queue_size: 16,
            backpressure_sleep: Some(std::time::Duration::from_millis(10)),
            _phantom: PhantomData,
        }
    }

    /// Create a new ThreadedIterFuture with custom queue size.
    pub fn with_queue_size(future_fn: F, queue_size: usize) -> Self {
        Self {
            future_fn,
            queue_size: queue_size.max(2),
            backpressure_sleep: Some(std::time::Duration::from_millis(10)),
            _phantom: PhantomData,
        }
    }

    /// Execute the future on a background worker thread.
    pub fn execute(
        self,
    ) -> crate::valtron::GenericResult<impl Iterator<Item = ThreadedValue<T, E>>> {
        let (sender, receiver) = mpp::bounded::<ThreadedValue<T, E>>(self.queue_size);
        let backpressure_sleep = self.backpressure_sleep;

        super::run_background_job(move || {
            let waker = create_noop_waker();
            let mut cx = Context::from_waker(&waker);
            let mut future = Box::pin((self.future_fn)());

            // Poll future until it produces iterator or error
            let iterator = loop {
                match future.as_mut().poll(&mut cx) {
                    Poll::Ready(Ok(iter)) => break Some(iter),
                    Poll::Ready(Err(e)) => {
                        let _ = sender.send(ThreadedValue::Value(Err(e)));
                        break None;
                    }
                    Poll::Pending => std::thread::yield_now(),
                }
            };

            // Stream iterator results through the channel
            if let Some(iter) = iterator {
                for result in iter {
                    let mut value = ThreadedValue::Value(result);
                    loop {
                        match sender.send(value) {
                            Ok(()) => break,
                            Err(SenderError::Full(v)) => {
                                value = v;
                                if let Some(sleep_dur) = backpressure_sleep {
                                    std::thread::sleep(sleep_dur);
                                } else {
                                    std::hint::spin_loop();
                                }
                            }
                            Err(SenderError::Closed(_)) => return,
                        }
                    }
                }
            }

            let _ = sender.close();
        })?;

        Ok(FutureIterator {
            iter: receiver.into_recv_iter(),
        })
    }

    /// Create a new ThreadedIterFuture with custom queue size and backpressure sleep.
    pub fn with_backpressure_sleep(
        future_fn: F,
        queue_size: usize,
        backpressure_sleep: Option<std::time::Duration>,
    ) -> Self {
        Self {
            future_fn,
            queue_size: queue_size.max(2),
            backpressure_sleep,
            _phantom: PhantomData,
        }
    }
}

// ============================================================================
// Single-threaded std implementation (std, not multi)
// ============================================================================

/// Iterator that polls a future on each `next()` call.
#[cfg(all(not(feature = "multi"), any(feature = "std", feature = "alloc")))]
pub struct FutureIterator<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<I, E>>,
    I: Iterator<Item = Result<T, E>>,
{
    future: Option<F>,
    inner_iter: Option<I>,
    _phantom: PhantomData<(Fut, T, E)>,
}

#[cfg(all(not(feature = "multi"), any(feature = "std", feature = "alloc")))]
impl<F, Fut, I, T, E> Iterator for FutureIterator<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<I, E>>,
    I: Iterator<Item = Result<T, E>>,
{
    type Item = ThreadedValue<T, E>;

    fn next(&mut self) -> Option<Self::Item> {
        // If we already have the inner iterator, pull from it
        if let Some(iter) = &mut self.inner_iter {
            return iter.next().map(ThreadedValue::Value);
        }

        // Otherwise, poll the future
        let mut future = {
            let f = self.future.take()?;
            Box::pin((f)())
        };

        let waker = get_noop_waker();
        let mut cx = Context::from_waker(&waker);

        match future.as_mut().poll(&mut cx) {
            Poll::Ready(Ok(iter)) => {
                self.inner_iter = Some(iter);
                // Return first item from the newly acquired iterator
                if let Some(iter) = &mut self.inner_iter {
                    return iter.next().map(ThreadedValue::Value);
                }
                None
            }
            Poll::Ready(Err(e)) => Some(ThreadedValue::Value(Err(e))),
            Poll::Pending => Some(ThreadedValue::Waiting),
        }
    }
}

/// A future executor for single-threaded std environments.
///
/// Polls the future inline on each `next()` call without spawning threads.
#[cfg(all(not(feature = "multi"), any(feature = "std", feature = "alloc")))]
pub struct ThreadedIterFuture<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<I, E>>,
    I: Iterator<Item = Result<T, E>>,
{
    future_fn: F,
    _phantom: PhantomData<(Fut, I, T, E)>,
}

#[cfg(all(not(feature = "multi"), any(feature = "std", feature = "alloc")))]
impl<F, Fut, I, T, E> ThreadedIterFuture<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<I, E>>,
    I: Iterator<Item = Result<T, E>>,
{
    pub fn new(future_fn: F) -> Self {
        Self {
            future_fn,
            _phantom: PhantomData,
        }
    }

    pub fn with_queue_size(future_fn: F, _queue_size: usize) -> Self {
        Self {
            future_fn,
            _phantom: PhantomData,
        }
    }

    pub fn execute(self) -> impl Iterator<Item = ThreadedValue<T, E>> {
        FutureIterator {
            future: Some(self.future_fn),
            inner_iter: None,
            _phantom: PhantomData,
        }
    }
}
