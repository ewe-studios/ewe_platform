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

#![allow(clippy::type_complexity)]

use crate::synca::mpp::{Stream, StreamIterator};
use crate::valtron::branches::CollectionState;
use std::sync::Arc;

use concurrent_queue::ConcurrentQueue;

/// Control enum for the `map_circuit` combinator.
///
/// Allows short-circuiting iterator chains with three possible outcomes:
/// - `Continue(item)` - Continue iteration with the transformed item
/// - `ReturnAndStop(item)` - Return this item and then stop iteration permanently
/// - `Stop` - Stop iteration without returning anything
///
/// This is useful for error handling patterns where you want to:
/// - Return an error value and immediately stop when an error is encountered
/// - Stop silently without returning anything in certain conditions
/// - Continue normal iteration otherwise
///
/// ## Example
///
/// ```ignore
/// let stream = execute(my_task)?
///     .map_circuit(|item| {
///         match item {
///             Stream::Next(err_result) if err_result.is_error() => {
///                 ShortCircuit::ReturnAndStop(Stream::Next(err_result))
///             }
///             Stream::Pending(_) | Stream::Next(_) => {
///                 ShortCircuit::Continue(item)
///             }
///             _ => ShortCircuit::Stop,
///         }
///     });
/// ```
pub enum ShortCircuit<D, P> {
    /// Continue iteration with the wrapped item
    Continue(Stream<D, P>),
    /// Return this item and then stop iteration permanently
    ReturnAndStop(Stream<D, P>),
    /// Stop iteration without returning anything
    Stop,
}

