//! Extension trait and combinators for `TaskIterator`.
//!
//! This module provides builder-style combinator methods for any type implementing
//! `TaskIterator`. These combinators are applied **BEFORE** calling `execute()`.
//!
//! ## Overview
//!
//! The `TaskIteratorExt` trait extends any `TaskIterator` with methods like:
//! - `map_ready()` - Transform Ready values
//! - `map_pending()` - Transform Pending values
//! - `filter_ready()` - Filter Ready values
//! - `stream_collect()` - Collect all Ready values into a Vec
//!
//! ## Usage Pattern
//!
//! ```ignore
//! // 1. Define your task (implements TaskIterator)
//! let task = MyAsyncTask::new();
//!
//! // 2. Apply TaskIteratorExt combinators BEFORE execute()
//! let task = task
//!     .map_ready(|v| transform(v))
//!     .filter_ready(|v| should_keep(v));
//!
//! // 3. Execute to get StreamIterator
//! let stream = execute(task)?;
//! ```
//!
//! ## Relationship to Other Modules
//!
//! - [`super::task`] - Contains `TaskStatus`, `TaskIterator`, `ExecutionAction` definitions
//! - [`super::executors::unified`] - Contains `execute()` entry point
//! - [`super::stream_iterators`] - Contains `StreamIteratorExt` for post-execute combinators

use crate::synca::mpp::Stream;
use crate::valtron::{branches::CollectionState, ExecutionAction, TaskIterator, TaskStatus};
use std::sync::Arc;

use concurrent_queue::ConcurrentQueue;

/// Extension trait providing builder-style combinator methods for any `TaskIterator`.
///
/// This trait is automatically implemented for any type that implements `TaskIterator`
/// with the appropriate bounds. This includes:
/// - Raw task iterators implementing `TaskIterator`
/// - Driven iterators like `DrivenRecvIterator` and `DrivenSendTaskIterator`
///
/// ## Combinators
///
/// - [`map_ready`](TaskIteratorExt::map_ready) - Transform Ready values
/// - [`map_pending`](TaskIteratorExt::map_pending) - Transform Pending values
/// - [`filter_ready`](TaskIteratorExt::filter_ready) - Filter Ready values
/// - [`stream_collect`](TaskIteratorExt::stream_collect) - Collect all Ready values
/// - [`split_collector`](TaskIteratorExt::split_collector) - Split into observer + continuation
/// - [`split_collect_one`](TaskIteratorExt::split_collect_one) - Split for first match
///
/// ## Example
///
/// ```ignore
/// let task = MyTask::new()
///     .map_ready(|v| v * 2)
///     .map_pending(|p| format!("Still waiting: {:?}", p))
///     .filter_ready(|v| v > 10);
/// ```
pub trait TaskIteratorExt: TaskIterator + Sized {
    /// Transform Ready values using the provided function.
    ///
    /// Pending, Delayed, Init, and Spawn states pass through unchanged.
    fn map_ready<F, R>(self, f: F) -> TMapReady<Self, R>
    where
        F: Fn(Self::Ready) -> R + Send + 'static,
        R: Send + 'static;

    /// Transform Pending values using the provided function.
    ///
    /// Ready, Delayed, Init, and Spawn states pass through unchanged.
    fn map_pending<F, R>(self, f: F) -> TMapPending<Self, R>
    where
        F: Fn(Self::Pending) -> R + Send + 'static,
        R: Send + 'static;

    /// Filter Ready values using the provided predicate.
    ///
    /// Non-Ready states pass through unchanged. Ready values that don't
    /// satisfy the predicate are returned as `TaskStatus::Ignore`.
    fn filter_ready<F>(self, f: F) -> TFilterReady<Self>
    where
        F: Fn(&Self::Ready) -> bool + Send + 'static;

    /// Collect all Ready values into a Vec.
    ///
    /// Unlike `std::Iterator::collect()`, this does NOT block waiting for all items.
    /// It passes through Pending, Delayed, Init, Spawn states unchanged,
    /// replaces Ready values with `TaskStatus::Ignore`, and only yields the
    /// collected `Vec<Ready>` when the inner iterator completes.
    fn stream_collect(self) -> TStreamCollect<Self>
    where
        Self::Ready: Clone + Send + 'static;

    /// Split the iterator into an observer branch and a continuation branch.
    ///
    /// The observer receives a copy of items matching the predicate,
    /// while the continuation continues the chain for further combinators.
    ///
    /// ## Type Requirements
    ///
    /// - `Ready` must be `Clone` (observer gets a copy)
    /// - `Pending` must be `Clone` (observer gets a copy)
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
    /// let (observer, continuation) = send_request_task
    ///     .split_collector(
    ///         |item| matches!(item, RequestIntro::Success { .. }),
    ///         1  // Queue size 1 for immediate delivery
    ///     );
    /// ```
    fn split_collector<P>(
        self,
        predicate: P,
        queue_size: usize,
    ) -> (
        CollectorStreamIterator<Self::Ready, Self::Pending>,
        SplitCollectorContinuation<Self>,
    )
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> bool + Send + 'static;

    /// Convenience method: `split_collector` with `queue_size` = 1.
    ///
    /// Sends the first matching item to the observer, then continues.
    /// Perfect for "get intro first, then body" patterns.
    ///
    /// ## Example
    ///
    /// ```ignore
    /// let (observer, continuation) = send_request_task
    ///     .split_collect_one(|item| matches!(item, RequestIntro::Success { .. }));
    /// ```
    fn split_collect_one<P>(
        self,
        predicate: P,
    ) -> (
        CollectorStreamIterator<Self::Ready, Self::Pending>,
        SplitCollectorContinuation<Self>,
    )
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> bool + Send + 'static;

    /// Split the iterator into an observer branch and a continuation branch,
    /// closing the observer when the predicate returns a close signal.
    ///
    /// The observer receives a copy of items based on the predicate's returned
    /// `CollectionState`. The predicate can decide to skip items, collect them,
    /// or close the observer (with or without collecting the final item).
    ///
    /// ## Type Requirements
    ///
    /// - `Ready` must be `Clone` (observer gets a copy)
    /// - `Pending` must be `Clone` (observer gets a copy)
    ///
    /// ## Arguments
    ///
    /// * `predicate` - Function returning `CollectionState` to control collection behavior
    /// * `queue_size` - Size of the `ConcurrentQueue` between branches
    ///
    /// ## Returns
    ///
    /// Tuple of:
    /// - `SplitUntilObserver` - Observer that receives items based on `CollectionState`
    /// - `SplitUntilContinuation` - Continuation that continues the chain
    ///
    /// ## Example
    ///
    /// ```ignore
    /// // Observer gets items until first Success, then closes
    /// let (observer, continuation) = send_request_task
    ///     .split_collect_until(
    ///         |item| match item {
    ///             RequestIntro::Success { .. } => CollectionState::Close(true),
    ///             _ => CollectionState::Collect,
    ///         },
    ///         1  // Queue size 1 for immediate delivery
    ///     );
    /// ```
    fn split_collect_until<P>(
        self,
        predicate: P,
        queue_size: usize,
    ) -> (
        SplitUntilObserver<Self::Ready, Self::Pending>,
        SplitUntilContinuation<Self>,
    )
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> CollectionState + Send + 'static;

