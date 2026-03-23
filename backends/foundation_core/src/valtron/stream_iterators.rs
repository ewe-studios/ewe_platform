//! Extension trait and combinators for `StreamIterator`.
//!
//! This module provides builder-style combinator methods for any type implementing
//! `StreamIterator`. These combinators are applied **AFTER** calling `execute()`.
//!
//! ## Overview
//!
//! The `StreamIteratorExt` trait extends any `StreamIterator` with methods like:
//! - `map_all_done()` - Transform Next (Done) values
//! - `map_all_pending()` - Transform Pending values
//! - `filter_all_done()` - Filter Next values
//! - `collect_all()` - Collect all Next values into a Vec (terminal operation)
//!
//! ## Usage Pattern
//!
//! ```ignore
//! // 1. Execute task to get StreamIterator
//! let stream = execute(my_task)?;
//!
//! // 2. Apply StreamIteratorExt combinators AFTER execute()
//! let stream = stream
//!     .map_all_done(|v| transform(v))
//!     .filter_all_done(|v| should_keep(v));
//!
//! // 3. Consume the stream
//! for item in stream {
//!     match item {
//!         Stream::Next(value) => { /* got result */ }
//!         Stream::Pending(p) => { /* still waiting */ }
//!         Stream::Delayed(d) => { /* will be delayed */ }
//!         Stream::Init => { /* initializing */ }
//!         Stream::Ignore => { /* ignore */ }
//!     }
//! }
//! ```
//!
//! ## Relationship to Other Modules
//!
//! - [`super::synca::mpp`] - Contains `Stream` enum and `StreamIterator` trait
//! - [`super::executors::drivers`] - Contains `DrivenStreamIterator`
//! - [`super::task_iterators`] - Contains `TaskIteratorExt` for pre-execute combinators

use crate::synca::mpp::{Stream, StreamIterator};
use crate::valtron::branches::CollectionState;
use std::sync::Arc;

use concurrent_queue::ConcurrentQueue;

/// Extension trait providing combinator methods for any `StreamIterator`.
///
/// This trait is automatically implemented for any type that implements
/// `StreamIterator` with the appropriate bounds. This includes:
/// - `DrivenStreamIterator` from `drivers.rs`
/// - Any custom iterator implementing `StreamIterator`
///
/// ## Combinators
///
/// - [`map_all_done`](StreamIteratorExt::map_all_done) - Transform Next (Done) values
/// - [`map_all_pending`](StreamIteratorExt::map_all_pending) - Transform Pending values
/// - [`map_all_pending_and_done`](StreamIteratorExt::map_all_pending_and_done) - Transform both
/// - [`filter_all_done`](StreamIteratorExt::filter_all_done) - Filter Next values
/// - [`map_all_delayed`](StreamIteratorExt::map_all_delayed) - Transform Delayed durations
/// - [`collect`](StreamIteratorExt::collect) - Collect all Next values (terminal)
/// - [`split_collector`](StreamIteratorExt::split_collector) - Split into observer + continuation
/// - [`split_collect_one`](StreamIteratorExt::split_collect_one) - Split for first match
///
/// ## Example
///
/// ```ignore
/// let stream = execute(my_task)?
///     .map_done(|v| v * 2)
///     .map_pending(|p| format!("Pending: {:?}", p))
///     .filter_done(|v| v > 10);
/// ```
pub trait StreamIteratorExt<D, P>: StreamIterator<D, P> + Sized {
    /// Transform Next (Done) values using the provided function.
    ///
    /// Init, Ignore, Delayed, and Pending states pass through unchanged.
    fn map_done<F, R>(self, f: F) -> MapDone<Self, D, P, R>
    where
        F: Fn(D) -> R + Send + 'static,
        R: Send + 'static;

    /// Transform Pending values using the provided function.
    ///
    /// Init, Ignore, Delayed, and Next states pass through unchanged.
    fn map_pending<F, R>(self, f: F) -> MapPending<Self, P, D, R>
    where
        F: Fn(P) -> R + Send + 'static,
        R: Send + 'static;

    /// Transform both Pending and Next values with a single function.
    ///
    /// Returns a unified output type for both states.
    fn map_pending_and_done<F, R>(self, f: F) -> MapPendingAndDone<Self, D, P, R>
    where
        F: Fn(Stream<D, P>) -> R + Send + 'static,
        R: Send + 'static;

    /// Filter Next (Done) values using the provided predicate.
    ///
    /// Non-Next states pass through unchanged. Next values that don't
    /// satisfy the predicate are skipped.
    fn filter_done<F>(self, f: F) -> FilterDone<Self, D, P>
    where
        F: Fn(&D) -> bool + Send + 'static;

    /// Transform Delayed durations.
    fn map_delayed<F>(self, f: F) -> MapDelayed<Self, D, P>
    where
        F: Fn(std::time::Duration) -> std::time::Duration + Send + 'static;

    /// Collect all Next values into a Vec.
    ///
    /// This is a non-blocking collect operation. It passes through
    /// Pending, Delayed, and Init states unchanged, and only yields
    /// the collected Vec<Done> when the stream completes.
    fn collect(self) -> Collect<Self, D, P>
    where
        D: Clone;

    /// Split the iterator into an observer branch and a continuation branch.
    ///
    /// The observer receives a copy of items matching the predicate,
    /// while the continuation continues the chain for further combinators.
    ///
    /// ## Type Requirements
    ///
    /// - `D` must be `Clone` (observer gets a copy)
    /// - `P` must be `Clone` (observer gets a copy)
    ///
    /// ## Arguments
    ///
    /// * `predicate` - Function determining which items to send to observer
    /// * `queue_size` - Size of the ConcurrentQueue between branches
    ///
    /// ## Returns
    ///
    /// Tuple of:
    /// - `CollectorStreamIterator` - Observer that receives matched items
    /// - `SplitCollectorContinuation` - Continuation that continues the chain
    ///
    /// ## Example
    ///
    /// ```ignore
    /// let (observer, continuation) = stream
    ///     .split_collector(
    ///         |item| matches!(item, Stream::Next(v) if v > 10),
    ///         1  // Queue size 1 for immediate delivery
    ///     );
    /// ```
    fn split_collector<Pred>(
        self,
        predicate: Pred,
        queue_size: usize,
    ) -> (
        SCollectorStreamIterator<D, P>,
        SSplitCollectorContinuation<Self, D, P>,
    )
    where
        Self: Sized,
        D: Clone,
        P: Clone,
        Pred: Fn(&Stream<D, P>) -> bool + Send + 'static;