/// Extension trait providing combinator methods for any `StreamIterator`.
///
/// This trait is automatically implemented for any type that implements
/// `StreamIterator` with the appropriate bounds. This includes:
/// - `DrivenStreamIterator` from `drivers.rs`
/// - Any custom iterator implementing `StreamIterator`
///
/// ## Combinators
///
/// - `map_all_done()` - Transform Next (Done) values
/// - `map_all_pending()` - Transform Pending values
/// - `map_all_pending_and_done()` - Transform both
/// - `filter_all_done()` - Filter Next values
/// - `map_all_delayed()` - Transform Delayed durations
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
    /// the collected `Vec<Done>` when the stream completes.
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

    /// Short-circuit the iterator based on a circuit function.
    ///
    /// The circuit function receives each item and returns a `ShortCircuit` enum
    /// that determines whether to:
    /// - `Continue(item)` - Continue iteration with the transformed item
    /// - `ReturnAndStop(item)` - Return this item and then stop permanently
    /// - `Stop` - Stop iteration without returning anything
    ///
    /// This is useful for error handling patterns where you want to return
    /// an error value and immediately stop when an error is encountered.
    ///
    /// ## Example
    ///
    /// ```ignore
    /// let stream = execute(my_task)?
    ///     .map_circuit(|item| {
    ///         match item {
    ///             Stream::Next(err_result) if err_result.is_error() => {
    ///                 ShortCircuit::ReturnAndStop(Stream::Next(err_result))
    ///             }
    ///             Stream::Pending(_) | Stream::Next(_) => {
    ///                 ShortCircuit::Continue(item)
    ///             }
    ///             _ => ShortCircuit::Stop,
    ///         }
    ///     });
    /// ```
    fn map_circuit<F>(self, f: F) -> MapCircuit<Self>
    where
        Self: Sized,
        F: Fn(Stream<Self::D, Self::P>) -> ShortCircuit<Self::D, Self::P> + Send + 'static;

    /// Returns the first `Next` value from the iterator, short-circuiting after finding it.
    ///
    /// Uses `map_circuit` internally to stop iteration after the first `Next` value.
    /// Returns `None` if no `Next` value is found before the iterator is exhausted.
    fn first_next(self) -> Option<Self::D>
    where
        Self: Sized + Send + 'static,
        Self::D: Send + 'static,
        Self::P: Send + 'static,
    {
        self.map_circuit(|stream| match stream {
            Stream::Next(value) => ShortCircuit::ReturnAndStop(Stream::Next(value)),
            _ => ShortCircuit::Continue(stream),
        })
        .next()
        .and_then(|stream| match stream {
            Stream::Next(value) => Some(value),
            _ => None,
        })
    }

    /// Returns the first `Pending` value from the iterator, short-circuiting after finding it.
    ///
    /// Uses `map_circuit` internally to stop iteration after the first `Pending` value.
    /// Returns `None` if no `Pending` value is found before the iterator is exhausted.
    fn first_pending(self) -> Option<Self::P>
    where
        Self: Sized + Send + 'static,
        Self::D: Send + 'static,
        Self::P: Send + 'static,
    {
        self.map_circuit(|stream| match stream {
            Stream::Pending(value) => ShortCircuit::ReturnAndStop(Stream::Pending(value)),
            _ => ShortCircuit::Continue(stream),
        })
        .next()
        .and_then(|stream| match stream {
            Stream::Pending(value) => Some(value),
            _ => None,
        })
    }

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

    /// Map only Next (Done) values to a new StreamIterator, passing through
    /// other states as Stream::Ignore.
    ///
    /// The returned `MapIter` drains each inner iterator until `None`, then polls
    /// the outer for the next item.
    fn map_iter_done<F, InnerIter, InnerD>(
        self,
        mapper: F,
    ) -> MapIterDone<Self, F, InnerIter, InnerD>
    where
        Self: Sized,
        F: Fn(Self::D) -> InnerIter + Send + 'static,
        InnerIter: StreamIterator<D = InnerD, P = Self::P> + Send + 'static,
    {
        MapIterDone {
            outer: self,
            mapper,
            current_inner: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Map only Pending values to a new StreamIterator, passing through
    /// other states as Stream::Ignore.
    ///
    /// The returned `MapIter` drains each inner iterator until `None`, then polls
    /// the outer for the next item.
    fn map_iter_pending<F, InnerIter, InnerP>(
        self,
        mapper: F,
    ) -> MapIterPending<Self, F, InnerIter, InnerP>
    where
        Self: Sized,
        F: Fn(Self::P) -> InnerIter + Send + 'static,
        InnerIter: StreamIterator<D = Self::D, P = InnerP> + Send + 'static,
    {
        MapIterPending {
            outer: self,
            mapper,
            current_inner: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Flatten Next (Done) values that implement `IntoIterator`.
    ///
    /// Input:  `StreamIterator`<D = `Vec<M>`, P = P>
    /// Output: `StreamIterator`<D = M, P = P>
    ///
    /// The user's Done type implements `IntoIterator`. We store the inner iterator
    /// and drain it over multiple `next()` calls. When exhausted (None), poll outer again.
    ///
    /// Returns `Stream::Ignore` when waiting for the inner iterator to produce more values.
    fn flatten_next(self) -> SFlattenNext<Self>
    where
        Self: Sized,
        Self::D: IntoIterator,
        <Self::D as IntoIterator>::Item: Send + 'static;

    /// Flatten Pending values that implement `IntoIterator`.
    ///
    /// Input:  `StreamIterator`<D = D, P = `Vec<M>`>
    /// Output: `StreamIterator`<D = D, P = M>
    ///
    /// The user's Pending type implements `IntoIterator`. We store the inner iterator
    /// and drain it over multiple `next()` calls. When exhausted (None), poll outer again.
    fn flatten_pending(self) -> SFlattenPending<Self>
    where
        Self: Sized,
        Self::P: IntoIterator,
        <Self::P as IntoIterator>::Item: Send + 'static;

    /// Flat map Next (Done) values - transform and flatten in one operation.
    ///
    /// Input:  `StreamIterator`<D = D, P = P>
    /// Output: `StreamIterator`<D = `U::Item`, P = P>
    ///
    /// The mapper function transforms each Done value into an `IntoIterator`,
    /// which is then flattened. Inner iterator is drained over multiple `next()` calls.
    fn flat_map_next<F, U>(self, f: F) -> SFlatMapNext<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::D) -> U + Send + 'static,
        U: IntoIterator,
        U::Item: Send + 'static;

    /// Flat map Pending values - transform and flatten in one operation.
    ///
    /// Input:  `StreamIterator`<D = D, P = P>
    /// Output: `StreamIterator`<D = D, P = `U::Item`>
    ///
    /// The mapper function transforms each Pending value into an `IntoIterator`,
    /// which is then flattened. Inner iterator is drained over multiple `next()` calls.
    fn flat_map_pending<F, U>(self, f: F) -> SFlatMapPending<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::P) -> U + Send + 'static,
        U: IntoIterator,
        U::Item: Send + 'static;

    // ===== Feature 08: Iterator Extension Completion =====

    /// Transform any Stream state with full state access.
    fn map_state<F, R>(self, f: F) -> SMapState<Self, F, R>
    where
        F: Fn(Stream<Self::D, Self::P>) -> Stream<R, Self::P> + Send + 'static,
        R: Send + 'static;

    /// Side-effect on any Stream state.
    fn inspect_state<F>(self, f: F) -> SInspectState<Self, F>
    where
        F: Fn(&Stream<Self::D, Self::P>) + Send + 'static;

    /// Filter based on full Stream state. Non-matching items return `Stream::Ignore`.
    fn filter_state<F>(self, f: F) -> SFilterState<Self, F>
    where
        F: Fn(&Stream<Self::D, Self::P>) -> bool + Send + 'static;

    /// Take items while state predicate returns true.
    fn take_while_state<F>(self, predicate: F) -> STakeWhileState<Self, F>
    where
        F: Fn(&Stream<Self::D, Self::P>) -> bool + Send + 'static;

    /// Skip items while state predicate returns true.
    fn skip_while_state<F>(self, predicate: F) -> SSkipWhileState<Self, F>
    where
        F: Fn(&Stream<Self::D, Self::P>) -> bool + Send + 'static;

    /// Take at most n items matching state predicate.
    fn take_state<F>(self, n: usize, state_predicate: F) -> STakeState<Self, F>
    where
        F: Fn(&Stream<Self::D, Self::P>) -> bool + Send + 'static;

    /// Skip first n items matching state predicate.
    fn skip_state<F>(self, n: usize, state_predicate: F) -> SSkipState<Self, F>
    where
        F: Fn(&Stream<Self::D, Self::P>) -> bool + Send + 'static;

    /// Take at most n Next items.
    fn take(self, n: usize) -> STakeState<Self, impl Fn(&Stream<Self::D, Self::P>) -> bool>
    where
        Self::D: Send + 'static,
    {
        self.take_state(n, |s| matches!(s, Stream::Next(_)))
    }

    /// Take at most n items of any state.
    fn take_all(self, n: usize) -> STakeState<Self, impl Fn(&Stream<Self::D, Self::P>) -> bool> {
        self.take_state(n, |_| true)
    }

    /// Skip first n Next items, return all others unchanged.
    fn skip(self, n: usize) -> SSkipState<Self, impl Fn(&Stream<Self::D, Self::P>) -> bool>
    where
        Self::D: Send + 'static,
    {
        self.skip_state(n, |s| matches!(s, Stream::Next(_)))
    }

    /// Skip first n items of any state.
    fn skip_all(self, n: usize) -> SSkipState<Self, impl Fn(&Stream<Self::D, Self::P>) -> bool> {
        self.skip_state(n, |_| true)
    }

    /// Take while predicate true on Next values, pass through non-Next.
    fn take_while<F>(
        self,
        f: F,
    ) -> STakeWhileState<Self, impl Fn(&Stream<Self::D, Self::P>) -> bool>
    where
        F: Fn(&Self::D) -> bool + Send + 'static,
    {
        self.take_while_state(move |s| match s {
            Stream::Next(v) => f(v),
            _ => true,
        })
    }

    /// Take while predicate true on ANY state.
    fn take_while_any<F>(self, f: F) -> STakeWhileState<Self, F>
    where
        F: Fn(&Stream<Self::D, Self::P>) -> bool + Send + 'static,
    {
        self.take_while_state(f)
    }

    /// Skip while predicate true on Next values, return all others.
    fn skip_while<F>(
        self,
        f: F,
    ) -> SSkipWhileState<Self, impl Fn(&Stream<Self::D, Self::P>) -> bool>
    where
        F: Fn(&Self::D) -> bool + Send + 'static,
    {
        self.skip_while_state(move |s| match s {
            Stream::Next(v) => f(v),
            _ => false,
        })
    }

    /// Skip while predicate true on ANY state.
    fn skip_while_any<F>(self, f: F) -> SSkipWhileState<Self, F>
    where
        F: Fn(&Stream<Self::D, Self::P>) -> bool + Send + 'static,
    {
        self.skip_while_state(f)
    }

    /// Add index to Next items, changing Done type from D to (usize, D).
    fn enumerate(self) -> SEnumerate<Self>;

    /// Find first item matching predicate.
    fn find<F>(self, predicate: F) -> SFind<Self, F>
    where
        F: Fn(&Self::D) -> bool + Send + 'static;

    /// Find first item mapping to Some value.
    fn find_map<F, R>(self, f: F) -> SFindMap<Self, F, R>
    where
        F: Fn(Self::D) -> Option<R> + Send + 'static,
        R: Send + 'static;

    /// Fold/accumulate values. Returns final accumulator when done.
    fn fold<F, R>(self, init: R, f: F) -> SFold<Self, F, R>
    where
        F: Fn(R, Self::D) -> R + Send + 'static,
        R: Send + 'static;

    /// Check if all Next items satisfy predicate.
    fn all<F>(self, f: F) -> SAll<Self, F>
    where
        F: Fn(Self::D) -> bool + Send + 'static;

    /// Check if any Next item satisfies predicate.
    fn any<F>(self, f: F) -> SAny<Self, F>
    where
        F: Fn(Self::D) -> bool + Send + 'static;

    /// Count Next items.
    fn count(self) -> SCount<Self>;

    /// Count all items (any state).
    fn count_all(self) -> SCountAll<Self>;
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

    fn map_circuit<F>(self, f: F) -> MapCircuit<Self>
    where
        Self: Sized,
        F: Fn(Stream<Self::D, Self::P>) -> ShortCircuit<Self::D, Self::P> + Send + 'static,
    {
        MapCircuit {
            inner: self,
            circuit: Box::new(f),
            stopped: false,
            _phantom: std::marker::PhantomData,
        }
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

    // ===== Feature 08 implementations =====

    fn map_state<F, R>(self, f: F) -> SMapState<Self, F, R>
    where
        F: Fn(Stream<Self::D, Self::P>) -> Stream<R, Self::P> + Send + 'static,
        R: Send + 'static,
    {
        SMapState {
            inner: self,
            mapper: f,
            _phantom: std::marker::PhantomData,
        }
    }

    fn inspect_state<F>(self, f: F) -> SInspectState<Self, F>
    where
        F: Fn(&Stream<Self::D, Self::P>) + Send + 'static,
    {
        SInspectState {
            inner: self,
            inspector: f,
        }
    }

    fn filter_state<F>(self, f: F) -> SFilterState<Self, F>
    where
        F: Fn(&Stream<Self::D, Self::P>) -> bool + Send + 'static,
    {
        SFilterState {
            inner: self,
            predicate: f,
        }
    }

    fn take_while_state<F>(self, predicate: F) -> STakeWhileState<Self, F>
    where
        F: Fn(&Stream<Self::D, Self::P>) -> bool + Send + 'static,
    {
        STakeWhileState {
            inner: self,
            predicate,
            done: false,
        }
    }

    fn skip_while_state<F>(self, predicate: F) -> SSkipWhileState<Self, F>
    where
        F: Fn(&Stream<Self::D, Self::P>) -> bool + Send + 'static,
    {
        SSkipWhileState {
            inner: self,
            predicate,
            done_skipping: false,
        }
    }

    fn take_state<F>(self, n: usize, state_predicate: F) -> STakeState<Self, F>
    where
        F: Fn(&Stream<Self::D, Self::P>) -> bool + Send + 'static,
    {
        STakeState {
            inner: self,
            remaining: n,
            state_predicate,
        }
    }

    fn skip_state<F>(self, n: usize, state_predicate: F) -> SSkipState<Self, F>
    where
        F: Fn(&Stream<Self::D, Self::P>) -> bool + Send + 'static,
    {
        SSkipState {
            inner: self,
            to_skip: n,
            state_predicate,
        }
    }

    fn enumerate(self) -> SEnumerate<Self> {
        SEnumerate {
            inner: self,
            count: 0,
        }
    }

    fn find<F>(self, predicate: F) -> SFind<Self, F>
    where
        F: Fn(&Self::D) -> bool + Send + 'static,
    {
        SFind {
            inner: self,
            predicate,
            found: false,
        }
    }

    fn find_map<F, R>(self, f: F) -> SFindMap<Self, F, R>
    where
        F: Fn(Self::D) -> Option<R> + Send + 'static,
        R: Send + 'static,
    {
        SFindMap {
            inner: self,
            mapper: f,
            found: false,
            _phantom: std::marker::PhantomData,
        }
    }

    fn fold<F, R>(self, init: R, f: F) -> SFold<Self, F, R>
    where
        F: Fn(R, Self::D) -> R + Send + 'static,
        R: Send + 'static,
    {
        SFold {
            inner: self,
            acc: Some(init),
            folder: f,
            _phantom: std::marker::PhantomData,
        }
    }

    fn all<F>(self, f: F) -> SAll<Self, F>
    where
        F: Fn(Self::D) -> bool + Send + 'static,
    {
        SAll {
            inner: self,
            predicate: f,
            all_true: true,
            done: false,
        }
    }

    fn any<F>(self, f: F) -> SAny<Self, F>
    where
        F: Fn(Self::D) -> bool + Send + 'static,
    {
        SAny {
            inner: self,
            predicate: f,
            any_true: false,
            done: false,
        }
    }

    fn count(self) -> SCount<Self> {
        SCount {
            inner: self,
            count: 0,
        }
    }

    fn count_all(self) -> SCountAll<Self> {
        SCountAll {
            inner: self,
            count: 0,
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

// impl<I, R> StreamIterator for MapDone<I, R>
// where
//     I: StreamIterator,
//     R: Send + 'static,
// {
//     type D = R;
//     type P = I::P;
// }

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

// impl<I, R> StreamIterator for MapPending<I, R>
// where
//     I: StreamIterator,
//     R: Send + 'static,
// {
//     type D = I::D;
//     type P = R;
// }

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

// impl<I, R> StreamIterator for MapPendingAndDone<I, R>
// where
//     I: StreamIterator,
//     R: Send + 'static,
// {
//     type D = R;
//     type P = R;
// }

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

// impl<I> StreamIterator for FilterDone<I>
// where
//     I: StreamIterator,
// {
//     type D = I::D;
//     type P = I::P;
// }

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

/// Wrapper type that collects all Next (Done) values into a Vec.
///
/// This is a non-blocking collect operation. It passes through
/// Pending, Delayed, and Init states unchanged, and only yields
/// the collected `Vec<Done>` when the stream completes.
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

/// Wrapper type that applies a circuit function to each item.
///
/// The circuit function determines whether to continue iteration,
/// return a value and stop, or just stop.
pub struct MapCircuit<I>
where
    I: StreamIterator,
{
    inner: I,
    circuit: Box<dyn Fn(Stream<I::D, I::P>) -> ShortCircuit<I::D, I::P> + Send>,
    stopped: bool,
    _phantom: std::marker::PhantomData<I::P>,
}

impl<I> Iterator for MapCircuit<I>
where
    I: StreamIterator,
    I::D: Send + 'static,
    I::P: Send + 'static,
{
    type Item = Stream<I::D, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        // If we've stopped, return None permanently
        if self.stopped {
            return None;
        }

        // Get the next item from the inner iterator
        let stream = self.inner.next()?;

        // Apply the circuit function
        match (self.circuit)(stream) {
            ShortCircuit::Continue(item) => Some(item),
            ShortCircuit::ReturnAndStop(item) => {
                // Mark as stopped so future calls return None
                self.stopped = true;
                Some(item)
            }
            ShortCircuit::Stop => {
                // Mark as stopped and return None
                self.stopped = true;
                None
            }
        }
    }
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

/// Continuation branch from `split_collector_map()` for `StreamIterator`.
///
/// Wraps the original iterator. The transform function returns `(bool, Option<Stream<DM, PM>>)`
/// where DM and PM are independent Done and Pending types for the observer:
/// - `true` + `Some(stream)` sends the stream to the observer queue
/// - `false` or `None` skips sending to observer
///
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
/// Yields `Stream::Next(Vec<D>)` when all sources complete.
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

/// Wrapper type for `map_iter` combinator that flattens nested iterator patterns.
///
/// Focused on mapping the Pending value.
///
/// The outer iterator yields `Stream` items which are transformed by the mapper
/// into inner iterators. Each inner iterator is drained until `None` before polling
/// the outer for the next item.
pub struct MapIterPending<Outer, F, Inner, InnerP>
where
    F: Fn(Outer::P) -> Inner,
    Outer: StreamIterator,
    Inner: StreamIterator<D = Outer::D, P = InnerP>,
{
    outer: Outer,
    mapper: F,
    current_inner: Option<Inner>,
    _phantom: std::marker::PhantomData<(Outer, Inner, InnerP)>,
}

impl<Outer, F, Inner, InnerP> Iterator for MapIterPending<Outer, F, Inner, InnerP>
where
    F: Fn(Outer::P) -> Inner,
    Outer: StreamIterator,
    Inner: StreamIterator<D = Outer::D, P = InnerP>,
{
    type Item = Stream<Outer::D, Inner::P>;

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
            Some(item) => match item {
                Stream::Pending(inner) => {
                    self.current_inner = Some((self.mapper)(inner));
                    Some(Stream::Ignore)
                }
                Stream::Next(inner) => Some(Stream::Next(inner)),
                Stream::Delayed(inner) => Some(Stream::Delayed(inner)),
                Stream::Init => Some(Stream::Init),
                Stream::Ignore => Some(Stream::Ignore),
            },
            None => None,
        }
    }
}

/// Wrapper type for `map_iter` combinator that flattens nested iterator patterns.
///
/// The outer iterator yields `Stream` items which are transformed by the mapper
/// into inner iterators. Each inner iterator is drained until `None` before polling
/// the outer for the next item.
pub struct MapIterDone<Outer, F, Inner, InnerD>
where
    F: Fn(Outer::D) -> Inner,
    Outer: StreamIterator,
    Inner: StreamIterator<D = InnerD, P = Outer::P>,
{
    outer: Outer,
    mapper: F,
    current_inner: Option<Inner>,
    _phantom: std::marker::PhantomData<(Outer, Inner, InnerD)>,
}

impl<Outer, F, Inner, InnerD> Iterator for MapIterDone<Outer, F, Inner, InnerD>
where
    F: Fn(Outer::D) -> Inner,
    Outer: StreamIterator,
    Inner: StreamIterator<D = InnerD, P = Outer::P>,
{
    type Item = Stream<Inner::D, Outer::P>;

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
            Some(item) => match item {
                Stream::Next(inner) => {
                    self.current_inner = Some((self.mapper)(inner));
                    Some(Stream::Ignore)
                }
                Stream::Pending(inner) => Some(Stream::Pending(inner)),
                Stream::Delayed(inner) => Some(Stream::Delayed(inner)),
                Stream::Init => Some(Stream::Init),
                Stream::Ignore => Some(Stream::Ignore),
            },
            None => None,
        }
    }
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

// ============================================================================
// Flatten/FlatMap Combinators for StreamIterator
// ============================================================================

/// Wrapper struct that flattens Next (Done) values that implement `IntoIterator`.
///
/// Input:  `StreamIterator`<D = `Vec<M>`, P = P>
/// Output: `StreamIterator`<D = M, P = P>
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

/// Wrapper struct that flattens Pending values that implement `IntoIterator`.
///
/// Input:  `StreamIterator`<D = D, P = `Vec<M>`>
/// Output: `StreamIterator`<D = D, P = M>
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

/// Wrapper struct that flat maps Next (Done) values - transform and flatten.
///
/// Input:  `StreamIterator`<D = D, P = P>
/// Output: `StreamIterator`<D = `U::Item`, P = P>
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

/// Wrapper struct that flat maps Pending values - transform and flatten.
///
/// Input:  `StreamIterator`<D = D, P = P>
/// Output: `StreamIterator`<D = D, P = `U::Item`>
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

// ===== Feature 08: Wrapper Structs =====

/// Wrapper for `map_state()` - transforms any Stream state
pub struct SMapState<I, F, R> {
    inner: I,
    mapper: F,
    _phantom: std::marker::PhantomData<R>,
}

impl<I, F, R> Iterator for SMapState<I, F, R>
where
    I: StreamIterator,
    F: Fn(Stream<I::D, I::P>) -> Stream<R, I::P> + Send + 'static,
    R: Send + 'static,
{
    type Item = Stream<R, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(&self.mapper)
    }
}

/// Wrapper for `inspect_state()` - side-effect on any Stream state
pub struct SInspectState<I, F> {
    inner: I,
    inspector: F,
}

impl<I, F> Iterator for SInspectState<I, F>
where
    I: StreamIterator,
    F: Fn(&Stream<I::D, I::P>) + Send + 'static,
{
    type Item = Stream<I::D, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().inspect(|item| (self.inspector)(item))
    }
}