    /// Split the iterator with transformation for observer.
    ///
    /// Like `split_collect_until` but observer receives transformed data D (which must be Clone),
    /// while continuation receives original Ready values. Useful when Ready is not Clone
    /// but extractable subset is.
    ///
    /// The single closure returns `(CollectionState, Option<D>)`:
    /// - `CollectionState` controls whether to skip, collect, or close the observer
    /// - `Option<D>` provides the transformed value (`None` skips sending to observer)
    ///
    /// ## Type Requirements
    ///
    /// - `D` must be `Clone` (observer gets transformed copy)
    /// - `Pending` must be `Clone` (observer gets transformed copy)
    ///
    /// ## Arguments
    ///
    /// * `transform` - Function returning (`CollectionState`, Option<D>) to control collection and transform
    /// * `queue_size` - Size of the `ConcurrentQueue` between branches
    ///
    /// ## Returns
    ///
    /// Tuple of:
    /// - `SplitUntilObserverMap` - Observer that receives transformed D values
    /// - `SplitUntilContinuationMap` - Continuation that continues with original Ready
    ///
    /// ## Example
    ///
    /// ```ignore
    /// // Observer gets RequestIntroData (cloneable), continuation gets full RequestIntro
    /// let (observer, continuation) = send_request_task
    ///     .split_collect_until_map(
    ///         |item| match item {
    ///             RequestIntro::Success { .. } => (CollectionState::Close(true), item.to_cloneable_data()),
    ///             _ => (CollectionState::Collect, item.to_cloneable_data()),
    ///         },
    ///         1
    ///     );
    /// ```
    fn split_collect_until_map<F, D>(
        self,
        transform: F,
        queue_size: usize,
    ) -> (
        SplitUntilObserverMap<D, Self::Pending>,
        SplitUntilContinuationMap<Self, D>,
    )
    where
        Self: Sized,
        D: Clone + Send + 'static,
        Self::Pending: Clone,
        F: Fn(&Self::Ready) -> (CollectionState, Option<D>) + Send + 'static;

    /// Split the iterator into an observer branch and a continuation branch,
    /// mapping matched Ready values to a different type before sending to the observer.
    ///
    /// Like `split_collector` but the observer receives transformed values of type `M`
    /// instead of cloned `Ready` values. The continuation continues with original `Ready`.
    /// Useful when `Ready` is not `Clone` but a subset can be extracted.
    ///
    /// The single closure returns `(bool, Option<M>)`:
    /// - `bool` - whether this item matched (true = matched, false = skip)
    /// - `Option<M>` - the transformed value (`None` skips sending to observer)
    ///
    /// ## Type Requirements
    ///
    /// - `M` must be `Clone + Send + 'static` (observer gets transformed copy)
    /// - `Pending` must be `Clone` (observer gets a copy)
    ///
    /// ## Arguments
    ///
    /// * `transform` - Function returning (bool, Option<M>) to control matching and transform
    /// * `queue_size` - Size of the `ConcurrentQueue` between branches
    ///
    /// ## Returns
    ///
    /// Tuple of:
    /// - `SplitCollectorMapObserver` - Observer that receives transformed M values
    /// - `SplitCollectorMapContinuation` - Continuation that continues with original Ready
    ///
    /// ## Example
    ///
    /// ```ignore
    /// // Observer gets u64 ids, continuation gets full Item
    /// let (observer, continuation) = task
    ///     .split_collector_map(
    ///         |item| (item.is_important(), Some(item.id())),
    ///         10
    ///     );
    /// ```
    fn split_collector_map<F, M>(
        self,
        transform: F,
        queue_size: usize,
    ) -> (
        SplitCollectorMapObserver<M, Self::Pending>,
        SplitCollectorMapContinuation<Self, M>,
    )
    where
        Self: Sized,
        M: Clone + Send + 'static,
        Self::Pending: Clone,
        F: Fn(&Self::Ready) -> (bool, Option<M>) + Send + 'static;

    /// Convenience method: `split_collector_map` with `queue_size` = 1.
    ///
    /// Sends the first matching transformed item to the observer, then continues.
    fn split_collect_one_map<F, M>(
        self,
        transform: F,
    ) -> (
        SplitCollectorMapObserver<M, Self::Pending>,
        SplitCollectorMapContinuation<Self, M>,
    )
    where
        Self: Sized,
        M: Clone + Send + 'static,
        Self::Pending: Clone,
        F: Fn(&Self::Ready) -> (bool, Option<M>) + Send + 'static,
    {
        self.split_collector_map(transform, 1)
    }

    /// Flatten nested iterator patterns where outer yields inner iterators.
    ///
    /// The mapper function transforms each `Ready` value into an inner iterator.
    /// The returned `TMapIter` drains each inner iterator until `None`, then polls
    /// the outer for the next item.
    ///
    /// Non-Ready states (Pending, Spawn) from the outer iterator pass through via `Into`,
    /// so `Self::Pending` must be convertible to `InnerP` and `Self::Spawner` to `InnerS`.
    ///
    /// ## Example
    ///
    /// ```ignore
    /// // Outer yields Ready(Vec<u8>), mapper returns vec.into_iter()
    /// let flattened = task.map_iter(|vec| vec.into_iter());
    /// ```
    fn map_iter<F, InnerIter, InnerR, InnerP, InnerS>(
        self,
        mapper: F,
    ) -> TMapIter<
        Self,
        F,
        InnerIter,
        InnerR,
        InnerP,
        InnerS,
        Self::Ready,
        Self::Pending,
        Self::Spawner,
    >
    where
        Self: Sized,
        F: Fn(Self::Ready) -> InnerIter + Send + 'static,
        InnerIter: Iterator<Item = TaskStatus<InnerR, InnerP, InnerS>> + Send + 'static,
        InnerR: Send + 'static,
        InnerP: Send + 'static,
        InnerS: ExecutionAction + Send + 'static,
        Self::Pending: Into<InnerP> + Send + 'static,
        Self::Spawner: Into<InnerS> + Send + 'static;

    /// Flatten Ready values that implement IntoIterator.
    ///
    /// Input:  TaskIterator<Ready = Vec<M>, Pending = P, Spawner = S>
    /// Output: TaskIterator<Ready = M, Pending = P, Spawner = S>
    fn flatten_ready(self) -> TFlattenReady<Self>
    where
        Self: Sized,
        Self::Ready: IntoIterator,
        <Self::Ready as IntoIterator>::Item: Send + 'static;

    /// Flatten Pending values that implement IntoIterator.
    ///
    /// Input:  TaskIterator<Ready = D, Pending = Vec<M>, Spawner = S>
    /// Output: TaskIterator<Ready = D, Pending = M, Spawner = S>
    fn flatten_pending(self) -> TFlattenPending<Self>
    where
        Self: Sized,
        Self::Pending: IntoIterator,
        <Self::Pending as IntoIterator>::Item: Send + 'static;

    /// Flat map Ready values - transform and flatten in one operation.
    ///
    /// Input:  TaskIterator<Ready = D, Pending = P, Spawner = S>
    /// Output: TaskIterator<Ready = U::Item, Pending = P, Spawner = S>
    fn flat_map_ready<F, U>(self, f: F) -> TFlatMapReady<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::Ready) -> U + Send + 'static,
        U: IntoIterator,
        U::Item: Send + 'static;

    /// Flat map Pending values - transform and flatten in one operation.
    ///
    /// Input:  TaskIterator<Ready = D, Pending = P, Spawner = S>
    /// Output: TaskIterator<Ready = D, Pending = U::Item, Spawner = S>
    fn flat_map_pending<F, U>(self, f: F) -> TFlatMapPending<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::Pending) -> U + Send + 'static,
        U: IntoIterator,
        U::Item: Send + 'static;
}

