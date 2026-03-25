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
pub trait StreamIteratorExt: StreamIterator + Sized {
    /// Transform Next (Done) values using the provided function.
    ///
    /// Init, Ignore, Delayed, and Pending states pass through unchanged.
    fn map_done<F, R>(self, f: F) -> MapDone<Self, R>
    where
        F: Fn(Self::D) -> R + Send + 'static,
        R: Send + 'static;

    /// Transform Pending values using the provided function.
    ///
    /// Init, Ignore, Delayed, and Next states pass through unchanged.
    fn map_pending<F, R>(self, f: F) -> MapPending<Self, R>
    where
        F: Fn(Self::P) -> R + Send + 'static,
        R: Send + 'static;

    /// Transform both Pending and Next values with a single function.
    ///
    /// Returns a unified output type for both states.
    fn map_pending_and_done<F, R>(self, f: F) -> MapPendingAndDone<Self, R>
    where
        F: Fn(Stream<Self::D, Self::P>) -> R + Send + 'static,
        R: Send + 'static;

    /// Filter Next (Done) values using the provided predicate.
    ///
    /// Non-Next states pass through unchanged. Next values that don't
    /// satisfy the predicate are skipped.
    fn filter_done<F>(self, f: F) -> FilterDone<Self>
    where
        F: Fn(&Self::D) -> bool + Send + 'static;

    /// Transform Delayed durations.
    fn map_delayed<F>(self, f: F) -> MapDelayed<Self>
    where
        F: Fn(std::time::Duration) -> std::time::Duration + Send + 'static;

    /// Collect all Next values into a Vec.
    ///
    /// This is a non-blocking collect operation. It passes through
    /// Pending, Delayed, and Init states unchanged, and only yields
    /// the collected Vec<Done> when the stream completes.
    fn collect(self) -> Collect<Self>
    where
        Self::D: Clone;

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
    /// * `queue_size` - Size of the `ConcurrentQueue` between branches
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
        SCollectorStreamIterator<Self::D, Self::P>,
        SSplitCollectorContinuation<Self, Self::D, Self::P>,
    )
    where
        Self: Sized,
        Self::D: Clone,
        Self::P: Clone,
        Pred: Fn(&Stream<Self::D, Self::P>) -> bool + Send + 'static;

    /// Convenience method: `split_collector` with `queue_size` = 1.
    fn split_collect_one<Pred>(
        self,
        predicate: Pred,
    ) -> (
        SCollectorStreamIterator<Self::D, Self::P>,
        SSplitCollectorContinuation<Self, Self::D, Self::P>,
    )
    where
        Self: Sized,
        Self::D: Clone,
        Self::P: Clone,
        Pred: Fn(&Stream<Self::D, Self::P>) -> bool + Send + 'static,
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
        SSplitUntilObserver<Self::D, Self::P>,
        SSplitUntilContinuation<Self, Self::D, Self::P>,
    )
    where
        Self: Sized,
        Self::D: Clone,
        Self::P: Clone,
        Pred: Fn(&Stream<Self::D, Self::P>) -> CollectionState + Send + 'static;

    /// Split the iterator into an observer branch and a continuation branch,
    /// mapping matched items to a different type before sending to the observer.
    fn split_collector_map<F, DM, PM>(
        self,
        transform: F,
        queue_size: usize,
    ) -> (
        SSplitCollectorMapObserver<DM, PM>,
        SSplitCollectorMapContinuation<Self, Self::D, Self::P, DM, PM>,
    )
    where
        Self: Sized,
        DM: Clone + Send + 'static,
        PM: Clone + Send + 'static,
        Self::D: Clone,
        Self::P: Clone,
        F: Fn(&Stream<Self::D, Self::P>) -> (bool, Option<Stream<DM, PM>>) + Send + 'static;

    /// Convenience method: `split_collector_map` with `queue_size` = 1.
    fn split_collect_one_map<F, DM, PM>(
        self,
        transform: F,
    ) -> (
        SSplitCollectorMapObserver<DM, PM>,
        SSplitCollectorMapContinuation<Self, Self::D, Self::P, DM, PM>,
    )
    where
        Self: Sized,
        DM: Clone + Send + 'static,
        PM: Clone + Send + 'static,
        Self::D: Clone,
        Self::P: Clone,
        F: Fn(&Stream<Self::D, Self::P>) -> (bool, Option<Stream<DM, PM>>) + Send + 'static,
    {
        self.split_collector_map(transform, 1)
    }

    /// Flatten nested iterator patterns where outer yields inner iterators.
    ///
    /// The mapper function receives the full `Stream<Outer::D, Outer::P>` from the outer iterator
    /// and returns an inner iterator. This allows the mapper to decide how to handle
    /// outer's non-Next states (Pending, Delayed, etc.).
    ///
    /// The returned `MapIter` drains each inner iterator until `None`, then polls
    /// the outer for the next item.
    fn map_iter<F, InnerIter>(self, mapper: F) -> MapIter<Self, F, InnerIter>
    where
        Self: Sized,
        F: Fn(Stream<Self::D, Self::P>) -> InnerIter + Send + 'static,
        InnerIter: StreamIterator + Send + 'static,
    {
        MapIter {
            outer: self,
            mapper,
            current_inner: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Flatten Next (Done) values that implement IntoIterator.
    ///
    /// Input:  StreamIterator<D = Vec<M>, P = P>
    /// Output: StreamIterator<D = M, P = P>
    ///
    /// The user's Done type implements IntoIterator. We store the inner iterator
    /// and drain it over multiple next() calls. When exhausted (None), poll outer again.
    ///
    /// Returns Stream::Ignore when waiting for the inner iterator to produce more values.
    fn flatten_next(self) -> SFlattenNext<Self>
    where
        Self: Sized,
        Self::D: IntoIterator,
        <Self::D as IntoIterator>::Item: Send + 'static;

    /// Flatten Pending values that implement IntoIterator.
    ///
    /// Input:  StreamIterator<D = D, P = Vec<M>>
    /// Output: StreamIterator<D = D, P = M>
    ///
    /// The user's Pending type implements IntoIterator. We store the inner iterator
    /// and drain it over multiple next() calls. When exhausted (None), poll outer again.
    fn flatten_pending(self) -> SFlattenPending<Self>
    where
        Self: Sized,
        Self::P: IntoIterator,
        <Self::P as IntoIterator>::Item: Send + 'static;

    /// Flat map Next (Done) values - transform and flatten in one operation.
    ///
    /// Input:  StreamIterator<D = D, P = P>
    /// Output: StreamIterator<D = U::Item, P = P>
    ///
    /// The mapper function transforms each Done value into an IntoIterator,
    /// which is then flattened. Inner iterator is drained over multiple next() calls.
    fn flat_map_next<F, U>(self, f: F) -> SFlatMapNext<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::D) -> U + Send + 'static,
        U: IntoIterator,
        U::Item: Send + 'static;

    /// Flat map Pending values - transform and flatten in one operation.
    ///
    /// Input:  StreamIterator<D = D, P = P>
    /// Output: StreamIterator<D = D, P = U::Item>
    ///
    /// The mapper function transforms each Pending value into an IntoIterator,
    /// which is then flattened. Inner iterator is drained over multiple next() calls.
    fn flat_map_pending<F, U>(self, f: F) -> SFlatMapPending<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::P) -> U + Send + 'static,
        U: IntoIterator,
        U::Item: Send + 'static;
}