    /// Convenience method: split_collector with queue_size = 1.
    fn split_collect_one<Pred>(
        self,
        predicate: Pred,
    ) -> (
        SCollectorStreamIterator<D, P>,
        SSplitCollectorContinuation<Self, D, P>,
    )
    where
        Self: Sized,
        D: Clone,
        P: Clone,
        Pred: Fn(&Stream<D, P>) -> bool + Send + 'static,
    {
        self.split_collector(predicate, 1)
    }

    /// Split the iterator into an observer branch and a continuation branch,
    /// closing the observer when the predicate signals close.
    fn split_collect_until<Pred>(
        self,
        predicate: Pred,
        queue_size: usize,
    ) -> (
        SSplitUntilObserver<D, P>,
        SSplitUntilContinuation<Self, D, P>,
    )
    where
        Self: Sized,
        D: Clone,
        P: Clone,
        Pred: Fn(&Stream<D, P>) -> CollectionState + Send + 'static;

    /// Split the iterator into an observer branch and a continuation branch,
    /// mapping matched items to a different type before sending to the observer.
    fn split_collector_map<F, DM, PM>(
        self,
        transform: F,
        queue_size: usize,
    ) -> (
        SSplitCollectorMapObserver<DM, PM>,
        SSplitCollectorMapContinuation<Self, D, P, DM, PM>,
    )
    where
        Self: Sized,
        DM: Clone + Send + 'static,
        PM: Clone + Send + 'static,
        D: Clone,
        P: Clone,
        F: Fn(&Stream<D, P>) -> (bool, Option<Stream<DM, PM>>) + Send + 'static;

    /// Convenience method: split_collector_map with queue_size = 1.
    fn split_collect_one_map<F, DM, PM>(
        self,
        transform: F,
    ) -> (
        SSplitCollectorMapObserver<DM, PM>,
        SSplitCollectorMapContinuation<Self, D, P, DM, PM>,
    )
    where
        Self: Sized,
        DM: Clone + Send + 'static,
        PM: Clone + Send + 'static,
        D: Clone,
        P: Clone,
        F: Fn(&Stream<D, P>) -> (bool, Option<Stream<DM, PM>>) + Send + 'static,
    {
        self.split_collector_map(transform, 1)
    }
}

// Blanket implementation: anything implementing StreamIterator gets StreamIteratorExt
impl<S, D, P> StreamIteratorExt<D, P> for S
where
    S: StreamIterator<D, P> + Send + 'static,
    D: Send + 'static,
    P: Send + 'static,
{
    fn map_done<F, R>(self, f: F) -> MapDone<Self, D, P, R>
    where
        F: Fn(D) -> R + Send + 'static,
        R: Send + 'static,
    {
        MapDone {
            inner: self,
            mapper: Box::new(f),
            _phantom: std::marker::PhantomData,
        }
    }

    fn map_pending<F, R>(self, f: F) -> MapPending<Self, P, D, R>
    where
        F: Fn(P) -> R + Send + 'static,
        R: Send + 'static,
    {
        MapPending {
            inner: self,
            mapper: Box::new(f),
            _phantom: std::marker::PhantomData,
        }
    }

    fn map_pending_and_done<F, R>(self, f: F) -> MapPendingAndDone<Self, D, P, R>
    where
        F: Fn(Stream<D, P>) -> R + Send + 'static,
        R: Send + 'static,
    {
        MapPendingAndDone {
            inner: self,
            mapper: Box::new(f),
        }
    }

    fn filter_done<F>(self, f: F) -> FilterDone<Self, D, P>
    where
        F: Fn(&D) -> bool + Send + 'static,
    {
        FilterDone {
            inner: self,
            predicate: Box::new(f),
            _phantom: std::marker::PhantomData,
        }
    }

    fn map_delayed<F>(self, f: F) -> MapDelayed<Self, D, P>
    where
        F: Fn(std::time::Duration) -> std::time::Duration + Send + 'static,
    {
        MapDelayed {
            inner: self,
            mapper: Box::new(f),
            _phantom: std::marker::PhantomData,
        }
    }

    fn collect(self) -> Collect<Self, D, P>
    where
        D: Clone,
    {
        Collect {
            inner: self,
            collected: Vec::new(),
            done: false,
            _phantom: std::marker::PhantomData,
        }
    }

    fn split_collector<Pred>(
        self,
        predicate: Pred,
        queue_size: usize,
    ) -> (
        SCollectorStreamIterator<D, P>,
        SSplitCollectorContinuation<Self, D, P>,
    )
    where
        Self: Sized,
        D: Clone,
        P: Clone,
        Pred: Fn(&Stream<D, P>) -> bool + Send + 'static,
    {
        let queue = Arc::new(ConcurrentQueue::bounded(queue_size));
        tracing::debug!(
            "split_collector: creating observer and continuation with queue_size={}",
            queue_size
        );

        let observer = SCollectorStreamIterator {
            queue: Arc::clone(&queue),
        };

        let continuation = SSplitCollectorContinuation {
            inner: self,
            queue,
            predicate: Box::new(predicate),
        };

        (observer, continuation)
    }

    fn split_collect_until<Pred>(
        self,
        predicate: Pred,
        queue_size: usize,
    ) -> (
        SSplitUntilObserver<D, P>,
        SSplitUntilContinuation<Self, D, P>,
    )
    where
        Self: Sized,
        D: Clone,
        P: Clone,
        Pred: Fn(&Stream<D, P>) -> CollectionState + Send + 'static,
    {
        let queue = Arc::new(ConcurrentQueue::bounded(queue_size));
        tracing::debug!(
            "split_collect_until: creating observer and continuation with queue_size={}",
            queue_size
        );

        let observer = SSplitUntilObserver {
            queue: Arc::clone(&queue),
        };

        let continuation = SSplitUntilContinuation {
            inner: self,
            queue,
            predicate: Box::new(predicate),
        };

        (observer, continuation)
    }

    fn split_collector_map<F, DM, PM>(
        self,
        transform: F,
        queue_size: usize,
    ) -> (
        SSplitCollectorMapObserver<DM, PM>,
        SSplitCollectorMapContinuation<Self, D, P, DM, PM>,
    )
    where
        Self: Sized,
        DM: Clone + Send + 'static,
        PM: Clone + Send + 'static,
        D: Clone,
        P: Clone,
        F: Fn(&Stream<D, P>) -> (bool, Option<Stream<DM, PM>>) + Send + 'static,
    {
        let queue = Arc::new(ConcurrentQueue::bounded(queue_size));
        tracing::debug!(
            "split_collector_map: creating observer and continuation with queue_size={}",
            queue_size
        );

        let observer = SSplitCollectorMapObserver {
            queue: Arc::clone(&queue),
        };

        let continuation = SSplitCollectorMapContinuation {
            inner: self,
            queue,
            transform: Box::new(transform),
        };

        (observer, continuation)
    }

    fn split_collect_one_map<F, DM, PM>(
        self,
        transform: F,
    ) -> (
        SSplitCollectorMapObserver<DM, PM>,
        SSplitCollectorMapContinuation<Self, D, P, DM, PM>,
    )
    where
        Self: Sized,
        DM: Clone + Send + 'static,
        PM: Clone + Send + 'static,
        D: Clone,
        P: Clone,
        F: Fn(&Stream<D, P>) -> (bool, Option<Stream<DM, PM>>) + Send + 'static,
    {
        self.split_collector_map(transform, 1)
    }
}