/// Wrapper for `filter_state()` - filter based on full Stream state
pub struct SFilterState<I, F> {
    inner: I,
    predicate: F,
}

impl<I, F> Iterator for SFilterState<I, F>
where
    I: StreamIterator,
    F: Fn(&Stream<I::D, I::P>) -> bool + Send + 'static,
{
    type Item = Stream<I::D, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.inner.next()?;
        if (self.predicate)(&item) {
            Some(item)
        } else {
            // Non-matching items return Ignore
            Some(Stream::Ignore)
        }
    }
}

/// Wrapper for `take_while_state()` - take while state predicate true
pub struct STakeWhileState<I, F> {
    inner: I,
    predicate: F,
    done: bool,
}

impl<I, F> Iterator for STakeWhileState<I, F>
where
    I: StreamIterator,
    F: Fn(&Stream<I::D, I::P>) -> bool + Send + 'static,
{
    type Item = Stream<I::D, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let item = self.inner.next()?;
        if (self.predicate)(&item) {
            Some(item)
        } else {
            self.done = true;
            None
        }
    }
}

/// Wrapper for `skip_while_state()` - skip while state predicate true
pub struct SSkipWhileState<I, F> {
    inner: I,
    predicate: F,
    done_skipping: bool,
}

impl<I, F> Iterator for SSkipWhileState<I, F>
where
    I: StreamIterator,
    F: Fn(&Stream<I::D, I::P>) -> bool + Send + 'static,
{
    type Item = Stream<I::D, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let item = self.inner.next()?;
            if self.done_skipping {
                return Some(item);
            }
            if !(self.predicate)(&item) {
                self.done_skipping = true;
                return Some(item);
            }
            // Still skipping, continue loop
        }
    }
}

