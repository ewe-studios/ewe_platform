#![cfg(feature = "multi")]

use concurrent_queue::ConcurrentQueue;

use std::sync::Arc;

use crate::valtron::branches::CollectionState;
use crate::valtron::{
    ExecutionAction, QueueReadiness, QueueVacancyReadiness, Stream, TaskIterator, TaskShortCircuit,
    TaskSpread, TaskStatus,
};

/// Extension trait providing builder-style combinator methods for any `TaskIterator`.
///
/// This trait is automatically implemented for any type that implements `TaskIterator`
/// with the appropriate bounds. This includes:
/// - Raw task iterators implementing `TaskIterator`
/// - Driven iterators like `DrivenRecvIterator` and `DrivenTaskIterator`
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

    /// Fan each matched `Ready` value out to **N** observer branches (each gets a
    /// clone), returning the `N` observers plus a continuation carrying the full
    /// stream onward (F45 N-way broadcast follow-on).
    ///
    /// ## Semantics — zero loss, slowest-gates lockstep
    ///
    /// A matched value is delivered to **every** open branch. Backpressure composes
    /// across all N: if **any** branch's queue is full the source parks on that
    /// branch's [`QueueVacancyReadiness`] and delivers to no branch until **all**
    /// have vacancy — so the slowest consumer gates the rest and **no value is
    /// dropped**. `queue_size = 1` is strict lockstep; larger gives buffered slack.
    /// A **dropped** observer closes its queue; the source stops copying to it but
    /// keeps forwarding to the survivors (never a deadlock).
    ///
    /// ## Type Requirements
    /// - `Ready` must be `Clone` (each branch gets a copy).
    /// - `Pending` must be `Clone` (branch element type is `Stream<Ready, Pending>`).
    ///
    /// ## Panics
    /// Never panics. (`queue_size` is clamped to at least 1.)
    fn broadcast<P>(
        self,
        n: usize,
        predicate: P,
        queue_size: usize,
    ) -> (
        Vec<CollectorStreamIterator<Self::Ready, Self::Pending>>,
        BroadcastContinuation<Self>,
    )
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> bool + Send + 'static;

    /// Like [`broadcast`](Self::broadcast) but a **full branch loses the item**
    /// instead of backpressuring the source (F45 N-way broadcast follow-on).
    ///
    /// This is the **loud, explicit** lossy opt-in: the source never stalls on a
    /// slow observer, so a branch that cannot keep up will **miss values**
    /// (drop-oldest per branch). Use only when staleness is preferable to
    /// backpressure (e.g. a best-effort metrics/observability tap). For zero-loss
    /// delivery use [`broadcast`](Self::broadcast).
    ///
    /// ## Panics
    /// Never panics. (`queue_size` is clamped to at least 1.)
    fn broadcast_lossy<P>(
        self,
        n: usize,
        predicate: P,
        queue_size: usize,
    ) -> (
        Vec<CollectorStreamIterator<Self::Ready, Self::Pending>>,
        BroadcastContinuation<Self>,
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
    /// * `transform` - Function returning `(CollectionState, Option<D>)` to control collection and transform
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
    /// * `transform` - Function returning `(bool, Option<M>)` to control matching and transform
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

    /// Short-circuit the iterator based on a circuit function.
    ///
    /// The circuit function receives each item and returns a `TaskShortCircuit` enum
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
    /// let task = my_task
    ///     .map_circuit(|status| {
    ///         match status {
    ///             TaskStatus::Ready(err) if err.is_error() => {
    ///                 TaskShortCircuit::ReturnAndStop(TaskStatus::Ready(err))
    ///             }
    ///             TaskStatus::Pending(_) | TaskStatus::Ready(_) => {
    ///                 TaskShortCircuit::Continue(status)
    ///             }
    ///             _ => TaskShortCircuit::Stop,
    ///         }
    ///     });
    /// ```
    fn map_circuit<F>(self, f: F) -> TMapCircuit<Self>
    where
        Self: Sized,
        F: Fn(
                TaskStatus<Self::Ready, Self::Pending, Self::Spawner>,
            ) -> TaskShortCircuit<Self::Ready, Self::Pending, Self::Spawner>
            + Send
            + 'static;

    /// Returns the first `Ready` value from the iterator, short-circuiting after finding it.
    ///
    /// Uses `map_circuit` internally to stop iteration after the first `Ready` value.
    /// Returns `None` if no `Ready` value is found before the iterator is exhausted.
    fn first_ready(self) -> Option<Self::Ready>
    where
        Self: Sized + Send + 'static,
        Self::Ready: Send + 'static,
        Self::Pending: Send + 'static,
        Self::Spawner: ExecutionAction + Send + 'static,
    {
        self.map_circuit(|status| match status {
            TaskStatus::Ready(value) => TaskShortCircuit::ReturnAndStop(TaskStatus::Ready(value)),
            _ => TaskShortCircuit::Continue(status),
        })
        .next()
        .and_then(|status| match status {
            TaskStatus::Ready(value) => Some(value),
            _ => None,
        })
    }

    /// Returns the first `Pending` value from the iterator, short-circuiting after finding it.
    ///
    /// Uses `map_circuit` internally to stop iteration after the first `Pending` value.
    /// Returns `None` if no `Pending` value is found before the iterator is exhausted.
    fn first_pending(self) -> Option<Self::Pending>
    where
        Self: Sized + Send + 'static,
        Self::Ready: Send + 'static,
        Self::Pending: Send + 'static,
        Self::Spawner: ExecutionAction + Send + 'static,
    {
        self.map_circuit(|status| match status {
            TaskStatus::Pending(value) => {
                TaskShortCircuit::ReturnAndStop(TaskStatus::Pending(value))
            }
            _ => TaskShortCircuit::Continue(status),
        })
        .next()
        .and_then(|status| match status {
            TaskStatus::Pending(value) => Some(value),
            _ => None,
        })
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

    /// Flatten Ready values that implement `IntoIterator`.
    ///
    /// Input:  `TaskIterator`<Ready = `Vec<M>`, Pending = P, Spawner = S>
    /// Output: `TaskIterator`<Ready = M, Pending = P, Spawner = S>
    fn flatten_ready(self) -> TFlattenReady<Self>
    where
        Self: Sized,
        Self::Ready: IntoIterator,
        <Self::Ready as IntoIterator>::Item: Send + 'static;

    /// Flatten Pending values that implement `IntoIterator`.
    ///
    /// Input:  `TaskIterator`<Ready = D, Pending = `Vec<M>`, Spawner = S>
    /// Output: `TaskIterator`<Ready = D, Pending = M, Spawner = S>
    fn flatten_pending(self) -> TFlattenPending<Self>
    where
        Self: Sized,
        Self::Pending: IntoIterator,
        <Self::Pending as IntoIterator>::Item: Send + 'static;

    /// Flat map Ready values - transform and flatten in one operation.
    ///
    /// Input:  `TaskIterator`<Ready = D, Pending = P, Spawner = S>
    /// Output: `TaskIterator`<Ready = `U::Item`, Pending = P, Spawner = S>
    fn flat_map_ready<F, U>(self, f: F) -> TFlatMapReady<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::Ready) -> U + Send + 'static,
        U: IntoIterator,
        U::Item: Send + 'static;

    /// Flat map Pending values - transform and flatten in one operation.
    ///
    /// Input:  `TaskIterator`<Ready = D, Pending = P, Spawner = S>
    /// Output: `TaskIterator`<Ready = D, Pending = `U::Item`, Spawner = S>
    fn flat_map_pending<F, U>(self, f: F) -> TFlatMapPending<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::Pending) -> U + Send + 'static,
        U: IntoIterator,
        U::Item: Send + 'static;

    // ===== Feature 08: Iterator Extension Completion =====

    /// Transform any `TaskStatus` with full state access.
    fn map_state<F>(self, f: F) -> TMapState<Self, F>
    where
        F: Fn(
                TaskStatus<Self::Ready, Self::Pending, Self::Spawner>,
            ) -> TaskStatus<Self::Ready, Self::Pending, Self::Spawner>
            + Send
            + 'static;

    /// Side-effect on any `TaskStatus`.
    fn inspect_state<F>(self, f: F) -> TInspectState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) + Send + 'static;

    /// Filter based on full `TaskStatus`. Non-matching items return `TaskStatus::Ignore`.
    fn filter_state<F>(self, f: F) -> TFilterState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

    /// Take items while state predicate returns true.
    fn take_while_state<F>(self, predicate: F) -> TTakeWhileState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

    /// Transform items until transformer returns None.
    /// When transformer returns None, iterator terminates.
    fn transform_until<F>(self, transformer: F) -> TTransformUntil<Self, F>
    where
        F: Fn(
                TaskStatus<Self::Ready, Self::Pending, Self::Spawner>,
            ) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>>
            + Send
            + 'static;

    /// Skip items while state predicate returns true.
    fn skip_while_state<F>(self, predicate: F) -> TSkipWhileState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

    /// Take at most n items matching state predicate.
    fn take_state<F>(self, n: usize, state_predicate: F) -> TTakeState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

    /// Skip first n items matching state predicate.
    fn skip_state<F>(self, n: usize, state_predicate: F) -> TSkipState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

    /// Take while predicate true on Ready values, pass through non-Ready.
    fn take<F>(
        self,
        n: usize,
    ) -> TTakeState<Self, impl Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool>
    where
        Self::Ready: Send + 'static,
    {
        self.take_state(n, |s| matches!(s, TaskStatus::Ready(_)))
    }

    /// Take at most n items of any state.
    fn take_all(
        self,
        n: usize,
    ) -> TTakeState<Self, impl Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool>
    {
        self.take_state(n, |_| true)
    }

    /// Skip first n Ready items, return all others unchanged.
    fn skip(
        self,
        n: usize,
    ) -> TSkipState<Self, impl Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool>
    where
        Self::Ready: Send + 'static,
    {
        self.skip_state(n, |s| matches!(s, TaskStatus::Ready(_)))
    }

    /// Skip first n items of any state.
    fn skip_all(
        self,
        n: usize,
    ) -> TSkipState<Self, impl Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool>
    {
        self.skip_state(n, |_| true)
    }

    /// Take while predicate true on Ready values, pass through non-Ready.
    fn take_while<F>(
        self,
        f: F,
    ) -> TTakeWhileState<
        Self,
        impl Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool,
    >
    where
        F: Fn(&Self::Ready) -> bool + Send + 'static,
    {
        self.take_while_state(move |s| match s {
            TaskStatus::Ready(v) => f(v),
            _ => true,
        })
    }

    /// Take while predicate true on ANY state.
    fn take_while_any<F>(self, f: F) -> TTakeWhileState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
    {
        self.take_while_state(f)
    }

    /// Skip while predicate true on Ready values, return all others.
    fn skip_while<F>(
        self,
        f: F,
    ) -> TSkipWhileState<
        Self,
        impl Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool,
    >
    where
        F: Fn(&Self::Ready) -> bool + Send + 'static,
    {
        self.skip_while_state(move |s| match s {
            TaskStatus::Ready(v) => f(v),
            _ => false,
        })
    }

    /// Skip while predicate true on ANY state.
    fn skip_while_any<F>(self, f: F) -> TSkipWhileState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
    {
        self.skip_while_state(f)
    }

    /// Add index to Ready items, changing Ready type from D to (usize, D).
    fn enumerate(self) -> TEnumerate<Self> {
        TEnumerate {
            inner: self,
            count: 0,
        }
    }

    /// Find first item matching predicate.
    fn find<F>(self, predicate: F) -> TFind<Self, F>
    where
        F: Fn(&Self::Ready) -> bool + Send + 'static;

    /// Find first item mapping to Some value.
    fn find_map<F, R>(self, f: F) -> TFindMap<Self, F, R>
    where
        F: Fn(Self::Ready) -> Option<R> + Send + 'static,
        R: Send + 'static;

    /// Fold/accumulate values. Returns final accumulator when done.
    fn fold<F, R>(self, init: R, f: F) -> TFold<Self, F, R>
    where
        F: Fn(R, Self::Ready) -> R + Send + 'static,
        R: Send + 'static;

    /// Check if all Ready items satisfy predicate.
    fn all<F>(self, f: F) -> TAll<Self, F>
    where
        F: Fn(Self::Ready) -> bool + Send + 'static;

    /// Check if any Ready item satisfies predicate.
    fn any<F>(self, f: F) -> TAny<Self, F>
    where
        F: Fn(Self::Ready) -> bool + Send + 'static;

    /// Count Ready items.
    fn count(self) -> TCount<Self>;

    /// Count all items (any state).
    fn count_all(self) -> TCountAll<Self>;

    /// Wrap the task in a [`TaskIter`](crate::valtron::TaskIter) — the valtron-native
    /// entry point that removes standard `Iterator` method ambiguity.
    fn into_task_iter(self) -> crate::valtron::TaskIter<Self>
    where
        Self: Sized,
    {
        crate::valtron::TaskIter(self)
    }

    /// Transform Ready values, terminating the task when the closure returns `None`.
    ///
    /// `Some(R)` → `Ready(R)`, `None` → task exhausts.
    fn try_map_ready<F, R>(self, f: F) -> TTryMapReady<Self, R>
    where
        F: Fn(Self::Ready) -> Option<R> + Send + 'static,
        R: Send + 'static;

    /// Sync-boundary collector: drain the task to completion and return every
    /// `Ready` value as a `Vec`. Non-`Ready` states are discarded.
    fn collect_ready(self) -> Vec<Self::Ready>;

    /// Sync-boundary collector: drain the task until the first `Ready`, returning
    /// it. Returns `None` if the task exhausts without producing a `Ready`.
    fn collect_one_ready(self) -> Option<Self::Ready>;
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
            pending_push: None,
            pending_forward: None,
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

    fn broadcast<P>(
        self,
        n: usize,
        predicate: P,
        queue_size: usize,
    ) -> (
        Vec<CollectorStreamIterator<Self::Ready, Self::Pending>>,
        BroadcastContinuation<Self>,
    )
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> bool + Send + 'static,
    {
        broadcast_build(self, n, Box::new(predicate), queue_size, false)
    }

    fn broadcast_lossy<P>(
        self,
        n: usize,
        predicate: P,
        queue_size: usize,
    ) -> (
        Vec<CollectorStreamIterator<Self::Ready, Self::Pending>>,
        BroadcastContinuation<Self>,
    )
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> bool + Send + 'static,
    {
        broadcast_build(self, n, Box::new(predicate), queue_size, true)
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
            pending_push: None,
            pending_forward: None,
            pending_close: false,
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
            pending_push: None,
            pending_forward: None,
            pending_close: false,
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
            pending_push: None,
            pending_forward: None,
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

    fn map_circuit<F>(self, f: F) -> TMapCircuit<Self>
    where
        Self: Sized,
        F: Fn(
                TaskStatus<Self::Ready, Self::Pending, Self::Spawner>,
            ) -> TaskShortCircuit<Self::Ready, Self::Pending, Self::Spawner>
            + Send
            + 'static,
    {
        TMapCircuit {
            inner: self,
            circuit: Box::new(f),
            stopped: false,
            _phantom: std::marker::PhantomData,
        }
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

    // ===== Feature 08 implementations =====

    fn map_state<F>(self, f: F) -> TMapState<Self, F>
    where
        F: Fn(
                TaskStatus<Self::Ready, Self::Pending, Self::Spawner>,
            ) -> TaskStatus<Self::Ready, Self::Pending, Self::Spawner>
            + Send
            + 'static,
    {
        TMapState {
            inner: self,
            mapper: f,
        }
    }

    fn inspect_state<F>(self, f: F) -> TInspectState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) + Send + 'static,
    {
        TInspectState {
            inner: self,
            inspector: f,
        }
    }

    fn filter_state<F>(self, f: F) -> TFilterState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
    {
        TFilterState {
            inner: self,
            predicate: f,
        }
    }

    fn take_while_state<F>(self, predicate: F) -> TTakeWhileState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
    {
        TTakeWhileState {
            inner: self,
            predicate,
            done: false,
        }
    }

    fn transform_until<F>(self, transformer: F) -> TTransformUntil<Self, F>
    where
        F: Fn(
                TaskStatus<Self::Ready, Self::Pending, Self::Spawner>,
            ) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>>
            + Send
            + 'static,
    {
        TTransformUntil {
            inner: self,
            transformer,
            done: false,
        }
    }

    fn skip_while_state<F>(self, predicate: F) -> TSkipWhileState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
    {
        TSkipWhileState {
            inner: self,
            predicate,
            done_skipping: false,
        }
    }

    fn take_state<F>(self, n: usize, state_predicate: F) -> TTakeState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
    {
        TTakeState {
            inner: self,
            remaining: n,
            state_predicate,
        }
    }

    fn skip_state<F>(self, n: usize, state_predicate: F) -> TSkipState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
    {
        TSkipState {
            inner: self,
            to_skip: n,
            state_predicate,
        }
    }

    fn find<F>(self, predicate: F) -> TFind<Self, F>
    where
        F: Fn(&Self::Ready) -> bool + Send + 'static,
    {
        TFind {
            inner: self,
            predicate,
            found: false,
        }
    }

    fn find_map<F, R>(self, f: F) -> TFindMap<Self, F, R>
    where
        F: Fn(Self::Ready) -> Option<R> + Send + 'static,
        R: Send + 'static,
    {
        TFindMap {
            inner: self,
            mapper: f,
            found: false,
            _phantom: std::marker::PhantomData,
        }
    }

    fn fold<F, R>(self, init: R, f: F) -> TFold<Self, F, R>
    where
        F: Fn(R, Self::Ready) -> R + Send + 'static,
        R: Send + 'static,
    {
        TFold {
            inner: self,
            acc: Some(init),
            folder: f,
            done: false,
            _phantom: std::marker::PhantomData,
        }
    }

    fn all<F>(self, f: F) -> TAll<Self, F>
    where
        F: Fn(Self::Ready) -> bool + Send + 'static,
    {
        TAll {
            inner: self,
            predicate: f,
            all_true: true,
            done: false,
        }
    }

    fn any<F>(self, f: F) -> TAny<Self, F>
    where
        F: Fn(Self::Ready) -> bool + Send + 'static,
    {
        TAny {
            inner: self,
            predicate: f,
            any_true: false,
            done: false,
        }
    }

    fn count(self) -> TCount<Self> {
        TCount {
            inner: self,
            count: 0,
        }
    }

    fn count_all(self) -> TCountAll<Self> {
        TCountAll {
            inner: self,
            count: 0,
            done: false,
        }
    }

    fn try_map_ready<F, R>(self, f: F) -> TTryMapReady<Self, R>
    where
        F: Fn(Self::Ready) -> Option<R> + Send + 'static,
        R: Send + 'static,
    {
        TTryMapReady {
            inner: self,
            mapper: Box::new(f),
            done: false,
        }
    }

    fn collect_ready(self) -> Vec<Self::Ready> {
        let mut out = Vec::new();
        let mut inner = self.into_task_iter();
        while let Some(status) = inner.next_status() {
            if let TaskStatus::Ready(v) = status {
                out.push(v);
            }
        }
        out
    }

    fn collect_one_ready(self) -> Option<Self::Ready> {
        let mut inner = self.into_task_iter();
        while let Some(status) = inner.next_status() {
            if let TaskStatus::Ready(v) = status {
                return Some(v);
            }
        }
        None
    }
}