/// Wrapper type that transforms Next (Done) values.
pub struct MapDone<I: StreamIterator<D, P>, D, P, R> {
    inner: I,
    mapper: Box<dyn Fn(D) -> R + Send>,
    _phantom: std::marker::PhantomData<P>,
}

impl<I, D, R, P> Iterator for MapDone<I, D, P, R>
where
    I: StreamIterator<D, P>,
    R: Send + 'static,
    P: Send + 'static,
{
    type Item = Stream<R, P>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|stream| match stream {
            Stream::Next(v) => Stream::Next((self.mapper)(v)),
            Stream::Pending(v) => Stream::Pending(v),
            Stream::Delayed(d) => Stream::Delayed(d),
            Stream::Init => Stream::Init,
            Stream::Ignore => Stream::Ignore,
        })
    }
}

impl<I, D, R, P> StreamIterator<R, P> for MapDone<I, D, P, R>
where
    I: StreamIterator<D, P>,
    R: Send + 'static,
    P: Send + 'static,
{
}

/// Wrapper type that transforms Pending values.
pub struct MapPending<I: StreamIterator<D, P>, P, D, R> {
    inner: I,
    mapper: Box<dyn Fn(P) -> R + Send>,
    _phantom: std::marker::PhantomData<D>,
}

impl<I, D, P, R> Iterator for MapPending<I, P, D, R>
where
    I: StreamIterator<D, P>,
    R: Send + 'static,
    D: Send + 'static,
{
    type Item = Stream<D, R>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|stream| match stream {
            Stream::Next(v) => Stream::Next(v),
            Stream::Pending(v) => Stream::Pending((self.mapper)(v)),
            Stream::Delayed(d) => Stream::Delayed(d),
            Stream::Init => Stream::Init,
            Stream::Ignore => Stream::Ignore,
        })
    }
}

impl<I, D, P, R> StreamIterator<D, R> for MapPending<I, P, D, R>
where
    I: StreamIterator<D, P>,
    R: Send + 'static,
    D: Send + 'static,
{
}

/// Wrapper type that transforms both Pending and Next values.
pub struct MapPendingAndDone<I: StreamIterator<D, P>, D, P, R> {
    inner: I,
    mapper: Box<dyn Fn(Stream<D, P>) -> R + Send>,
}

impl<I, D, P, R> Iterator for MapPendingAndDone<I, D, P, R>
where
    I: StreamIterator<D, P>,
    R: Send + 'static,
{
    type Item = Stream<R, R>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|stream| match &stream {
            Stream::Next(_) | Stream::Pending(_) => Stream::Next((self.mapper)(stream)),
            Stream::Delayed(d) => Stream::Delayed(*d),
            Stream::Init => Stream::Init,
            Stream::Ignore => Stream::Ignore,
        })
    }
}

impl<I, D, P, R> StreamIterator<R, R> for MapPendingAndDone<I, D, P, R>
where
    I: StreamIterator<D, P>,
    R: Send + 'static,
{
}

/// Wrapper type that filters Next (Done) values.
///
/// Filtered-out Next values are returned as `Stream::Ignore` to avoid blocking.
pub struct FilterDone<I: StreamIterator<D, P>, D, P> {
    inner: I,
    predicate: Box<dyn Fn(&D) -> bool + Send>,
    _phantom: std::marker::PhantomData<P>,
}

impl<I, D, P> Iterator for FilterDone<I, D, P>
where
    I: StreamIterator<D, P>,
    D: Send + 'static,
    P: Send + 'static,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        let stream = self.inner.next()?;
        match &stream {
            Stream::Next(v) => {
                if (self.predicate)(v) {
                    Some(stream)
                } else {
                    Some(Stream::Ignore)
                }
            }
            _ => Some(stream), // Pass through non-Next states
        }
    }
}

impl<I, D, P> StreamIterator<D, P> for FilterDone<I, D, P>
where
    I: StreamIterator<D, P>,
    D: Send + 'static,
    P: Send + 'static,
{
}

/// Wrapper type that transforms Delayed durations.
pub struct MapDelayed<I: StreamIterator<D, P>, D, P> {
    inner: I,
    mapper: Box<dyn Fn(std::time::Duration) -> std::time::Duration + Send>,
    _phantom: std::marker::PhantomData<(D, P)>,
}

impl<I, D, P> Iterator for MapDelayed<I, D, P>
where
    I: StreamIterator<D, P>,
    D: Send + 'static,
    P: Send + 'static,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|stream| match stream {
            Stream::Next(v) => Stream::Next(v),
            Stream::Pending(v) => Stream::Pending(v),
            Stream::Delayed(d) => Stream::Delayed((self.mapper)(d)),
            Stream::Init => Stream::Init,
            Stream::Ignore => Stream::Ignore,
        })
    }
}

impl<I, D, P> StreamIterator<D, P> for MapDelayed<I, D, P>
where
    I: StreamIterator<D, P>,
    D: Send + 'static,
    P: Send + 'static,
{
}

/// Wrapper type that collects all Next (Done) values into a Vec.
///
/// This is a non-blocking collect operation. It passes through
/// Pending, Delayed, and Init states unchanged, and only yields
/// the collected Vec<Done> when the stream completes.
pub struct Collect<I, D, P> {
    inner: I,
    collected: Vec<D>,
    done: bool,
    _phantom: std::marker::PhantomData<P>,
}

