//! Future-to-TaskIterator adapter for seamless async integration.
//!
//! This module provides adapters that allow standard Rust `Future`s and `Stream`s
//! to be executed through the valtron executor system without requiring a full
//! async runtime.

use super::{NoAction, TaskIterator, TaskStatus};
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

#[cfg(feature = "std")]
use std::boxed::Box;

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::boxed::Box;

// ============================================================================
// No-Op Waker (no_std compatible)
// ============================================================================

/// Creates a no-op waker for use with Future polling.
///
/// WHY: Valtron executor drives polling loop, no actual waking needed
/// WHAT: Returns a Waker that does nothing when wake() is called
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

/// Get a no-op waker (cached with thread-local on std, created fresh on no_std).
///
/// WHY: Reduce allocations on std by caching waker; no_std must create each time
/// WHAT: Returns cached waker on std, new waker on no_std
#[cfg(feature = "std")]
fn get_noop_waker() -> Waker {
    thread_local! {
        static NOOP_WAKER: Waker = create_noop_waker();
    }
    NOOP_WAKER.with(|w| w.clone())
}

#[cfg(not(feature = "std"))]
fn get_noop_waker() -> Waker {
    create_noop_waker()
}

// ============================================================================
// FutureTask - Wrap Future as TaskIterator (requires Box)
// ============================================================================

/// State reported while future is being polled.
///
/// WHY: TaskIterator needs a Pending type to communicate progress
/// WHAT: Simple enum indicating future is still pending
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuturePollState {
    /// Future returned Poll::Pending
    Pending,
}

/// Wraps a Future and implements TaskIterator to poll it.
///
/// WHY: Enables executing async code through valtron without async runtime
/// WHAT: Polls the future on each next() call until Ready or exhausted
///
/// Requires `std` or `alloc` feature for Box<Future>.
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

    pub fn from_pinned(future: Pin<Box<F>>) -> Self {
        Self {
            future,
            completed: false,
        }
    }
}

// Native implementation with Send bounds
#[cfg(all(any(feature = "std", feature = "alloc"), not(target_arch = "wasm32")))]
impl<F> TaskIterator for FutureTask<F>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    type Ready = F::Output;
    type Pending = FuturePollState;
    type Spawner = NoAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
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

// WASM implementation without Send bounds
#[cfg(all(any(feature = "std", feature = "alloc"), target_arch = "wasm32"))]
impl<F> TaskIterator for FutureTask<F>
where
    F: Future + 'static,
    F::Output: 'static,
{
    type Ready = F::Output;
    type Pending = FuturePollState;
    type Spawner = NoAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
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
// Convenience Functions
// ============================================================================

/// Wrap a future into a TaskIterator (native - requires Send).
///
/// WHY: Convenient helper to create FutureTask
/// WHAT: Returns FutureTask wrapping the given future
#[cfg(all(any(feature = "std", feature = "alloc"), not(target_arch = "wasm32")))]
pub fn from_future<F>(future: F) -> FutureTask<F>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    FutureTask::new(future)
}

/// Wrap a future into a TaskIterator (WASM - no Send required).
///
/// WHY: WASM is single-threaded, Send not needed
/// WHAT: Returns FutureTask wrapping the given future without Send bounds
#[cfg(all(any(feature = "std", feature = "alloc"), target_arch = "wasm32"))]
pub fn from_future<F>(future: F) -> FutureTask<F>
where
    F: Future + 'static,
    F::Output: 'static,
{
    FutureTask::new(future)
}

// ============================================================================
// StreamTask - Wrap Stream as TaskIterator
// ============================================================================

/// State for stream polling.
///
/// WHY: TaskIterator needs a Pending type for streams
/// WHAT: Simple enum indicating stream is pending more items
#[cfg(any(feature = "std", feature = "alloc"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamPollState {
    /// Stream returned Poll::Pending
    Pending,
}