// Blanket implementation: anything implementing TaskIterator gets TaskIteratorExt
impl<I> TaskIteratorExt for I
where
    I: TaskIterator + Send + 'static,
    I::Ready: Send + 'static,
    I::Pending: Send + 'static,
    I::Spawner: ExecutionAction + Send + 'static,
{
    fn map_ready<F, R>(self, f: F) -> TMapReady<Self, R>
    where
        F: Fn(Self::Ready) -> R + Send + 'static,
        R: Send + 'static,
    {
        TMapReady {
            inner: self,
            mapper: Box::new(f),
        }
    }

    fn map_pending<F, R>(self, f: F) -> TMapPending<Self, R>
    where
        F: Fn(Self::Pending) -> R + Send + 'static,
        R: Send + 'static,
    {
        TMapPending {
            inner: self,
            mapper: Box::new(f),
        }
    }

    fn filter_ready<F>(self, f: F) -> TFilterReady<Self>
    where
        F: Fn(&Self::Ready) -> bool + Send + 'static,
    {
        TFilterReady {
            inner: self,
            predicate: Box::new(f),
        }
    }

    fn stream_collect(self) -> TStreamCollect<Self>
    where
        Self::Ready: Send + 'static,
    {
        TStreamCollect {
            inner: self,
            collected: Vec::new(),
            done: false,
        }
    }

    fn split_collector<P>(
        self,
        predicate: P,
        queue_size: usize,
    ) -> (
        CollectorStreamIterator<Self::Ready, Self::Pending>,
        SplitCollectorContinuation<Self>,
    )
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> bool + Send + 'static,
    {
        let queue = Arc::new(ConcurrentQueue::bounded(queue_size));
        tracing::debug!(
            "split_collector: creating observer and continuation with queue_size={}",
            queue_size
        );

        let observer = CollectorStreamIterator {
            queue: Arc::clone(&queue),
        };

        let continuation = SplitCollectorContinuation {
            inner: self,
            queue,
            predicate: Box::new(predicate),
        };

        (observer, continuation)
    }

    fn split_collect_one<P>(
        self,
        predicate: P,
    ) -> (
        CollectorStreamIterator<Self::Ready, Self::Pending>,
        SplitCollectorContinuation<Self>,
    )
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> bool + Send + 'static,
    {
        self.split_collector(predicate, 1)
    }

    fn split_collect_until<P>(
        self,
        predicate: P,
        queue_size: usize,
    ) -> (
        SplitUntilObserver<Self::Ready, Self::Pending>,
        SplitUntilContinuation<Self>,
    )
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> CollectionState + Send + 'static,
    {
        let queue = Arc::new(ConcurrentQueue::bounded(queue_size));
        tracing::debug!(
            "split_collect_until: creating observer and continuation with queue_size={}",
            queue_size
        );

        let observer = SplitUntilObserver {
            queue: Arc::clone(&queue),
        };

        let continuation = SplitUntilContinuation {
            inner: self,
            queue,
            predicate: Box::new(predicate),
        };

        (observer, continuation)
    }

    fn split_collect_until_map<F, D>(
        self,
        transform: F,
        queue_size: usize,
    ) -> (
        SplitUntilObserverMap<D, Self::Pending>,
        SplitUntilContinuationMap<Self, D>,
    )
    where
        Self: Sized,
        D: Clone + Send + 'static,
        Self::Pending: Clone,
        F: Fn(&Self::Ready) -> (CollectionState, Option<D>) + Send + 'static,
    {
        let queue = Arc::new(ConcurrentQueue::bounded(queue_size));
        tracing::debug!(
            "split_collect_until_map: creating observer and continuation with queue_size={}",
            queue_size
        );

        let observer = SplitUntilObserverMap {
            queue: Arc::clone(&queue),
        };

        let continuation = SplitUntilContinuationMap {
            inner: self,
            queue,
            transform: Box::new(transform),
        };

        (observer, continuation)
    }

    fn split_collector_map<F, M>(
        self,
        transform: F,
        queue_size: usize,
    ) -> (
        SplitCollectorMapObserver<M, Self::Pending>,
        SplitCollectorMapContinuation<Self, M>,
    )
    where
        Self: Sized,
        M: Clone + Send + 'static,
        Self::Pending: Clone,
        F: Fn(&Self::Ready) -> (bool, Option<M>) + Send + 'static,
    {
        let queue = Arc::new(ConcurrentQueue::bounded(queue_size));
        tracing::debug!(
            "split_collector_map: creating observer and continuation with queue_size={}",
            queue_size
        );

        let observer = SplitCollectorMapObserver {
            queue: Arc::clone(&queue),
        };

        let continuation = SplitCollectorMapContinuation {
            inner: self,
            queue,
            transform: Box::new(transform),
        };

        (observer, continuation)
    }

    fn split_collect_one_map<F, M>(
        self,
        transform: F,
    ) -> (
        SplitCollectorMapObserver<M, Self::Pending>,
        SplitCollectorMapContinuation<Self, M>,
    )
    where
        Self: Sized,
        M: Clone + Send + 'static,
        Self::Pending: Clone,
        F: Fn(&Self::Ready) -> (bool, Option<M>) + Send + 'static,
    {
        self.split_collector_map(transform, 1)
    }

    fn map_iter<F, InnerIter, InnerR, InnerP, InnerS>(
        self,
        mapper: F,
    ) -> TMapIter<
        Self,
        F,
        InnerIter,
        InnerR,
        InnerP,
        InnerS,
        Self::Ready,
        Self::Pending,
        Self::Spawner,
    >
    where
        Self: Sized,
        F: Fn(Self::Ready) -> InnerIter + Send + 'static,
        InnerIter: Iterator<Item = TaskStatus<InnerR, InnerP, InnerS>> + Send + 'static,
        InnerR: Send + 'static,
        InnerP: Send + 'static,
        InnerS: ExecutionAction + Send + 'static,
        Self::Pending: Into<InnerP> + Send + 'static,
        Self::Spawner: Into<InnerS> + Send + 'static,
    {
        TMapIter {
            outer: self,
            mapper,
            current_inner: None,
            _phantom: std::marker::PhantomData,
        }
    }

    fn flatten_ready(self) -> TFlattenReady<Self>
    where
        Self: Sized,
        Self::Ready: IntoIterator,
        <Self::Ready as IntoIterator>::Item: Send + 'static,
    {
        TFlattenReady {
            inner: self,
            current_inner: None,
        }
    }

    fn flatten_pending(self) -> TFlattenPending<Self>
    where
        Self: Sized,
        Self::Pending: IntoIterator,
        <Self::Pending as IntoIterator>::Item: Send + 'static,
    {
        TFlattenPending {
            inner: self,
            current_inner: None,
        }
    }

    fn flat_map_ready<F, U>(self, f: F) -> TFlatMapReady<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::Ready) -> U + Send + 'static,
        U: IntoIterator,
        U::Item: Send + 'static,
    {
        TFlatMapReady {
            inner: self,
            mapper: f,
            current_inner: None,
        }
    }

    fn flat_map_pending<F, U>(self, f: F) -> TFlatMapPending<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::Pending) -> U + Send + 'static,
        U: IntoIterator,
        U::Item: Send + 'static,
    {
        TFlatMapPending {
            inner: self,
            mapper: f,
            current_inner: None,
        }
    }
}

/// Wrapper type that transforms Ready values.
pub struct TMapReady<I: TaskIterator, R> {
    inner: I,
    mapper: Box<dyn Fn(I::Ready) -> R + Send>,
}

impl<I, R> Iterator for TMapReady<I, R>
where
    I: TaskIterator,
    R: Send + 'static,
{
    type Item = TaskStatus<R, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next_status().map(|status| match status {
            TaskStatus::Ready(v) => TaskStatus::Ready((self.mapper)(v)),
            TaskStatus::Pending(v) => TaskStatus::Pending(v),
            TaskStatus::Delayed(d) => TaskStatus::Delayed(d),
            TaskStatus::Ignore => TaskStatus::Ignore,
            TaskStatus::Init => TaskStatus::Init,
            TaskStatus::Spawn(s) => TaskStatus::Spawn(s),
        })
    }
}