// ============================================================================
// TTryMapReady — Ready → Option<R>, exhausts on None
// ============================================================================

/// Wrapper that maps Ready values through `Fn(Ready) -> Option<R>`.  When the
/// closure returns `None`, the task terminates (exhausts).  `Some(R)`
/// becomes `TaskStatus::Ready(R)`.
///
/// For `TaskStatus::Spread`, each `Ready` is mapped individually.  If any
/// maps to `None`, the task exhausts on the next `next_status()` call
/// (the current Spread is emitted with the successfully-mapped items).
pub struct TTryMapReady<I: TaskIterator, R> {
    inner: I,
    mapper: Box<dyn Fn(I::Ready) -> Option<R> + Send>,
    done: bool,
}

impl<I, R> Iterator for TTryMapReady<I, R>
where
    I: TaskIterator,
    R: Send + 'static,
{
    type Item = TaskStatus<R, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let status = self.inner.next_status()?;
        Some(match status {
            TaskStatus::Ready(v) => match (self.mapper)(v) {
                Some(r) => TaskStatus::Ready(r),
                None => {
                    self.done = true;
                    return None;
                }
            },
            TaskStatus::Spread(items) => {
                let mut any_none = false;
                let mapped: Vec<_> = items
                    .into_iter()
                    .filter_map(|item| match item {
                        TaskSpread::Ready(d) => match (self.mapper)(d) {
                            Some(r) => Some(TaskSpread::Ready(r)),
                            None => {
                                any_none = true;
                                None
                            }
                        },
                        TaskSpread::Pending(p) => Some(TaskSpread::Pending(p)),
                    })
                    .collect();
                if any_none {
                    self.done = true;
                }
                TaskStatus::Spread(mapped)
            },
            TaskStatus::Pending(v) => TaskStatus::Pending(v),
            TaskStatus::Delayed(d) => TaskStatus::Delayed(d),
            TaskStatus::Ignore => TaskStatus::Ignore,
            TaskStatus::Init => TaskStatus::Init,
            TaskStatus::Spawn(s) => TaskStatus::Spawn(s),
            TaskStatus::Wait => TaskStatus::Wait,
            TaskStatus::Depends(signal) => TaskStatus::Depends(signal),
        })
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
            TaskStatus::Wait => TaskStatus::Wait,
            TaskStatus::Depends(signal) => TaskStatus::Depends(signal),
            TaskStatus::Spread(items) => TaskStatus::Spread(
                items
                    .into_iter()
                    .map(|item| match item {
                        TaskSpread::Ready(d) => TaskSpread::Ready((self.mapper)(d)),
                        TaskSpread::Pending(p) => TaskSpread::Pending(p),
                    })
                    .collect(),
            ),
        })
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
            TaskStatus::Wait => TaskStatus::Wait,
            TaskStatus::Depends(signal) => TaskStatus::Depends(signal),
            TaskStatus::Spread(items) => TaskStatus::Spread(
                items
                    .into_iter()
                    .map(|item| match item {
                        TaskSpread::Ready(d) => TaskSpread::Ready(d),
                        TaskSpread::Pending(p) => TaskSpread::Pending((self.mapper)(p)),
                    })
                    .collect(),
            ),
        })
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
                Some(TaskStatus::Wait) => return Some(TaskStatus::Wait),
                Some(TaskStatus::Depends(signal)) => return Some(TaskStatus::Depends(signal)),
                Some(TaskStatus::Spread(items)) => {
                    // Separate Pending items (convert via Into) from Ready items
                    let mut pending_items = Vec::new();
                    let mut has_ready = false;
                    for item in items {
                        match item {
                            TaskSpread::Ready(d) => {
                                // Map through mapper to get inner iterators
                                let _iter: InnerIter = (self.mapper)(d);
                                has_ready = true;
                                // Complex to chain multiple inner iterators here;
                                // treat as Ignore for now
                            }
                            TaskSpread::Pending(p) => {
                                pending_items.push(p.into());
                            }
                        }
                    }
                    if !pending_items.is_empty() {
                        return Some(TaskStatus::Spread(
                            pending_items
                                .into_iter()
                                .map(TaskSpread::Pending)
                                .collect(),
                        ));
                    }
                    if has_ready {
                        return Some(TaskStatus::Ignore);
                    }
                    // Empty spread
                    return Some(TaskStatus::Spread(Vec::new()));
                }
                None => return None, // Outer exhausted
            }
        }
    }
}

