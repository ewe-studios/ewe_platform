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
use crate::valtron::{NoAction, TaskIterator, TaskStatus};
#[cfg(any(feature = "std", feature = "alloc"))]
use core::pin::Pin;
#[cfg(any(feature = "std", feature = "alloc"))]
use core::task::{Context, Poll};

#[cfg(feature = "std")]
use std::boxed::Box;

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::boxed::Box;

#[cfg(all(feature = "std", feature = "multi"))]
use crate::synca::mpp::{self, SenderError};

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
#[cfg(feature = "std")]
fn get_noop_waker() -> Waker {
    thread_local! {
        static NOOP_WAKER: Waker = create_noop_waker();
    }
    NOOP_WAKER.with(std::clone::Clone::clone)
}

#[cfg(all(feature = "alloc", not(feature = "std")))]
fn get_noop_waker() -> Waker {
    create_noop_waker()
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
        }
    }

    #[must_use]
    pub fn from_pinned(future: Pin<Box<F>>) -> Self {
        Self {
            future,
            completed: false,
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

        let waker = get_noop_waker();
        let mut cx = Context::from_waker(&waker);

        match self.future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => {
                self.completed = true;
                Some(TaskStatus::Ready(output))
            }
            Poll::Pending => Some(TaskStatus::Pending(FuturePollState::Pending)),
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

        let waker = get_noop_waker();
        let mut cx = Context::from_waker(&waker);

        match self.future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => {
                self.completed = true;
                Some(TaskStatus::Ready(output))
            }
            Poll::Pending => Some(TaskStatus::Pending(FuturePollState::Pending)),
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

        let waker = get_noop_waker();
        let mut cx = Context::from_waker(&waker);

        match self.stream.as_mut().poll_next(&mut cx) {
            Poll::Ready(Some(item)) => Some(TaskStatus::Ready(Some(item))),
            Poll::Ready(None) => {
                self.exhausted = true;
                Some(TaskStatus::Ready(None))
            }
            Poll::Pending => Some(TaskStatus::Pending(StreamPollState::Pending)),
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

        let waker = get_noop_waker();
        let mut cx = Context::from_waker(&waker);

        match self.stream.as_mut().poll_next(&mut cx) {
            Poll::Ready(Some(item)) => Some(TaskStatus::Ready(Some(item))),
            Poll::Ready(None) => {
                self.exhausted = true;
                Some(TaskStatus::Ready(None))
            }
            Poll::Pending => Some(TaskStatus::Pending(StreamPollState::Pending)),
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
                return None;
            }
            Poll::Ready(Err(e)) => return Some(ThreadedValue::Value(Err(e))),
            Poll::Pending => return Some(ThreadedValue::Waiting),
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