impl<I, R> TaskIterator for TMapReady<I, R>
where
    I: TaskIterator,
    R: Send + 'static,
{
    type Ready = R;
    type Pending = I::Pending;
    type Spawner = I::Spawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

/// Wrapper type that transforms Pending values.
pub struct TMapPending<I: TaskIterator, R> {
    inner: I,
    mapper: Box<dyn Fn(I::Pending) -> R + Send>,
}

impl<I, R> Iterator for TMapPending<I, R>
where
    I: TaskIterator,
    R: Send + 'static,
{
    type Item = TaskStatus<I::Ready, R, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next_status().map(|status| match status {
            TaskStatus::Ready(v) => TaskStatus::Ready(v),
            TaskStatus::Pending(v) => TaskStatus::Pending((self.mapper)(v)),
            TaskStatus::Delayed(d) => TaskStatus::Delayed(d),
            TaskStatus::Init => TaskStatus::Init,
            TaskStatus::Ignore => TaskStatus::Ignore,
            TaskStatus::Spawn(s) => TaskStatus::Spawn(s),
        })
    }
}

impl<I, R> TaskIterator for TMapPending<I, R>
where
    I: TaskIterator,
    R: Send + 'static,
{
    type Ready = I::Ready;
    type Pending = R;
    type Spawner = I::Spawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

/// Wrapper type that filters Ready values.
///
/// Filtered-out Ready values are returned as `TaskStatus::Ignore` to avoid blocking.
pub struct TFilterReady<I: TaskIterator> {
    inner: I,
    predicate: Box<dyn Fn(&I::Ready) -> bool + Send>,
}

impl<I> Iterator for TFilterReady<I>
where
    I: TaskIterator,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let status = self.inner.next_status()?;
        match &status {
            TaskStatus::Ready(v) => {
                if (self.predicate)(v) {
                    Some(status)
                } else {
                    Some(TaskStatus::Ignore)
                }
            }
            _ => Some(status), // Pass through non-Ready states
        }
    }
}

impl<I> TaskIterator for TFilterReady<I>
where
    I: TaskIterator,
{
    type Ready = I::Ready;
    type Pending = I::Pending;
    type Spawner = I::Spawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

/// Wrapper type for `map_iter` combinator that flattens nested iterator patterns.
///
/// The outer iterator yields `TaskStatus::Ready(inner_iter)` values.
/// The mapper function transforms each `Ready` value into an inner iterator.
/// `TMapIter` drains each inner iterator until `None`, then polls the outer
/// for the next item to continue flattening.
///
/// Non-Ready states (Pending, Spawn) from the outer iterator pass through via `Into`,
/// so `P` must be convertible to `InnerP` and `S` to `InnerS`.
pub struct TMapIter<I, F, InnerIter, InnerR, InnerP, InnerS, R, P, S>
where
    I: TaskIterator<Ready = R, Pending = P, Spawner = S>,
    F: Fn(R) -> InnerIter,
    InnerIter: Iterator<Item = TaskStatus<InnerR, InnerP, InnerS>>,
    InnerS: ExecutionAction,
    S: ExecutionAction,
{
    outer: I,
    mapper: F,
    current_inner: Option<InnerIter>,
    _phantom: std::marker::PhantomData<(InnerR, InnerP, InnerS, R, P, S)>,
}

impl<I, F, InnerIter, InnerR, InnerP, InnerS, R, P, S> Iterator
    for TMapIter<I, F, InnerIter, InnerR, InnerP, InnerS, R, P, S>
where
    I: TaskIterator<Ready = R, Pending = P, Spawner = S>,
    F: Fn(R) -> InnerIter,
    InnerIter: Iterator<Item = TaskStatus<InnerR, InnerP, InnerS>>,
    InnerR: Send + 'static,
    InnerP: Send + 'static,
    InnerS: ExecutionAction + Send + 'static,
    P: Into<InnerP> + Send + 'static,
    S: Into<InnerS> + ExecutionAction + Send + 'static,
{
    type Item = TaskStatus<InnerR, InnerP, InnerS>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // First, try to drain the current inner iterator
            if let Some(ref mut inner) = self.current_inner {
                if let Some(item) = inner.next() {
                    return Some(item);
                }
                // Inner exhausted, clear it and continue to poll outer
                self.current_inner = None;
            }

            // Poll outer for next item
            match self.outer.next_status() {
                Some(TaskStatus::Ready(d)) => {
                    // Mapper returns a new inner iterator, start draining it
                    let new_inner = (self.mapper)(d);
                    self.current_inner = Some(new_inner);
                }
                Some(TaskStatus::Pending(p)) => return Some(TaskStatus::Pending(p.into())),
                Some(TaskStatus::Delayed(d)) => return Some(TaskStatus::Delayed(d)),
                Some(TaskStatus::Init) => return Some(TaskStatus::Init),
                Some(TaskStatus::Ignore) => return Some(TaskStatus::Ignore),
                Some(TaskStatus::Spawn(s)) => return Some(TaskStatus::Spawn(s.into())),
                None => return None, // Outer exhausted
            }
        }
    }
}

impl<I, F, InnerIter, InnerR, InnerP, InnerS, R, P, S> TaskIterator
    for TMapIter<I, F, InnerIter, InnerR, InnerP, InnerS, R, P, S>
where
    I: TaskIterator<Ready = R, Pending = P, Spawner = S>,
    F: Fn(R) -> InnerIter,
    InnerIter: Iterator<Item = TaskStatus<InnerR, InnerP, InnerS>>,
    InnerR: Send + 'static,
    InnerP: Send + 'static,
    InnerS: ExecutionAction + Send + 'static,
    P: Into<InnerP> + Send + 'static,
    S: Into<InnerS> + ExecutionAction + Send + 'static,
{
    type Ready = InnerR;
    type Pending = InnerP;
    type Spawner = InnerS;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

// ============================================================================
// Flatten Ready / Pending
// ============================================================================

/// Wrapper type that flattens Ready values that implement `IntoIterator`.
///
/// The Ready type must implement `IntoIterator`. We store the inner iterator
/// and drain it over multiple `next()` calls. When exhausted, poll outer again.
///
/// **Important**: Returns `TaskStatus::Ignore` when waiting for inner iterator,
/// never blocks in a loop.
pub struct TFlattenReady<I: TaskIterator>
where
    I::Ready: IntoIterator,
{
    inner: I,
    current_inner: Option<<I::Ready as IntoIterator>::IntoIter>,
}

impl<I> Iterator for TFlattenReady<I>
where
    I: TaskIterator,
    I::Ready: IntoIterator,
    <I::Ready as IntoIterator>::Item: Send + 'static,
    I::Pending: Send + 'static,
    I::Spawner: Send + 'static,
{
    type Item = TaskStatus<<I::Ready as IntoIterator>::Item, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // First drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(TaskStatus::Ready(item));
            }
            // Inner exhausted - clear and fall through to poll outer
            self.current_inner = None;
        }

        // Get next from outer iterator
        let status = self.inner.next_status()?;

        match status {
            TaskStatus::Ready(iterable) => {
                // Store iterator, drain on NEXT call
                self.current_inner = Some(iterable.into_iter());
                // Return Ignore to signal "still working, no Ready value yet"
                Some(TaskStatus::Ignore)
            }
            // Pass through non-Ready states unchanged (Pending/Spawner type unchanged)
            TaskStatus::Pending(p) => Some(TaskStatus::Pending(p)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
        }
    }
}