/// Wraps an async Stream and yields values through TaskIterator.
///
/// WHY: Enables processing async streams through valtron
/// WHAT: Polls stream on each next(), yields Some(item) or None when exhausted
///
/// Requires `std` or `alloc` feature for Box<Stream>.
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
#[cfg(all(any(feature = "std", feature = "alloc"), not(target_arch = "wasm32")))]
impl<S> TaskIterator for StreamTask<S>
where
    S: futures_core::Stream + Send + 'static,
    S::Item: Send + 'static,
{
    type Ready = Option<S::Item>;
    type Pending = StreamPollState;
    type Spawner = NoAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
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

// WASM implementation without Send bounds
#[cfg(all(any(feature = "std", feature = "alloc"), target_arch = "wasm32"))]
impl<S> TaskIterator for StreamTask<S>
where
    S: futures_core::Stream + 'static,
    S::Item: 'static,
{
    type Ready = Option<S::Item>;
    type Pending = StreamPollState;
    type Spawner = NoAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
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

/// Wrap a stream into a TaskIterator (native - requires Send).
///
/// WHY: Convenient helper to create StreamTask
/// WHAT: Returns StreamTask wrapping the given stream
#[cfg(all(any(feature = "std", feature = "alloc"), not(target_arch = "wasm32")))]
pub fn from_stream<S>(stream: S) -> StreamTask<S>
where
    S: futures_core::Stream + Send + 'static,
    S::Item: Send + 'static,
{
    StreamTask::new(stream)
}

/// Wrap a stream into a TaskIterator (WASM - no Send required).
///
/// WHY: WASM is single-threaded, Send not needed
/// WHAT: Returns StreamTask wrapping the given stream without Send bounds
#[cfg(all(any(feature = "std", feature = "alloc"), target_arch = "wasm32"))]
pub fn from_stream<S>(stream: S) -> StreamTask<S>
where
    S: futures_core::Stream + 'static,
    S::Item: 'static,
{
    StreamTask::new(stream)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// WHY: FutureTask must poll and complete a simple future
    /// WHAT: Future that returns Ready immediately should complete on first poll
    #[test]
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn test_future_task_immediate_ready() {
        let future = core::future::ready(42);
        let mut task = FutureTask::new(future);

        // First poll should return Ready(42)
        match task.next() {
            Some(TaskStatus::Ready(42)) => {}
            other => panic!("Expected Ready(42), got {:?}", other),
        }

        // Task is complete
        assert!(task.next().is_none());
    }

    /// WHY: FutureTask must handle pending futures correctly
    /// WHAT: Future that returns Pending should yield Pending status
    #[test]
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn test_future_task_pending() {
        use core::future::Future;
        use core::pin::Pin;
        use core::task::{Context, Poll};

        struct PendingThenReady {
            polled: bool,
        }

        impl Future for PendingThenReady {
            type Output = i32;

            fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
                if self.polled {
                    Poll::Ready(100)
                } else {
                    self.polled = true;
                    Poll::Pending
                }
            }
        }

        let future = PendingThenReady { polled: false };
        let mut task = FutureTask::new(future);

        // First poll should be Pending
        match task.next() {
            Some(TaskStatus::Pending(FuturePollState::Pending)) => {}
            other => panic!("Expected Pending, got {:?}", other),
        }

        // Second poll should be Ready
        match task.next() {
            Some(TaskStatus::Ready(100)) => {}
            other => panic!("Expected Ready(100), got {:?}", other),
        }

        // Task is complete
        assert!(task.next().is_none());
    }

    /// WHY: from_future() convenience function must work
    /// WHAT: Creates FutureTask correctly
    #[test]
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn test_from_future_convenience() {
        let future = core::future::ready("hello");
        let mut task = from_future(future);

        match task.next() {
            Some(TaskStatus::Ready("hello")) => {}
            other => panic!("Expected Ready(hello), got {:?}", other),
        }
    }

    /// WHY: No-op waker must be created successfully
    /// WHAT: create_noop_waker() returns a valid Waker
    #[test]
    fn test_noop_waker_creation() {
        let waker = create_noop_waker();
        // Waker should be usable without panicking
        waker.wake_by_ref();
        let waker2 = waker.clone();
        waker2.wake();
    }

    /// WHY: get_noop_waker() must return a valid waker
    /// WHAT: Function compiles and returns waker
    #[test]
    fn test_get_noop_waker() {
        let waker = get_noop_waker();
        waker.wake_by_ref();
    }
}

// ============================================================================
// run_future - Execute Future through unified executor
// ============================================================================

/// Execute a future using the unified executor (native - requires Send).
///
/// WHY: Simplest way to run async code through valtron
/// WHAT: Wraps future in FutureTask and executes via unified executor
///
/// Note: This requires the unified executor to be available and properly configured.
#[cfg(all(any(feature = "std", feature = "alloc"), not(target_arch = "wasm32")))]
pub fn run_future<F>(future: F) -> crate::valtron::GenericResult<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    use super::unified;
    let task = FutureTask::new(future);
    unified::execute(task)
}

/// Execute a future using the unified executor (WASM - no Send required).
///
/// WHY: WASM is single-threaded, Send not needed
/// WHAT: Wraps future in FutureTask and executes via unified executor
#[cfg(all(any(feature = "std", feature = "alloc"), target_arch = "wasm32"))]
pub fn run_future<F>(future: F) -> crate::valtron::GenericResult<F::Output>
where
    F: Future + 'static,
    F::Output: 'static,
{
    use super::unified;
    let task = FutureTask::new(future);
    unified::execute(task)
}