/// Wrapper for `take_state()` - take at most n items matching state predicate
pub struct STakeState<I, F> {
    inner: I,
    remaining: usize,
    state_predicate: F,
}

impl<I, F> Iterator for STakeState<I, F>
where
    I: StreamIterator,
    F: Fn(&Stream<I::D, I::P>) -> bool + Send + 'static,
{
    type Item = Stream<I::D, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let item = self.inner.next()?;
        if (self.state_predicate)(&item) {
            self.remaining -= 1;
            Some(item)
        } else {
            Some(item)
        }
    }
}

/// Wrapper for `skip_state()` - skip first n items matching state predicate
pub struct SSkipState<I, F> {
    inner: I,
    to_skip: usize,
    state_predicate: F,
}

impl<I, F> Iterator for SSkipState<I, F>
where
    I: StreamIterator,
    F: Fn(&Stream<I::D, I::P>) -> bool + Send + 'static,
{
    type Item = Stream<I::D, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let item = self.inner.next()?;
            if self.to_skip > 0 && (self.state_predicate)(&item) {
                self.to_skip -= 1;
                continue;
            }
            return Some(item);
        }
    }
}

/// Wrapper for `enumerate()` - adds index to Next items
pub struct SEnumerate<I> {
    inner: I,
    count: usize,
}

