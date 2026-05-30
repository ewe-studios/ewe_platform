//! Stream-to-Future async bridge for valtron.
//!
//! Wrapper types that convert any [`StreamIterator`] into standard Rust `Future`
//! and `Stream` types, bridging valtron's sync iterator model with async code.

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use futures_core::Stream as FuturesStream;

use super::streams::{Stream, StreamIterator, StreamSpread};

// ============================================================================
// StreamCollectFuture
// ============================================================================

/// Drives a [`StreamIterator`] to completion, collecting all `Next` values into a `Vec`.
#[cfg(any(feature = "std", feature = "alloc"))]
pub struct StreamCollectFuture<SI: StreamIterator> {
    inner: Option<SI>,
    collected: Vec<SI::D>,
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<SI: StreamIterator> StreamCollectFuture<SI> {
    pub fn new(iter: SI) -> Self {
        Self {
            inner: Some(iter),
            collected: Vec::new(),
        }
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<SI> Future for StreamCollectFuture<SI>
where
    SI: StreamIterator + Unpin,
    SI::D: Unpin,
{
    type Output = Vec<SI::D>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let iter = this.inner.as_mut().expect("polled after completion");

        loop {
            match iter.next() {
                Some(Stream::Next(v)) => this.collected.push(v),
                Some(Stream::Ignore) => {}
                Some(
                    Stream::Pending(_)
                    | Stream::Delayed(_)
                    | Stream::Init
                    | Stream::Wait
                    | Stream::Spread(_),
                ) => {
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
                None => {
                    return Poll::Ready(std::mem::take(&mut this.collected));
                }
            }
        }
    }
}

// ============================================================================
// StreamReadyFuture
// ============================================================================

/// Drives a [`StreamIterator`] until the first `Next(D)`, returning the value and remaining iterator.
pub struct StreamReadyFuture<SI: StreamIterator> {
    inner: Option<SI>,
}

impl<SI: StreamIterator> StreamReadyFuture<SI> {
    pub fn new(iter: SI) -> Self {
        Self { inner: Some(iter) }
    }
}

impl<SI> Future for StreamReadyFuture<SI>
where
    SI: StreamIterator + Unpin,
{
    type Output = Option<(SI::D, SI)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let iter = this.inner.as_mut().expect("polled after completion");

        loop {
            match iter.next() {
                Some(Stream::Next(v)) => {
                    let remaining = this.inner.take().expect("inner should exist");
                    return Poll::Ready(Some((v, remaining)));
                }
                Some(Stream::Ignore) => {}
                Some(Stream::Pending(_) | Stream::Delayed(_) | Stream::Init | Stream::Wait) => {
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
                Some(Stream::Spread(items)) => {
                    for item in items {
                        if let StreamSpread::Done(v) = item {
                            let remaining = this.inner.take().expect("inner should exist");
                            return Poll::Ready(Some((v, remaining)));
                        }
                    }
                }
                None => return Poll::Ready(None),
            }
        }
    }
}

// ============================================================================
// StreamPendingFuture
// ============================================================================

/// Drives a [`StreamIterator`] until the first `Pending(P)`, returning the context and remaining iterator.
pub struct StreamPendingFuture<SI: StreamIterator> {
    inner: Option<SI>,
}

impl<SI: StreamIterator> StreamPendingFuture<SI> {
    pub fn new(iter: SI) -> Self {
        Self { inner: Some(iter) }
    }
}

impl<SI> Future for StreamPendingFuture<SI>
where
    SI: StreamIterator + Unpin,
{
    type Output = Option<(SI::P, SI)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let iter = this.inner.as_mut().expect("polled after completion");

        loop {
            match iter.next() {
                Some(Stream::Pending(p)) => {
                    let remaining = this.inner.take().expect("inner should exist");
                    return Poll::Ready(Some((p, remaining)));
                }
                Some(Stream::Ignore) => {}
                Some(Stream::Next(_) | Stream::Delayed(_) | Stream::Init | Stream::Wait) => {
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
                Some(Stream::Spread(items)) => {
                    for item in items {
                        if let StreamSpread::Pending(p) = item {
                            let remaining = this.inner.take().expect("inner should exist");
                            return Poll::Ready(Some((p, remaining)));
                        }
                    }
                }
                None => return Poll::Ready(None),
            }
        }
    }
}

// ============================================================================
// StreamAsFutureStream
// ============================================================================

/// Wraps a [`StreamIterator`] as a `futures_core::Stream`, yielding each `Stream<D, P>` item as-is.
pub struct StreamAsFutureStream<SI: StreamIterator> {
    inner: Option<SI>,
}

impl<SI: StreamIterator> StreamAsFutureStream<SI> {
    pub fn new(iter: SI) -> Self {
        Self { inner: Some(iter) }
    }
}

impl<SI> FuturesStream for StreamAsFutureStream<SI>
where
    SI: StreamIterator + Unpin,
{
    type Item = Stream<SI::D, SI::P>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let iter = this.inner.as_mut().expect("polled after completion");
        Poll::Ready(iter.next())
    }
}