impl<I> TaskIterator for TFlattenReady<I>
where
    I: TaskIterator,
    I::Ready: IntoIterator,
    <I::Ready as IntoIterator>::Item: Send + 'static,
    I::Pending: Send + 'static,
    I::Spawner: Send + 'static,
{
    type Ready = <I::Ready as IntoIterator>::Item;
    type Pending = I::Pending;
    type Spawner = I::Spawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

// ============================================================================
// Flatten Pending
// ============================================================================

/// Wrapper type that flattens Pending values that implement `IntoIterator`.
///
/// The Pending type must implement `IntoIterator`. We store the inner iterator
/// and drain it over multiple `next()` calls. When exhausted, poll outer again.
///
/// **Important**: Returns `TaskStatus::Ignore` when waiting for inner iterator,
/// never blocks in a loop.
pub struct TFlattenPending<I: TaskIterator>
where
    I::Pending: IntoIterator,
{
    inner: I,
    current_inner: Option<<I::Pending as IntoIterator>::IntoIter>,
}

impl<I> Iterator for TFlattenPending<I>
where
    I: TaskIterator,
    I::Pending: IntoIterator,
    <I::Pending as IntoIterator>::Item: Send + 'static,
    I::Ready: Send + 'static,
    I::Spawner: Send + 'static,
{
    type Item = TaskStatus<I::Ready, <I::Pending as IntoIterator>::Item, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // First drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(TaskStatus::Pending(item));
            }
            self.current_inner = None;
        }

        let status = self.inner.next_status()?;

        match status {
            TaskStatus::Pending(iterable) => {
                self.current_inner = Some(iterable.into_iter());
                Some(TaskStatus::Ignore)
            }
            // Pass through non-Pending states unchanged
            TaskStatus::Ready(r) => Some(TaskStatus::Ready(r)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
        }
    }
}

impl<I> TaskIterator for TFlattenPending<I>
where
    I: TaskIterator,
    I::Pending: IntoIterator,
    <I::Pending as IntoIterator>::Item: Send + 'static,
    I::Ready: Send + 'static,
    I::Spawner: Send + 'static,
{
    type Ready = I::Ready;
    type Pending = <I::Pending as IntoIterator>::Item;
    type Spawner = I::Spawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

// ============================================================================
// Flat Map Ready
// ============================================================================

/// Wrapper type that maps Ready to `IntoIterator`, then flattens.
///
/// User provides function `Ready -> IntoIterator`. We store the returned iterator
/// and drain it over multiple `next()` calls.
///
/// **Important**: Returns `TaskStatus::Ignore` when waiting for inner iterator,
/// never blocks in a loop.
pub struct TFlatMapReady<I: TaskIterator, F, U: IntoIterator> {
    inner: I,
    mapper: F,
    current_inner: Option<U::IntoIter>,
}

impl<I, F, U> Iterator for TFlatMapReady<I, F, U>
where
    I: TaskIterator,
    F: Fn(I::Ready) -> U + Send + 'static,
    U: IntoIterator,
    U::Item: Send + 'static,
    I::Pending: Send + 'static,
    I::Spawner: Send + 'static,
{
    type Item = TaskStatus<U::Item, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // First drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(TaskStatus::Ready(item));
            }
            self.current_inner = None;
        }

        let status = self.inner.next_status()?;

        match status {
            TaskStatus::Ready(v) => {
                let iterable = (self.mapper)(v);
                self.current_inner = Some(iterable.into_iter());
                Some(TaskStatus::Ignore)
            }
            // Pass through non-Ready states (Pending/Spawner type unchanged)
            TaskStatus::Pending(p) => Some(TaskStatus::Pending(p)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
        }
    }
}

impl<I, F, U> TaskIterator for TFlatMapReady<I, F, U>
where
    I: TaskIterator,
    F: Fn(I::Ready) -> U + Send + 'static,
    U: IntoIterator,
    U::Item: Send + 'static,
    I::Pending: Send + 'static,
    I::Spawner: Send + 'static,
{
    type Ready = U::Item;
    type Pending = I::Pending;
    type Spawner = I::Spawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

// ============================================================================
// Flat Map Pending
// ============================================================================

/// Wrapper type that maps Pending to `IntoIterator`, then flattens.
///
/// User provides function `Pending -> IntoIterator`. We store the returned iterator
/// and drain it over multiple `next()` calls.
///
/// **Important**: Returns `TaskStatus::Ignore` when waiting for inner iterator,
/// never blocks in a loop.
pub struct TFlatMapPending<I: TaskIterator, F, U: IntoIterator> {
    inner: I,
    mapper: F,
    current_inner: Option<U::IntoIter>,
}

impl<I, F, U> Iterator for TFlatMapPending<I, F, U>
where
    I: TaskIterator,
    F: Fn(I::Pending) -> U + Send + 'static,
    U: IntoIterator,
    U::Item: Send + 'static,
    I::Ready: Send + 'static,
    I::Spawner: Send + 'static,
{
    type Item = TaskStatus<I::Ready, U::Item, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // First drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(TaskStatus::Pending(item));
            }
            self.current_inner = None;
        }

        let status = self.inner.next_status()?;

        match status {
            TaskStatus::Pending(v) => {
                let iterable = (self.mapper)(v);
                self.current_inner = Some(iterable.into_iter());
                Some(TaskStatus::Ignore)
            }
            // Pass through non-Pending states (Ready/Spawner type unchanged)
            TaskStatus::Ready(r) => Some(TaskStatus::Ready(r)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
        }
    }
}

impl<I, F, U> TaskIterator for TFlatMapPending<I, F, U>
where
    I: TaskIterator,
    F: Fn(I::Pending) -> U + Send + 'static,
    U: IntoIterator,
    U::Item: Send + 'static,
    I::Ready: Send + 'static,
    I::Spawner: Send + 'static,
{
    type Ready = I::Ready;
    type Pending = U::Item;
    type Spawner = I::Spawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

/// Wrapper type that collects all Ready values into a Vec.
///
/// Passes through Pending, Delayed, Init, Spawn states unchanged.
/// Ready values are collected and replaced with `TaskStatus::Ignore`.
/// Only yields the collected Vec when the inner iterator completes.
/// Does NOT require Ready to implement Clone.
pub struct TStreamCollect<I: TaskIterator> {
    inner: I,
    collected: Vec<I::Ready>,
    done: bool,
}

impl<I> Iterator for TStreamCollect<I>
where
    I: TaskIterator,
    I::Ready: Send + 'static,
{
    type Item = TaskStatus<Vec<I::Ready>, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // If we've already yielded the collected result, we're done
        if self.done {
            return None;
        }

        match self.inner.next_status() {
            Some(TaskStatus::Ready(value)) => {
                self.collected.push(value);
                // Keep collecting, return Ignore to signal collected but continue
                Some(TaskStatus::Ignore)
            }
            Some(TaskStatus::Pending(p)) => Some(TaskStatus::Pending(p)),
            Some(TaskStatus::Delayed(d)) => Some(TaskStatus::Delayed(d)),
            Some(TaskStatus::Init) => Some(TaskStatus::Init),
            Some(TaskStatus::Spawn(s)) => Some(TaskStatus::Spawn(s)),
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            None => {
                // Inner iterator is done, yield the collected result
                self.done = true;
                let collected = std::mem::take(&mut self.collected);
                Some(TaskStatus::Ready(collected))
            }
        }
    }
}

impl<I> TaskIterator for TStreamCollect<I>
where
    I: TaskIterator,
    I::Ready: Send + 'static,
{
    type Ready = Vec<I::Ready>;
    type Pending = I::Pending;
    type Spawner = I::Spawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

// ============================================================================
// Split Collector Combinators (Feature 07)
// ============================================================================

/// Observer branch from `split_collector()`.
///
/// Receives copies of items matching the predicate via a `ConcurrentQueue`.
/// Yields `Stream::Next` for matched items, forwards Pending/Delayed from source.
pub struct CollectorStreamIterator<D, P> {
    /// Shared queue receiving copied items from the splitter
    queue: Arc<ConcurrentQueue<Stream<D, P>>>,
}

impl<D, P> Iterator for CollectorStreamIterator<D, P>
where
    D: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        // Try to get item from queue
        match self.queue.pop() {
            Ok(item) => {
                tracing::trace!("CollectorStreamIterator: received item from queue");
                Some(item)
            }
            Err(concurrent_queue::PopError::Empty) => {
                // Queue is empty, check if source is done (queue closed)
                if self.queue.is_closed() {
                    tracing::debug!("CollectorStreamIterator: queue closed, returning None");
                    None
                } else {
                    // Still waiting for items - return Ignore to signal still pending
                    tracing::trace!(
                        "CollectorStreamIterator: queue empty but not closed, returning Ignore"
                    );
                    Some(Stream::Ignore)
                }
            }
            Err(concurrent_queue::PopError::Closed) => {
                tracing::debug!("CollectorStreamIterator: queue closed, returning None");
                None
            }
        }
    }
}