// impl<I, F, InnerIter, InnerR, InnerP, InnerS, R, P, S> TaskIterator
//     for TMapIter<I, F, InnerIter, InnerR, InnerP, InnerS, R, P, S>
// where
//     I: TaskIterator<Ready = R, Pending = P, Spawner = S>,
//     F: Fn(R) -> InnerIter,
//     InnerIter: Iterator<Item = TaskStatus<InnerR, InnerP, InnerS>>,
//     InnerR: Send + 'static,
//     InnerP: Send + 'static,
//     InnerS: ExecutionAction + Send + 'static,
//     P: Into<InnerP> + Send + 'static,
//     S: Into<InnerS> + ExecutionAction + Send + 'static,
// {
//     type Ready = InnerR;
//     type Pending = InnerP;
//     type Spawner = InnerS;

//     fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
//         Iterator::next(self)
//     }
// }

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
            TaskStatus::Wait => Some(TaskStatus::Wait),
            TaskStatus::Depends(signal) => Some(TaskStatus::Depends(signal)),
            TaskStatus::Spread(items) => {
                // Flatten Ready items, pass through Pending
                Some(TaskStatus::Spread(
                    items
                        .into_iter()
                        .flat_map(|item| match item {
                            TaskSpread::Ready(iterable) => {
                                iterable.into_iter().map(TaskSpread::Ready).collect::<Vec<_>>()
                            }
                            TaskSpread::Pending(p) => vec![TaskSpread::Pending(p)],
                        })
                        .collect(),
                ))
            }
        }
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
            TaskStatus::Wait => Some(TaskStatus::Wait),
            TaskStatus::Depends(signal) => Some(TaskStatus::Depends(signal)),
            TaskStatus::Spread(items) => {
                // Flatten Pending items, pass through Ready
                Some(TaskStatus::Spread(
                    items
                        .into_iter()
                        .flat_map(|item| match item {
                            TaskSpread::Ready(d) => vec![TaskSpread::Ready(d)],
                            TaskSpread::Pending(iterable) => {
                                iterable.into_iter().map(TaskSpread::Pending).collect::<Vec<_>>()
                            }
                        })
                        .collect(),
                ))
            }
        }
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
            TaskStatus::Wait => Some(TaskStatus::Wait),
            TaskStatus::Depends(signal) => Some(TaskStatus::Depends(signal)),
            TaskStatus::Spread(items) => {
                // Flat map Ready items, pass through Pending
                Some(TaskStatus::Spread(
                    items
                        .into_iter()
                        .flat_map(|item| match item {
                            TaskSpread::Ready(v) => (self.mapper)(v)
                                .into_iter()
                                .map(TaskSpread::Ready)
                                .collect::<Vec<_>>(),
                            TaskSpread::Pending(p) => vec![TaskSpread::Pending(p)],
                        })
                        .collect(),
                ))
            }
        }
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
            TaskStatus::Wait => Some(TaskStatus::Wait),
            TaskStatus::Depends(signal) => Some(TaskStatus::Depends(signal)),
            TaskStatus::Spread(items) => {
                // Flat map Pending items, pass through Ready
                Some(TaskStatus::Spread(
                    items
                        .into_iter()
                        .flat_map(|item| match item {
                            TaskSpread::Ready(d) => vec![TaskSpread::Ready(d)],
                            TaskSpread::Pending(v) => (self.mapper)(v)
                                .into_iter()
                                .map(TaskSpread::Pending)
                                .collect::<Vec<_>>(),
                        })
                        .collect(),
                ))
            }
        }
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
            Some(TaskStatus::Wait) => Some(TaskStatus::Wait),
            Some(TaskStatus::Depends(signal)) => Some(TaskStatus::Depends(signal)),
            Some(TaskStatus::Spread(items)) => {
                // Extend collected with Ready items, pass through Pending
                let mut pending = Vec::new();
                for item in items {
                    match item {
                        TaskSpread::Ready(d) => self.collected.push(d),
                        TaskSpread::Pending(p) => pending.push(p),
                    }
                }
                if pending.is_empty() {
                    Some(TaskStatus::Ignore)
                } else {
                    Some(TaskStatus::Spread(
                        pending.into_iter().map(TaskSpread::Pending).collect(),
                    ))
                }
            }
            None => {
                // Inner iterator is done, yield the collected result
                self.done = true;
                let collected = std::mem::take(&mut self.collected);
                Some(TaskStatus::Ready(collected))
            }
        }
    }
}