impl<I> Iterator for SEnumerate<I>
where
    I: StreamIterator,
{
    type Item = Stream<(usize, I::D), I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|item| match item {
            Stream::Next(d) => Stream::Next((self.count, d)),
            Stream::Pending(p) => Stream::Pending(p),
            Stream::Delayed(d) => Stream::Delayed(d),
            Stream::Init => Stream::Init,
            Stream::Ignore => Stream::Ignore,
        })
    }
}

/// Wrapper for `find()` - find first item matching predicate
pub struct SFind<I, F> {
    inner: I,
    predicate: F,
    found: bool,
}

impl<I, F> Iterator for SFind<I, F>
where
    I: StreamIterator,
    F: Fn(&I::D) -> bool + Send + 'static,
{
    type Item = Stream<Option<I::D>, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.found {
            return None;
        }
        loop {
            match self.inner.next()? {
                Stream::Next(d) => {
                    if (self.predicate)(&d) {
                        self.found = true;
                        return Some(Stream::Next(Some(d)));
                    }
                    // Continue searching
                }
                Stream::Pending(p) => return Some(Stream::Pending(p)),
                Stream::Delayed(d) => return Some(Stream::Delayed(d)),
                Stream::Init => return Some(Stream::Init),
                Stream::Ignore => return Some(Stream::Ignore),
            }
        }
    }
}