impl<D, P> crate::synca::mpp::StreamIterator for CollectorStreamIterator<D, P>
where
    D: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type D = D;
    type P = P;
}

/// Continuation branch from `split_collector()`.
///
/// Wraps the original iterator, copying matched items to the observer queue
/// while continuing the chain for further combinators.
pub struct SplitCollectorContinuation<I: TaskIterator> {
    /// The wrapped iterator
    inner: I,
    /// Queue to send copied items to observer
    queue: Arc<ConcurrentQueue<Stream<I::Ready, I::Pending>>>,
    /// Predicate to determine which items to copy
    predicate: Box<dyn Fn(&I::Ready) -> bool + Send>,
}

impl<I> Iterator for SplitCollectorContinuation<I>
where
    I: TaskIterator,
    I::Ready: Clone,
    I::Pending: Clone,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = if let Some(item) = self.inner.next_status() {
            item
        } else {
            // Source iterator is naturally exhausted, close the queue
            self.queue.close();
            tracing::debug!("SplitCollectorContinuation: source exhausted, queue closed");
            return None;
        };

        // Copy matched items to observer queue
        if let TaskStatus::Ready(value) = &item {
            if (self.predicate)(value) {
                let stream_item = Stream::Next(value.clone());
                if let Err(e) = self.queue.force_push(stream_item) {
                    tracing::error!("SplitCollectorContinuation: failed to push to queue: {}", e);
                } else {
                    tracing::trace!(
                        "SplitCollectorContinuation: copied matched item to observer queue"
                    );
                }
            }
        }

        // Always forward to continuation
        Some(item)
    }
}

impl<I> TaskIterator for SplitCollectorContinuation<I>
where
    I: TaskIterator,
    I::Ready: Clone,
    I::Pending: Clone,
{
    type Ready = I::Ready;
    type Pending = I::Pending;
    type Spawner = I::Spawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

impl<I> Drop for SplitCollectorContinuation<I>
where
    I: TaskIterator,
{
    fn drop(&mut self) {
        // Close the queue to signal that the source is done
        self.queue.close();
        tracing::debug!("SplitCollectorContinuation: dropped, queue closed");
    }
}

// ============================================================================
// Split Collect Until Combinator
// ============================================================================

/// Observer branch from `split_collect_until()`.
///
/// Receives copies of items until the predicate is met, then the queue
/// is closed and the observer completes.
pub struct SplitUntilObserver<D, P> {
    /// Shared queue receiving copied items from the splitter
    queue: Arc<ConcurrentQueue<Stream<D, P>>>,
}

impl<D, P> Iterator for SplitUntilObserver<D, P>
where
    D: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.queue.pop() {
            Ok(item) => {
                tracing::trace!("SplitUntilObserver: received item from queue");
                Some(item)
            }
            Err(concurrent_queue::PopError::Empty) => {
                if self.queue.is_closed() {
                    tracing::debug!("SplitUntilObserver: queue closed, returning None");
                    None
                } else {
                    tracing::trace!(
                        "SplitUntilObserver: queue empty but not closed, returning Ignore"
                    );
                    Some(Stream::Ignore)
                }
            }
            Err(concurrent_queue::PopError::Closed) => {
                tracing::debug!("SplitUntilObserver: queue closed, returning None");
                None
            }
        }
    }
}

impl<D, P> crate::synca::mpp::StreamIterator for SplitUntilObserver<D, P>
where
    D: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type D = D;
    type P = P;
}

/// Continuation branch from `split_collect_until()`.
///
/// Wraps the original iterator, copying items to the observer queue
/// until the predicate is met. When predicate returns true, that item
/// is sent and the queue is closed (observer completes).
pub struct SplitUntilContinuation<I: TaskIterator> {
    /// The wrapped iterator
    inner: I,
    /// Queue to send copied items to observer
    queue: Arc<ConcurrentQueue<Stream<I::Ready, I::Pending>>>,
    /// Predicate to determine when to close observer
    predicate: Box<dyn Fn(&I::Ready) -> CollectionState + Send>,
}

impl<I> Iterator for SplitUntilContinuation<I>
where
    I: TaskIterator,
    I::Ready: Clone,
    I::Pending: Clone,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = if let Some(item) = self.inner.next_status() {
            item
        } else {
            // Source iterator is naturally exhausted, close the queue
            self.queue.close();
            tracing::debug!("SplitUntilContinuation: source exhausted, queue closed");
            return None;
        };

        // Handle items based on CollectionState from predicate
        if let TaskStatus::Ready(value) = &item {
            match (self.predicate)(value) {
                CollectionState::Skip => {
                    // Skip this item - don't send to observer
                    tracing::trace!(
                        "SplitUntilContinuation: skipping item (CollectionState::Skip)"
                    );
                }
                CollectionState::Collect => {
                    // Collect this item for the observer
                    let stream_item = Stream::Next(value.clone());
                    if let Err(e) = self.queue.force_push(stream_item) {
                        tracing::error!("SplitUntilContinuation: failed to push to queue: {}", e);
                    } else {
                        tracing::trace!("SplitUntilContinuation: collected item for observer");
                    }
                }
                CollectionState::Close(collect_this) => {
                    // Close the observer after optionally collecting this item
                    if collect_this {
                        let stream_item = Stream::Next(value.clone());
                        if let Err(e) = self.queue.force_push(stream_item) {
                            tracing::error!(
                                "SplitUntilContinuation: failed to push to queue: {}",
                                e
                            );
                        } else {
                            tracing::trace!("SplitUntilContinuation: collecting final item and closing observer queue");
                        }
                    } else {
                        tracing::trace!("SplitUntilContinuation: closing observer queue without collecting final item");
                    }
                    self.queue.close();
                    tracing::debug!("SplitUntilContinuation: observer queue closed after CollectionState::Close");
                }
            }
        }

        // Always forward to continuation
        Some(item)
    }
}

impl<I> TaskIterator for SplitUntilContinuation<I>
where
    I: TaskIterator,
    I::Ready: Clone,
    I::Pending: Clone,
{
    type Ready = I::Ready;
    type Pending = I::Pending;
    type Spawner = I::Spawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

impl<I> Drop for SplitUntilContinuation<I>
where
    I: TaskIterator,
{
    fn drop(&mut self) {
        // Close the queue as backup if not already closed
        if !self.queue.is_closed() {
            tracing::debug!("SplitUntilContinuation: dropped before completion, closing queue");
            self.queue.close();
        }
    }
}

// ============================================================================
// Split Collect Until Map Combinator
// ============================================================================

/// Observer branch from `split_collect_until_map()`.
///
/// Receives transformed copies of items until the predicate is met, then the queue
/// is closed and the observer completes.
pub struct SplitUntilObserverMap<D, P> {
    /// Shared queue receiving transformed copied items from the splitter
    queue: Arc<ConcurrentQueue<Stream<D, P>>>,
}

impl<D, P> Iterator for SplitUntilObserverMap<D, P>
where
    D: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.queue.pop() {
            Ok(item) => {
                tracing::trace!("SplitUntilObserverMap: received item from queue");
                Some(item)
            }
            Err(concurrent_queue::PopError::Empty) => {
                if self.queue.is_closed() {
                    tracing::debug!("SplitUntilObserverMap: queue closed, returning None");
                    None
                } else {
                    tracing::trace!(
                        "SplitUntilObserverMap: queue empty but not closed, returning Ignore"
                    );
                    Some(Stream::Ignore)
                }
            }
            Err(concurrent_queue::PopError::Closed) => {
                tracing::debug!("SplitUntilObserverMap: queue closed, returning None");
                None
            }
        }
    }
}