// ============================================================================
// Split Collector Combinators (Feature 07)
// ============================================================================

/// WHY: A split continuation must never drop a matched item on a full observer
/// queue (Decision 00 §L1b: no `force_push` drop, no bare busy-spin). It also
/// must not stall the whole stream when the observer branch has simply been
/// dropped.
///
/// WHAT: Attempt to hand `item` to a split observer `queue`, reporting back
/// whether the source has to park on the queue's vacancy.
///
/// HOW: On `Ok` the item is delivered; on `Closed` the observer is gone so we
/// stop copying but let the source keep forwarding (returns `None` — proceed);
/// on `Full` the item is returned in `Some(..)` so the caller can stash it and
/// park the source on [`QueueVacancyReadiness`].
///
/// # Panics
/// Never panics.
#[inline]
fn split_observer_push<T>(queue: &ConcurrentQueue<T>, item: T) -> Option<T> {
    match queue.push(item) {
        Ok(()) => None,
        Err(concurrent_queue::PushError::Full(item)) => Some(item),
        Err(concurrent_queue::PushError::Closed(_)) => None,
    }
}

/// Observer branch from `split_collector()`.
///
/// Receives copies of items matching the predicate via a `ConcurrentQueue`.
/// Yields `Stream::Next` for matched items, forwards Pending/Delayed from source.
pub struct CollectorStreamIterator<D, P> {
    /// Shared queue receiving copied items from the splitter
    queue: Arc<ConcurrentQueue<Stream<D, P>>>,
}

impl<D, P> CollectorStreamIterator<D, P> {
    /// WHY: A task-path consumer parks on an observer natively via
    /// `Depends(QueueReadiness)` (F45 Resolution 3), but the observer's queue is
    /// private. WHAT: expose a [`QueueReadiness`] over the shared queue. HOW:
    /// clones the `Arc<ConcurrentQueue<..>>` the observer reads from.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn readiness(&self) -> QueueReadiness<Stream<D, P>> {
        QueueReadiness::new(self.queue.clone())
    }
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
                    None
                } else {
                    // F45 Resolution 2: yield Wait (not Ignore) on empty-open so
                    // `into_ready_future()`/`into_pending_future()` yield to the
                    // executor instead of hard-spinning inside `poll()`.
                    Some(Stream::Wait)
                }
            }
            Err(concurrent_queue::PopError::Closed) => None,
        }
    }
}

impl<D, P> Drop for CollectorStreamIterator<D, P> {
    /// WHY: A dropped observer must not leave the source parked forever on a full
    /// queue (F45 Part C1 observer-dropped policy). WHAT/HOW: closing the shared
    /// queue makes the continuation's next push observe `Closed`, so it stops
    /// copying to this branch and keeps forwarding.
    fn drop(&mut self) {
        self.queue.close();
    }
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
    /// A matched observer item that could not be delivered because the observer
    /// queue was full; retried on the next `next()` before pulling new work
    /// (F45 Part C1 backpressure).
    pending_push: Option<Stream<I::Ready, I::Pending>>,
    /// The source item held back while `pending_push` is being retried, forwarded
    /// downstream only once the observer accepts the copy (keeps both branches in
    /// lockstep).
    pending_forward: Option<TaskStatus<I::Ready, I::Pending, I::Spawner>>,
}

impl<I> Iterator for SplitCollectorContinuation<I>
where
    I: TaskIterator,
    I::Ready: Clone + Send + 'static,
    I::Pending: Clone + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // Retry a stashed observer push before pulling new work: a full observer
        // parks the source on vacancy instead of dropping the item (F45 Part C1).
        if let Some(stream_item) = self.pending_push.take() {
            if let Some(stashed) = split_observer_push(&self.queue, stream_item) {
                self.pending_push = Some(stashed);
                return Some(TaskStatus::Depends(Arc::new(QueueVacancyReadiness::new(
                    self.queue.clone(),
                ))));
            }
            return self.pending_forward.take();
        }

        let item = if let Some(item) = self.inner.next_status() {
            item
        } else {
            // Source iterator is naturally exhausted, close the queue
            self.queue.close();
            tracing::debug!("SplitCollectorContinuation: source exhausted, queue closed");
            return None;
        };

        // Copy matched items to observer queue, parking on backpressure.
        let mut park = false;
        if let TaskStatus::Ready(value) = &item {
            if (self.predicate)(value) {
                if let Some(stashed) = split_observer_push(&self.queue, Stream::Next(value.clone()))
                {
                    self.pending_push = Some(stashed);
                    park = true;
                } else {
                    tracing::trace!(
                        "SplitCollectorContinuation: copied matched item to observer queue"
                    );
                }
            }
        }

        if park {
            // Hold the source item back until the observer drains a slot.
            self.pending_forward = Some(item);
            return Some(TaskStatus::Depends(Arc::new(QueueVacancyReadiness::new(
                self.queue.clone(),
            ))));
        }

        // Always forward to continuation
        Some(item)
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
// N-way Broadcast Combinator (F45 N-way follow-on)
// ============================================================================

/// WHY: `broadcast`/`broadcast_lossy` share all wiring except the full-branch
/// policy; a single builder keeps the two entry points a one-line difference.
///
/// WHAT: allocate `n` bounded branch queues, hand back `n`
/// [`CollectorStreamIterator`] observers (reused from the 2-way split — they
/// already yield `Stream::Wait` on empty-open and close their queue on drop) plus a
/// [`BroadcastContinuation`] carrying the source onward with the chosen policy.
///
/// HOW: `queue_size` is clamped to at least 1 (a 0-capacity queue is invalid).
///
/// # Panics
/// Never panics.
fn broadcast_build<I>(
    inner: I,
    n: usize,
    predicate: Box<dyn Fn(&I::Ready) -> bool + Send>,
    queue_size: usize,
    lossy: bool,
) -> (
    Vec<CollectorStreamIterator<I::Ready, I::Pending>>,
    BroadcastContinuation<I>,
)
where
    I: TaskIterator,
{
    let cap = queue_size.max(1);
    let mut observers = Vec::with_capacity(n);
    let mut queues = Vec::with_capacity(n);
    for _ in 0..n {
        let queue = Arc::new(ConcurrentQueue::bounded(cap));
        observers.push(CollectorStreamIterator {
            queue: Arc::clone(&queue),
        });
        queues.push(queue);
    }
    tracing::debug!(
        "broadcast: {n} branches, queue_size={cap}, lossy={lossy}"
    );
    let continuation = BroadcastContinuation {
        inner,
        queues,
        predicate,
        lossy,
        pending_value: None,
        pending_forward: None,
    };
    (observers, continuation)
}

