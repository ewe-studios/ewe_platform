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
use std::sync::atomic::{AtomicBool, Ordering};
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
    fn map_done<F, R>(self, f: F) -> MapDone<Self, F, D, P>
    where
        F: Fn(D) -> R + Send + 'static,
        R: Send + 'static;

    /// Transform Pending values using the provided function.
    ///
    /// Init, Ignore, Delayed, and Next states pass through unchanged.
    fn map_pending<F, R>(self, f: F) -> MapPending<Self, F, P, D>
    where
        F: Fn(P) -> R + Send + 'static,
        R: Send + 'static;

    /// Transform both Pending and Next values with a single function.
    ///
    /// Returns a unified output type for both states.
    fn map_pending_and_done<F, R>(self, f: F) -> MapPendingAndDone<Self, F, D, P>
    where
        F: Fn(Stream<D, P>) -> R + Send + 'static,
        R: Send + 'static;

    /// Filter Next (Done) values using the provided predicate.
    ///
    /// Non-Next states pass through unchanged. Next values that don't
    /// satisfy the predicate are skipped.
    fn filter_done<F>(self, f: F) -> FilterDone<Self, F, D, P>
    where
        F: Fn(&D) -> bool + Send + 'static;

    /// Transform Delayed durations.
    fn map_delayed<F>(self, f: F) -> MapDelayed<Self, F, D, P>
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
        SSplitCollectorContinuation<Self, Pred, D, P>,
    )
    where
        Self: Sized,
        D: Clone,
        P: Clone,
        Pred: Fn(&Stream<D, P>) -> bool + Send + 'static;

    /// Convenience method: split_collector with queue_size = 1.
    ///
    /// Sends the first matching item to the observer, then continues.
    ///
    /// ## Example
    ///
    /// ```ignore
    /// let (observer, continuation) = stream
    ///     .split_collect_one(|item| matches!(item, Stream::Next(_)));
    /// ```
    fn split_collect_one<Pred>(
        self,
        predicate: Pred,
    ) -> (
        SCollectorStreamIterator<D, P>,
        SSplitCollectorContinuation<Self, Pred, D, P>,
    )
    where
        Self: Sized,
        D: Clone,
        P: Clone,
        Pred: Fn(&Stream<D, P>) -> bool + Send + 'static,
    {
        self.split_collector(predicate, 1)
    }
}