impl<D, P> crate::synca::mpp::StreamIterator for SplitUntilObserverMap<D, P>
where
    D: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type D = D;
    type P = P;
}

/// Continuation branch from `split_collect_until_map()`.
///
/// Wraps the original iterator. The transform function returns
/// `(CollectionState, Option<D>)` to control both collection behavior and transformation.
/// The continuation continues with original Ready values unchanged.
pub struct SplitUntilContinuationMap<I: TaskIterator, D> {
    /// The wrapped iterator
    inner: I,
    /// Queue to send transformed copied items to observer
    queue: Arc<ConcurrentQueue<Stream<D, I::Pending>>>,
    /// Combined predicate + transform function
    transform: Box<dyn Fn(&I::Ready) -> (CollectionState, Option<D>) + Send>,
}

impl<I, D> Iterator for SplitUntilContinuationMap<I, D>
where
    I: TaskIterator,
    I::Pending: Clone,
    D: Clone + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = if let Some(item) = self.inner.next_status() {
            item
        } else {
            // Source iterator is naturally exhausted, close the queue
            self.queue.close();
            tracing::debug!("SplitUntilContinuationMap: source exhausted, queue closed");
            return None;
        };

        // Handle items based on (CollectionState, Option<D>) from transform
        if let TaskStatus::Ready(value) = &item {
            let (state, transformed) = (self.transform)(value);
            match state {
                CollectionState::Skip => {
                    // Skip this item - don't send to observer
                    tracing::trace!(
                        "SplitUntilContinuationMap: skipping item (CollectionState::Skip)"
                    );
                }
                CollectionState::Collect => {
                    // Collect this item for the observer (with transformation)
                    if let Some(transformed) = transformed {
                        let stream_item = Stream::Next(transformed);
                        if let Err(e) = self.queue.force_push(stream_item) {
                            tracing::error!(
                                "SplitUntilContinuationMap: failed to push to queue: {}",
                                e
                            );
                        } else {
                            tracing::trace!("SplitUntilContinuationMap: collected transformed item for observer");
                        }
                    }
                }
                CollectionState::Close(collect_this) => {
                    // Close the observer after optionally collecting this item
                    if collect_this {
                        if let Some(transformed) = transformed {
                            let stream_item = Stream::Next(transformed);
                            if let Err(e) = self.queue.force_push(stream_item) {
                                tracing::error!(
                                    "SplitUntilContinuationMap: failed to push to queue: {}",
                                    e
                                );
                            } else {
                                tracing::trace!("SplitUntilContinuationMap: collecting final transformed item and closing observer queue");
                            }
                        }
                    } else {
                        tracing::trace!("SplitUntilContinuationMap: closing observer queue without collecting final item");
                    }
                    self.queue.close();
                    tracing::debug!("SplitUntilContinuationMap: observer queue closed after CollectionState::Close");
                }
            }
        }

        // Always forward to continuation
        Some(item)
    }
}

impl<I, D> TaskIterator for SplitUntilContinuationMap<I, D>
where
    I: TaskIterator,
    I::Pending: Clone,
    D: Clone + Send + 'static,
{
    type Ready = I::Ready;
    type Pending = I::Pending;
    type Spawner = I::Spawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

impl<I, D> Drop for SplitUntilContinuationMap<I, D>
where
    I: TaskIterator,
{
    fn drop(&mut self) {
        // Close the queue as backup if not already closed
        if !self.queue.is_closed() {
            tracing::debug!("SplitUntilContinuationMap: dropped before completion, closing queue");
            self.queue.close();
        }
    }
}

// ============================================================================
// Split Collector Map Combinator
// ============================================================================

/// Observer branch from `split_collector_map()`.
///
/// Receives transformed copies of matched Ready items via a `ConcurrentQueue`.
/// The observer yields `Stream<M, P>` where M is the mapped type from the transform function.
pub struct SplitCollectorMapObserver<M, P> {
    /// Shared queue receiving transformed items from the continuation
    queue: Arc<ConcurrentQueue<Stream<M, P>>>,
}

impl<M, P> Iterator for SplitCollectorMapObserver<M, P>
where
    M: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type Item = Stream<M, P>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.queue.pop() {
            Ok(item) => {
                tracing::trace!("SplitCollectorMapObserver: received item from queue");
                Some(item)
            }
            Err(concurrent_queue::PopError::Empty) => {
                if self.queue.is_closed() {
                    tracing::debug!("SplitCollectorMapObserver: queue closed, returning None");
                    None
                } else {
                    tracing::trace!(
                        "SplitCollectorMapObserver: queue empty but not closed, returning Ignore"
                    );
                    Some(Stream::Ignore)
                }
            }
            Err(concurrent_queue::PopError::Closed) => {
                tracing::debug!("SplitCollectorMapObserver: queue closed, returning None");
                None
            }
        }
    }
}

impl<M, P> crate::synca::mpp::StreamIterator for SplitCollectorMapObserver<M, P>
where
    M: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type D = M;
    type P = P;
}

/// Continuation branch from `split_collector_map()`.
///
/// Wraps the original iterator. The transform function returns `(bool, Option<M>)`:
/// - `true` + `Some(m)` sends `m` to the observer queue
/// - `false` or `None` skips sending to observer
/// The continuation continues with original Ready values unchanged.
pub struct SplitCollectorMapContinuation<I: TaskIterator, M> {
    /// The wrapped iterator
    inner: I,
    /// Queue to send transformed items to observer
    queue: Arc<ConcurrentQueue<Stream<M, I::Pending>>>,
    /// Combined predicate + transform function
    transform: Box<dyn Fn(&I::Ready) -> (bool, Option<M>) + Send>,
}

impl<I, M> Iterator for SplitCollectorMapContinuation<I, M>
where
    I: TaskIterator,
    I::Pending: Clone,
    M: Clone + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = if let Some(item) = self.inner.next_status() {
            item
        } else {
            self.queue.close();
            tracing::debug!("SplitCollectorMapContinuation: source exhausted, queue closed");
            return None;
        };

        if let TaskStatus::Ready(value) = &item {
            let (matched, transformed) = (self.transform)(value);
            if matched {
                if let Some(transformed) = transformed {
                    let stream_item = Stream::Next(transformed);
                    if let Err(e) = self.queue.force_push(stream_item) {
                        tracing::error!(
                            "SplitCollectorMapContinuation: failed to push to queue: {}",
                            e
                        );
                    } else {
                        tracing::trace!(
                            "SplitCollectorMapContinuation: copied transformed item to observer queue"
                        );
                    }
                }
            }
        }

        Some(item)
    }
}