impl<I, D, P> Iterator for Collect<I, D, P>
where
    I: StreamIterator<D, P>,
    D: Clone + Send + 'static,
    P: Send + 'static,
{
    type Item = Stream<Vec<D>, P>;

    fn next(&mut self) -> Option<Self::Item> {
        // If we've already yielded the collected result, we're done
        if self.done {
            return None;
        }

        match self.inner.next() {
            Some(Stream::Next(value)) => {
                self.collected.push(value);
                // Keep collecting, return Ignore to signal still collecting
                Some(Stream::Ignore)
            }
            Some(Stream::Pending(p)) => {
                // Pass through Pending
                Some(Stream::Pending(p))
            }
            Some(Stream::Delayed(d)) => Some(Stream::Delayed(d)),
            Some(Stream::Init) => Some(Stream::Init),
            Some(Stream::Ignore) => Some(Stream::Ignore),
            None => {
                // Inner iterator is done, yield the collected result
                self.done = true;
                Some(Stream::Next(self.collected.clone()))
            }
        }
    }
}

impl<I, D, P> StreamIterator<Vec<D>, P> for Collect<I, D, P>
where
    I: StreamIterator<D, P>,
    D: Clone + Send + 'static,
    P: Send + 'static,
{
}

// ============================================================================
// Split Collector Combinators (Feature 07)
// ============================================================================

/// Observer branch from split_collector() for StreamIterator.
///
/// Receives copies of items matching the predicate via a ConcurrentQueue.
/// Yields Stream::Next for matched items, forwards Pending/Delayed from source.
pub struct SCollectorStreamIterator<D, P> {
    /// Shared queue receiving copied items from the splitter
    queue: Arc<ConcurrentQueue<Stream<D, P>>>,
}

impl<D, P> Iterator for SCollectorStreamIterator<D, P>
where
    D: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        // Try to get item from queue
        match self.queue.pop() {
            Ok(item) => {
                tracing::trace!("SCollectorStreamIterator: received item from queue");
                Some(item)
            }
            Err(concurrent_queue::PopError::Empty) => {
                // Queue is empty, check if source is done (queue closed)
                if self.queue.is_closed() {
                    tracing::debug!("SCollectorStreamIterator: queue closed, returning None");
                    None
                } else {
                    // Still waiting for items - return Ignore to signal still pending
                    tracing::trace!(
                        "SCollectorStreamIterator: queue empty but not closed, returning Ignore"
                    );
                    Some(Stream::Ignore)
                }
            }
            Err(concurrent_queue::PopError::Closed) => {
                tracing::debug!("SCollectorStreamIterator: queue closed, returning None");
                None
            }
        }
    }
}

impl<D, P> StreamIterator<D, P> for SCollectorStreamIterator<D, P>
where
    D: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
}

/// Continuation branch from split_collector() for StreamIterator.
///
/// Wraps the original iterator, copying matched items to the observer queue
/// while continuing the chain for further combinators.
pub struct SSplitCollectorContinuation<I: StreamIterator<D, P>, D, P> {
    /// The wrapped iterator
    inner: I,
    /// Queue to send copied items to observer
    queue: Arc<ConcurrentQueue<Stream<D, P>>>,
    /// Predicate to determine which items to copy
    predicate: Box<dyn Fn(&Stream<D, P>) -> bool + Send>,
}

impl<I, D, P> Iterator for SSplitCollectorContinuation<I, D, P>
where
    I: StreamIterator<D, P>,
    D: Clone,
    P: Clone,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = match self.inner.next() {
            Some(item) => item,
            None => {
                // Source iterator is naturally exhausted, close the queue
                self.queue.close();
                tracing::debug!("SSplitCollectorContinuation: source exhausted, queue closed");
                return None;
            }
        };

        // Copy matched items to observer queue
        if (self.predicate)(&item) {
            if let Err(e) = self.queue.force_push(item.clone()) {
                tracing::error!(
                    "SSplitCollectorContinuation: failed to push to queue: {}",
                    e
                );
            } else {
                tracing::trace!(
                    "SSplitCollectorContinuation: copied matched item to observer queue"
                );
            }
        }

        // Always forward to continuation
        Some(item)
    }
}

impl<I, D, P> StreamIterator<D, P> for SSplitCollectorContinuation<I, D, P>
where
    I: StreamIterator<D, P>,
    D: Clone,
    P: Clone,
{
}

impl<I, D, P> Drop for SSplitCollectorContinuation<I, D, P>
where
    I: StreamIterator<D, P>,
{
    fn drop(&mut self) {
        // Close the queue to signal that the source is done
        self.queue.close();
        tracing::debug!("SSplitCollectorContinuation: dropped, queue closed");
    }
}

// ============================================================================
// Split Collect Until Combinator
// ============================================================================

/// Observer branch from split_collect_until() for StreamIterator.
///
/// Receives copies of items until the predicate is met, then the queue
/// is closed and the observer completes.
pub struct SSplitUntilObserver<D, P> {
    /// Shared queue receiving copied items from the splitter
    queue: Arc<ConcurrentQueue<Stream<D, P>>>,
}

impl<D, P> Iterator for SSplitUntilObserver<D, P>
where
    D: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.queue.pop() {
            Ok(item) => {
                tracing::trace!("SSplitUntilObserver: received item from queue");
                Some(item)
            }
            Err(concurrent_queue::PopError::Empty) => {
                if self.queue.is_closed() {
                    tracing::debug!("SSplitUntilObserver: queue closed, returning None");
                    None
                } else {
                    tracing::trace!(
                        "SSplitUntilObserver: queue empty but not closed, returning Ignore"
                    );
                    Some(Stream::Ignore)
                }
            }
            Err(concurrent_queue::PopError::Closed) => {
                tracing::debug!("SSplitUntilObserver: queue closed, returning None");
                None
            }
        }
    }
}

impl<D, P> StreamIterator<D, P> for SSplitUntilObserver<D, P>
where
    D: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
}

/// Continuation branch from split_collect_until() for StreamIterator.
///
/// Wraps the original iterator, copying items to the observer queue
/// based on the predicate's `CollectionState`. When predicate returns
/// `Close`, the queue is closed (observer completes).
pub struct SSplitUntilContinuation<I: StreamIterator<D, P>, D, P> {
    /// The wrapped iterator
    inner: I,
    /// Queue to send copied items to observer
    queue: Arc<ConcurrentQueue<Stream<D, P>>>,
    /// Predicate to determine when to close observer
    predicate: Box<dyn Fn(&Stream<D, P>) -> CollectionState + Send>,
}