// Blanket implementation: anything implementing StreamIterator gets StreamIteratorExt
impl<S, D, P> StreamIteratorExt<D, P> for S
where
    S: StreamIterator<D, P> + Send + 'static,
    D: Send + 'static,
    P: Send + 'static,
{
    fn map_done<F, R>(self, f: F) -> MapDone<Self, F, D, P>
    where
        F: Fn(D) -> R + Send + 'static,
        R: Send + 'static,
    {
        MapDone {
            inner: self,
            mapper: f,
            _phantom: std::marker::PhantomData,
        }
    }

    fn map_pending<F, R>(self, f: F) -> MapPending<Self, F, P, D>
    where
        F: Fn(P) -> R + Send + 'static,
        R: Send + 'static,
    {
        MapPending {
            inner: self,
            mapper: f,
            _phantom: std::marker::PhantomData,
        }
    }

    fn map_pending_and_done<F, R>(self, f: F) -> MapPendingAndDone<Self, F, D, P>
    where
        F: Fn(Stream<D, P>) -> R + Send + 'static,
        R: Send + 'static,
    {
        MapPendingAndDone {
            inner: self,
            mapper: f,
            _phantom: std::marker::PhantomData,
        }
    }

    fn filter_done<F>(self, f: F) -> FilterDone<Self, F, D, P>
    where
        F: Fn(&D) -> bool + Send + 'static,
    {
        FilterDone {
            inner: self,
            predicate: f,
            _phantom: std::marker::PhantomData,
        }
    }

    fn map_delayed<F>(self, f: F) -> MapDelayed<Self, F, D, P>
    where
        F: Fn(std::time::Duration) -> std::time::Duration + Send + 'static,
    {
        MapDelayed {
            inner: self,
            mapper: f,
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
        SSplitCollectorContinuation<Self, Pred, D, P>,
    )
    where
        Self: Sized,
        D: Clone,
        P: Clone,
        Pred: Fn(&Stream<D, P>) -> bool + Send + 'static,
    {
        let queue = Arc::new(ConcurrentQueue::bounded(queue_size));
        let source_done = Arc::new(AtomicBool::new(false));

        let observer = SCollectorStreamIterator {
            queue: Arc::clone(&queue),
            source_done: Arc::clone(&source_done),
            _phantom: std::marker::PhantomData,
        };

        let continuation = SSplitCollectorContinuation {
            inner: self,
            queue,
            source_done,
            predicate,
            _phantom: std::marker::PhantomData,
        };

        (observer, continuation)
    }
}

/// Wrapper type that transforms Next (Done) values.
pub struct MapDone<I, F, D, P> {
    inner: I,
    mapper: F,
    _phantom: std::marker::PhantomData<(D, P)>,
}

impl<I, F, D, R, P> Iterator for MapDone<I, F, D, P>
where
    I: StreamIterator<D, P>,
    F: Fn(D) -> R + Send + 'static,
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

impl<I, F, D, R, P> StreamIterator<R, P> for MapDone<I, F, D, P>
where
    I: StreamIterator<D, P>,
    F: Fn(D) -> R + Send + 'static,
    R: Send + 'static,
    P: Send + 'static,
{
}

/// Wrapper type that transforms Pending values.
pub struct MapPending<I, F, P, D> {
    inner: I,
    mapper: F,
    _phantom: std::marker::PhantomData<(P, D)>,
}

impl<I, F, D, P, R> Iterator for MapPending<I, F, P, D>
where
    I: StreamIterator<D, P>,
    F: Fn(P) -> R + Send + 'static,
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

impl<I, F, D, P, R> StreamIterator<D, R> for MapPending<I, F, P, D>
where
    I: StreamIterator<D, P>,
    F: Fn(P) -> R + Send + 'static,
    R: Send + 'static,
    D: Send + 'static,
{
}

/// Wrapper type that transforms both Pending and Next values.
pub struct MapPendingAndDone<I, F, D, P> {
    inner: I,
    mapper: F,
    _phantom: std::marker::PhantomData<(D, P)>,
}

impl<I, F, D, P, R> Iterator for MapPendingAndDone<I, F, D, P>
where
    I: StreamIterator<D, P>,
    F: Fn(Stream<D, P>) -> R + Send + 'static,
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

impl<I, F, D, P, R> StreamIterator<R, R> for MapPendingAndDone<I, F, D, P>
where
    I: StreamIterator<D, P>,
    F: Fn(Stream<D, P>) -> R + Send + 'static,
    R: Send + 'static,
{
}

/// Wrapper type that filters Next (Done) values.
///
/// Filtered-out Next values are returned as `Stream::Ignore` to avoid blocking.
pub struct FilterDone<I, F, D, P> {
    inner: I,
    predicate: F,
    _phantom: std::marker::PhantomData<(D, P)>,
}

impl<I, F, D, P> Iterator for FilterDone<I, F, D, P>
where
    I: StreamIterator<D, P>,
    F: Fn(&D) -> bool + Send + 'static,
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

impl<I, F, D, P> StreamIterator<D, P> for FilterDone<I, F, D, P>
where
    I: StreamIterator<D, P>,
    F: Fn(&D) -> bool + Send + 'static,
    D: Send + 'static,
    P: Send + 'static,
{
}

/// Wrapper type that transforms Delayed durations.
pub struct MapDelayed<I, F, D, P> {
    inner: I,
    mapper: F,
    _phantom: std::marker::PhantomData<(D, P)>,
}

impl<I, F, D, P> Iterator for MapDelayed<I, F, D, P>
where
    I: StreamIterator<D, P>,
    F: Fn(std::time::Duration) -> std::time::Duration + Send + 'static,
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

impl<I, F, D, P> StreamIterator<D, P> for MapDelayed<I, F, D, P>
where
    I: StreamIterator<D, P>,
    F: Fn(std::time::Duration) -> std::time::Duration + Send + 'static,
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
    _phantom: std::marker::PhantomData<(D, P)>,
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
    /// Track if source is done
    source_done: Arc<AtomicBool>,
    _phantom: std::marker::PhantomData<(D, P)>,
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
            Ok(item) => Some(item),
            Err(concurrent_queue::PopError::Empty) => {
                // Check if source is done
                if self.source_done.load(Ordering::SeqCst) {
                    None // No more items
                } else {
                    // Still waiting - return Ignore
                    Some(Stream::Ignore)
                }
            }
            Err(concurrent_queue::PopError::Closed) => None,
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
pub struct SSplitCollectorContinuation<I, Pred, D, P>
where
    I: StreamIterator<D, P>,
{
    /// The wrapped iterator
    inner: I,
    /// Queue to send copied items to observer
    queue: Arc<ConcurrentQueue<Stream<D, P>>>,
    /// Flag to signal when source is done
    source_done: Arc<AtomicBool>,
    /// Predicate to determine which items to copy
    predicate: Pred,
    _phantom: std::marker::PhantomData<(D, P)>,
}

impl<I, Pred, D, P> Iterator for SSplitCollectorContinuation<I, Pred, D, P>
where
    I: StreamIterator<D, P>,
    D: Clone,
    P: Clone,
    Pred: Fn(&Stream<D, P>) -> bool,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.inner.next()?;

        // Copy matched items to observer queue
        if (self.predicate)(&item) {
            let _ = self.queue.force_push(item.clone());
        }

        // Always forward to continuation
        Some(item)
    }
}

impl<I, Pred, D, P> StreamIterator<D, P> for SSplitCollectorContinuation<I, Pred, D, P>
where
    I: StreamIterator<D, P>,
    D: Clone,
    P: Clone,
    Pred: Fn(&Stream<D, P>) -> bool,
{
}

impl<I, Pred, D, P> Drop for SSplitCollectorContinuation<I, Pred, D, P>
where
    I: StreamIterator<D, P>,
{
    fn drop(&mut self) {
        // Signal that the source is done
        self.source_done.store(true, Ordering::SeqCst);
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
    _phantom: std::marker::PhantomData<(D, P)>,
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
    _phantom: std::marker::PhantomData<(D, P, O)>,
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