impl<I, M> TaskIterator for SplitCollectorMapContinuation<I, M>
where
    I: TaskIterator,
    I::Pending: Clone,
    M: Clone + Send + 'static,
{
    type Ready = I::Ready;
    type Pending = I::Pending;
    type Spawner = I::Spawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

impl<I, M> Drop for SplitCollectorMapContinuation<I, M>
where
    I: TaskIterator,
{
    fn drop(&mut self) {
        if !self.queue.is_closed() {
            self.queue.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple test task iterator for unit tests
    struct TestTask {
        items: Vec<TaskStatus<u32, String, crate::valtron::NoAction>>,
    }

    impl TestTask {
        fn new(items: Vec<TaskStatus<u32, String, crate::valtron::NoAction>>) -> Self {
            Self { items }
        }
    }

    impl Iterator for TestTask {
        type Item = TaskStatus<u32, String, crate::valtron::NoAction>;

        fn next(&mut self) -> Option<Self::Item> {
            self.items.pop()
        }
    }

    impl TaskIterator for TestTask {
        type Ready = u32;
        type Pending = String;
        type Spawner = crate::valtron::NoAction;

        fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
            Iterator::next(self)
        }
    }

    #[test]
    fn test_map_ready() {
        let items = vec![
            TaskStatus::Pending("wait".to_string()),
            TaskStatus::Ready(10),
            TaskStatus::Ready(5),
        ];
        let task = TestTask::new(items);
        let mut mapped = task.map_ready(|x| x * 2);

        // pop() returns items in reverse order: 5, 10, Pending
        // After mapping: 10 (5*2), 20 (10*2), Pending
        assert_eq!(Iterator::next(&mut mapped), Some(TaskStatus::Ready(10)));
        assert_eq!(Iterator::next(&mut mapped), Some(TaskStatus::Ready(20)));
        assert_eq!(
            Iterator::next(&mut mapped),
            Some(TaskStatus::Pending("wait".to_string()))
        );
    }

    #[test]
    fn test_map_pending() {
        let items = vec![
            TaskStatus::Pending("wait".to_string()),
            TaskStatus::Ready(5),
        ];
        let task = TestTask::new(items);
        let mut mapped = task.map_pending(|s| s.len());

        assert_eq!(Iterator::next(&mut mapped), Some(TaskStatus::Ready(5)));
        assert_eq!(Iterator::next(&mut mapped), Some(TaskStatus::Pending(4)));
    }

    #[test]
    fn test_filter_ready() {
        let items = vec![
            TaskStatus::Ready(3),
            TaskStatus::Ready(10),
            TaskStatus::Ready(5),
        ];
        let task = TestTask::new(items);
        let mut filtered = task.filter_ready(|x| *x > 5);

        // pop() returns: Ready(5), Ready(10), Ready(3)
        // filter: 5 > 5 = false → Ignore, 10 > 5 = true → Ready(10), 3 > 5 = false → Ignore
        assert_eq!(Iterator::next(&mut filtered), Some(TaskStatus::Ignore)); // 5 was filtered out
        assert_eq!(Iterator::next(&mut filtered), Some(TaskStatus::Ready(10)));
        assert_eq!(Iterator::next(&mut filtered), Some(TaskStatus::Ignore)); // 3 was filtered out
        assert_eq!(Iterator::next(&mut filtered), None);
    }

    #[test]
    fn test_split_collector_map_sends_transformed_items() {
        let items = vec![
            TaskStatus::Pending("wait".to_string()),
            TaskStatus::Ready(10),
            TaskStatus::Ready(3),
            TaskStatus::Ready(20),
        ];
        let task = TestTask::new(items);

        // Observer gets string representations of items > 5
        let (observer, mut continuation) =
            task.split_collector_map(|x| (*x > 5, Some(format!("val:{}", x))), 10);

        // Drive the continuation to completion
        while Iterator::next(&mut continuation).is_some() {}

        // Observer should have transformed items
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
        let items = vec![TaskStatus::Ready(10), TaskStatus::Ready(5)];
        let task = TestTask::new(items);

        // Matched but transform returns None for odd numbers → skipped
        let (observer, mut continuation) = task.split_collector_map(
            |x| (true, if x % 2 == 0 { Some(*x as u64) } else { None }),
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
        let items = vec![
            TaskStatus::Ready(3),
            TaskStatus::Ready(10),
            TaskStatus::Ready(20),
        ];
        let task = TestTask::new(items);

        let (observer, mut continuation) =
            task.split_collect_one_map(|x| (*x > 5, Some(x.to_string())));

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
    fn test_stream_collect() {
        let items = vec![
            TaskStatus::Ready(2),
            TaskStatus::Pending("wait".to_string()),
            TaskStatus::Ready(1),
        ];
        let task = TestTask::new(items);
        let mut collected = task.stream_collect();

        // First Ready is collected, returns Ignore
        assert_eq!(Iterator::next(&mut collected), Some(TaskStatus::Ignore));

        // Should pass through Pending
        assert_eq!(
            Iterator::next(&mut collected),
            Some(TaskStatus::Pending("wait".to_string()))
        );

        // Second Ready is collected, returns Ignore
        assert_eq!(Iterator::next(&mut collected), Some(TaskStatus::Ignore));

        // Should yield collected Vec at the end
        match Iterator::next(&mut collected) {
            Some(TaskStatus::Ready(vec)) => {
                assert_eq!(vec.len(), 2);
                assert!(vec.contains(&1));
                assert!(vec.contains(&2));
            }
            other => panic!("Expected Ready(Vec), got {:?}", other),
        }

        // Should be done after yielding the collected result
        assert_eq!(Iterator::next(&mut collected), None);
    }

    #[test]
    fn test_map_iter_flattens_nested_iterators() {
        use crate::valtron::task::NoAction;

        // Test using the extension method with a simple test task
        struct VecTask {
            items: std::vec::IntoIter<TaskStatus<Vec<u32>, String, NoAction>>,
        }

        impl Iterator for VecTask {
            type Item = TaskStatus<Vec<u32>, String, NoAction>;

            fn next(&mut self) -> Option<Self::Item> {
                self.items.next()
            }
        }

        impl TaskIterator for VecTask {
            type Ready = Vec<u32>;
            type Pending = String;
            type Spawner = NoAction;

            fn next_status(
                &mut self,
            ) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
                Iterator::next(self)
            }
        }

        let items = vec![
            TaskStatus::Ready(vec![1u32, 2u32]),
            TaskStatus::Ready(vec![3u32, 4u32, 5u32]),
        ];
        let task = VecTask {
            items: items.into_iter(),
        };

        let mut flattened: TMapIter<_, _, _, u32, String, NoAction, Vec<u32>, String, NoAction> =
            task.map_iter(|vec| {
                vec.into_iter()
                    .map(TaskStatus::Ready)
                    .collect::<Vec<_>>()
                    .into_iter()
            });

        // Should flatten: 1, 2, 3, 4, 5
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(TaskStatus::Ready(1))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(TaskStatus::Ready(2))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(TaskStatus::Ready(3))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(TaskStatus::Ready(4))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(TaskStatus::Ready(5))
        ));
        assert_eq!(Iterator::next(&mut flattened), None);
    }

    #[test]
    fn test_map_iter_passes_through_pending() {
        use crate::valtron::task::NoAction;

        // Test using the extension method with a simple test task
        struct VecTask {
            items: std::vec::IntoIter<TaskStatus<Vec<u32>, String, NoAction>>,
        }

        impl Iterator for VecTask {
            type Item = TaskStatus<Vec<u32>, String, NoAction>;

            fn next(&mut self) -> Option<Self::Item> {
                self.items.next()
            }
        }

        impl TaskIterator for VecTask {
            type Ready = Vec<u32>;
            type Pending = String;
            type Spawner = NoAction;

            fn next_status(
                &mut self,
            ) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
                Iterator::next(self)
            }
        }

        let items = vec![
            TaskStatus::Ready(vec![1u32]),
            TaskStatus::Pending("wait".to_string()),
            TaskStatus::Ready(vec![2u32, 3u32]),
        ];
        let task = VecTask {
            items: items.into_iter(),
        };

        let mut flattened: TMapIter<_, _, _, u32, String, NoAction, Vec<u32>, String, NoAction> =
            task.map_iter(|vec| {
                vec.into_iter()
                    .map(TaskStatus::Ready)
                    .collect::<Vec<_>>()
                    .into_iter()
            });

        // Should flatten with pending passed through
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(TaskStatus::Ready(1))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(TaskStatus::Pending(ref s)) if s == "wait"
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(TaskStatus::Ready(2))
        ));
        assert!(matches!(
            Iterator::next(&mut flattened),
            Some(TaskStatus::Ready(3))
        ));
        assert_eq!(Iterator::next(&mut flattened), None);
    }
}