// Blanket implementation: anything implementing StreamIterator gets StreamIteratorExt
impl<S> StreamIteratorExt for S
where
    S: StreamIterator + Send + 'static,
{
    fn map_done<F, R>(self, f: F) -> MapDone<Self, R>
    where
        F: Fn(Self::D) -> R + Send + 'static,
        R: Send + 'static,
    {
        MapDone {
            inner: self,
            mapper: Box::new(f),
            _phantom: std::marker::PhantomData,
        }
    }

    fn map_pending<F, R>(self, f: F) -> MapPending<Self, R>
    where
        F: Fn(Self::P) -> R + Send + 'static,
        R: Send + 'static,
    {
        MapPending {
            inner: self,
            mapper: Box::new(f),
            _phantom: std::marker::PhantomData,
        }
    }

    fn map_pending_and_done<F, R>(self, f: F) -> MapPendingAndDone<Self, R>
    where
        F: Fn(Stream<Self::D, Self::P>) -> R + Send + 'static,
        R: Send + 'static,
    {
        MapPendingAndDone {
            inner: self,
            mapper: Box::new(f),
        }
    }

    fn filter_done<F>(self, f: F) -> FilterDone<Self>
    where
        F: Fn(&Self::D) -> bool + Send + 'static,
    {
        FilterDone {
            inner: self,
            predicate: Box::new(f),
            _phantom: std::marker::PhantomData,
        }
    }

    fn map_delayed<F>(self, f: F) -> MapDelayed<Self>
    where
        F: Fn(std::time::Duration) -> std::time::Duration + Send + 'static,
    {
        MapDelayed {
            inner: self,
            mapper: Box::new(f),
            _phantom: std::marker::PhantomData,
        }
    }

    fn collect(self) -> Collect<Self>
    where
        Self::D: Clone,
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
        SCollectorStreamIterator<Self::D, Self::P>,
        SSplitCollectorContinuation<Self, Self::D, Self::P>,
    )
    where
        Self: Sized,
        Self::D: Clone,
        Self::P: Clone,
        Pred: Fn(&Stream<Self::D, Self::P>) -> bool + Send + 'static,
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
        SSplitUntilObserver<Self::D, Self::P>,
        SSplitUntilContinuation<Self, Self::D, Self::P>,
    )
    where
        Self: Sized,
        Self::D: Clone,
        Self::P: Clone,
        Pred: Fn(&Stream<Self::D, Self::P>) -> CollectionState + Send + 'static,
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
        SSplitCollectorMapContinuation<Self, Self::D, Self::P, DM, PM>,
    )
    where
        Self: Sized,
        DM: Clone + Send + 'static,
        PM: Clone + Send + 'static,
        Self::D: Clone,
        Self::P: Clone,
        F: Fn(&Stream<Self::D, Self::P>) -> (bool, Option<Stream<DM, PM>>) + Send + 'static,
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
        SSplitCollectorMapContinuation<Self, Self::D, Self::P, DM, PM>,
    )
    where
        Self: Sized,
        DM: Clone + Send + 'static,
        PM: Clone + Send + 'static,
        Self::D: Clone,
        Self::P: Clone,
        F: Fn(&Stream<Self::D, Self::P>) -> (bool, Option<Stream<DM, PM>>) + Send + 'static,
    {
        self.split_collector_map(transform, 1)
    }

    fn flatten_next(self) -> SFlattenNext<Self>
    where
        Self: Sized,
        Self::D: IntoIterator,
        <Self::D as IntoIterator>::Item: Send + 'static,
    {
        SFlattenNext {
            inner: self,
            current_inner: None,
            _phantom: std::marker::PhantomData,
        }
    }

    fn flatten_pending(self) -> SFlattenPending<Self>
    where
        Self: Sized,
        Self::P: IntoIterator,
        <Self::P as IntoIterator>::Item: Send + 'static,
    {
        SFlattenPending {
            inner: self,
            current_inner: None,
            _phantom: std::marker::PhantomData,
        }
    }

    fn flat_map_next<F, U>(self, f: F) -> SFlatMapNext<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::D) -> U + Send + 'static,
        U: IntoIterator,
        U::Item: Send + 'static,
    {
        SFlatMapNext {
            inner: self,
            mapper: f,
            current_inner: None,
            _phantom: std::marker::PhantomData,
        }
    }

    fn flat_map_pending<F, U>(self, f: F) -> SFlatMapPending<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::P) -> U + Send + 'static,
        U: IntoIterator,
        U::Item: Send + 'static,
    {
        SFlatMapPending {
            inner: self,
            mapper: f,
            current_inner: None,
            _phantom: std::marker::PhantomData,
        }
    }
}