/// Continuation branch from [`broadcast`](TaskIteratorExt::broadcast) /
/// [`broadcast_lossy`](TaskIteratorExt::broadcast_lossy).
///
/// Wraps the source, fanning each matched `Ready` value out to every open branch
/// queue while forwarding the full stream onward. See the trait methods for the
/// zero-loss (slowest-gates lockstep) vs. lossy (drop-oldest per branch) semantics.
pub struct BroadcastContinuation<I: TaskIterator> {
    /// The wrapped source iterator.
    inner: I,
    /// One bounded queue per observer branch.
    queues: Vec<Arc<ConcurrentQueue<Stream<I::Ready, I::Pending>>>>,
    /// Which `Ready` values to fan out.
    predicate: Box<dyn Fn(&I::Ready) -> bool + Send>,
    /// `true` = a full branch drops (drop-oldest); `false` = a full branch parks.
    lossy: bool,
    /// A matched value awaiting delivery to all branches (zero-loss backpressure):
    /// held until **every** open branch has vacancy, then broadcast in one shot.
    pending_value: Option<I::Ready>,
    /// The source item held back until `pending_value` is delivered (lockstep).
    pending_forward: Option<TaskStatus<I::Ready, I::Pending, I::Spawner>>,
}

impl<I: TaskIterator> BroadcastContinuation<I> {
    /// Index of the first branch that is **open and full** (the one that gates
    /// delivery). Closed branches are skipped — a dropped observer never blocks the
    /// source. Returns `None` when every open branch has vacancy.
    fn first_full_branch(&self) -> Option<usize> {
        self.queues
            .iter()
            .position(|q| !q.is_closed() && q.is_full())
    }

    /// Close every branch queue (source exhausted or continuation dropped).
    fn close_all(&self) {
        for queue in &self.queues {
            queue.close();
        }
    }
}

impl<I> BroadcastContinuation<I>
where
    I: TaskIterator,
    I::Ready: Clone,
    I::Pending: Clone,
{
    /// Deliver one clone of `value` to every open branch. For zero-loss this is
    /// only called once all open branches have vacancy, so each `push` succeeds; a
    /// `Closed` race (observer dropped mid-flight) is skipped. For lossy, a full
    /// branch drops its oldest via `force_push`.
    fn deliver(&self, value: &I::Ready) {
        for queue in &self.queues {
            if queue.is_closed() {
                continue;
            }
            if self.lossy {
                // Loud lossy opt-in: drop-oldest so the source never stalls.
                let _ = queue.force_push(Stream::Next(value.clone()));
            } else {
                let _ = queue.push(Stream::Next(value.clone()));
            }
        }
    }
}

impl<I> Iterator for BroadcastContinuation<I>
where
    I: TaskIterator,
    I::Ready: Clone + Send + 'static,
    I::Pending: Clone + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // Resume a parked broadcast (zero-loss only): deliver to all once every
        // open branch has vacancy, otherwise re-park on the still-full branch.
        if let Some(value) = self.pending_value.take() {
            if let Some(idx) = self.first_full_branch() {
                self.pending_value = Some(value);
                return Some(TaskStatus::Depends(Arc::new(QueueVacancyReadiness::new(
                    self.queues[idx].clone(),
                ))));
            }
            self.deliver(&value);
            return self.pending_forward.take();
        }

        let item = if let Some(item) = self.inner.next_status() {
            item
        } else {
            self.close_all();
            tracing::debug!("BroadcastContinuation: source exhausted, branches closed");
            return None;
        };

        if let TaskStatus::Ready(value) = &item {
            if (self.predicate)(value) {
                if self.lossy {
                    // Never parks: drop-oldest on any full branch.
                    self.deliver(value);
                } else if let Some(idx) = self.first_full_branch() {
                    // Slowest-gates lockstep: deliver to NO branch until all have
                    // vacancy, so no branch is double-delivered on retry and none
                    // is dropped. Hold the source item back until then.
                    self.pending_value = Some(value.clone());
                    self.pending_forward = Some(item);
                    return Some(TaskStatus::Depends(Arc::new(QueueVacancyReadiness::new(
                        self.queues[idx].clone(),
                    ))));
                } else {
                    self.deliver(value);
                }
            }
        }

        Some(item)
    }
}

impl<I> Drop for BroadcastContinuation<I>
where
    I: TaskIterator,
{
    fn drop(&mut self) {
        self.close_all();
        tracing::debug!("BroadcastContinuation: dropped, branches closed");
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

impl<D, P> SplitUntilObserver<D, P> {
    /// Expose a [`QueueReadiness`] over the shared queue so a task-path consumer
    /// can park natively via `Depends` (F45 Resolution 3).
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn readiness(&self) -> QueueReadiness<Stream<D, P>> {
        QueueReadiness::new(self.queue.clone())
    }
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
                    None
                } else {
                    // F45 Resolution 2: yield Wait (not Ignore) on empty-open so
                    // `into_ready_future()`/`into_pending_future()` yield to the
                    // executor instead of hard-spinning inside `poll()`.
                    Some(Stream::Wait)
                }
            }
            Err(concurrent_queue::PopError::Closed) => {
                tracing::debug!("SplitUntilObserver: queue closed, returning None");
                None
            }
        }
    }
}

impl<D, P> Drop for SplitUntilObserver<D, P> {
    /// Close the shared queue on drop so a dropped observer never parks the
    /// source on a full queue (F45 Part C1 observer-dropped policy).
    fn drop(&mut self) {
        self.queue.close();
    }
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
    /// A matched observer item awaiting a free slot (F45 Part C1 backpressure).
    pending_push: Option<Stream<I::Ready, I::Pending>>,
    /// The source item held back until the stashed copy is accepted.
    pending_forward: Option<TaskStatus<I::Ready, I::Pending, I::Spawner>>,
    /// Whether the observer queue must be closed once `pending_push` lands (the
    /// `CollectionState::Close(true)` case parked on a full queue).
    pending_close: bool,
}

impl<I> Iterator for SplitUntilContinuation<I>
where
    I: TaskIterator,
    I::Ready: Clone + Send + 'static,
    I::Pending: Clone + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // Retry a stashed observer push before pulling new work (F45 Part C1).
        if let Some(stream_item) = self.pending_push.take() {
            if let Some(stashed) = split_observer_push(&self.queue, stream_item) {
                self.pending_push = Some(stashed);
                return Some(TaskStatus::Depends(Arc::new(QueueVacancyReadiness::new(
                    self.queue.clone(),
                ))));
            }
            if self.pending_close {
                self.pending_close = false;
                self.queue.close();
            }
            return self.pending_forward.take();
        }

        let item = if let Some(item) = self.inner.next_status() {
            item
        } else {
            // Source iterator is naturally exhausted, close the queue
            self.queue.close();
            tracing::debug!("SplitUntilContinuation: source exhausted, queue closed");
            return None;
        };

        // Handle items based on CollectionState from predicate
        let mut park = false;
        if let TaskStatus::Ready(value) = &item {
            match (self.predicate)(value) {
                CollectionState::Skip => {
                    // Skip this item - don't send to observer
                    tracing::trace!(
                        "SplitUntilContinuation: skipping item (CollectionState::Skip)"
                    );
                }
                CollectionState::Collect => {
                    // Collect this item for the observer, parking on backpressure.
                    if let Some(stashed) =
                        split_observer_push(&self.queue, Stream::Next(value.clone()))
                    {
                        self.pending_push = Some(stashed);
                        park = true;
                    } else {
                        tracing::trace!("SplitUntilContinuation: collected item for observer");
                    }
                }
                CollectionState::Close(collect_this) => {
                    // Close the observer after optionally collecting this item.
                    if collect_this {
                        if let Some(stashed) =
                            split_observer_push(&self.queue, Stream::Next(value.clone()))
                        {
                            // Park now; close the queue once the final item lands.
                            self.pending_push = Some(stashed);
                            self.pending_close = true;
                            park = true;
                        } else {
                            tracing::trace!("SplitUntilContinuation: collecting final item and closing observer queue");
                        }
                    } else {
                        tracing::trace!("SplitUntilContinuation: closing observer queue without collecting final item");
                    }
                    if !park {
                        self.queue.close();
                        tracing::debug!("SplitUntilContinuation: observer queue closed after CollectionState::Close");
                    }
                }
            }
        }

        if park {
            self.pending_forward = Some(item);
            return Some(TaskStatus::Depends(Arc::new(QueueVacancyReadiness::new(
                self.queue.clone(),
            ))));
        }

        // Always forward to continuation
        Some(item)
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