/// Wrapper for `find_map()` - find first item mapping to Some
pub struct SFindMap<I, F, R> {
    inner: I,
    mapper: F,
    found: bool,
    _phantom: std::marker::PhantomData<R>,
}

impl<I, F, R> Iterator for SFindMap<I, F, R>
where
    I: StreamIterator,
    F: Fn(I::D) -> Option<R> + Send + 'static,
    R: Send + 'static,
{
    type Item = Stream<Option<R>, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.found {
            return None;
        }
        loop {
            match self.inner.next()? {
                Stream::Next(d) => {
                    if let Some(r) = (self.mapper)(d) {
                        self.found = true;
                        return Some(Stream::Next(Some(r)));
                    }
                    // Continue searching
                }
                Stream::Pending(p) => return Some(Stream::Pending(p)),
                Stream::Delayed(d) => return Some(Stream::Delayed(d)),
                Stream::Init => return Some(Stream::Init),
                Stream::Ignore => return Some(Stream::Ignore),
            }
        }
    }
}

/// Wrapper for `fold()` - accumulate values
pub struct SFold<I, F, R> {
    inner: I,
    acc: Option<R>,
    folder: F,
    _phantom: std::marker::PhantomData<(I, R)>,
}

impl<I, F, R> Iterator for SFold<I, F, R>
where
    I: StreamIterator,
    F: Fn(R, I::D) -> R + Send + 'static,
    R: Send + 'static,
{
    type Item = Stream<R, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next()? {
                Stream::Next(v) => {
                    if let Some(acc) = self.acc.take() {
                        self.acc = Some((self.folder)(acc, v));
                    }
                }
                Stream::Pending(p) => return Some(Stream::Pending(p)),
                Stream::Delayed(d) => return Some(Stream::Delayed(d)),
                Stream::Init => return Some(Stream::Init),
                Stream::Ignore => {}
            }
        }
    }
}