/// Wrapper type that transforms Next (Done) values.
pub struct MapDone<I, R>
where
    I: StreamIterator,
{
    inner: I,
    mapper: Box<dyn Fn(I::D) -> R + Send>,
    _phantom: std::marker::PhantomData<(I::P, R)>,
}

impl<I, R> Iterator for MapDone<I, R>
where
    I: StreamIterator,
    R: Send + 'static,
{
    type Item = Stream<R, I::P>;

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

impl<I, R> StreamIterator for MapDone<I, R>
where
    I: StreamIterator,
    R: Send + 'static,
{
    type D = R;
    type P = I::P;
}

/// Wrapper type that transforms Pending values.
pub struct MapPending<I, R>
where
    I: StreamIterator,
{
    inner: I,
    mapper: Box<dyn Fn(I::P) -> R + Send>,
    _phantom: std::marker::PhantomData<(I::D, R)>,
}

impl<I, R> Iterator for MapPending<I, R>
where
    I: StreamIterator,
    R: Send + 'static,
{
    type Item = Stream<I::D, R>;

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

impl<I, R> StreamIterator for MapPending<I, R>
where
    I: StreamIterator,
    R: Send + 'static,
{
    type D = I::D;
    type P = R;
}

/// Wrapper type that transforms both Pending and Next values.
pub struct MapPendingAndDone<I, R>
where
    I: StreamIterator,
{
    inner: I,
    mapper: Box<dyn Fn(Stream<I::D, I::P>) -> R + Send>,
}

impl<I, R> Iterator for MapPendingAndDone<I, R>
where
    I: StreamIterator,
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

impl<I, R> StreamIterator for MapPendingAndDone<I, R>
where
    I: StreamIterator,
    R: Send + 'static,
{
    type D = R;
    type P = R;
}

/// Wrapper type that filters Next (Done) values.
///
/// Filtered-out Next values are returned as `Stream::Ignore` to avoid blocking.
pub struct FilterDone<I>
where
    I: StreamIterator,
{
    inner: I,
    predicate: Box<dyn Fn(&I::D) -> bool + Send>,
    _phantom: std::marker::PhantomData<I::P>,
}

impl<I> Iterator for FilterDone<I>
where
    I: StreamIterator,
{
    type Item = Stream<I::D, I::P>;

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

impl<I> StreamIterator for FilterDone<I>
where
    I: StreamIterator,
{
    type D = I::D;
    type P = I::P;
}

/// Wrapper type that transforms Delayed durations.
pub struct MapDelayed<I>
where
    I: StreamIterator,
{
    inner: I,
    mapper: Box<dyn Fn(std::time::Duration) -> std::time::Duration + Send>,
    _phantom: std::marker::PhantomData<(I::D, I::P)>,
}

impl<I> Iterator for MapDelayed<I>
where
    I: StreamIterator,
{
    type Item = Stream<I::D, I::P>;

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

impl<I> StreamIterator for MapDelayed<I>
where
    I: StreamIterator,
{
    type D = I::D;
    type P = I::P;
}

/// Wrapper type that collects all Next (Done) values into a Vec.
///
/// This is a non-blocking collect operation. It passes through
/// Pending, Delayed, and Init states unchanged, and only yields
/// the collected Vec<Done> when the stream completes.
pub struct Collect<I>
where
    I: StreamIterator,
{
    inner: I,
    collected: Vec<I::D>,
    done: bool,
    _phantom: std::marker::PhantomData<I::P>,
}

impl<I> Iterator for Collect<I>
where
    I: StreamIterator,
    I::D: Clone + Send + 'static,
    I::P: Send + 'static,
{
    type Item = Stream<Vec<I::D>, I::P>;

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

impl<I> StreamIterator for Collect<I>
where
    I: StreamIterator,
    I::D: Clone + Send + 'static,
    I::P: Send + 'static,
{
    type D = Vec<I::D>;
    type P = I::P;
}

// ============================================================================
// Split Collector Combinators (Feature 07)
// ============================================================================

/// Observer branch from `split_collector()` for `StreamIterator`.
///
/// Receives copies of items matching the predicate via a `ConcurrentQueue`.
/// Yields `Stream::Next` for matched items, forwards Pending/Delayed from source.
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

impl<D, P> StreamIterator for SCollectorStreamIterator<D, P>
where
    D: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type D = D;
    type P = P;
}

/// Continuation branch from `split_collector()` for `StreamIterator`.
///
/// Wraps the original iterator, copying matched items to the observer queue
/// while continuing the chain for further combinators.
pub struct SSplitCollectorContinuation<I, D, P>
where
    I: StreamIterator<D = D, P = P>,
{
    /// The wrapped iterator
    inner: I,
    /// Queue to send copied items to observer
    queue: Arc<ConcurrentQueue<Stream<D, P>>>,
    /// Predicate to determine which items to copy
    predicate: Box<dyn Fn(&Stream<D, P>) -> bool + Send>,
}

impl<I, D, P> Iterator for SSplitCollectorContinuation<I, D, P>
where
    I: StreamIterator<D = D, P = P>,
    D: Clone,
    P: Clone,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = if let Some(item) = self.inner.next() {
            item
        } else {
            // Source iterator is naturally exhausted, close the queue
            self.queue.close();
            tracing::debug!("SSplitCollectorContinuation: source exhausted, queue closed");
            return None;
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

impl<I, D, P> StreamIterator for SSplitCollectorContinuation<I, D, P>
where
    I: StreamIterator<D = D, P = P>,
    D: Clone,
    P: Clone,
{
    type D = D;
    type P = P;
}

impl<I, D, P> Drop for SSplitCollectorContinuation<I, D, P>
where
    I: StreamIterator<D = D, P = P>,
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

/// Observer branch from `split_collect_until()` for `StreamIterator`.
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

impl<D, P> StreamIterator for SSplitUntilObserver<D, P>
where
    D: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type D = D;
    type P = P;
}

/// Continuation branch from `split_collect_until()` for `StreamIterator`.
///
/// Wraps the original iterator, copying items to the observer queue
/// based on the predicate's `CollectionState`. When predicate returns
/// `Close`, the queue is closed (observer completes).
pub struct SSplitUntilContinuation<I: StreamIterator<D = D, P = P>, D, P> {
    /// The wrapped iterator
    inner: I,
    /// Queue to send copied items to observer
    queue: Arc<ConcurrentQueue<Stream<D, P>>>,
    /// Predicate to determine when to close observer
    predicate: Box<dyn Fn(&Stream<D, P>) -> CollectionState + Send>,
}

impl<I, D, P> Iterator for SSplitUntilContinuation<I, D, P>
where
    I: StreamIterator<D = D, P = P>,
    D: Clone,
    P: Clone,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = if let Some(item) = self.inner.next() {
            item
        } else {
            // Source iterator is naturally exhausted, close the queue
            self.queue.close();
            tracing::debug!("SSplitUntilContinuation: source exhausted, queue closed");
            return None;
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

impl<I, D, P> StreamIterator for SSplitUntilContinuation<I, D, P>
where
    I: StreamIterator<D = D, P = P>,
    D: Clone,
    P: Clone,
{
    type D = D;
    type P = P;
}

impl<I, D, P> Drop for SSplitUntilContinuation<I, D, P>
where
    I: StreamIterator<D = D, P = P>,
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

/// Observer branch from `split_collector_map()` for `StreamIterator`.
///
/// Receives transformed copies of matched items via a `ConcurrentQueue`.
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

impl<DM, PM> StreamIterator for SSplitCollectorMapObserver<DM, PM>
where
    DM: Clone + Send + 'static,
    PM: Clone + Send + 'static,
{
    type D = DM;
    type P = PM;
}

/// Continuation branch from `split_collector_map()` for `StreamIterator`.
///
/// Wraps the original iterator. The transform function returns `(bool, Option<Stream<DM, PM>>)`
/// where DM and PM are independent Done and Pending types for the observer:
/// - `true` + `Some(stream)` sends the stream to the observer queue
/// - `false` or `None` skips sending to observer
/// The continuation continues with original Stream<D, P> values unchanged.
pub struct SSplitCollectorMapContinuation<I: StreamIterator<D = D, P = P>, D, P, DM, PM> {
    /// The wrapped iterator
    inner: I,
    /// Queue to send transformed items to observer
    queue: Arc<ConcurrentQueue<Stream<DM, PM>>>,
    /// Combined predicate + transform function
    transform: Box<dyn Fn(&Stream<D, P>) -> (bool, Option<Stream<DM, PM>>) + Send>,
}

impl<I, D, P, DM, PM> Iterator for SSplitCollectorMapContinuation<I, D, P, DM, PM>
where
    I: StreamIterator<D = D, P = P>,
    D: Clone,
    P: Clone,
    DM: Clone + Send + 'static,
    PM: Clone + Send + 'static,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = if let Some(item) = self.inner.next() {
            item
        } else {
            self.queue.close();
            tracing::debug!("SSplitCollectorMapContinuation: source exhausted, queue closed");
            return None;
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

impl<I, D, P, DM, PM> StreamIterator for SSplitCollectorMapContinuation<I, D, P, DM, PM>
where
    I: StreamIterator<D = D, P = P>,
    D: Clone,
    P: Clone,
    DM: Clone + Send + 'static,
    PM: Clone + Send + 'static,
{
    type D = D;
    type P = P;
}

impl<I, D, P, DM, PM> Drop for SSplitCollectorMapContinuation<I, D, P, DM, PM>
where
    I: StreamIterator<D = D, P = P>,
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

/// Extension trait for multi-source `StreamIterator` combinators.
///
/// These combinators work with multiple `StreamIterators` simultaneously,
/// aggregating their outputs or mapping them together.
pub trait MultiSourceStreamIteratorExt<D, P> {
    /// Collect all outputs from multiple `StreamIterators` into a single Vec.
    ///
    /// Polls all sources in round-robin fashion, collecting Next values.
    /// Yields `Stream::Pending` with count while any source is still producing.
    /// Yields `Stream::Next` with all collected values when all sources complete.
    fn collect_all<I>(iterators: Vec<I>) -> CollectAll<I, D, P>
    where
        I: StreamIterator<D = D, P = P> + Send + 'static,
        D: Clone + Send + 'static,
        P: Send + 'static;

    /// Map all values - only when all sources reach Done state.
    ///
    /// Buffers values from all sources until all have produced a Next value.
    /// Then applies the mapper to the collected Vec and yields the result.
    fn map_all_done<I, F, O>(iterators: Vec<I>, mapper: F) -> MapAllDone<I, F, D, P, O>
    where
        I: StreamIterator<D = D, P = P> + Send + 'static,
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
        I: StreamIterator<D = D, P = P> + Send + 'static,
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
        I: StreamIterator<D = D, P = P> + Send + 'static,
        D: Clone + Send + 'static,
        P: Send + 'static,
    {
        CollectAll::new(iterators)
    }

    fn map_all_done<I, F, O>(iterators: Vec<I>, mapper: F) -> MapAllDone<I, F, D, P, O>
    where
        I: StreamIterator<D = D, P = P> + Send + 'static,
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
        I: StreamIterator<D = D, P = P> + Send + 'static,
        F: Fn(Vec<Stream<D, P>>) -> O + Send + 'static,
        O: Send + 'static,
        D: Clone + Send + 'static,
        P: Send + 'static,
    {
        MapAllPendingAndDone::new(iterators, mapper)
    }
}

/// Multi-source collector that aggregates outputs from multiple `StreamIterators`.
///
/// Polls all sources in round-robin, collecting Next values.
/// Yields `Stream::Pending(count)` while gathering.
/// Yields `Stream::Next(Vec`<D>) when all sources complete.
pub struct CollectAll<I, D, P> {
    sources: Vec<I>,
    collected: Vec<D>,
    done: bool,
    _phantom: std::marker::PhantomData<P>,
}

impl<I, D, P> CollectAll<I, D, P>
where
    I: StreamIterator<D = D, P = P>,
    D: Clone + Send + 'static,
    P: Send + 'static,
{
    #[must_use]
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
    I: StreamIterator<D = D, P = P> + Send + 'static,
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

impl<I, D, P> StreamIterator for CollectAll<I, D, P>
where
    I: StreamIterator<D = D, P = P> + Send + 'static,
    D: Clone + Send + 'static,
    P: Send + 'static,
{
    type D = Vec<D>;
    type P = usize;
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
    I: StreamIterator<D = D, P = P>,
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
    I: StreamIterator<D = D, P = P> + Send + 'static,
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
        if self.buffer.iter().all(std::option::Option::is_some) {
            self.done = true;
            let values: Vec<D> = self.buffer.drain(..).flatten().collect();
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

impl<I, F, D, P, O> StreamIterator for MapAllDone<I, F, D, P, O>
where
    I: StreamIterator<D = D, P = P> + Send + 'static,
    F: Fn(Vec<D>) -> O + Send + 'static,
    O: Send + 'static,
    D: Clone + Send + 'static,
    P: Send + 'static,
{
    type D = O;
    type P = usize;
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
    I: StreamIterator<D = D, P = P>,
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
    I: StreamIterator<D = D, P = P> + Send + 'static,
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
            if let Some(state) = source.next() {
                states.push(state);
            } else {
                // Source exhausted
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

impl<I, F, D, P, O> StreamIterator for MapAllPendingAndDone<I, F, D, P, O>
where
    I: StreamIterator<D = D, P = P> + Send + 'static,
    F: Fn(Vec<Stream<D, P>>) -> O + Send + 'static,
    O: Send + 'static,
    D: Clone + Send + 'static,
    P: Send + 'static,
{
    type D = O;
    type P = P;
}

/// Wrapper type for `map_iter` combinator that flattens nested iterator patterns.
///
/// The outer iterator yields `Stream` items which are transformed by the mapper
/// into inner iterators. Each inner iterator is drained until `None` before polling
/// the outer for the next item.
pub struct MapIter<Outer, F, Inner>
where
    Outer: StreamIterator,
    F: Fn(Stream<Outer::D, Outer::P>) -> Inner,
    Inner: StreamIterator,
{
    outer: Outer,
    mapper: F,
    current_inner: Option<Inner>,
    _phantom: std::marker::PhantomData<(Outer, Inner)>,
}

impl<Outer, F, Inner> Iterator for MapIter<Outer, F, Inner>
where
    Outer: StreamIterator,
    F: Fn(Stream<Outer::D, Outer::P>) -> Inner,
    Inner: StreamIterator,
{
    type Item = Stream<Inner::D, Inner::P>;

    fn next(&mut self) -> Option<Self::Item> {
        // First drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(item);
            }
            self.current_inner = None;
        }

        // Poll outer for next item
        match self.outer.next() {
            Some(item) => {
                self.current_inner = Some((self.mapper)(item));
                // Return Ignore to signal "still working, no value yet"
                // The executor will call next() again to drain the new inner
                Some(Stream::Ignore)
            }
            None => None,
        }
    }
}

impl<Outer, F, Inner> StreamIterator for MapIter<Outer, F, Inner>
where
    Outer: StreamIterator,
    F: Fn(Stream<Outer::D, Outer::P>) -> Inner,
    Inner: StreamIterator,
{
    type D = Inner::D;
    type P = Inner::P;
}

// ============================================================================
// Flatten/FlatMap Combinators for StreamIterator
// ============================================================================

/// Wrapper struct that flattens Next (Done) values that implement IntoIterator.
///
/// Input:  StreamIterator<D = Vec<M>, P = P>
/// Output: StreamIterator<D = M, P = P>
pub struct SFlattenNext<I>
where
    I: StreamIterator,
    I::D: IntoIterator,
{
    inner: I,
    current_inner: Option<<I::D as IntoIterator>::IntoIter>,
    _phantom: std::marker::PhantomData<I::P>,
}

impl<I> Iterator for SFlattenNext<I>
where
    I: StreamIterator,
    I::D: IntoIterator,
    <I::D as IntoIterator>::Item: Send + 'static,
    I::P: Send + 'static,
{
    type Item = Stream<<I::D as IntoIterator>::Item, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        // First drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(Stream::Next(item));
            }
            self.current_inner = None;
        }

        // Poll outer for next item
        match self.inner.next() {
            Some(Stream::Next(iterable)) => {
                self.current_inner = Some(iterable.into_iter());
                // Return Ignore to signal "still working, setting up inner iterator"
                Some(Stream::Ignore)
            }
            Some(Stream::Pending(p)) => Some(Stream::Pending(p)),
            Some(Stream::Delayed(d)) => Some(Stream::Delayed(d)),
            Some(Stream::Init) => Some(Stream::Init),
            Some(Stream::Ignore) => Some(Stream::Ignore),
            None => None,
        }
    }
}

impl<I> StreamIterator for SFlattenNext<I>
where
    I: StreamIterator,
    I::D: IntoIterator,
    <I::D as IntoIterator>::Item: Send + 'static,
    I::P: Send + 'static,
{
    type D = <I::D as IntoIterator>::Item;
    type P = I::P;
}

/// Wrapper struct that flattens Pending values that implement IntoIterator.
///
/// Input:  StreamIterator<D = D, P = Vec<M>>
/// Output: StreamIterator<D = D, P = M>
pub struct SFlattenPending<I>
where
    I: StreamIterator,
    I::P: IntoIterator,
{
    inner: I,
    current_inner: Option<<I::P as IntoIterator>::IntoIter>,
    _phantom: std::marker::PhantomData<I::D>,
}

impl<I> Iterator for SFlattenPending<I>
where
    I: StreamIterator,
    I::P: IntoIterator,
    <I::P as IntoIterator>::Item: Send + 'static,
    I::D: Send + 'static,
{
    type Item = Stream<I::D, <I::P as IntoIterator>::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        // First drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(Stream::Pending(item));
            }
            self.current_inner = None;
        }

        // Poll outer for next item
        match self.inner.next() {
            Some(Stream::Pending(iterable)) => {
                self.current_inner = Some(iterable.into_iter());
                // Return Ignore to signal "still working, setting up inner iterator"
                Some(Stream::Ignore)
            }
            Some(Stream::Next(d)) => Some(Stream::Next(d)),
            Some(Stream::Delayed(d)) => Some(Stream::Delayed(d)),
            Some(Stream::Init) => Some(Stream::Init),
            Some(Stream::Ignore) => Some(Stream::Ignore),
            None => None,
        }
    }
}

impl<I> StreamIterator for SFlattenPending<I>
where
    I: StreamIterator,
    I::P: IntoIterator,
    <I::P as IntoIterator>::Item: Send + 'static,
    I::D: Send + 'static,
{
    type D = I::D;
    type P = <I::P as IntoIterator>::Item;
}

/// Wrapper struct that flat maps Next (Done) values - transform and flatten.
///
/// Input:  StreamIterator<D = D, P = P>
/// Output: StreamIterator<D = U::Item, P = P>
pub struct SFlatMapNext<I, F, U>
where
    I: StreamIterator,
    F: Fn(I::D) -> U,
    U: IntoIterator,
{
    inner: I,
    mapper: F,
    current_inner: Option<U::IntoIter>,
    _phantom: std::marker::PhantomData<(I::P, U)>,
}

impl<I, F, U> Iterator for SFlatMapNext<I, F, U>
where
    I: StreamIterator,
    F: Fn(I::D) -> U,
    U: IntoIterator,
    U::Item: Send + 'static,
    I::P: Send + 'static,
{
    type Item = Stream<U::Item, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        // First drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(Stream::Next(item));
            }
            self.current_inner = None;
        }

        // Poll outer for next item
        match self.inner.next() {
            Some(Stream::Next(d)) => {
                let iterable = (self.mapper)(d);
                self.current_inner = Some(iterable.into_iter());
                // Return Ignore to signal "still working, setting up inner iterator"
                Some(Stream::Ignore)
            }
            Some(Stream::Pending(p)) => Some(Stream::Pending(p)),
            Some(Stream::Delayed(d)) => Some(Stream::Delayed(d)),
            Some(Stream::Init) => Some(Stream::Init),
            Some(Stream::Ignore) => Some(Stream::Ignore),
            None => None,
        }
    }
}