impl<D, P> SplitUntilObserverMap<D, P> {
    /// Expose a [`QueueReadiness`] over the shared queue so a task-path consumer
    /// can park natively via `Depends` (F45 Resolution 3).
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn readiness(&self) -> QueueReadiness<Stream<D, P>> {
        QueueReadiness::new(self.queue.clone())
    }
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
                    None
                } else {
                    // F45 Resolution 2: yield Wait (not Ignore) on empty-open so
                    // `into_ready_future()`/`into_pending_future()` yield to the
                    // executor instead of hard-spinning inside `poll()`.
                    Some(Stream::Wait)
                }
            }
            Err(concurrent_queue::PopError::Closed) => {
                tracing::debug!("SplitUntilObserverMap: queue closed, returning None");
                None
            }
        }
    }
}

impl<D, P> Drop for SplitUntilObserverMap<D, P> {
    /// Close the shared queue on drop so a dropped observer never parks the
    /// source on a full queue (F45 Part C1 observer-dropped policy).
    fn drop(&mut self) {
        self.queue.close();
    }
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
    /// A transformed observer item awaiting a free slot (F45 Part C1 backpressure).
    pending_push: Option<Stream<D, I::Pending>>,
    /// The source item held back until the stashed copy is accepted.
    pending_forward: Option<TaskStatus<I::Ready, I::Pending, I::Spawner>>,
    /// Whether the observer queue must be closed once `pending_push` lands.
    pending_close: bool,
}

impl<I, D> Iterator for SplitUntilContinuationMap<I, D>
where
    I: TaskIterator,
    I::Pending: Clone + Send + 'static,
    D: Clone + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // Retry a stashed observer push before pulling new work (F45 Part C1).
        if let Some(stream_item) = self.pending_push.take() {
            if let Some(stashed) = split_observer_push(&self.queue, stream_item) {
                self.pending_push = Some(stashed);
                return Some(TaskStatus::Depends(Arc::new(QueueVacancyReadiness::new(
                    self.queue.clone(),
                ))));
            }
            if self.pending_close {
                self.pending_close = false;
                self.queue.close();
            }
            return self.pending_forward.take();
        }

        let item = if let Some(item) = self.inner.next_status() {
            item
        } else {
            // Source iterator is naturally exhausted, close the queue
            self.queue.close();
            tracing::debug!("SplitUntilContinuationMap: source exhausted, queue closed");
            return None;
        };

        // Handle items based on (CollectionState, Option<D>) from transform
        let mut park = false;
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
                    // Collect this item for the observer (with transformation).
                    if let Some(transformed) = transformed {
                        if let Some(stashed) =
                            split_observer_push(&self.queue, Stream::Next(transformed))
                        {
                            self.pending_push = Some(stashed);
                            park = true;
                        } else {
                            tracing::trace!("SplitUntilContinuationMap: collected transformed item for observer");
                        }
                    }
                }
                CollectionState::Close(collect_this) => {
                    // Close the observer after optionally collecting this item.
                    if collect_this {
                        if let Some(transformed) = transformed {
                            if let Some(stashed) =
                                split_observer_push(&self.queue, Stream::Next(transformed))
                            {
                                self.pending_push = Some(stashed);
                                self.pending_close = true;
                                park = true;
                            } else {
                                tracing::trace!("SplitUntilContinuationMap: collecting final transformed item and closing observer queue");
                            }
                        }
                    } else {
                        tracing::trace!("SplitUntilContinuationMap: closing observer queue without collecting final item");
                    }
                    if !park {
                        self.queue.close();
                        tracing::debug!("SplitUntilContinuationMap: observer queue closed after CollectionState::Close");
                    }
                }
            }
        }

        if park {
            self.pending_forward = Some(item);
            return Some(TaskStatus::Depends(Arc::new(QueueVacancyReadiness::new(
                self.queue.clone(),
            ))));
        }

        // Always forward to continuation
        Some(item)
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

impl<M, P> SplitCollectorMapObserver<M, P> {
    /// Expose a [`QueueReadiness`] over the shared queue so a task-path consumer
    /// can park natively via `Depends` (F45 Resolution 3).
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn readiness(&self) -> QueueReadiness<Stream<M, P>> {
        QueueReadiness::new(self.queue.clone())
    }
}

impl<M, P> Iterator for SplitCollectorMapObserver<M, P>
where
    M: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type Item = Stream<M, P>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.queue.pop() {
            Ok(item) => Some(item),
            Err(concurrent_queue::PopError::Empty) => {
                if self.queue.is_closed() {
                    None
                } else {
                    // F45 Resolution 2: yield Wait (not Ignore) on empty-open so
                    // `into_ready_future()`/`into_pending_future()` yield to the
                    // executor instead of hard-spinning inside `poll()`.
                    Some(Stream::Wait)
                }
            }
            Err(concurrent_queue::PopError::Closed) => None,
        }
    }
}

impl<M, P> Drop for SplitCollectorMapObserver<M, P> {
    /// Close the shared queue on drop so a dropped observer never parks the
    /// source on a full queue (F45 Part C1 observer-dropped policy).
    fn drop(&mut self) {
        self.queue.close();
    }
}

/// Continuation branch from `split_collector_map()`.
///
/// Wraps the original iterator. The transform function returns `(bool, Option<M>)`:
/// - `true` + `Some(m)` sends `m` to the observer queue
/// - `false` or `None` skips sending to observer
///
/// The continuation continues with original Ready values unchanged.
pub struct SplitCollectorMapContinuation<I: TaskIterator, M> {
    /// The wrapped iterator
    inner: I,
    /// Queue to send transformed items to observer
    queue: Arc<ConcurrentQueue<Stream<M, I::Pending>>>,
    /// Combined predicate + transform function
    transform: Box<dyn Fn(&I::Ready) -> (bool, Option<M>) + Send>,
    /// A transformed observer item awaiting a free slot (F45 Part C1 backpressure).
    pending_push: Option<Stream<M, I::Pending>>,
    /// The source item held back until the stashed copy is accepted.
    pending_forward: Option<TaskStatus<I::Ready, I::Pending, I::Spawner>>,
}

impl<I, M> Iterator for SplitCollectorMapContinuation<I, M>
where
    I: TaskIterator,
    I::Pending: Clone + Send + 'static,
    M: Clone + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // Retry a stashed observer push before pulling new work (F45 Part C1).
        if let Some(stream_item) = self.pending_push.take() {
            if let Some(stashed) = split_observer_push(&self.queue, stream_item) {
                self.pending_push = Some(stashed);
                return Some(TaskStatus::Depends(Arc::new(QueueVacancyReadiness::new(
                    self.queue.clone(),
                ))));
            }
            return self.pending_forward.take();
        }

        let item = if let Some(item) = self.inner.next_status() {
            item
        } else {
            self.queue.close();
            tracing::debug!("SplitCollectorMapContinuation: source exhausted, queue closed");
            return None;
        };

        let mut park = false;
        if let TaskStatus::Ready(value) = &item {
            let (matched, transformed) = (self.transform)(value);
            if matched {
                if let Some(transformed) = transformed {
                    if let Some(stashed) =
                        split_observer_push(&self.queue, Stream::Next(transformed))
                    {
                        self.pending_push = Some(stashed);
                        park = true;
                    } else {
                        tracing::trace!(
                            "SplitCollectorMapContinuation: copied transformed item to observer queue"
                        );
                    }
                }
            }
        }

        if park {
            self.pending_forward = Some(item);
            return Some(TaskStatus::Depends(Arc::new(QueueVacancyReadiness::new(
                self.queue.clone(),
            ))));
        }

        Some(item)
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

// ===== Feature 08: Iterator Extension Completion Wrapper Structs =====

/// Wrapper for `map_state()` - transforms any `TaskStatus`
pub struct TMapState<I, F> {
    inner: I,
    mapper: F,
}

impl<I, F> Iterator for TMapState<I, F>
where
    I: TaskIterator,
    F: Fn(
            TaskStatus<I::Ready, I::Pending, I::Spawner>,
        ) -> TaskStatus<I::Ready, I::Pending, I::Spawner>
        + Send
        + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next_status().map(&self.mapper)
    }
}

/// Wrapper for `inspect_state()` - side-effect on any `TaskStatus`
pub struct TInspectState<I, F> {
    inner: I,
    inspector: F,
}