impl<I, D, P> Iterator for SSplitUntilContinuation<I, D, P>
where
    I: StreamIterator<D, P>,
    D: Clone,
    P: Clone,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = match self.inner.next() {
            Some(item) => item,
            None => {
                // Source iterator is naturally exhausted, close the queue
                self.queue.close();
                tracing::debug!("SSplitUntilContinuation: source exhausted, queue closed");
                return None;
            }
        };

        // Handle items based on CollectionState from predicate
        match (self.predicate)(&item) {
            CollectionState::Skip => {
                // Skip this item - don't send to observer
                tracing::trace!("SSplitUntilContinuation: skipping item (CollectionState::Skip)");
            }
            CollectionState::Collect => {
                // Collect this item for the observer
                if let Err(e) = self.queue.force_push(item.clone()) {
                    tracing::error!("SSplitUntilContinuation: failed to push to queue: {}", e);
                } else {
                    tracing::trace!("SSplitUntilContinuation: collected item for observer");
                }
            }
            CollectionState::Close(collect_this) => {
                // Close the observer after optionally collecting this item
                if collect_this {
                    if let Err(e) = self.queue.force_push(item.clone()) {
                        tracing::error!("SSplitUntilContinuation: failed to push to queue: {}", e);
                    } else {
                        tracing::trace!("SSplitUntilContinuation: collecting final item and closing observer queue");
                    }
                } else {
                    tracing::trace!("SSplitUntilContinuation: closing observer queue without collecting final item");
                }
                self.queue.close();
                tracing::debug!(
                    "SSplitUntilContinuation: observer queue closed after CollectionState::Close"
                );
            }
        }

        // Always forward to continuation
        Some(item)
    }
}

impl<I, D, P> StreamIterator<D, P> for SSplitUntilContinuation<I, D, P>
where
    I: StreamIterator<D, P>,
    D: Clone,
    P: Clone,
{
}

impl<I, D, P> Drop for SSplitUntilContinuation<I, D, P>
where
    I: StreamIterator<D, P>,
{
    fn drop(&mut self) {
        // Close the queue as backup if not already closed
        if !self.queue.is_closed() {
            tracing::debug!("SSplitUntilContinuation: dropped before completion, closing queue");
            self.queue.close();
        }
    }
}

// ============================================================================
// Split Collector Map Combinator
// ============================================================================

/// Observer branch from split_collector_map() for StreamIterator.
///
/// Receives transformed copies of matched items via a ConcurrentQueue.
/// The observer yields `Stream<DM, PM>` where DM and PM are the mapped types
/// from the transform function. This allows independent Done and Pending types.
pub struct SSplitCollectorMapObserver<DM, PM> {
    /// Shared queue receiving transformed items from the continuation
    queue: Arc<ConcurrentQueue<Stream<DM, PM>>>,
}

impl<DM, PM> Iterator for SSplitCollectorMapObserver<DM, PM>
where
    DM: Clone + Send + 'static,
    PM: Clone + Send + 'static,
{
    type Item = Stream<DM, PM>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.queue.pop() {
            Ok(item) => {
                tracing::trace!("SSplitCollectorMapObserver: received item from queue");
                Some(item)
            }
            Err(concurrent_queue::PopError::Empty) => {
                if self.queue.is_closed() {
                    tracing::debug!("SSplitCollectorMapObserver: queue closed, returning None");
                    None
                } else {
                    tracing::trace!(
                        "SSplitCollectorMapObserver: queue empty but not closed, returning Ignore"
                    );
                    Some(Stream::Ignore)
                }
            }
            Err(concurrent_queue::PopError::Closed) => {
                tracing::debug!("SSplitCollectorMapObserver: queue closed, returning None");
                None
            }
        }
    }
}

impl<DM, PM> StreamIterator<DM, PM> for SSplitCollectorMapObserver<DM, PM>
where
    DM: Clone + Send + 'static,
    PM: Clone + Send + 'static,
{
}

/// Continuation branch from split_collector_map() for StreamIterator.
///
/// Wraps the original iterator. The transform function returns `(bool, Option<Stream<DM, PM>>)`
/// where DM and PM are independent Done and Pending types for the observer:
/// - `true` + `Some(stream)` sends the stream to the observer queue
/// - `false` or `None` skips sending to observer
/// The continuation continues with original Stream<D, P> values unchanged.
pub struct SSplitCollectorMapContinuation<I: StreamIterator<D, P>, D, P, DM, PM> {
    /// The wrapped iterator
    inner: I,
    /// Queue to send transformed items to observer
    queue: Arc<ConcurrentQueue<Stream<DM, PM>>>,
    /// Combined predicate + transform function
    transform: Box<dyn Fn(&Stream<D, P>) -> (bool, Option<Stream<DM, PM>>) + Send>,
}

impl<I, D, P, DM, PM> Iterator for SSplitCollectorMapContinuation<I, D, P, DM, PM>
where
    I: StreamIterator<D, P>,
    D: Clone,
    P: Clone,
    DM: Clone + Send + 'static,
    PM: Clone + Send + 'static,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = match self.inner.next() {
            Some(item) => item,
            None => {
                self.queue.close();
                tracing::debug!("SSplitCollectorMapContinuation: source exhausted, queue closed");
                return None;
            }
        };

        let (matched, transformed) = (self.transform)(&item);
        if matched {
            if let Some(transformed) = transformed {
                if let Err(e) = self.queue.force_push(transformed) {
                    tracing::error!(
                        "SSplitCollectorMapContinuation: failed to push to queue: {}",
                        e
                    );
                } else {
                    tracing::trace!(
                        "SSplitCollectorMapContinuation: copied transformed item to observer queue"
                    );
                }
            }
        }

        Some(item)
    }
}

impl<I, D, P, DM, PM> StreamIterator<D, P> for SSplitCollectorMapContinuation<I, D, P, DM, PM>
where
    I: StreamIterator<D, P>,
    D: Clone,
    P: Clone,
    DM: Clone + Send + 'static,
    PM: Clone + Send + 'static,
{
}

impl<I, D, P, DM, PM> Drop for SSplitCollectorMapContinuation<I, D, P, DM, PM>
where
    I: StreamIterator<D, P>,
{
    fn drop(&mut self) {
        if !self.queue.is_closed() {
            tracing::debug!(
                "SSplitCollectorMapContinuation: dropped before completion, closing queue"
            );
            self.queue.close();
        }
    }
}

// ============================================================================
// Multi-Source Combinators (Feature 02)
// ============================================================================