impl<I, F, U> StreamIterator for SFlatMapNext<I, F, U>
where
    I: StreamIterator,
    F: Fn(I::D) -> U,
    U: IntoIterator,
    U::Item: Send + 'static,
    I::P: Send + 'static,
{
    type D = U::Item;
    type P = I::P;
}

/// Wrapper struct that flat maps Pending values - transform and flatten.
///
/// Input:  StreamIterator<D = D, P = P>
/// Output: StreamIterator<D = D, P = U::Item>
pub struct SFlatMapPending<I, F, U>
where
    I: StreamIterator,
    F: Fn(I::P) -> U,
    U: IntoIterator,
{
    inner: I,
    mapper: F,
    current_inner: Option<U::IntoIter>,
    _phantom: std::marker::PhantomData<(I::D, U)>,
}

impl<I, F, U> Iterator for SFlatMapPending<I, F, U>
where
    I: StreamIterator,
    F: Fn(I::P) -> U,
    U: IntoIterator,
    U::Item: Send + 'static,
    I::D: Send + 'static,
{
    type Item = Stream<I::D, U::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        // First drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(Stream::Pending(item));
            }
            self.current_inner = None;
        }

        // Poll outer for next item
        match self.inner.next() {
            Some(Stream::Pending(p)) => {
                let iterable = (self.mapper)(p);
                self.current_inner = Some(iterable.into_iter());
                // Return Ignore to signal "still working, setting up inner iterator"
                Some(Stream::Ignore)
            }
            Some(Stream::Next(d)) => Some(Stream::Next(d)),
            Some(Stream::Delayed(d)) => Some(Stream::Delayed(d)),
            Some(Stream::Init) => Some(Stream::Init),
            Some(Stream::Ignore) => Some(Stream::Ignore),
            None => None,
        }
    }
}