impl<I, F> Iterator for TInspectState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let status = self.inner.next_status()?;
        (self.inspector)(&status);
        Some(status)
    }
}

/// Wrapper for `filter_state()` - filter based on full `TaskStatus`
pub struct TFilterState<I, F> {
    inner: I,
    predicate: F,
}

impl<I, F> Iterator for TFilterState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let status = self.inner.next_status()?;
        if (self.predicate)(&status) {
            Some(status)
        } else {
            Some(TaskStatus::Ignore)
        }
    }
}

/// Wrapper for `transform_until()` - transform until transformer returns None
///
/// When the transformer returns None, the iterator terminates.
/// This is useful for early termination based on transformation results.
pub struct TTransformUntil<I, F> {
    inner: I,
    transformer: F,
    done: bool,
}

impl<I, F> Iterator for TTransformUntil<I, F>
where
    I: TaskIterator,
    F: Fn(
            TaskStatus<I::Ready, I::Pending, I::Spawner>,
        ) -> Option<TaskStatus<I::Ready, I::Pending, I::Spawner>>
        + Send
        + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let status = self.inner.next_status()?;
        match (self.transformer)(status) {
            Some(transformed) => Some(transformed),
            None => {
                self.done = true;
                None
            }
        }
    }
}

/// Wrapper for `take_while_state()` - take while state predicate true
pub struct TTakeWhileState<I, F> {
    inner: I,
    predicate: F,
    done: bool,
}

impl<I, F> Iterator for TTakeWhileState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let status = self.inner.next_status()?;
        if (self.predicate)(&status) {
            Some(status)
        } else {
            self.done = true;
            None
        }
    }
}

/// Wrapper for `skip_while_state()` - skip while state predicate true
pub struct TSkipWhileState<I, F> {
    inner: I,
    predicate: F,
    done_skipping: bool,
}

impl<I, F> Iterator for TSkipWhileState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let status = self.inner.next_status()?;
        if !self.done_skipping && (self.predicate)(&status) {
            return Some(TaskStatus::Ignore);
        }
        self.done_skipping = true;
        Some(status)
    }
}

/// Wrapper for `take_state()` - take at most n items matching predicate
pub struct TTakeState<I, F> {
    inner: I,
    remaining: usize,
    state_predicate: F,
}

impl<I, F> Iterator for TTakeState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let status = self.inner.next_status()?;
        if (self.state_predicate)(&status) {
            self.remaining -= 1;
        }
        Some(status)
    }
}

/// Wrapper for `skip_state()` - skip first n items matching predicate
pub struct TSkipState<I, F> {
    inner: I,
    to_skip: usize,
    state_predicate: F,
}

impl<I, F> Iterator for TSkipState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let status = self.inner.next_status()?;
        if self.to_skip > 0 && (self.state_predicate)(&status) {
            self.to_skip -= 1;
            return Some(TaskStatus::Ignore);
        }
        Some(status)
    }
}

/// Wrapper for `enumerate()` - adds index to Ready items
pub struct TEnumerate<I> {
    inner: I,
    count: usize,
}

impl<I> Iterator for TEnumerate<I>
where
    I: TaskIterator,
{
    type Item = TaskStatus<(usize, I::Ready), I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let status = self.inner.next_status()?;
        match status {
            TaskStatus::Ready(v) => {
                let item = TaskStatus::Ready((self.count, v));
                self.count += 1;
                Some(item)
            }
            TaskStatus::Pending(p) => Some(TaskStatus::Pending(p)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
            TaskStatus::Wait => Some(TaskStatus::Wait),
            TaskStatus::Depends(signal) => Some(TaskStatus::Depends(signal)),
            TaskStatus::Spread(items) => {
                let start = self.count;
                let mut idx = start;
                let mut result = Vec::with_capacity(items.len());
                for item in items {
                    match item {
                        TaskSpread::Ready(v) => {
                            result.push(TaskSpread::Ready((idx, v)));
                            idx += 1;
                        }
                        TaskSpread::Pending(p) => {
                            result.push(TaskSpread::Pending(p));
                        }
                    }
                }
                self.count = idx;
                Some(TaskStatus::Spread(result))
            }
        }
    }
}

/// Wrapper for `find()` - find first Ready matching predicate
pub struct TFind<I, F> {
    inner: I,
    predicate: F,
    found: bool,
}

impl<I, F> Iterator for TFind<I, F>
where
    I: TaskIterator,
    F: Fn(&I::Ready) -> bool + Send + 'static,
{
    type Item = TaskStatus<Option<I::Ready>, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.found {
            return None;
        }

        let status = self.inner.next_status()?;
        match status {
            TaskStatus::Ready(v) => {
                if (self.predicate)(&v) {
                    self.found = true;
                    Some(TaskStatus::Ready(Some(v)))
                } else {
                    Some(TaskStatus::Ignore)
                }
            }
            TaskStatus::Pending(p) => Some(TaskStatus::Pending(p)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
            TaskStatus::Wait => Some(TaskStatus::Wait),
            TaskStatus::Depends(signal) => Some(TaskStatus::Depends(signal)),
            TaskStatus::Spread(items) => {
                if let Some(v) = items.into_iter().find_map(|item| {
                    match item {
                        TaskSpread::Ready(v) if (self.predicate)(&v) => Some(v),
                        _ => None,
                    }
                }) {
                    self.found = true;
                    Some(TaskStatus::Ready(Some(v)))
                } else {
                    Some(TaskStatus::Ignore)
                }
            }
        }
    }
}

/// Wrapper for `find_map()` - find first Ready that maps to Some
pub struct TFindMap<I, F, R> {
    inner: I,
    mapper: F,
    found: bool,
    _phantom: std::marker::PhantomData<R>,
}

impl<I, F, R> Iterator for TFindMap<I, F, R>
where
    I: TaskIterator,
    F: Fn(I::Ready) -> Option<R> + Send + 'static,
    R: Send + 'static,
{
    type Item = TaskStatus<Option<R>, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.found {
            return None;
        }

        let status = self.inner.next_status()?;
        match status {
            TaskStatus::Ready(v) => {
                if let Some(r) = (self.mapper)(v) {
                    self.found = true;
                    Some(TaskStatus::Ready(Some(r)))
                } else {
                    Some(TaskStatus::Ignore)
                }
            }
            TaskStatus::Pending(p) => Some(TaskStatus::Pending(p)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
            TaskStatus::Wait => Some(TaskStatus::Wait),
            TaskStatus::Depends(signal) => Some(TaskStatus::Depends(signal)),
            TaskStatus::Spread(items) => {
                if let Some(r) = items.into_iter().find_map(|item| {
                    match item {
                        TaskSpread::Ready(v) => (self.mapper)(v),
                        _ => None,
                    }
                }) {
                    self.found = true;
                    Some(TaskStatus::Ready(Some(r)))
                } else {
                    Some(TaskStatus::Ignore)
                }
            }
        }
    }
}

/// Wrapper for `fold()` - accumulate values
pub struct TFold<I, F, R> {
    inner: I,
    acc: Option<R>,
    folder: F,
    done: bool,
    _phantom: std::marker::PhantomData<(I, R)>,
}

impl<I, F, R> Iterator for TFold<I, F, R>
where
    I: TaskIterator,
    F: Fn(R, I::Ready) -> R + Send + 'static,
    R: Send + 'static,
{
    type Item = TaskStatus<R, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        match self.inner.next_status() {
            Some(TaskStatus::Ready(v)) => {
                if let Some(acc) = self.acc.take() {
                    self.acc = Some((self.folder)(acc, v));
                }
                Some(TaskStatus::Ignore)
            }
            Some(TaskStatus::Pending(p)) => Some(TaskStatus::Pending(p)),
            Some(TaskStatus::Delayed(d)) => Some(TaskStatus::Delayed(d)),
            Some(TaskStatus::Init) => Some(TaskStatus::Init),
            Some(TaskStatus::Spawn(s)) => Some(TaskStatus::Spawn(s)),
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            Some(TaskStatus::Wait) => Some(TaskStatus::Wait),
            Some(TaskStatus::Depends(signal)) => Some(TaskStatus::Depends(signal)),
            Some(TaskStatus::Spread(items)) => {
                if let Some(acc) = self.acc.take() {
                    self.acc = Some(
                        items
                            .into_iter()
                            .fold(acc, |acc, item| match item {
                                TaskSpread::Ready(v) => (self.folder)(acc, v),
                                _ => acc,
                            }),
                    );
                }
                Some(TaskStatus::Ignore)
            }
            None => {
                // Inner exhausted, yield final accumulated value
                self.done = true;
                self.acc.take().map(TaskStatus::Ready)
            }
        }
    }
}