impl<I, F, R> Drop for SFold<I, F, R> {
    fn drop(&mut self) {
        // Iterator was not fully consumed
    }
}

/// Wrapper for `all()` - check if all Next items satisfy predicate
pub struct SAll<I, F> {
    inner: I,
    predicate: F,
    all_true: bool,
    done: bool,
}

impl<I, F> Iterator for SAll<I, F>
where
    I: StreamIterator,
    F: Fn(I::D) -> bool + Send + 'static,
{
    type Item = Stream<bool, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        loop {
            match self.inner.next()? {
                Stream::Next(v) => {
                    if !(self.predicate)(v) {
                        self.all_true = false;
                        self.done = true;
                        return Some(Stream::Next(false));
                    }
                }
                Stream::Pending(p) => return Some(Stream::Pending(p)),
                Stream::Delayed(d) => return Some(Stream::Delayed(d)),
                Stream::Init => return Some(Stream::Init),
                Stream::Ignore => {}
            }
        }
    }
}

/// Wrapper for `any()` - check if any Next item satisfies predicate
pub struct SAny<I, F> {
    inner: I,
    predicate: F,
    any_true: bool,
    done: bool,
}

impl<I, F> Iterator for SAny<I, F>
where
    I: StreamIterator,
    F: Fn(I::D) -> bool + Send + 'static,
{
    type Item = Stream<bool, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        loop {
            match self.inner.next()? {
                Stream::Next(v) => {
                    if (self.predicate)(v) {
                        self.any_true = true;
                        self.done = true;
                        return Some(Stream::Next(true));
                    }
                }
                Stream::Pending(p) => return Some(Stream::Pending(p)),
                Stream::Delayed(d) => return Some(Stream::Delayed(d)),
                Stream::Init => return Some(Stream::Init),
                Stream::Ignore => {}
            }
        }
    }
}

/// Wrapper for `count()` - count Next items
pub struct SCount<I> {
    inner: I,
    count: usize,
}

impl<I> Iterator for SCount<I>
where
    I: StreamIterator,
{
    type Item = Stream<usize, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next()? {
                Stream::Next(_) => {
                    self.count += 1;
                }
                Stream::Pending(p) => return Some(Stream::Pending(p)),
                Stream::Delayed(d) => return Some(Stream::Delayed(d)),
                Stream::Init => return Some(Stream::Init),
                Stream::Ignore => {}
            }
        }
    }
}

/// Wrapper for `count_all()` - count all items
pub struct SCountAll<I> {
    inner: I,
    count: usize,
}

impl<I> Iterator for SCountAll<I>
where
    I: StreamIterator,
{
    type Item = Stream<usize, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next()? {
                Stream::Next(_) | Stream::Pending(_) | Stream::Delayed(_) | Stream::Init => {
                    self.count += 1;
                }
                Stream::Ignore => {}
            }
            // Continue until inner is done, then yield final count
            if self.inner.next().is_none() {
                return Some(Stream::Next(self.count));
            }
        }
    }
}