impl<I, F, U> StreamIterator for SFlatMapPending<I, F, U>
where
    I: StreamIterator,
    F: Fn(I::P) -> U,
    U: IntoIterator,
    U::Item: Send + 'static,
    I::D: Send + 'static,
{
    type D = I::D;
    type P = U::Item;
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

    impl StreamIterator for TestStream {
        type D = u32;
        type P = String;
    }

    // Simple stream iterator wrapper for Vec<Stream<D, P>>
    struct SimpleStream<D, P> {
        items: std::vec::IntoIter<Stream<D, P>>,
    }

    impl<D, P> SimpleStream<D, P> {
        fn from_vec(vec: Vec<D>) -> Self {
            let items = vec
                .into_iter()
                .map(Stream::Next)
                .collect::<Vec<_>>()
                .into_iter();
            Self { items }
        }
    }

    impl<D, P> Iterator for SimpleStream<D, P> {
        type Item = Stream<D, P>;

        fn next(&mut self) -> Option<Self::Item> {
            self.items.next()
        }
    }

    impl<D, P> StreamIterator for SimpleStream<D, P> {
        type D = D;
        type P = P;
    }

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
        let mut mapped = stream.map_pending(|s: String| s.len());

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

    #[test]
    fn test_map_iter_flattens_nested_iterators() {
        // Test using the extension method with a simple test stream
        struct VecStream {
            items: std::vec::IntoIter<Stream<Vec<u32>, String>>,
        }

        impl Iterator for VecStream {
            type Item = Stream<Vec<u32>, String>;

            fn next(&mut self) -> Option<Self::Item> {
                self.items.next()
            }
        }

        impl StreamIterator for VecStream {
            type D = Vec<u32>;
            type P = String;
        }

        let items = vec![
            Stream::Next(vec![1u32, 2u32]),
            Stream::Next(vec![3u32, 4u32, 5u32]),
        ];
        let stream = VecStream {
            items: items.into_iter(),
        };

        let mut flattened = stream.map_iter(|item: Stream<Vec<u32>, String>| {
            let vec = match item {
                Stream::Next(v) => v,
                _ => vec![],
            };
            SimpleStream::<u32, String>::from_vec(vec)
        });

        // First inner setup returns Ignore, then 1, 2
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Ignore)
        )); // Setting up first inner
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(1))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(2))
        ));
        // Second inner setup returns Ignore, then 3, 4, 5
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Ignore)
        )); // Setting up second inner
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(3))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(4))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(5))
        ));
        assert_eq!(Iterator::next(&mut flattened), None);
    }

    #[test]
    fn test_map_iter_passes_through_pending() {
        // Test using the extension method with a simple test stream
        struct VecStream {
            items: std::vec::IntoIter<Stream<Vec<u32>, String>>,
        }

        impl Iterator for VecStream {
            type Item = Stream<Vec<u32>, String>;

            fn next(&mut self) -> Option<Self::Item> {
                self.items.next()
            }
        }

        impl StreamIterator for VecStream {
            type D = Vec<u32>;
            type P = String;
        }

        let items = vec![
            Stream::Next(vec![1u32]),
            Stream::Pending("wait".to_string()),
            Stream::Next(vec![2u32, 3u32]),
        ];
        let stream = VecStream {
            items: items.into_iter(),
        };

        let mut flattened = stream.map_iter(|item: Stream<Vec<u32>, String>| {
            let vec = match item {
                Stream::Next(v) => v,
                // For non-Next items, return empty stream (they are consumed)
                _ => vec![],
            };
            SimpleStream::<u32, String>::from_vec(vec)
        });

        // Ignore when setting up first inner, then 1
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Ignore)
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(1))
        ));
        // Pending consumed by mapper (returns empty vec) = Ignore
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Ignore)
        ));
        // Empty inner exhausted, setup next inner from Next(vec![2,3]) = Ignore
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Ignore)
        ));
        // Then 2, 3 from second inner
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(2))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(3))
        ));
        assert_eq!(Iterator::next(&mut flattened), None);
    }

    #[test]
    fn test_map_iter_different_pending_types() {
        // Mapper receives full Stream and can decide what to do with non-Next states
        struct VecStream {
            items: std::vec::IntoIter<Stream<Vec<u32>, String>>,
        }

        impl Iterator for VecStream {
            type Item = Stream<Vec<u32>, String>;

            fn next(&mut self) -> Option<Self::Item> {
                self.items.next()
            }
        }

        impl StreamIterator for VecStream {
            type D = Vec<u32>;
            type P = String;
        }

        let items = vec![
            Stream::Next(vec![1u32, 2u32]),
            Stream::Pending("outer_pending".to_string()),
            Stream::Next(vec![3u32]),
        ];
        let stream = VecStream {
            items: items.into_iter(),
        };

        let mut flattened = stream.map_iter(|item: Stream<Vec<u32>, String>| {
            let vec = match item {
                Stream::Next(v) => v,
                _ => vec![],
            };
            SimpleStream::<u32, String>::from_vec(vec)
        });

        // Ignore for setup, then 1, 2 from first inner
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Ignore)
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(1))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(2))
        ));
        // Outer's Pending consumed by mapper (returns empty stream) - setup = Ignore
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Ignore)
        ));
        // Then setup next inner = Ignore, then 3
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Ignore)
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(3))
        ));
        assert_eq!(Iterator::next(&mut flattened), None);
    }

    #[test]
    fn test_flatten_next() {
        struct VecStream {
            items: std::vec::IntoIter<Stream<Vec<u32>, String>>,
        }

        impl Iterator for VecStream {
            type Item = Stream<Vec<u32>, String>;

            fn next(&mut self) -> Option<Self::Item> {
                self.items.next()
            }
        }

        impl StreamIterator for VecStream {
            type D = Vec<u32>;
            type P = String;
        }

        let items = vec![
            Stream::Next(vec![1u32, 2u32]),
            Stream::Pending("wait".to_string()),
            Stream::Next(vec![3u32, 4u32, 5u32]),
        ];
        let stream = VecStream {
            items: items.into_iter(),
        };

        let mut flattened = stream.flatten_next();

        // First inner yields 1, 2 (with Ignore when setting up)
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Ignore)
        )); // Setting up first inner
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(1))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(2))
        ));
        // Pending passes through
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Pending(_))
        ));
        // Setting up second inner
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Ignore)
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(3))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(4))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(5))
        ));
        assert_eq!(Iterator::next(&mut flattened), None);
    }

    #[test]
    fn test_flat_map_next() {
        struct NumStream {
            items: std::vec::IntoIter<Stream<u32, String>>,
        }

        impl Iterator for NumStream {
            type Item = Stream<u32, String>;

            fn next(&mut self) -> Option<Self::Item> {
                self.items.next()
            }
        }

        impl StreamIterator for NumStream {
            type D = u32;
            type P = String;
        }

        let items = vec![
            Stream::Next(2u32),
            Stream::Next(3u32),
            Stream::Pending("wait".to_string()),
        ];
        let stream = NumStream {
            items: items.into_iter(),
        };

        // Map each number to a range [0, n), then flatten
        let mut flattened = stream.flat_map_next(|n| 0..n);

        // 2 -> [0, 1]
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Ignore)
        )); // Setting up inner for 2
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(0))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(1))
        ));
        // 3 -> [0, 1, 2]
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Ignore)
        )); // Setting up inner for 3
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(0))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(1))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(2))
        ));
        // Pending passes through
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Pending(_))
        ));
        assert_eq!(Iterator::next(&mut flattened), None);
    }

    #[test]
    fn test_flatten_pending() {
        struct VecStream {
            items: std::vec::IntoIter<Stream<u32, Vec<String>>>,
        }

        impl Iterator for VecStream {
            type Item = Stream<u32, Vec<String>>;

            fn next(&mut self) -> Option<Self::Item> {
                self.items.next()
            }
        }

        impl StreamIterator for VecStream {
            type D = u32;
            type P = Vec<String>;
        }

        let items = vec![
            Stream::Pending(vec!["a".to_string(), "b".to_string()]),
            Stream::Next(42u32),
            Stream::Pending(vec!["c".to_string()]),
        ];
        let stream = VecStream {
            items: items.into_iter(),
        };

        let mut flattened = stream.flatten_pending();

        // First inner yields "a", "b"
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Ignore)
        )); // Setting up first inner
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Pending(ref s)) if s == "a"
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Pending(ref s)) if s == "b"
        ));
        // Next passes through
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(42))
        ));
        // Setting up second inner
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Ignore)
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Pending(ref s)) if s == "c"
        ));
        assert_eq!(Iterator::next(&mut flattened), None);
    }

    #[test]
    fn test_flat_map_pending() {
        struct NumStream {
            items: std::vec::IntoIter<Stream<u32, u32>>,
        }

        impl Iterator for NumStream {
            type Item = Stream<u32, u32>;

            fn next(&mut self) -> Option<Self::Item> {
                self.items.next()
            }
        }

        impl StreamIterator for NumStream {
            type D = u32;
            type P = u32;
        }

        let items = vec![
            Stream::Pending(2u32),
            Stream::Next(99u32),
            Stream::Pending(3u32),
        ];
        let stream = NumStream {
            items: items.into_iter(),
        };

        // Map each pending number to a range [0, n), then flatten
        let mut flattened = stream.flat_map_pending(|n| 0..n);

        // 2 -> [0, 1]
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Ignore)
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Pending(0))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Pending(1))
        ));
        // Next passes through
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Next(99))
        ));
        // 3 -> [0, 1, 2]
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Ignore)
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Pending(0))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Pending(1))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(Stream::Pending(2))
        ));
        assert_eq!(Iterator::next(&mut flattened), None);
    }
}