impl<I, F, R> Drop for TFold<I, F, R> {
    fn drop(&mut self) {
        // Iterator was not fully consumed
    }
}

/// Wrapper for `all()` - check if all Ready satisfy predicate
pub struct TAll<I, F> {
    inner: I,
    predicate: F,
    all_true: bool,
    done: bool,
}

impl<I, F> Iterator for TAll<I, F>
where
    I: TaskIterator,
    F: Fn(I::Ready) -> bool + Send + 'static,
{
    type Item = TaskStatus<bool, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        if !self.all_true {
            return None;
        }

        match self.inner.next_status() {
            Some(TaskStatus::Ready(v)) => {
                if !(self.predicate)(v) {
                    self.all_true = false;
                    self.done = true;
                    Some(TaskStatus::Ready(false))
                } else {
                    Some(TaskStatus::Ignore)
                }
            }
            Some(TaskStatus::Pending(p)) => Some(TaskStatus::Pending(p)),
            Some(TaskStatus::Delayed(d)) => Some(TaskStatus::Delayed(d)),
            Some(TaskStatus::Init) => Some(TaskStatus::Init),
            Some(TaskStatus::Spawn(s)) => Some(TaskStatus::Spawn(s)),
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            Some(TaskStatus::Wait) => Some(TaskStatus::Wait),
            Some(TaskStatus::Depends(signal)) => Some(TaskStatus::Depends(signal)),
            Some(TaskStatus::Spread(items)) => {
                // Separate Ready and Pending items
                let mut pending_inner = Vec::new();
                let mut any_false = false;
                for item in items {
                    match item {
                        TaskSpread::Ready(v) => {
                            if !(self.predicate)(v) {
                                any_false = true;
                                break;
                            }
                        }
                        TaskSpread::Pending(p) => pending_inner.push(p),
                    }
                }
                if any_false {
                    self.all_true = false;
                    self.done = true;
                    if pending_inner.is_empty() {
                        Some(TaskStatus::Ready(false))
                    } else {
                        Some(TaskStatus::Spread(
                            pending_inner.into_iter().map(TaskSpread::Pending).collect(),
                        ))
                    }
                } else if pending_inner.is_empty() {
                    Some(TaskStatus::Ignore)
                } else {
                    Some(TaskStatus::Spread(
                        pending_inner.into_iter().map(TaskSpread::Pending).collect(),
                    ))
                }
            }
            None => {
                self.done = true;
                Some(TaskStatus::Ready(true))
            }
        }
    }
}

/// Wrapper for `any()` - check if any Ready satisfies predicate
pub struct TAny<I, F> {
    inner: I,
    predicate: F,
    any_true: bool,
    done: bool,
}

impl<I, F> Iterator for TAny<I, F>
where
    I: TaskIterator,
    F: Fn(I::Ready) -> bool + Send + 'static,
{
    type Item = TaskStatus<bool, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        if self.any_true {
            return None;
        }

        match self.inner.next_status() {
            Some(TaskStatus::Ready(v)) => {
                if (self.predicate)(v) {
                    self.any_true = true;
                    self.done = true;
                    Some(TaskStatus::Ready(true))
                } else {
                    Some(TaskStatus::Ignore)
                }
            }
            Some(TaskStatus::Pending(p)) => Some(TaskStatus::Pending(p)),
            Some(TaskStatus::Delayed(d)) => Some(TaskStatus::Delayed(d)),
            Some(TaskStatus::Init) => Some(TaskStatus::Init),
            Some(TaskStatus::Spawn(s)) => Some(TaskStatus::Spawn(s)),
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            Some(TaskStatus::Wait) => Some(TaskStatus::Wait),
            Some(TaskStatus::Depends(signal)) => Some(TaskStatus::Depends(signal)),
            Some(TaskStatus::Spread(items)) => {
                // Scan all Ready items first for a match, collect Pending
                let mut pending_items: Vec<I::Pending> = Vec::new();
                for item in items {
                    match item {
                        TaskSpread::Ready(v) => {
                            if (self.predicate)(v) {
                                // Found a match — return immediately.
                                // Pending items are irrelevant once the answer is determined.
                                self.any_true = true;
                                self.done = true;
                                return Some(TaskStatus::Ready(true));
                            }
                        }
                        TaskSpread::Pending(p) => pending_items.push(p),
                    }
                }
                // No Ready matched — return Pending items to be retried later
                if pending_items.is_empty() {
                    Some(TaskStatus::Ignore)
                } else {
                    Some(TaskStatus::Spread(
                        pending_items.into_iter().map(TaskSpread::Pending).collect(),
                    ))
                }
            }
            None => {
                self.done = true;
                Some(TaskStatus::Ready(false))
            }
        }
    }
}

/// Wrapper for `count()` - count Ready items
pub struct TCount<I> {
    inner: I,
    count: usize,
}

impl<I> Iterator for TCount<I>
where
    I: TaskIterator,
{
    type Item = TaskStatus<usize, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next_status() {
            Some(TaskStatus::Ready(_)) => {
                self.count += 1;
                Some(TaskStatus::Ignore)
            }
            Some(TaskStatus::Pending(p)) => Some(TaskStatus::Pending(p)),
            Some(TaskStatus::Delayed(d)) => Some(TaskStatus::Delayed(d)),
            Some(TaskStatus::Init) => Some(TaskStatus::Init),
            Some(TaskStatus::Spawn(s)) => Some(TaskStatus::Spawn(s)),
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            Some(TaskStatus::Wait) => Some(TaskStatus::Wait),
            Some(TaskStatus::Depends(signal)) => Some(TaskStatus::Depends(signal)),
            Some(TaskStatus::Spread(items)) => {
                let mut pending_items: Vec<TaskSpread<usize, I::Pending>> = Vec::new();
                for item in items {
                    match item {
                        TaskSpread::Ready(_) => {
                            self.count += 1;
                        }
                        TaskSpread::Pending(p) => {
                            pending_items.push(TaskSpread::Pending(p));
                        }
                    }
                }
                if pending_items.is_empty() {
                    Some(TaskStatus::Ignore)
                } else {
                    Some(TaskStatus::Spread(pending_items))
                }
            }
            None => Some(TaskStatus::Ready(self.count)),
        }
    }
}

/// Wrapper for `count_all()` - count all items
pub struct TCountAll<I> {
    inner: I,
    count: usize,
    done: bool,
}

impl<I> Iterator for TCountAll<I>
where
    I: TaskIterator,
{
    type Item = TaskStatus<usize, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        match self.inner.next_status() {
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            Some(TaskStatus::Depends(signal)) => Some(TaskStatus::Depends(signal)),
            Some(TaskStatus::Ready(_))
            | Some(TaskStatus::Pending(_))
            | Some(TaskStatus::Delayed(_))
            | Some(TaskStatus::Init)
            | Some(TaskStatus::Spawn(_))
            | Some(TaskStatus::Wait)
            | Some(TaskStatus::Spread(_)) => {
                self.count += 1;
                Some(TaskStatus::Ignore)
            }
            None => {
                self.done = true;
                Some(TaskStatus::Ready(self.count))
            }
        }
    }
}

// ============================================================================
// TaskIterator implementations for wrapped types
// ============================================================================

// Note: Implementations for &, &mut, Box, Rc, RefCell, Arc, Mutex are in task.rs
// ===== map_circuit combinator =====

/// Wrapper for `map_circuit()` - applies a circuit function to each `TaskStatus`.
///
/// The circuit function determines whether to continue iteration,
/// return a value and stop, or just stop.
pub struct TMapCircuit<I: TaskIterator> {
    inner: I,
    circuit: Box<
        dyn Fn(
                TaskStatus<I::Ready, I::Pending, I::Spawner>,
            ) -> TaskShortCircuit<I::Ready, I::Pending, I::Spawner>
            + Send,
    >,
    stopped: bool,
    _phantom: std::marker::PhantomData<I>,
}

impl<I> Iterator for TMapCircuit<I>
where
    I: TaskIterator + Send + 'static,
    I::Ready: Send + 'static,
    I::Pending: Send + 'static,
    I::Spawner: ExecutionAction + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // If we've stopped, return None permanently
        if self.stopped {
            return None;
        }

        // Get the next item from the inner iterator
        let status = self.inner.next_status()?;

        // Apply the circuit function
        match (self.circuit)(status) {
            TaskShortCircuit::Continue(item) => Some(item),
            TaskShortCircuit::ReturnAndStop(item) => {
                // Mark as stopped so future calls return None
                self.stopped = true;
                Some(item)
            }
            TaskShortCircuit::Stop => {
                // Mark as stopped and return None
                self.stopped = true;
                None
            }
        }
    }
}