/// Extension trait for multi-source StreamIterator combinators.
///
/// These combinators work with multiple StreamIterators simultaneously,
/// aggregating their outputs or mapping them together.
pub trait MultiSourceStreamIteratorExt<D, P> {
    /// Collect all outputs from multiple StreamIterators into a single Vec.
    ///
    /// Polls all sources in round-robin fashion, collecting Next values.
    /// Yields Stream::Pending with count while any source is still producing.
    /// Yields Stream::Next with all collected values when all sources complete.
    fn collect_all<I>(iterators: Vec<I>) -> CollectAll<I, D, P>
    where
        I: StreamIterator<D, P> + Send + 'static,
        D: Clone + Send + 'static,
        P: Send + 'static;

    /// Map all values - only when all sources reach Done state.
    ///
    /// Buffers values from all sources until all have produced a Next value.
    /// Then applies the mapper to the collected Vec and yields the result.
    fn map_all_done<I, F, O>(iterators: Vec<I>, mapper: F) -> MapAllDone<I, F, D, P, O>
    where
        I: StreamIterator<D, P> + Send + 'static,
        F: Fn(Vec<D>) -> O + Send + 'static,
        O: Send + 'static,
        D: Clone + Send + 'static,
        P: Send + 'static;

    /// Map all values processing both Pending and Done states together.
    ///
    /// Buffers Stream<D, P> from all sources and applies the mapper
    /// to the Vec of all states, enabling visibility into which sources
    /// are pending vs done.
    fn map_all_pending_and_done<I, F, O>(
        iterators: Vec<I>,
        mapper: F,
    ) -> MapAllPendingAndDone<I, F, D, P, O>
    where
        I: StreamIterator<D, P> + Send + 'static,
        F: Fn(Vec<Stream<D, P>>) -> O + Send + 'static,
        O: Send + 'static,
        D: Clone + Send + 'static,
        P: Send + 'static;
}

impl<D, P> MultiSourceStreamIteratorExt<D, P> for ()
where
    D: Clone + Send + 'static,
    P: Send + 'static,
{
    fn collect_all<I>(iterators: Vec<I>) -> CollectAll<I, D, P>
    where
        I: StreamIterator<D, P> + Send + 'static,
        D: Clone + Send + 'static,
        P: Send + 'static,
    {
        CollectAll::new(iterators)
    }

    fn map_all_done<I, F, O>(iterators: Vec<I>, mapper: F) -> MapAllDone<I, F, D, P, O>
    where
        I: StreamIterator<D, P> + Send + 'static,
        F: Fn(Vec<D>) -> O + Send + 'static,
        O: Send + 'static,
        D: Clone + Send + 'static,
        P: Send + 'static,
    {
        MapAllDone::new(iterators, mapper)
    }

    fn map_all_pending_and_done<I, F, O>(
        iterators: Vec<I>,
        mapper: F,
    ) -> MapAllPendingAndDone<I, F, D, P, O>
    where
        I: StreamIterator<D, P> + Send + 'static,
        F: Fn(Vec<Stream<D, P>>) -> O + Send + 'static,
        O: Send + 'static,
        D: Clone + Send + 'static,
        P: Send + 'static,
    {
        MapAllPendingAndDone::new(iterators, mapper)
    }
}

/// Multi-source collector that aggregates outputs from multiple StreamIterators.
///
/// Polls all sources in round-robin, collecting Next values.
/// Yields Stream::Pending(count) while gathering.
/// Yields Stream::Next(Vec<D>) when all sources complete.
pub struct CollectAll<I, D, P> {
    sources: Vec<I>,
    collected: Vec<D>,
    done: bool,
    _phantom: std::marker::PhantomData<P>,
}

impl<I, D, P> CollectAll<I, D, P>
where
    I: StreamIterator<D, P>,
    D: Clone + Send + 'static,
    P: Send + 'static,
{
    pub fn new(iterators: Vec<I>) -> Self {
        Self {
            sources: iterators,
            collected: Vec::new(),
            done: false,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<I, D, P> Iterator for CollectAll<I, D, P>
where
    I: StreamIterator<D, P> + Send + 'static,
    D: Clone + Send + 'static,
    P: Send + 'static,
{
    type Item = Stream<Vec<D>, usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let mut all_done = true;
        let mut has_pending = false;
        let mut max_delayed: Option<std::time::Duration> = None;

        // Poll all sources in round-robin
        for source in &mut self.sources {
            match source.next() {
                Some(Stream::Next(value)) => {
                    self.collected.push(value);
                    all_done = false;
                }
                Some(Stream::Pending(_)) => {
                    all_done = false;
                    has_pending = true;
                }
                Some(Stream::Delayed(d)) => {
                    all_done = false;
                    max_delayed = Some(match max_delayed {
                        Some(current) => current.max(d),
                        None => d,
                    });
                }
                Some(Stream::Init) => {
                    all_done = false;
                }
                Some(Stream::Ignore) => {
                    // Ignore internal events, keep collecting
                    all_done = false;
                }
                None => {
                    // This source is exhausted
                }
            }
        }

        if all_done {
            // All sources exhausted, yield collected results
            self.done = true;
            if self.collected.is_empty() {
                return None;
            }
            Some(Stream::Next(std::mem::take(&mut self.collected)))
        } else if let Some(delay) = max_delayed {
            Some(Stream::Delayed(delay))
        } else if has_pending {
            Some(Stream::Pending(self.collected.len()))
        } else {
            // Still collecting, return pending with count
            Some(Stream::Pending(self.collected.len()))
        }
    }
}

impl<I, D, P> StreamIterator<Vec<D>, usize> for CollectAll<I, D, P>
where
    I: StreamIterator<D, P> + Send + 'static,
    D: Clone + Send + 'static,
    P: Send + 'static,
{
}

/// Multi-source mapper that applies a function when all sources reach Done.
pub struct MapAllDone<I, F, D, P, O> {
    sources: Vec<I>,
    mapper: F,
    buffer: Vec<Option<D>>,
    done: bool,
    _phantom: std::marker::PhantomData<(P, O)>,
}

impl<I, F, D, P, O> MapAllDone<I, F, D, P, O>
where
    I: StreamIterator<D, P>,
    F: Fn(Vec<D>) -> O + Send + 'static,
    O: Send + 'static,
    D: Clone + Send + 'static,
    P: Send + 'static,
{
    pub fn new(iterators: Vec<I>, mapper: F) -> Self {
        let len = iterators.len();
        Self {
            sources: iterators,
            mapper,
            buffer: (0..len).map(|_| None).collect(),
            done: false,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<I, F, D, P, O> Iterator for MapAllDone<I, F, D, P, O>
where
    I: StreamIterator<D, P> + Send + 'static,
    F: Fn(Vec<D>) -> O + Send + 'static,
    O: Send + 'static,
    D: Clone + Send + 'static,
    P: Send + 'static,
{
    type Item = Stream<O, usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let mut all_done = true;
        let mut has_pending = false;

        for (i, source) in self.sources.iter_mut().enumerate() {
            if self.buffer[i].is_some() {
                continue; // Already have value from this source
            }

            match source.next() {
                Some(Stream::Next(value)) => {
                    self.buffer[i] = Some(value);
                }
                Some(Stream::Pending(_)) => {
                    all_done = false;
                    has_pending = true;
                }
                Some(Stream::Delayed(d)) => return Some(Stream::Delayed(d)),
                Some(Stream::Init) => {
                    all_done = false;
                }
                Some(Stream::Ignore) => {
                    all_done = false;
                }
                None => {
                    // Source exhausted without producing - treat as done
                }
            }
        }

        // Check if all sources have produced a value
        if self.buffer.iter().all(|x| x.is_some()) {
            self.done = true;
            let values: Vec<D> = self.buffer.drain(..).filter_map(|x| x).collect();
            let result = (self.mapper)(values);
            return Some(Stream::Next(result));
        }

        if all_done {
            // All sources exhausted but not all produced values
            self.done = true;
            return None;
        }

        if has_pending {
            let collected: usize = self.buffer.iter().filter(|x| x.is_some()).count();
            Some(Stream::Pending(collected))
        } else {
            Some(Stream::Init)
        }
    }
}

impl<I, F, D, P, O> StreamIterator<O, usize> for MapAllDone<I, F, D, P, O>
where
    I: StreamIterator<D, P> + Send + 'static,
    F: Fn(Vec<D>) -> O + Send + 'static,
    O: Send + 'static,
    D: Clone + Send + 'static,
    P: Send + 'static,
{
}

/// Multi-source mapper that processes both Pending and Done states together.
pub struct MapAllPendingAndDone<I, F, D, P, O> {
    sources: Vec<I>,
    mapper: F,
    done: bool,
    _phantom: std::marker::PhantomData<(D, P, O)>,
}

impl<I, F, D, P, O> MapAllPendingAndDone<I, F, D, P, O>
where
    I: StreamIterator<D, P>,
    F: Fn(Vec<Stream<D, P>>) -> O + Send + 'static,
    O: Send + 'static,
    D: Clone + Send + 'static,
    P: Send + 'static,
{
    pub fn new(iterators: Vec<I>, mapper: F) -> Self {
        Self {
            sources: iterators,
            mapper,
            done: false,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<I, F, D, P, O> Iterator for MapAllPendingAndDone<I, F, D, P, O>
where
    I: StreamIterator<D, P> + Send + 'static,
    F: Fn(Vec<Stream<D, P>>) -> O + Send + 'static,
    O: Send + 'static,
    D: Clone + Send + 'static,
    P: Send + 'static,
{
    type Item = Stream<O, P>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let mut states: Vec<Stream<D, P>> = Vec::with_capacity(self.sources.len());

        for source in &mut self.sources {
            match source.next() {
                Some(state) => {
                    states.push(state);
                }
                None => {
                    // Source exhausted
                }
            }
        }

        if states.is_empty() {
            self.done = true;
            return None;
        }

        let result = (self.mapper)(states);
        Some(Stream::Next(result))
    }
}

impl<I, F, D, P, O> StreamIterator<O, P> for MapAllPendingAndDone<I, F, D, P, O>
where
    I: StreamIterator<D, P> + Send + 'static,
    F: Fn(Vec<Stream<D, P>>) -> O + Send + 'static,
    O: Send + 'static,
    D: Clone + Send + 'static,
    P: Send + 'static,
{
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple test stream iterator for unit tests
    struct TestStream {
        items: Vec<Stream<u32, String>>,
    }

    impl TestStream {
        fn new(items: Vec<Stream<u32, String>>) -> Self {
            Self { items }
        }
    }

    impl Iterator for TestStream {
        type Item = Stream<u32, String>;

        fn next(&mut self) -> Option<Self::Item> {
            self.items.pop()
        }
    }

    impl StreamIterator<u32, String> for TestStream {}

    #[test]
    fn test_map_done() {
        let items = vec![
            Stream::Pending("wait".to_string()),
            Stream::Next(10),
            Stream::Next(5),
        ];
        let stream = TestStream::new(items);
        let mut mapped = stream.map_done(|x| x * 2);

        // pop() returns items in reverse order: 5, 10, Pending
        // After mapping: 10 (5*2), 20 (10*2), Pending
        assert_eq!(Iterator::next(&mut mapped), Some(Stream::Next(10)));
        assert_eq!(Iterator::next(&mut mapped), Some(Stream::Next(20)));
        assert_eq!(
            Iterator::next(&mut mapped),
            Some(Stream::Pending("wait".to_string()))
        );
    }

    #[test]
    fn test_map_pending() {
        let items = vec![Stream::Pending("wait".to_string()), Stream::Next(5)];
        let stream = TestStream::new(items);
        let mut mapped = stream.map_pending(|s| s.len());

        assert_eq!(Iterator::next(&mut mapped), Some(Stream::Next(5)));
        assert_eq!(Iterator::next(&mut mapped), Some(Stream::Pending(4)));
    }

    #[test]
    fn test_filter_done() {
        let items = vec![Stream::Next(3), Stream::Next(10), Stream::Next(5)];
        let stream = TestStream::new(items);
        let mut filtered = stream.filter_done(|x| *x > 5);

        // pop() returns: Next(5), Next(10), Next(3)
        // filter: 5 > 5 = false → Ignore, 10 > 5 = true → Next(10), 3 > 5 = false → Ignore
        assert_eq!(Iterator::next(&mut filtered), Some(Stream::Ignore)); // 5 was filtered out
        assert_eq!(Iterator::next(&mut filtered), Some(Stream::Next(10)));
        assert_eq!(Iterator::next(&mut filtered), Some(Stream::Ignore)); // 3 was filtered out
        assert_eq!(Iterator::next(&mut filtered), None);
    }

    #[test]
    fn test_collect() {
        let items = vec![
            Stream::Next(3),
            Stream::Next(2),
            Stream::Pending("wait".to_string()),
            Stream::Next(1),
        ];
        let stream = TestStream::new(items);
        let mut collected = StreamIteratorExt::collect(stream);

        // pop() returns: Next(1), Pending, Next(2), Next(3)
        // First Next(1) is collected, returns Ignore
        assert_eq!(Iterator::next(&mut collected), Some(Stream::Ignore));

        // Pending passes through unchanged
        assert_eq!(
            Iterator::next(&mut collected),
            Some(Stream::Pending("wait".to_string()))
        );

        // Next(2) is collected, returns Ignore
        assert_eq!(Iterator::next(&mut collected), Some(Stream::Ignore));

        // Next(3) is collected, returns Ignore
        assert_eq!(Iterator::next(&mut collected), Some(Stream::Ignore));

        // Should yield collected Vec at the end
        match Iterator::next(&mut collected) {
            Some(Stream::Next(vec)) => {
                assert_eq!(vec.len(), 3);
                assert!(vec.contains(&1));
                assert!(vec.contains(&2));
                assert!(vec.contains(&3));
            }
            other => panic!("Expected Next(Vec), got {:?}", other),
        }

        // Should be done after yielding the collected result
        assert_eq!(Iterator::next(&mut collected), None);
    }

    #[test]
    fn test_map_delayed() {
        use std::time::Duration;

        let items = vec![Stream::Delayed(Duration::from_secs(1)), Stream::Next(5)];
        let stream = TestStream::new(items);
        let mut mapped = stream.map_delayed(|d| d * 2);

        assert_eq!(Iterator::next(&mut mapped), Some(Stream::Next(5)));
        assert_eq!(
            Iterator::next(&mut mapped),
            Some(Stream::Delayed(Duration::from_secs(2)))
        );
    }

    #[test]
    fn test_split_collector_map_sends_transformed_items() {
        let items = vec![
            Stream::Pending("wait".to_string()),
            Stream::Next(10),
            Stream::Next(3),
            Stream::Next(20),
        ];
        let stream = TestStream::new(items);

        // Observer gets string representations of Next items > 5
        let (observer, mut continuation) = stream.split_collector_map::<_, String, ()>(
            |item| match item {
                Stream::Next(v) if *v > 5 => (true, Some(Stream::Next(format!("val:{}", v)))),
                _ => (false, None),
            },
            10,
        );

        // Drive the continuation to completion
        while Iterator::next(&mut continuation).is_some() {}

        let collected: Vec<_> = observer
            .filter_map(|item| match item {
                Stream::Next(v) => Some(v),
                _ => None,
            })
            .collect();

        assert_eq!(collected.len(), 2);
        assert!(collected.contains(&"val:20".to_string()));
        assert!(collected.contains(&"val:10".to_string()));
    }

    #[test]
    fn test_split_collector_map_transform_returns_none_skips() {
        let items = vec![Stream::Next(10), Stream::Next(5)];
        let stream = TestStream::new(items);

        // Matched but transform returns None for odd numbers → skipped
        let (observer, mut continuation) = stream.split_collector_map::<_, u64, ()>(
            |item| match item {
                Stream::Next(v) if v % 2 == 0 => (true, Some(Stream::Next(*v as u64))),
                Stream::Next(_) => (true, None),
                _ => (false, None),
            },
            10,
        );

        while Iterator::next(&mut continuation).is_some() {}

        let collected: Vec<_> = observer
            .filter_map(|item| match item {
                Stream::Next(v) => Some(v),
                _ => None,
            })
            .collect();

        assert_eq!(collected, vec![10u64]);
    }

    #[test]
    fn test_split_collect_one_map() {
        let items = vec![Stream::Next(3), Stream::Next(10), Stream::Next(20)];
        let stream = TestStream::new(items);

        let (observer, mut continuation) =
            stream.split_collect_one_map::<_, String, ()>(|item| match item {
                Stream::Next(v) if *v > 5 => (true, Some(Stream::Next(v.to_string()))),
                _ => (false, None),
            });

        while Iterator::next(&mut continuation).is_some() {}

        let collected: Vec<_> = observer
            .filter_map(|item| match item {
                Stream::Next(v) => Some(v),
                _ => None,
            })
            .collect();

        // Queue size 1, so only first match gets through
        assert_eq!(collected.len(), 1);
    }

    #[test]
    fn test_collect_all() {
        // Create two test streams
        let stream1 = TestStream::new(vec![Stream::Next(1u32), Stream::Next(2)]);
        let stream2 = TestStream::new(vec![Stream::Next(10u32), Stream::Next(20)]);

        let mut combined = CollectAll::new(vec![stream1, stream2]);

        // Should collect all values and yield single Next with combined results
        let mut collected_values = Vec::new();
        for item in &mut combined {
            match item {
                Stream::Next(values) => collected_values.extend(values),
                Stream::Pending(_) | Stream::Ignore => {}
                _ => {}
            }
        }

        assert_eq!(collected_values.len(), 4);
        assert!(collected_values.contains(&1));
        assert!(collected_values.contains(&2));
        assert!(collected_values.contains(&10));
        assert!(collected_values.contains(&20));
    }

    #[test]
    fn test_map_all_done() {
        // Create two test streams
        let stream1 = TestStream::new(vec![Stream::Next(1u32)]);
        let stream2 = TestStream::new(vec![Stream::Next(10u32)]);

        let mut mapper = MapAllDone::new(vec![stream1, stream2], |values: Vec<u32>| {
            values.iter().sum::<u32>()
        });

        // Should apply mapper when all sources produce values
        let mut got_result = false;
        for item in &mut mapper {
            match item {
                Stream::Next(sum) => {
                    assert_eq!(sum, 11); // 1 + 10
                    got_result = true;
                }
                Stream::Pending(_) | Stream::Init => {}
                _ => {}
            }
        }

        assert!(got_result, "Should have produced mapped result");
    }

    #[test]
    fn test_map_all_pending_and_done() {
        // Create two test streams
        let stream1 = TestStream::new(vec![Stream::Next(1u32)]);
        let stream2 = TestStream::new(vec![Stream::Next(10u32)]);

        let mut mapper = MapAllPendingAndDone::new(
            vec![stream1, stream2],
            |states: Vec<Stream<u32, String>>| states.len(),
        );

        // Should apply mapper to all states
        let mut got_result = false;
        for item in &mut mapper {
            match item {
                Stream::Next(count) => {
                    assert_eq!(count, 2); // Two states
                    got_result = true;
                }
                _ => {}
            }
        }

        assert!(got_result, "Should have produced mapped result");
    }
}
