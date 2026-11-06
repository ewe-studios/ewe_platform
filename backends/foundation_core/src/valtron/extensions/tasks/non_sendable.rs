#![cfg(not(feature = "multi"))]

use crate::valtron::EventQueue;

use std::sync::Arc;

use crate::valtron::branches::{BroadcastPolicy, CollectionState};
use crate::valtron::{
    Ctx, QueueReadiness, Stream, TaskIterator, TaskShortCircuit, TaskSpread, TaskStatus,
};

/// The branch observers and continuation returned by a broadcast split.
pub type BroadcastPair<I> = (
    Vec<CollectorStreamIterator<<I as TaskIterator>::Ready, <I as TaskIterator>::Pending>>,
    BroadcastContinuation<I>,
);

/// A state predicate over a task status.
pub type BoxedTaskPredicate<R, P> = Box<dyn Fn(&TaskStatus<R, P>) -> bool>;

/// A predicate over a task's ready value.
pub type BoxedReadyPredicate<I> = Box<dyn Fn(&<I as TaskIterator>::Ready) -> bool>;

/// A predicate that maps a ready value to a collection action.
pub type BoxedReadyCollectionPredicate<I> =
    Box<dyn Fn(&<I as TaskIterator>::Ready) -> CollectionState>;

/// A transform from a ready value to a collection action and an optional value.
pub type BoxedReadyTransform<I, D> =
    Box<dyn Fn(&<I as TaskIterator>::Ready) -> (CollectionState, Option<D>)>;

/// A transform from a ready value to a match flag and an optional value.
pub type BoxedReadyMatchedTransform<I, M> =
    Box<dyn Fn(&<I as TaskIterator>::Ready) -> (bool, Option<M>)>;

/// A circuit that maps a task status to a short-circuit action.
pub type BoxedTaskCircuit<I> = Box<
    dyn Fn(
        TaskStatus<<I as TaskIterator>::Ready, <I as TaskIterator>::Pending>,
    ) -> TaskShortCircuit<<I as TaskIterator>::Ready, <I as TaskIterator>::Pending>,
>;

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
/// ```
/// use foundation_core::valtron::{Ctx, TaskIterator, TaskIteratorExt, TaskStatus};
///
/// # struct MyTask(std::vec::IntoIter<TaskStatus<u32, &'static str>>);
/// # impl MyTask {
/// #     fn new(items: Vec<TaskStatus<u32, &'static str>>) -> Self {
/// #         Self(items.into_iter())
/// #     }
/// # }
/// # impl TaskIterator for MyTask {
/// #     type Ready = u32;
/// #     type Pending = &'static str;
/// #     fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
/// #         self.0.next()
/// #     }
/// # }
/// let ctx = Ctx::new();
/// let mut task = MyTask::new(vec![
///     TaskStatus::Ready(5),
///     TaskStatus::Pending("waiting"),
///     TaskStatus::Ready(12),
/// ])
/// .map_ready(|v| v * 2)
/// .map_pending(|p| format!("Still waiting: {p}"))
/// .filter_ready(|v| *v > 10);
///
/// // 5 becomes 10 and fails the filter, so it yields Ignore.
/// assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ignore));
/// // Pending values are transformed, never filtered.
/// assert_eq!(
///     task.next_status(ctx.clone()),
///     Some(TaskStatus::Pending("Still waiting: waiting".to_string()))
/// );
/// // 12 becomes 24 and passes the filter.
/// assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ready(24)));
/// assert_eq!(task.next_status(ctx.clone()), None);
/// ```
pub trait TaskIteratorExt: TaskIterator + Sized {
    /// Transform Ready values using the provided function.
    ///
    /// Pending, Delayed, Init, and Spawn states pass through unchanged.
    fn map_ready<F, R>(self, f: F) -> TMapReady<Self, R>
    where
        F: Fn(Self::Ready) -> R + 'static,
        R: 'static;

    /// Transform Pending values using the provided function.
    ///
    /// Ready, Delayed, Init, and Spawn states pass through unchanged.
    fn map_pending<F, R>(self, f: F) -> TMapPending<Self, R>
    where
        F: Fn(Self::Pending) -> R + 'static,
        R: 'static;

    /// Filter Ready values using the provided predicate.
    ///
    /// Non-Ready states pass through unchanged. Ready values that don't
    /// satisfy the predicate are returned as `TaskStatus::Ignore`.
    fn filter_ready<F>(self, f: F) -> TFilterReady<Self>
    where
        F: Fn(&Self::Ready) -> bool + 'static;

    /// Collect all Ready values into a Vec.
    ///
    /// Unlike `std::Iterator::collect()`, this does NOT block waiting for all items.
    /// It passes through Pending, Delayed, Init, Spawn states unchanged,
    /// replaces Ready values with `TaskStatus::Ignore`, and only yields the
    /// collected `Vec<Ready>` when the inner iterator completes.
    fn stream_collect(self) -> TStreamCollect<Self>
    where
        Self::Ready: Clone + 'static;

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
    /// ```
    /// use foundation_core::valtron::{Ctx, Stream, TaskIterator, TaskIteratorExt, TaskStatus};
    ///
    /// # #[derive(Clone, Debug, PartialEq)]
    /// # enum RequestIntro {
    /// #     Success { status: u16 },
    /// #     Pending,
    /// # }
    /// # struct SendRequestTask(std::vec::IntoIter<TaskStatus<RequestIntro, String>>);
    /// # impl SendRequestTask {
    /// #     fn new(items: Vec<TaskStatus<RequestIntro, String>>) -> Self {
    /// #         Self(items.into_iter())
    /// #     }
    /// # }
    /// # impl TaskIterator for SendRequestTask {
    /// #     type Ready = RequestIntro;
    /// #     type Pending = String;
    /// #     fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
    /// #         self.0.next()
    /// #     }
    /// # }
    /// let ctx = Ctx::new();
    /// let send_request_task = SendRequestTask::new(vec![
    ///     TaskStatus::Ready(RequestIntro::Success { status: 200 }),
    ///     TaskStatus::Ready(RequestIntro::Pending),
    /// ]);
    /// let (mut observer, mut continuation) = send_request_task
    ///     .split_collector(
    ///         |item| matches!(item, RequestIntro::Success { .. }),
    ///         1  // Queue size 1 for immediate delivery
    ///     );
    ///
    /// // Drive the continuation; it still forwards every item downstream.
    /// let mut forwarded = Vec::new();
    /// while let Some(status) = continuation.next_status(ctx.clone()) {
    ///     if let TaskStatus::Ready(item) = status {
    ///         forwarded.push(item);
    ///     }
    /// }
    ///
    /// // The observer receives a copy of the matched item only.
    /// let observed: Vec<_> = observer
    ///     .filter_map(|item| match item {
    ///         Stream::Next(item) => Some(item),
    ///         _ => None,
    ///     })
    ///     .collect();
    ///
    /// assert_eq!(
    ///     forwarded,
    ///     vec![RequestIntro::Success { status: 200 }, RequestIntro::Pending]
    /// );
    /// assert_eq!(observed, vec![RequestIntro::Success { status: 200 }]);
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
        P: Fn(&Self::Ready) -> bool + 'static;

    /// Convenience method: `split_collector` with `queue_size` = 1.
    ///
    /// Sends the first matching item to the observer, then continues.
    /// Perfect for "get intro first, then body" patterns.
    ///
    /// ## Example
    ///
    /// ```
    /// use foundation_core::valtron::{Ctx, Stream, TaskIterator, TaskIteratorExt, TaskStatus};
    ///
    /// # #[derive(Clone, Debug, PartialEq)]
    /// # enum RequestIntro {
    /// #     Success { status: u16 },
    /// #     Pending,
    /// # }
    /// # struct SendRequestTask(std::vec::IntoIter<TaskStatus<RequestIntro, String>>);
    /// # impl SendRequestTask {
    /// #     fn new(items: Vec<TaskStatus<RequestIntro, String>>) -> Self {
    /// #         Self(items.into_iter())
    /// #     }
    /// # }
    /// # impl TaskIterator for SendRequestTask {
    /// #     type Ready = RequestIntro;
    /// #     type Pending = String;
    /// #     fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
    /// #         self.0.next()
    /// #     }
    /// # }
    /// let ctx = Ctx::new();
    /// let send_request_task = SendRequestTask::new(vec![
    ///     TaskStatus::Ready(RequestIntro::Pending),
    ///     TaskStatus::Ready(RequestIntro::Success { status: 200 }),
    /// ]);
    /// let (mut observer, mut continuation) = send_request_task
    ///     .split_collect_one(|item| matches!(item, RequestIntro::Success { .. }));
    ///
    /// let mut forwarded = Vec::new();
    /// while let Some(status) = continuation.next_status(ctx.clone()) {
    ///     if let TaskStatus::Ready(item) = status {
    ///         forwarded.push(item);
    ///     }
    /// }
    ///
    /// let observed: Vec<_> = observer
    ///     .filter_map(|item| match item {
    ///         Stream::Next(item) => Some(item),
    ///         _ => None,
    ///     })
    ///     .collect();
    ///
    /// assert_eq!(
    ///     forwarded,
    ///     vec![RequestIntro::Pending, RequestIntro::Success { status: 200 }]
    /// );
    /// assert_eq!(observed, vec![RequestIntro::Success { status: 200 }]);
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
        P: Fn(&Self::Ready) -> bool + 'static;

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
    fn broadcast<P>(self, n: usize, predicate: P, queue_size: usize) -> BroadcastPair<Self>
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> bool + 'static;

    /// Like [`broadcast`](Self::broadcast) but the source **never stalls** — a full
    /// branch drops the **incoming (newest)** value ([`BroadcastPolicy::DropNewest`]).
    ///
    /// A branch that cannot keep up holds onto its buffered backlog and stops seeing
    /// new values until it drains — it falls off the *live* edge of the stream. Use
    /// when a slow consumer should process what it already has rather than the
    /// freshest data. For "always latest" semantics use
    /// [`broadcast_latest`](Self::broadcast_latest); for zero loss use
    /// [`broadcast`](Self::broadcast).
    ///
    /// ## Panics
    /// Never panics. (`queue_size` is clamped to at least 1.)
    fn broadcast_lossy<P>(self, n: usize, predicate: P, queue_size: usize) -> BroadcastPair<Self>
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> bool + 'static;

    /// Like [`broadcast`](Self::broadcast) but the source **never stalls** and every
    /// branch is guaranteed the **latest** values — a full branch evicts its
    /// **oldest** ([`BroadcastPolicy::DropOldest`], via `force_push`).
    ///
    /// No receiver is ever kicked off the live edge: whatever a consumer reads is
    /// always the freshest available; only intermediate history is lost under
    /// pressure. Ideal for live feeds, telemetry, "latest wins" fan-out. For zero
    /// loss use [`broadcast`](Self::broadcast); to instead keep the backlog and drop
    /// new values use [`broadcast_lossy`](Self::broadcast_lossy).
    ///
    /// ## Panics
    /// Never panics. (`queue_size` is clamped to at least 1.)
    fn broadcast_latest<P>(self, n: usize, predicate: P, queue_size: usize) -> BroadcastPair<Self>
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> bool + 'static;

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
    /// ```
    /// use foundation_core::valtron::CollectionState;
    /// use foundation_core::valtron::{Ctx, Stream, TaskIterator, TaskIteratorExt, TaskStatus};
    ///
    /// # #[derive(Clone, Debug, PartialEq)]
    /// # enum RequestIntro {
    /// #     Success { status: u16 },
    /// #     Pending,
    /// # }
    /// # struct SendRequestTask(std::vec::IntoIter<TaskStatus<RequestIntro, String>>);
    /// # impl SendRequestTask {
    /// #     fn new(items: Vec<TaskStatus<RequestIntro, String>>) -> Self {
    /// #         Self(items.into_iter())
    /// #     }
    /// # }
    /// # impl TaskIterator for SendRequestTask {
    /// #     type Ready = RequestIntro;
    /// #     type Pending = String;
    /// #     fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
    /// #         self.0.next()
    /// #     }
    /// # }
    /// let ctx = Ctx::new();
    /// let send_request_task = SendRequestTask::new(vec![
    ///     TaskStatus::Ready(RequestIntro::Pending),
    ///     TaskStatus::Ready(RequestIntro::Success { status: 200 }),
    /// ]);
    /// // Observer gets items until first Success, then closes
    /// let (mut observer, mut continuation) = send_request_task
    ///     .split_collect_until(
    ///         |item| match item {
    ///             RequestIntro::Success { .. } => CollectionState::Close(true),
    ///             _ => CollectionState::Collect,
    ///         },
    ///         1  // Queue size 1 for immediate delivery
    ///     );
    ///
    /// let mut forwarded = Vec::new();
    /// let mut observed = Vec::new();
    /// while let Some(status) = continuation.next_status(ctx.clone()) {
    ///     if let TaskStatus::Ready(item) = status {
    ///         forwarded.push(item);
    ///     }
    ///     // Drain the observer so a full queue never parks the continuation.
    ///     loop {
    ///         match observer.next() {
    ///             Some(Stream::Next(item)) => observed.push(item),
    ///             Some(Stream::Wait) | None => break,
    ///             Some(_) => {}
    ///         }
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     forwarded,
    ///     vec![RequestIntro::Pending, RequestIntro::Success { status: 200 }]
    /// );
    /// assert_eq!(
    ///     observed,
    ///     vec![RequestIntro::Pending, RequestIntro::Success { status: 200 }]
    /// );
    /// // The observer queue closed when the first Success arrived.
    /// assert_eq!(observer.next(), None);
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
        P: Fn(&Self::Ready) -> CollectionState + 'static;

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
    /// ```
    /// use foundation_core::valtron::CollectionState;
    /// use foundation_core::valtron::{Ctx, Stream, TaskIterator, TaskIteratorExt, TaskStatus};
    ///
    /// # #[derive(Clone, Debug, PartialEq)]
    /// # enum RequestIntro {
    /// #     Success { status: u16 },
    /// #     Pending,
    /// # }
    /// # impl RequestIntro {
    /// #     fn to_cloneable_data(&self) -> u16 {
    /// #         match self {
    /// #             RequestIntro::Success { status } => *status,
    /// #             RequestIntro::Pending => 0,
    /// #         }
    /// #     }
    /// # }
    /// # struct SendRequestTask(std::vec::IntoIter<TaskStatus<RequestIntro, String>>);
    /// # impl SendRequestTask {
    /// #     fn new(items: Vec<TaskStatus<RequestIntro, String>>) -> Self {
    /// #         Self(items.into_iter())
    /// #     }
    /// # }
    /// # impl TaskIterator for SendRequestTask {
    /// #     type Ready = RequestIntro;
    /// #     type Pending = String;
    /// #     fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
    /// #         self.0.next()
    /// #     }
    /// # }
    /// let ctx = Ctx::new();
    /// let send_request_task = SendRequestTask::new(vec![
    ///     TaskStatus::Ready(RequestIntro::Pending),
    ///     TaskStatus::Ready(RequestIntro::Success { status: 200 }),
    /// ]);
    /// // Observer gets RequestIntroData (cloneable), continuation gets full RequestIntro
    /// let (mut observer, mut continuation) = send_request_task
    ///     .split_collect_until_map(
    ///         |item| match item {
    ///             RequestIntro::Success { .. } => {
    ///                 (CollectionState::Close(true), Some(item.to_cloneable_data()))
    ///             }
    ///             _ => (CollectionState::Collect, Some(item.to_cloneable_data())),
    ///         },
    ///         1
    ///     );
    ///
    /// let mut forwarded = Vec::new();
    /// let mut observed = Vec::new();
    /// while let Some(status) = continuation.next_status(ctx.clone()) {
    ///     if let TaskStatus::Ready(item) = status {
    ///         forwarded.push(item);
    ///     }
    ///     // Drain the observer so a full queue never parks the continuation.
    ///     loop {
    ///         match observer.next() {
    ///             Some(Stream::Next(item)) => observed.push(item),
    ///             Some(Stream::Wait) | None => break,
    ///             Some(_) => {}
    ///         }
    ///     }
    /// }
    ///
    /// // The continuation keeps the full values; the observer gets only the data.
    /// assert_eq!(
    ///     forwarded,
    ///     vec![RequestIntro::Pending, RequestIntro::Success { status: 200 }]
    /// );
    /// assert_eq!(observed, vec![0, 200]);
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
        D: Clone + 'static,
        Self::Pending: Clone,
        F: Fn(&Self::Ready) -> (CollectionState, Option<D>) + 'static;

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
    /// - `M` must be `Clone +  'static` (observer gets transformed copy)
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
    /// ```
    /// use foundation_core::valtron::{Ctx, Stream, TaskIterator, TaskIteratorExt, TaskStatus};
    ///
    /// # #[derive(Debug, PartialEq)]
    /// # struct Item {
    /// #     id: u64,
    /// #     important: bool,
    /// # }
    /// # impl Item {
    /// #     fn is_important(&self) -> bool {
    /// #         self.important
    /// #     }
    /// #     fn id(&self) -> u64 {
    /// #         self.id
    /// #     }
    /// # }
    /// # struct ItemTask(std::vec::IntoIter<TaskStatus<Item, String>>);
    /// # impl ItemTask {
    /// #     fn new(items: Vec<TaskStatus<Item, String>>) -> Self {
    /// #         Self(items.into_iter())
    /// #     }
    /// # }
    /// # impl TaskIterator for ItemTask {
    /// #     type Ready = Item;
    /// #     type Pending = String;
    /// #     fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
    /// #         self.0.next()
    /// #     }
    /// # }
    /// let ctx = Ctx::new();
    /// let task = ItemTask::new(vec![
    ///     TaskStatus::Ready(Item { id: 1, important: false }),
    ///     TaskStatus::Ready(Item { id: 2, important: true }),
    ///     TaskStatus::Ready(Item { id: 3, important: true }),
    /// ]);
    /// // Observer gets u64 ids, continuation gets full Item
    /// let (mut observer, mut continuation) = task
    ///     .split_collector_map(
    ///         |item| (item.is_important(), Some(item.id())),
    ///         10
    ///     );
    ///
    /// let mut forwarded = Vec::new();
    /// let mut observed = Vec::new();
    /// while let Some(status) = continuation.next_status(ctx.clone()) {
    ///     if let TaskStatus::Ready(item) = status {
    ///         forwarded.push(item);
    ///     }
    ///     // Drain the observer so a full queue never parks the continuation.
    ///     loop {
    ///         match observer.next() {
    ///             Some(Stream::Next(item)) => observed.push(item),
    ///             Some(Stream::Wait) | None => break,
    ///             Some(_) => {}
    ///         }
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     forwarded,
    ///     vec![
    ///         Item { id: 1, important: false },
    ///         Item { id: 2, important: true },
    ///         Item { id: 3, important: true },
    ///     ]
    /// );
    /// assert_eq!(observed, vec![2, 3]);
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
        M: Clone + 'static,
        Self::Pending: Clone,
        F: Fn(&Self::Ready) -> (bool, Option<M>) + 'static;

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
        M: Clone + 'static,
        Self::Pending: Clone,
        F: Fn(&Self::Ready) -> (bool, Option<M>) + 'static,
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
    /// ```
    /// use foundation_core::valtron::{
    ///     Ctx, TaskIterator, TaskIteratorExt, TaskShortCircuit, TaskStatus,
    /// };
    ///
    /// # #[derive(Debug, PartialEq)]
    /// # struct Response {
    /// #     status: u16,
    /// # }
    /// # impl Response {
    /// #     fn is_error(&self) -> bool {
    /// #         self.status >= 400
    /// #     }
    /// # }
    /// # struct ResponseTask(std::vec::IntoIter<TaskStatus<Response, String>>);
    /// # impl ResponseTask {
    /// #     fn new(items: Vec<TaskStatus<Response, String>>) -> Self {
    /// #         Self(items.into_iter())
    /// #     }
    /// # }
    /// # impl TaskIterator for ResponseTask {
    /// #     type Ready = Response;
    /// #     type Pending = String;
    /// #     fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
    /// #         self.0.next()
    /// #     }
    /// # }
    /// let ctx = Ctx::new();
    /// let my_task = ResponseTask::new(vec![
    ///     TaskStatus::Ready(Response { status: 200 }),
    ///     TaskStatus::Pending("retrying".to_string()),
    ///     TaskStatus::Ready(Response { status: 500 }),
    ///     TaskStatus::Ready(Response { status: 200 }),
    /// ]);
    /// let mut task = my_task
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
    ///
    /// assert_eq!(
    ///     task.next_status(ctx.clone()),
    ///     Some(TaskStatus::Ready(Response { status: 200 }))
    /// );
    /// assert_eq!(
    ///     task.next_status(ctx.clone()),
    ///     Some(TaskStatus::Pending("retrying".to_string()))
    /// );
    /// // The first error is returned, and the task stops after it.
    /// assert_eq!(
    ///     task.next_status(ctx.clone()),
    ///     Some(TaskStatus::Ready(Response { status: 500 }))
    /// );
    /// assert_eq!(task.next_status(ctx.clone()), None);
    /// ```
    fn map_circuit<F>(self, f: F) -> TMapCircuit<Self>
    where
        Self: Sized,
        F: Fn(
                TaskStatus<Self::Ready, Self::Pending>,
            ) -> TaskShortCircuit<Self::Ready, Self::Pending>
            + 'static;

    /// Flatten nested iterator patterns where outer yields inner iterators.
    ///
    /// The mapper function transforms each `Ready` value into an inner iterator.
    /// The returned `TMapIter` drains each inner iterator until `None`, then polls
    /// the outer for the next item.
    ///
    /// Non-Ready states (Pending, Spawn) from the outer iterator pass through via `Into`,
    /// so `Self::Pending` must be convertible to `InnerP`.
    ///
    /// ## Example
    ///
    /// ```
    /// use foundation_core::valtron::{Ctx, TaskIterator, TaskIteratorExt, TaskStatus};
    ///
    /// # struct ChunkTask(std::vec::IntoIter<TaskStatus<Vec<u8>, String>>);
    /// # impl ChunkTask {
    /// #     fn new(items: Vec<TaskStatus<Vec<u8>, String>>) -> Self {
    /// #         Self(items.into_iter())
    /// #     }
    /// # }
    /// # impl TaskIterator for ChunkTask {
    /// #     type Ready = Vec<u8>;
    /// #     type Pending = String;
    /// #     fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
    /// #         self.0.next()
    /// #     }
    /// # }
    /// let ctx = Ctx::new();
    /// let task = ChunkTask::new(vec![
    ///     TaskStatus::Ready(vec![1, 2]),
    ///     TaskStatus::Ready(vec![3]),
    ///     TaskStatus::Ready(Vec::new()),
    ///     TaskStatus::Ready(vec![4]),
    /// ]);
    /// // Outer yields Ready(Vec<u8>); the mapper returns an inner iterator
    /// // that yields each byte as a Ready status.
    /// let mut flattened = task.map_iter(|values: Vec<u8>| {
    ///     values
    ///         .into_iter()
    ///         .map(|value| TaskStatus::<u8, String>::Ready(value))
    /// });
    ///
    /// let mut ready = Vec::new();
    /// while let Some(status) = flattened.next_status(ctx.clone()) {
    ///     if let TaskStatus::Ready(value) = status {
    ///         ready.push(value);
    ///     }
    /// }
    /// assert_eq!(ready, vec![1, 2, 3, 4]);
    /// ```
    fn map_iter<F, InnerIter, InnerR, InnerP>(
        self,
        mapper: F,
    ) -> TMapIter<Self, F, InnerIter, InnerR, InnerP, Self::Ready, Self::Pending>
    where
        Self: Sized,
        F: Fn(Self::Ready) -> InnerIter + 'static,
        InnerIter: Iterator<Item = TaskStatus<InnerR, InnerP>> + 'static,
        InnerR: 'static,
        InnerP: 'static,
        Self::Pending: Into<InnerP> + 'static;

    /// Flatten Ready values that implement `IntoIterator`.
    ///
    /// Input:  `TaskIterator`<Ready = `Vec<M>`, Pending = P>
    /// Output: `TaskIterator`<Ready = M, Pending = P>
    fn flatten_ready(self) -> TFlattenReady<Self>
    where
        Self: Sized,
        Self::Ready: IntoIterator,
        <Self::Ready as IntoIterator>::Item: 'static;

    /// Flatten Pending values that implement `IntoIterator`.
    ///
    /// Input:  `TaskIterator`<Ready = D, Pending = `Vec<M>`>
    /// Output: `TaskIterator`<Ready = D, Pending = M>
    fn flatten_pending(self) -> TFlattenPending<Self>
    where
        Self: Sized,
        Self::Pending: IntoIterator,
        <Self::Pending as IntoIterator>::Item: 'static;

    /// Flat map Ready values - transform and flatten in one operation.
    ///
    /// Input:  `TaskIterator`<Ready = D, Pending = P>
    /// Output: `TaskIterator`<Ready = `U::Item`, Pending = P>
    fn flat_map_ready<F, U>(self, f: F) -> TFlatMapReady<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::Ready) -> U + 'static,
        U: IntoIterator,
        U::Item: 'static;

    /// Flat map Pending values - transform and flatten in one operation.
    ///
    /// Input:  `TaskIterator`<Ready = D, Pending = P>
    /// Output: `TaskIterator`<Ready = D, Pending = `U::Item`>
    fn flat_map_pending<F, U>(self, f: F) -> TFlatMapPending<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::Pending) -> U + 'static,
        U: IntoIterator,
        U::Item: 'static;

    // ===== Feature 08: Iterator Extension Completion =====

    /// Transform any `TaskStatus` with full state access.
    fn map_state<F>(self, f: F) -> TMapState<Self, F>
    where
        F: Fn(TaskStatus<Self::Ready, Self::Pending>) -> TaskStatus<Self::Ready, Self::Pending>
            + 'static;

    /// Side-effect on any `TaskStatus`.
    fn inspect_state<F>(self, f: F) -> TInspectState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending>) + 'static;

    /// Filter based on full `TaskStatus`. Non-matching items return `TaskStatus::Ignore`.
    fn filter_state<F>(self, f: F) -> TFilterState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending>) -> bool + 'static;

    /// Take items while state predicate returns true.
    fn take_while_state<F>(self, predicate: F) -> TTakeWhileState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending>) -> bool + 'static;

    /// Transform items until transformer returns None.
    /// When transformer returns None, iterator terminates.
    fn transform_until<F>(self, transformer: F) -> TTransformUntil<Self, F>
    where
        F: Fn(
                TaskStatus<Self::Ready, Self::Pending>,
            ) -> Option<TaskStatus<Self::Ready, Self::Pending>>
            + 'static;

    /// Skip items while state predicate returns true.
    fn skip_while_state<F>(self, predicate: F) -> TSkipWhileState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending>) -> bool + 'static;

    /// Take at most n items matching state predicate.
    fn take_state<F>(self, n: usize, state_predicate: F) -> TTakeState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending>) -> bool + 'static;

    /// Skip first n items matching state predicate.
    fn skip_state<F>(self, n: usize, state_predicate: F) -> TSkipState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending>) -> bool + 'static;

    /// Take while predicate true on Ready values, pass through non-Ready.
    fn take<F>(self, n: usize) -> TTakeState<Self, BoxedTaskPredicate<Self::Ready, Self::Pending>>
    where
        Self::Ready: 'static,
        Self::Pending: 'static,
    {
        let predicate: BoxedTaskPredicate<Self::Ready, Self::Pending> =
            Box::new(|s| matches!(s, TaskStatus::Ready(_)));
        self.take_state(n, predicate)
    }

    /// Take at most n items of any state.
    fn take_all(self, n: usize) -> TTakeState<Self, BoxedTaskPredicate<Self::Ready, Self::Pending>>
    where
        Self::Ready: 'static,
        Self::Pending: 'static,
    {
        let predicate: BoxedTaskPredicate<Self::Ready, Self::Pending> = Box::new(|_| true);
        self.take_state(n, predicate)
    }

    /// Skip first n Ready items, return all others unchanged.
    fn skip(self, n: usize) -> TSkipState<Self, BoxedTaskPredicate<Self::Ready, Self::Pending>>
    where
        Self::Ready: 'static,
        Self::Pending: 'static,
    {
        let predicate: BoxedTaskPredicate<Self::Ready, Self::Pending> =
            Box::new(|s| matches!(s, TaskStatus::Ready(_)));
        self.skip_state(n, predicate)
    }

    /// Skip first n items of any state.
    fn skip_all(self, n: usize) -> TSkipState<Self, BoxedTaskPredicate<Self::Ready, Self::Pending>>
    where
        Self::Ready: 'static,
        Self::Pending: 'static,
    {
        let predicate: BoxedTaskPredicate<Self::Ready, Self::Pending> = Box::new(|_| true);
        self.skip_state(n, predicate)
    }

    /// Take while predicate true on Ready values, pass through non-Ready.
    fn take_while<F>(
        self,
        f: F,
    ) -> TTakeWhileState<Self, BoxedTaskPredicate<Self::Ready, Self::Pending>>
    where
        F: Fn(&Self::Ready) -> bool + 'static,
        Self::Ready: 'static,
        Self::Pending: 'static,
    {
        let predicate: BoxedTaskPredicate<Self::Ready, Self::Pending> =
            Box::new(move |s| match s {
                TaskStatus::Ready(v) => f(v),
                _ => true,
            });
        self.take_while_state(predicate)
    }

    /// Take while predicate true on ANY state.
    fn take_while_any<F>(self, f: F) -> TTakeWhileState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending>) -> bool + 'static,
    {
        self.take_while_state(f)
    }

    /// Skip while predicate true on Ready values, return all others.
    fn skip_while<F>(
        self,
        f: F,
    ) -> TSkipWhileState<Self, BoxedTaskPredicate<Self::Ready, Self::Pending>>
    where
        F: Fn(&Self::Ready) -> bool + 'static,
        Self::Ready: 'static,
        Self::Pending: 'static,
    {
        let predicate: BoxedTaskPredicate<Self::Ready, Self::Pending> =
            Box::new(move |s| match s {
                TaskStatus::Ready(v) => f(v),
                _ => false,
            });
        self.skip_while_state(predicate)
    }

    /// Skip while predicate true on ANY state.
    fn skip_while_any<F>(self, f: F) -> TSkipWhileState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending>) -> bool + 'static,
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
        F: Fn(&Self::Ready) -> bool + 'static;

    /// Find first item mapping to Some value.
    fn find_map<F, R>(self, f: F) -> TFindMap<Self, F, R>
    where
        F: Fn(Self::Ready) -> Option<R> + 'static,
        R: 'static;

    /// Fold/accumulate values. Returns final accumulator when done.
    fn fold<F, R>(self, init: R, f: F) -> TFold<Self, F, R>
    where
        F: Fn(R, Self::Ready) -> R + 'static,
        R: 'static;

    /// Check if all Ready items satisfy predicate.
    fn all<F>(self, f: F) -> TAll<Self, F>
    where
        F: Fn(Self::Ready) -> bool + 'static;

    /// Check if any Ready item satisfies predicate.
    fn any<F>(self, f: F) -> TAny<Self, F>
    where
        F: Fn(Self::Ready) -> bool + 'static;

    /// Count Ready items.
    fn count(self) -> TCount<Self>;

    /// Count all items (any state).
    fn count_all(self) -> TCountAll<Self>;

    fn into_task_iter(self) -> crate::valtron::TaskIter<Self>
    where
        Self: Sized,
    {
        crate::valtron::TaskIter(self)
    }

    fn try_map_ready<F, R>(self, f: F) -> TTryMapReady<Self, R>
    where
        F: Fn(Self::Ready) -> Option<R> + 'static,
        R: 'static;
}

// Blanket implementation: anything implementing TaskIterator gets TaskIteratorExt
impl<I> TaskIteratorExt for I
where
    I: TaskIterator + 'static,
    I::Ready: 'static,
    I::Pending: 'static,
{
    fn map_ready<F, R>(self, f: F) -> TMapReady<Self, R>
    where
        F: Fn(Self::Ready) -> R + 'static,
        R: 'static,
    {
        TMapReady {
            inner: self,
            mapper: Box::new(f),
        }
    }

    fn map_pending<F, R>(self, f: F) -> TMapPending<Self, R>
    where
        F: Fn(Self::Pending) -> R + 'static,
        R: 'static,
    {
        TMapPending {
            inner: self,
            mapper: Box::new(f),
        }
    }

    fn filter_ready<F>(self, f: F) -> TFilterReady<Self>
    where
        F: Fn(&Self::Ready) -> bool + 'static,
    {
        TFilterReady {
            inner: self,
            predicate: Box::new(f),
        }
    }

    fn stream_collect(self) -> TStreamCollect<Self>
    where
        Self::Ready: 'static,
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
        P: Fn(&Self::Ready) -> bool + 'static,
    {
        let queue = EventQueue::bounded(queue_size);
        tracing::debug!(
            "split_collector: creating observer and continuation with queue_size={}",
            queue_size
        );

        let observer = CollectorStreamIterator {
            queue: queue.clone(),
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
        P: Fn(&Self::Ready) -> bool + 'static,
    {
        self.split_collector(predicate, 1)
    }

    fn broadcast<P>(self, n: usize, predicate: P, queue_size: usize) -> BroadcastPair<Self>
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> bool + 'static,
    {
        broadcast_build(
            self,
            n,
            Box::new(predicate),
            queue_size,
            BroadcastPolicy::Backpressure,
        )
    }

    fn broadcast_lossy<P>(self, n: usize, predicate: P, queue_size: usize) -> BroadcastPair<Self>
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> bool + 'static,
    {
        broadcast_build(
            self,
            n,
            Box::new(predicate),
            queue_size,
            BroadcastPolicy::DropNewest,
        )
    }

    fn broadcast_latest<P>(self, n: usize, predicate: P, queue_size: usize) -> BroadcastPair<Self>
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> bool + 'static,
    {
        broadcast_build(
            self,
            n,
            Box::new(predicate),
            queue_size,
            BroadcastPolicy::DropOldest,
        )
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
        P: Fn(&Self::Ready) -> CollectionState + 'static,
    {
        let queue = EventQueue::bounded(queue_size);
        tracing::debug!(
            "split_collect_until: creating observer and continuation with queue_size={}",
            queue_size
        );

        let observer = SplitUntilObserver {
            queue: queue.clone(),
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
        D: Clone + 'static,
        Self::Pending: Clone,
        F: Fn(&Self::Ready) -> (CollectionState, Option<D>) + 'static,
    {
        let queue = EventQueue::bounded(queue_size);
        tracing::debug!(
            "split_collect_until_map: creating observer and continuation with queue_size={}",
            queue_size
        );

        let observer = SplitUntilObserverMap {
            queue: queue.clone(),
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
        M: Clone + 'static,
        Self::Pending: Clone,
        F: Fn(&Self::Ready) -> (bool, Option<M>) + 'static,
    {
        let queue = EventQueue::bounded(queue_size);
        tracing::debug!(
            "split_collector_map: creating observer and continuation with queue_size={}",
            queue_size
        );

        let observer = SplitCollectorMapObserver {
            queue: queue.clone(),
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
        M: Clone + 'static,
        Self::Pending: Clone,
        F: Fn(&Self::Ready) -> (bool, Option<M>) + 'static,
    {
        self.split_collector_map(transform, 1)
    }

    fn map_circuit<F>(self, f: F) -> TMapCircuit<Self>
    where
        Self: Sized,
        F: Fn(
                TaskStatus<Self::Ready, Self::Pending>,
            ) -> TaskShortCircuit<Self::Ready, Self::Pending>
            + 'static,
    {
        TMapCircuit {
            inner: self,
            circuit: Box::new(f),
            stopped: false,
            _phantom: std::marker::PhantomData,
        }
    }

    fn map_iter<F, InnerIter, InnerR, InnerP>(
        self,
        mapper: F,
    ) -> TMapIter<Self, F, InnerIter, InnerR, InnerP, Self::Ready, Self::Pending>
    where
        Self: Sized,
        F: Fn(Self::Ready) -> InnerIter + 'static,
        InnerIter: Iterator<Item = TaskStatus<InnerR, InnerP>> + 'static,
        InnerR: 'static,
        InnerP: 'static,
        Self::Pending: Into<InnerP> + 'static,
    {
        TMapIter {
            outer: self,
            mapper,
            current_inner: None,
            spread_queue: std::collections::VecDeque::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    fn flatten_ready(self) -> TFlattenReady<Self>
    where
        Self: Sized,
        Self::Ready: IntoIterator,
        <Self::Ready as IntoIterator>::Item: 'static,
    {
        TFlattenReady {
            inner: self,
            current_inner: None,
            spread_buffer: std::collections::VecDeque::new(),
        }
    }

    fn flatten_pending(self) -> TFlattenPending<Self>
    where
        Self: Sized,
        Self::Pending: IntoIterator,
        <Self::Pending as IntoIterator>::Item: 'static,
    {
        TFlattenPending {
            inner: self,
            current_inner: None,
            spread_buffer: std::collections::VecDeque::new(),
        }
    }

    fn flat_map_ready<F, U>(self, f: F) -> TFlatMapReady<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::Ready) -> U + 'static,
        U: IntoIterator,
        U::Item: 'static,
    {
        TFlatMapReady {
            inner: self,
            mapper: f,
            current_inner: None,
            spread_queue: std::collections::VecDeque::new(),
        }
    }

    fn flat_map_pending<F, U>(self, f: F) -> TFlatMapPending<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::Pending) -> U + 'static,
        U: IntoIterator,
        U::Item: 'static,
    {
        TFlatMapPending {
            inner: self,
            mapper: f,
            current_inner: None,
            spread_queue: std::collections::VecDeque::new(),
        }
    }

    // ===== Feature 08 implementations =====

    fn map_state<F>(self, f: F) -> TMapState<Self, F>
    where
        F: Fn(TaskStatus<Self::Ready, Self::Pending>) -> TaskStatus<Self::Ready, Self::Pending>
            + 'static,
    {
        TMapState {
            inner: self,
            mapper: f,
        }
    }

    fn inspect_state<F>(self, f: F) -> TInspectState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending>) + 'static,
    {
        TInspectState {
            inner: self,
            inspector: f,
        }
    }

    fn filter_state<F>(self, f: F) -> TFilterState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending>) -> bool + 'static,
    {
        TFilterState {
            inner: self,
            predicate: f,
        }
    }

    fn take_while_state<F>(self, predicate: F) -> TTakeWhileState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending>) -> bool + 'static,
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
                TaskStatus<Self::Ready, Self::Pending>,
            ) -> Option<TaskStatus<Self::Ready, Self::Pending>>
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
        F: Fn(&TaskStatus<Self::Ready, Self::Pending>) -> bool + 'static,
    {
        TSkipWhileState {
            inner: self,
            predicate,
            done_skipping: false,
        }
    }

    fn take_state<F>(self, n: usize, state_predicate: F) -> TTakeState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending>) -> bool + 'static,
    {
        TTakeState {
            inner: self,
            remaining: n,
            state_predicate,
        }
    }

    fn skip_state<F>(self, n: usize, state_predicate: F) -> TSkipState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending>) -> bool + 'static,
    {
        TSkipState {
            inner: self,
            to_skip: n,
            state_predicate,
        }
    }

    fn find<F>(self, predicate: F) -> TFind<Self, F>
    where
        F: Fn(&Self::Ready) -> bool + 'static,
    {
        TFind {
            inner: self,
            predicate,
            found: false,
        }
    }

    fn find_map<F, R>(self, f: F) -> TFindMap<Self, F, R>
    where
        F: Fn(Self::Ready) -> Option<R> + 'static,
        R: 'static,
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
        F: Fn(R, Self::Ready) -> R + 'static,
        R: 'static,
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
        F: Fn(Self::Ready) -> bool + 'static,
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
        F: Fn(Self::Ready) -> bool + 'static,
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
        F: Fn(Self::Ready) -> Option<R> + 'static,
        R: 'static,
    {
        TTryMapReady {
            inner: self,
            mapper: Box::new(f),
            done: false,
        }
    }
}

// ============================================================================
// TTryMapReady — Ready → Option<R>, exhausts on None
// ============================================================================

pub struct TTryMapReady<I: TaskIterator, R> {
    inner: I,
    mapper: Box<dyn Fn(I::Ready) -> Option<R>>,
    done: bool,
}

impl<I, R> TaskIterator for TTryMapReady<I, R>
where
    I: TaskIterator,
    R: 'static,
{
    type Ready = R;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.done {
            return None;
        }
        let status = self.inner.next_status(ctx)?;
        Some(match status {
            TaskStatus::Ready(v) => {
                if let Some(r) = (self.mapper)(v) {
                    TaskStatus::Ready(r)
                } else {
                    self.done = true;
                    return None;
                }
            }
            TaskStatus::Spread(items) => {
                let mut any_none = false;
                let mapped: Vec<_> = items
                    .into_iter()
                    .filter_map(|item| match item {
                        TaskSpread::Ready(d) => {
                            if let Some(r) = (self.mapper)(d) {
                                Some(TaskSpread::Ready(r))
                            } else {
                                any_none = true;
                                None
                            }
                        }
                        TaskSpread::Pending(p) => Some(TaskSpread::Pending(p)),
                    })
                    .collect();
                if any_none {
                    self.done = true;
                }
                TaskStatus::Spread(mapped)
            }
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
    mapper: Box<dyn Fn(I::Ready) -> R>,
}

impl<I, R> TaskIterator for TMapReady<I, R>
where
    I: TaskIterator,
    R: 'static,
{
    type Ready = R;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.inner.next_status(ctx).map(|status| match status {
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
    mapper: Box<dyn Fn(I::Pending) -> R>,
}

impl<I, R> TaskIterator for TMapPending<I, R>
where
    I: TaskIterator,
    R: 'static,
{
    type Ready = I::Ready;
    type Pending = R;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.inner.next_status(ctx).map(|status| match status {
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
    predicate: BoxedReadyPredicate<I>,
}

impl<I> TaskIterator for TFilterReady<I>
where
    I: TaskIterator,
{
    type Ready = I::Ready;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        let status = self.inner.next_status(ctx)?;
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
pub struct TMapIter<I, F, InnerIter, InnerR, InnerP, R, P>
where
    I: TaskIterator<Ready = R, Pending = P>,
    F: Fn(R) -> InnerIter,
    InnerIter: Iterator<Item = TaskStatus<InnerR, InnerP>>,
{
    outer: I,
    mapper: F,
    current_inner: Option<InnerIter>,
    spread_queue: std::collections::VecDeque<TaskStatus<InnerR, InnerP>>,
    _phantom: std::marker::PhantomData<(InnerR, InnerP, R, P)>,
}

impl<I, F, InnerIter, InnerR, InnerP, R, P> TaskIterator
    for TMapIter<I, F, InnerIter, InnerR, InnerP, R, P>
where
    I: TaskIterator<Ready = R, Pending = P>,
    F: Fn(R) -> InnerIter,
    InnerIter: Iterator<Item = TaskStatus<InnerR, InnerP>>,
    InnerR: 'static,
    InnerP: 'static,
    P: Into<InnerP> + 'static,
{
    type Ready = InnerR;
    type Pending = InnerP;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        loop {
            // First, drain spread queue
            if let Some(item) = self.spread_queue.pop_front() {
                return Some(item);
            }

            // Next, try to drain the current inner iterator
            if let Some(ref mut inner) = self.current_inner {
                if let Some(item) = inner.next() {
                    return Some(item);
                }
                // Inner exhausted, clear it and continue to poll outer
                self.current_inner = None;
            }

            // Poll outer for next item
            match self.outer.next_status(ctx.clone()) {
                Some(TaskStatus::Ready(d)) => {
                    // Mapper returns a new inner iterator, start draining it
                    let new_inner = (self.mapper)(d);
                    self.current_inner = Some(new_inner);
                }
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
                            pending_items.into_iter().map(TaskSpread::Pending).collect(),
                        ));
                    }
                    if has_ready {
                        return Some(TaskStatus::Ignore);
                    }
                    // Empty spread
                    return Some(TaskStatus::Spread(Vec::new()));
                }
                Some(TaskStatus::Pending(p)) => return Some(TaskStatus::Pending(p.into())),
                Some(TaskStatus::Delayed(d)) => return Some(TaskStatus::Delayed(d)),
                Some(TaskStatus::Init) => return Some(TaskStatus::Init),
                Some(TaskStatus::Ignore) => return Some(TaskStatus::Ignore),
                Some(TaskStatus::Spawn(s)) => return Some(TaskStatus::Spawn(s)),
                Some(TaskStatus::Wait) => return Some(TaskStatus::Wait),
                Some(TaskStatus::Depends(signal)) => return Some(TaskStatus::Depends(signal)),
                None => return None, // Outer exhausted
            }
        }
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
    spread_buffer: std::collections::VecDeque<<I::Ready as IntoIterator>::Item>,
}

impl<I> TaskIterator for TFlattenReady<I>
where
    I: TaskIterator,
    I::Ready: IntoIterator,
    <I::Ready as IntoIterator>::Item: 'static,
    I::Pending: 'static,
{
    type Ready = <I::Ready as IntoIterator>::Item;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        // First drain spread buffer
        if let Some(item) = self.spread_buffer.pop_front() {
            return Some(TaskStatus::Ready(item));
        }

        // Next drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(TaskStatus::Ready(item));
            }
            // Inner exhausted - clear and fall through to poll outer
            self.current_inner = None;
        }

        // Get next from outer iterator
        let status = self.inner.next_status(ctx)?;

        match status {
            TaskStatus::Ready(iterable) => {
                // Store iterator, drain on NEXT call
                self.current_inner = Some(iterable.into_iter());
                // Return Ignore to signal "still working, no Ready value yet"
                Some(TaskStatus::Ignore)
            }
            TaskStatus::Spread(items) => {
                // Flatten Ready items, pass through Pending
                Some(TaskStatus::Spread(
                    items
                        .into_iter()
                        .flat_map(|item| match item {
                            TaskSpread::Ready(iterable) => iterable
                                .into_iter()
                                .map(TaskSpread::Ready)
                                .collect::<Vec<_>>(),
                            TaskSpread::Pending(p) => vec![TaskSpread::Pending(p)],
                        })
                        .collect(),
                ))
            }
            TaskStatus::Pending(p) => Some(TaskStatus::Pending(p)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
            TaskStatus::Wait => Some(TaskStatus::Wait),
            TaskStatus::Depends(signal) => Some(TaskStatus::Depends(signal)),
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
    spread_buffer: std::collections::VecDeque<<I::Pending as IntoIterator>::Item>,
}

impl<I> TaskIterator for TFlattenPending<I>
where
    I: TaskIterator,
    I::Pending: IntoIterator,
    <I::Pending as IntoIterator>::Item: 'static,
    I::Ready: 'static,
{
    type Ready = I::Ready;
    type Pending = <I::Pending as IntoIterator>::Item;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        // First drain spread buffer
        if let Some(item) = self.spread_buffer.pop_front() {
            return Some(TaskStatus::Pending(item));
        }

        // Next drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(TaskStatus::Pending(item));
            }
            self.current_inner = None;
        }

        let status = self.inner.next_status(ctx)?;

        match status {
            TaskStatus::Pending(iterable) => {
                self.current_inner = Some(iterable.into_iter());
                Some(TaskStatus::Ignore)
            }
            TaskStatus::Spread(items) => {
                // Flatten Pending items, pass through Ready
                Some(TaskStatus::Spread(
                    items
                        .into_iter()
                        .flat_map(|item| match item {
                            TaskSpread::Ready(d) => vec![TaskSpread::Ready(d)],
                            TaskSpread::Pending(iterable) => iterable
                                .into_iter()
                                .map(TaskSpread::Pending)
                                .collect::<Vec<_>>(),
                        })
                        .collect(),
                ))
            }
            // Pass through non-Pending states
            TaskStatus::Ready(r) => Some(TaskStatus::Ready(r)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
            TaskStatus::Wait => Some(TaskStatus::Wait),
            TaskStatus::Depends(signal) => Some(TaskStatus::Depends(signal)),
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
    spread_queue: std::collections::VecDeque<TaskStatus<U::Item, I::Pending>>,
}

impl<I, F, U> TaskIterator for TFlatMapReady<I, F, U>
where
    I: TaskIterator,
    F: Fn(I::Ready) -> U + 'static,
    U: IntoIterator,
    U::Item: 'static,
    I::Pending: 'static,
{
    type Ready = U::Item;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        // First drain spread queue
        if let Some(item) = self.spread_queue.pop_front() {
            return Some(item);
        }

        // Next drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(TaskStatus::Ready(item));
            }
            self.current_inner = None;
        }

        let status = self.inner.next_status(ctx)?;

        match status {
            TaskStatus::Ready(v) => {
                let iterable = (self.mapper)(v);
                self.current_inner = Some(iterable.into_iter());
                Some(TaskStatus::Ignore)
            }
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
            // Pass through non-Ready states
            TaskStatus::Pending(p) => Some(TaskStatus::Pending(p)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
            TaskStatus::Wait => Some(TaskStatus::Wait),
            TaskStatus::Depends(signal) => Some(TaskStatus::Depends(signal)),
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
    spread_queue: std::collections::VecDeque<TaskStatus<I::Ready, U::Item>>,
}

impl<I, F, U> TaskIterator for TFlatMapPending<I, F, U>
where
    I: TaskIterator,
    F: Fn(I::Pending) -> U + 'static,
    U: IntoIterator,
    U::Item: 'static,
    I::Ready: 'static,
{
    type Ready = I::Ready;
    type Pending = U::Item;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        // First drain spread queue
        if let Some(item) = self.spread_queue.pop_front() {
            return Some(item);
        }

        // Next drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(TaskStatus::Pending(item));
            }
            self.current_inner = None;
        }

        let status = self.inner.next_status(ctx)?;

        match status {
            TaskStatus::Pending(v) => {
                let iterable = (self.mapper)(v);
                self.current_inner = Some(iterable.into_iter());
                Some(TaskStatus::Ignore)
            }
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
            // Pass through non-Pending states
            TaskStatus::Ready(r) => Some(TaskStatus::Ready(r)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
            TaskStatus::Wait => Some(TaskStatus::Wait),
            TaskStatus::Depends(signal) => Some(TaskStatus::Depends(signal)),
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

impl<I> TaskIterator for TStreamCollect<I>
where
    I: TaskIterator,
    I::Ready: 'static,
{
    type Ready = Vec<I::Ready>;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        // If we've already yielded the collected result, we're done
        if self.done {
            return None;
        }

        match self.inner.next_status(ctx) {
            Some(TaskStatus::Ready(value)) => {
                self.collected.push(value);
                // Keep collecting, return Ignore to signal collected but continue
                Some(TaskStatus::Ignore)
            }
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
            Some(TaskStatus::Pending(p)) => Some(TaskStatus::Pending(p)),
            Some(TaskStatus::Delayed(d)) => Some(TaskStatus::Delayed(d)),
            Some(TaskStatus::Init) => Some(TaskStatus::Init),
            Some(TaskStatus::Spawn(s)) => Some(TaskStatus::Spawn(s)),
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            Some(TaskStatus::Wait) => Some(TaskStatus::Wait),
            Some(TaskStatus::Depends(signal)) => Some(TaskStatus::Depends(signal)),
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
fn split_observer_push<T>(queue: &EventQueue<T>, item: T) -> Option<T> {
    match queue.push(item) {
        Ok(()) | Err(concurrent_queue::PushError::Closed(_)) => None,
        Err(concurrent_queue::PushError::Full(item)) => Some(item),
    }
}

/// Observer branch from `split_collector()`.
///
/// Receives copies of items matching the predicate via a `ConcurrentQueue`.
/// Yields `Stream::Next` for matched items, forwards Pending/Delayed from source.
pub struct CollectorStreamIterator<D, P> {
    /// Shared queue receiving copied items from the splitter
    queue: EventQueue<Stream<D, P>>,
}

impl<D, P> CollectorStreamIterator<D, P> {
    /// WHY: A task-path consumer parks on an observer natively via
    /// `Depends(QueueReadiness)` (F45 Resolution 3), but the observer's queue is
    /// private. WHAT: expose a [`QueueReadiness`] over the shared queue. HOW:
    /// clones the `EventQueue<..>` the observer reads from.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn readiness(&self) -> QueueReadiness<Stream<D, P>> {
        self.queue.readiness()
    }
}

impl<D, P> Iterator for CollectorStreamIterator<D, P>
where
    D: Clone + 'static,
    P: Clone + 'static,
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
        let _ = self.queue.close();
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
    queue: EventQueue<Stream<I::Ready, I::Pending>>,
    /// Predicate to determine which items to copy
    predicate: BoxedReadyPredicate<I>,
    /// A matched observer item that could not be delivered because the observer
    /// queue was full; retried on the next `next()` before pulling new work
    /// (F45 Part C1 backpressure).
    pending_push: Option<Stream<I::Ready, I::Pending>>,
    /// The source item held back while `pending_push` is being retried, forwarded
    /// downstream only once the observer accepts the copy (keeps both branches in
    /// lockstep).
    pending_forward: Option<TaskStatus<I::Ready, I::Pending>>,
}

impl<I> TaskIterator for SplitCollectorContinuation<I>
where
    I: TaskIterator,
    I::Ready: Clone + 'static,
    I::Pending: Clone + 'static,
{
    type Ready = I::Ready;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        // Retry a stashed observer push before pulling new work: a full observer
        // parks the source on vacancy instead of dropping the item (F45 Part C1).
        if let Some(stream_item) = self.pending_push.take() {
            if let Some(stashed) = split_observer_push(&self.queue, stream_item) {
                self.pending_push = Some(stashed);
                return Some(TaskStatus::Depends(Arc::new(self.queue.vacancy())));
            }
            return self.pending_forward.take();
        }

        let Some(item) = self.inner.next_status(ctx) else {
            // Source iterator is naturally exhausted, close the queue
            let _ = self.queue.close();
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
            return Some(TaskStatus::Depends(Arc::new(self.queue.vacancy())));
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
        let _ = self.queue.close();
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
    predicate: BoxedReadyPredicate<I>,
    queue_size: usize,
    policy: BroadcastPolicy,
) -> BroadcastPair<I>
where
    I: TaskIterator,
{
    let cap = queue_size.max(1);
    let mut observers = Vec::with_capacity(n);
    let mut queues = Vec::with_capacity(n);
    for _ in 0..n {
        let queue = EventQueue::bounded(cap);
        observers.push(CollectorStreamIterator {
            queue: queue.clone(),
        });
        queues.push(queue);
    }
    tracing::debug!("broadcast: {n} branches, queue_size={cap}, policy={policy}");
    let continuation = BroadcastContinuation {
        inner,
        queues,
        predicate,
        policy,
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
    queues: Vec<EventQueue<Stream<I::Ready, I::Pending>>>,
    /// Which `Ready` values to fan out.
    predicate: BoxedReadyPredicate<I>,
    /// How a full branch is handled (park / drop-newest / drop-oldest).
    policy: BroadcastPolicy,
    /// A matched value awaiting delivery to all branches (zero-loss backpressure):
    /// held until **every** open branch has vacancy, then broadcast in one shot.
    pending_value: Option<I::Ready>,
    /// The source item held back until `pending_value` is delivered (lockstep).
    pending_forward: Option<TaskStatus<I::Ready, I::Pending>>,
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
            let _ = queue.close();
        }
    }
}

impl<I> BroadcastContinuation<I>
where
    I: TaskIterator,
    I::Ready: Clone,
    I::Pending: Clone,
{
    /// Deliver one clone of `value` to every open branch per [`BroadcastPolicy`].
    ///
    /// `Backpressure` is only called once all open branches have vacancy, so its
    /// `push` always succeeds; `DropNewest` lets a full-branch `push` fail and drops
    /// the value; `DropOldest` evicts the oldest via `force_push`. A `Closed` race
    /// (observer dropped mid-flight) is skipped in every case.
    fn deliver(&self, value: &I::Ready) {
        for queue in &self.queues {
            if queue.is_closed() {
                continue;
            }
            match self.policy {
                // Drop-oldest: evict to make room for the newest (keep latest).
                BroadcastPolicy::DropOldest => {
                    let _ = queue.force_push(Stream::Next(value.clone()));
                }
                // Backpressure (room pre-checked) / DropNewest (full → dropped).
                BroadcastPolicy::Backpressure | BroadcastPolicy::DropNewest => {
                    let _ = queue.push(Stream::Next(value.clone()));
                }
            }
        }
    }
}

impl<I> TaskIterator for BroadcastContinuation<I>
where
    I: TaskIterator,
    I::Ready: Clone + 'static,
    I::Pending: Clone + 'static,
{
    type Ready = I::Ready;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        // Resume a parked broadcast (zero-loss only): deliver to all once every
        // open branch has vacancy, otherwise re-park on the still-full branch.
        if let Some(value) = self.pending_value.take() {
            if let Some(idx) = self.first_full_branch() {
                self.pending_value = Some(value);
                return Some(TaskStatus::Depends(Arc::new(self.queues[idx].vacancy())));
            }
            self.deliver(&value);
            return self.pending_forward.take();
        }

        let Some(item) = self.inner.next_status(ctx) else {
            self.close_all();
            tracing::debug!("BroadcastContinuation: source exhausted, branches closed");
            return None;
        };

        if let TaskStatus::Ready(value) = &item {
            if (self.predicate)(value) {
                match self.policy {
                    BroadcastPolicy::Backpressure => {
                        if let Some(idx) = self.first_full_branch() {
                            // Slowest-gates lockstep: deliver to NO branch until all
                            // have vacancy, so no branch is double-delivered on retry
                            // and none is dropped. Hold the source item back.
                            self.pending_value = Some(value.clone());
                            self.pending_forward = Some(item);
                            return Some(TaskStatus::Depends(Arc::new(self.queues[idx].vacancy())));
                        }
                        self.deliver(value);
                    }
                    // Never parks: drop-newest / drop-oldest handled in `deliver`.
                    BroadcastPolicy::DropNewest | BroadcastPolicy::DropOldest => {
                        self.deliver(value);
                    }
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
    queue: EventQueue<Stream<D, P>>,
}

impl<D, P> SplitUntilObserver<D, P> {
    /// Expose a [`QueueReadiness`] over the shared queue so a task-path consumer
    /// can park natively via `Depends` (F45 Resolution 3).
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn readiness(&self) -> QueueReadiness<Stream<D, P>> {
        self.queue.readiness()
    }
}

impl<D, P> Iterator for SplitUntilObserver<D, P>
where
    D: Clone + 'static,
    P: Clone + 'static,
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
                    // The producer can push its last item and close between the
                    // `pop` above and this check. A closed queue still hands out
                    // what it holds, so pop once more rather than drop that item
                    // (it lost an H1 response head under load).
                    self.queue.pop().ok()
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
        let _ = self.queue.close();
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
    queue: EventQueue<Stream<I::Ready, I::Pending>>,
    /// Predicate to determine when to close observer
    predicate: BoxedReadyCollectionPredicate<I>,
    /// A matched observer item awaiting a free slot (F45 Part C1 backpressure).
    pending_push: Option<Stream<I::Ready, I::Pending>>,
    /// The source item held back until the stashed copy is accepted.
    pending_forward: Option<TaskStatus<I::Ready, I::Pending>>,
    /// Whether the observer queue must be closed once `pending_push` lands (the
    /// `CollectionState::Close(true)` case parked on a full queue).
    pending_close: bool,
}

impl<I> TaskIterator for SplitUntilContinuation<I>
where
    I: TaskIterator,
    I::Ready: Clone + 'static,
    I::Pending: Clone + 'static,
{
    type Ready = I::Ready;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        // Retry a stashed observer push before pulling new work (F45 Part C1).
        if let Some(stream_item) = self.pending_push.take() {
            if let Some(stashed) = split_observer_push(&self.queue, stream_item) {
                self.pending_push = Some(stashed);
                return Some(TaskStatus::Depends(Arc::new(self.queue.vacancy())));
            }
            if self.pending_close {
                self.pending_close = false;
                let _ = self.queue.close();
            }
            return self.pending_forward.take();
        }

        let Some(item) = self.inner.next_status(ctx) else {
            // Source iterator is naturally exhausted, close the queue
            let _ = self.queue.close();
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
                        let _ = self.queue.close();
                        tracing::debug!("SplitUntilContinuation: observer queue closed after CollectionState::Close");
                    }
                }
            }
        }

        if park {
            self.pending_forward = Some(item);
            return Some(TaskStatus::Depends(Arc::new(self.queue.vacancy())));
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
            let _ = self.queue.close();
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
    queue: EventQueue<Stream<D, P>>,
}

impl<D, P> SplitUntilObserverMap<D, P> {
    /// Expose a [`QueueReadiness`] over the shared queue so a task-path consumer
    /// can park natively via `Depends` (F45 Resolution 3).
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn readiness(&self) -> QueueReadiness<Stream<D, P>> {
        self.queue.readiness()
    }
}

impl<D, P> Iterator for SplitUntilObserverMap<D, P>
where
    D: Clone + 'static,
    P: Clone + 'static,
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
                    // The producer can push its last item and close between the
                    // `pop` above and this check. A closed queue still hands out
                    // what it holds, so pop once more rather than drop that item
                    // (it lost an H1 response head under load).
                    self.queue.pop().ok()
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
        let _ = self.queue.close();
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
    queue: EventQueue<Stream<D, I::Pending>>,
    /// Combined predicate + transform function
    transform: BoxedReadyTransform<I, D>,
    /// A transformed observer item awaiting a free slot (F45 Part C1 backpressure).
    pending_push: Option<Stream<D, I::Pending>>,
    /// The source item held back until the stashed copy is accepted.
    pending_forward: Option<TaskStatus<I::Ready, I::Pending>>,
    /// Whether the observer queue must be closed once `pending_push` lands.
    pending_close: bool,
}

impl<I, D> TaskIterator for SplitUntilContinuationMap<I, D>
where
    I: TaskIterator,
    I::Pending: Clone + 'static,
    D: Clone + 'static,
{
    type Ready = I::Ready;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        // Retry a stashed observer push before pulling new work (F45 Part C1).
        if let Some(stream_item) = self.pending_push.take() {
            if let Some(stashed) = split_observer_push(&self.queue, stream_item) {
                self.pending_push = Some(stashed);
                return Some(TaskStatus::Depends(Arc::new(self.queue.vacancy())));
            }
            if self.pending_close {
                self.pending_close = false;
                let _ = self.queue.close();
            }
            return self.pending_forward.take();
        }

        let Some(item) = self.inner.next_status(ctx) else {
            // Source iterator is naturally exhausted, close the queue
            let _ = self.queue.close();
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
                        let _ = self.queue.close();
                        tracing::debug!("SplitUntilContinuationMap: observer queue closed after CollectionState::Close");
                    }
                }
            }
        }

        if park {
            self.pending_forward = Some(item);
            return Some(TaskStatus::Depends(Arc::new(self.queue.vacancy())));
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
            let _ = self.queue.close();
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
    queue: EventQueue<Stream<M, P>>,
}

impl<M, P> SplitCollectorMapObserver<M, P> {
    /// Expose a [`QueueReadiness`] over the shared queue so a task-path consumer
    /// can park natively via `Depends` (F45 Resolution 3).
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn readiness(&self) -> QueueReadiness<Stream<M, P>> {
        self.queue.readiness()
    }
}

impl<M, P> Iterator for SplitCollectorMapObserver<M, P>
where
    M: Clone + 'static,
    P: Clone + 'static,
{
    type Item = Stream<M, P>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.queue.pop() {
            Ok(item) => Some(item),
            Err(concurrent_queue::PopError::Empty) => {
                if self.queue.is_closed() {
                    // The producer can push its last item and close between the
                    // `pop` above and this check. A closed queue still hands out
                    // what it holds, so pop once more rather than drop that item
                    // (it lost an H1 response head under load).
                    self.queue.pop().ok()
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
        let _ = self.queue.close();
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
    queue: EventQueue<Stream<M, I::Pending>>,
    /// Combined predicate + transform function
    transform: BoxedReadyMatchedTransform<I, M>,
    /// A transformed observer item awaiting a free slot (F45 Part C1 backpressure).
    pending_push: Option<Stream<M, I::Pending>>,
    /// The source item held back until the stashed copy is accepted.
    pending_forward: Option<TaskStatus<I::Ready, I::Pending>>,
}

impl<I, M> TaskIterator for SplitCollectorMapContinuation<I, M>
where
    I: TaskIterator,
    I::Pending: Clone + 'static,
    M: Clone + 'static,
{
    type Ready = I::Ready;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        // Retry a stashed observer push before pulling new work (F45 Part C1).
        if let Some(stream_item) = self.pending_push.take() {
            if let Some(stashed) = split_observer_push(&self.queue, stream_item) {
                self.pending_push = Some(stashed);
                return Some(TaskStatus::Depends(Arc::new(self.queue.vacancy())));
            }
            return self.pending_forward.take();
        }

        let Some(item) = self.inner.next_status(ctx) else {
            let _ = self.queue.close();
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
            return Some(TaskStatus::Depends(Arc::new(self.queue.vacancy())));
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
            let _ = self.queue.close();
        }
    }
}

// ===== Feature 08: Iterator Extension Completion Wrapper Structs =====

/// Wrapper for `map_state()` - transforms any `TaskStatus`
pub struct TMapState<I, F> {
    inner: I,
    mapper: F,
}

impl<I, F> TaskIterator for TMapState<I, F>
where
    I: TaskIterator,
    F: Fn(TaskStatus<I::Ready, I::Pending>) -> TaskStatus<I::Ready, I::Pending> + 'static,
{
    type Ready = I::Ready;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.inner.next_status(ctx).map(&self.mapper)
    }
}

/// Wrapper for `inspect_state()` - side-effect on any `TaskStatus`
pub struct TInspectState<I, F> {
    inner: I,
    inspector: F,
}

impl<I, F> TaskIterator for TInspectState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending>) + 'static,
{
    type Ready = I::Ready;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        let status = self.inner.next_status(ctx)?;
        (self.inspector)(&status);
        Some(status)
    }
}

/// Wrapper for `filter_state()` - filter based on full `TaskStatus`
pub struct TFilterState<I, F> {
    inner: I,
    predicate: F,
}

impl<I, F> TaskIterator for TFilterState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending>) -> bool + 'static,
{
    type Ready = I::Ready;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        let status = self.inner.next_status(ctx)?;
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

impl<I, F> TaskIterator for TTransformUntil<I, F>
where
    I: TaskIterator,
    F: Fn(TaskStatus<I::Ready, I::Pending>) -> Option<TaskStatus<I::Ready, I::Pending>> + 'static,
{
    type Ready = I::Ready;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.done {
            return None;
        }
        let status = self.inner.next_status(ctx)?;
        if let Some(transformed) = (self.transformer)(status) {
            Some(transformed)
        } else {
            self.done = true;
            None
        }
    }
}

/// Wrapper for `take_while_state()` - take while state predicate true
pub struct TTakeWhileState<I, F> {
    inner: I,
    predicate: F,
    done: bool,
}

impl<I, F> TaskIterator for TTakeWhileState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending>) -> bool + 'static,
{
    type Ready = I::Ready;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.done {
            return None;
        }
        let status = self.inner.next_status(ctx)?;
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

impl<I, F> TaskIterator for TSkipWhileState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending>) -> bool + 'static,
{
    type Ready = I::Ready;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        let status = self.inner.next_status(ctx)?;
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

impl<I, F> TaskIterator for TTakeState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending>) -> bool + 'static,
{
    type Ready = I::Ready;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.remaining == 0 {
            return None;
        }
        let status = self.inner.next_status(ctx)?;
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

impl<I, F> TaskIterator for TSkipState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending>) -> bool + 'static,
{
    type Ready = I::Ready;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        let status = self.inner.next_status(ctx)?;
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

impl<I> TaskIterator for TEnumerate<I>
where
    I: TaskIterator,
{
    type Ready = (usize, I::Ready);
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        let status = self.inner.next_status(ctx)?;
        match status {
            TaskStatus::Ready(v) => {
                let item = TaskStatus::Ready((self.count, v));
                self.count += 1;
                Some(item)
            }
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
            TaskStatus::Pending(p) => Some(TaskStatus::Pending(p)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
            TaskStatus::Wait => Some(TaskStatus::Wait),
            TaskStatus::Depends(signal) => Some(TaskStatus::Depends(signal)),
        }
    }
}

/// Wrapper for `find()` - find first Ready matching predicate
pub struct TFind<I, F> {
    inner: I,
    predicate: F,
    found: bool,
}

impl<I, F> TaskIterator for TFind<I, F>
where
    I: TaskIterator,
    F: Fn(&I::Ready) -> bool + 'static,
{
    type Ready = Option<I::Ready>;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.found {
            return None;
        }

        let status = self.inner.next_status(ctx)?;
        match status {
            TaskStatus::Ready(v) => {
                if (self.predicate)(&v) {
                    self.found = true;
                    Some(TaskStatus::Ready(Some(v)))
                } else {
                    Some(TaskStatus::Ignore)
                }
            }
            TaskStatus::Spread(items) => {
                if let Some(v) = items.into_iter().find_map(|item| match item {
                    TaskSpread::Ready(v) if (self.predicate)(&v) => Some(v),
                    _ => None,
                }) {
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

impl<I, F, R> TaskIterator for TFindMap<I, F, R>
where
    I: TaskIterator,
    F: Fn(I::Ready) -> Option<R> + 'static,
    R: 'static,
{
    type Ready = Option<R>;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.found {
            return None;
        }

        let status = self.inner.next_status(ctx)?;
        match status {
            TaskStatus::Ready(v) => {
                if let Some(r) = (self.mapper)(v) {
                    self.found = true;
                    Some(TaskStatus::Ready(Some(r)))
                } else {
                    Some(TaskStatus::Ignore)
                }
            }
            TaskStatus::Spread(items) => {
                if let Some(r) = items.into_iter().find_map(|item| match item {
                    TaskSpread::Ready(v) => (self.mapper)(v),
                    TaskSpread::Pending(_) => None,
                }) {
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

impl<I, F, R> TaskIterator for TFold<I, F, R>
where
    I: TaskIterator,
    F: Fn(R, I::Ready) -> R + 'static,
    R: 'static,
{
    type Ready = R;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.done {
            return None;
        }

        match self.inner.next_status(ctx) {
            Some(TaskStatus::Ready(v)) => {
                if let Some(acc) = self.acc.take() {
                    self.acc = Some((self.folder)(acc, v));
                }
                Some(TaskStatus::Ignore)
            }
            Some(TaskStatus::Spread(items)) => {
                if let Some(acc) = self.acc.take() {
                    self.acc = Some(items.into_iter().fold(acc, |acc, item| match item {
                        TaskSpread::Ready(v) => (self.folder)(acc, v),
                        TaskSpread::Pending(_) => acc,
                    }));
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

impl<I, F> TaskIterator for TAll<I, F>
where
    I: TaskIterator,
    F: Fn(I::Ready) -> bool + 'static,
{
    type Ready = bool;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.done {
            return None;
        }

        if !self.all_true {
            return None;
        }

        match self.inner.next_status(ctx) {
            Some(TaskStatus::Ready(v)) => {
                if (self.predicate)(v) {
                    Some(TaskStatus::Ignore)
                } else {
                    self.all_true = false;
                    self.done = true;
                    Some(TaskStatus::Ready(false))
                }
            }
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
            Some(TaskStatus::Pending(p)) => Some(TaskStatus::Pending(p)),
            Some(TaskStatus::Delayed(d)) => Some(TaskStatus::Delayed(d)),
            Some(TaskStatus::Init) => Some(TaskStatus::Init),
            Some(TaskStatus::Spawn(s)) => Some(TaskStatus::Spawn(s)),
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            Some(TaskStatus::Wait) => Some(TaskStatus::Wait),
            Some(TaskStatus::Depends(signal)) => Some(TaskStatus::Depends(signal)),
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

impl<I, F> TaskIterator for TAny<I, F>
where
    I: TaskIterator,
    F: Fn(I::Ready) -> bool + 'static,
{
    type Ready = bool;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.done {
            return None;
        }

        if self.any_true {
            return None;
        }

        match self.inner.next_status(ctx) {
            Some(TaskStatus::Ready(v)) => {
                if (self.predicate)(v) {
                    self.any_true = true;
                    self.done = true;
                    Some(TaskStatus::Ready(true))
                } else {
                    Some(TaskStatus::Ignore)
                }
            }
            Some(TaskStatus::Spread(items)) => {
                // Scan all Ready items first for a match, collect Pending
                let mut pending_items: Vec<I::Pending> = Vec::new();
                for item in items {
                    match item {
                        TaskSpread::Ready(v) => {
                            if (self.predicate)(v) {
                                self.any_true = true;
                                self.done = true;
                                return Some(TaskStatus::Ready(true));
                            }
                        }
                        TaskSpread::Pending(p) => pending_items.push(p),
                    }
                }
                if pending_items.is_empty() {
                    Some(TaskStatus::Ignore)
                } else {
                    Some(TaskStatus::Spread(
                        pending_items.into_iter().map(TaskSpread::Pending).collect(),
                    ))
                }
            }
            Some(TaskStatus::Pending(p)) => Some(TaskStatus::Pending(p)),
            Some(TaskStatus::Delayed(d)) => Some(TaskStatus::Delayed(d)),
            Some(TaskStatus::Init) => Some(TaskStatus::Init),
            Some(TaskStatus::Spawn(s)) => Some(TaskStatus::Spawn(s)),
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            Some(TaskStatus::Wait) => Some(TaskStatus::Wait),
            Some(TaskStatus::Depends(signal)) => Some(TaskStatus::Depends(signal)),
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

impl<I> TaskIterator for TCount<I>
where
    I: TaskIterator,
{
    type Ready = usize;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        match self.inner.next_status(ctx) {
            Some(TaskStatus::Ready(_)) => {
                self.count += 1;
                Some(TaskStatus::Ignore)
            }
            Some(TaskStatus::Spread(items)) => {
                self.count += items.len();
                Some(TaskStatus::Ignore)
            }
            Some(TaskStatus::Pending(p)) => Some(TaskStatus::Pending(p)),
            Some(TaskStatus::Delayed(d)) => Some(TaskStatus::Delayed(d)),
            Some(TaskStatus::Init) => Some(TaskStatus::Init),
            Some(TaskStatus::Spawn(s)) => Some(TaskStatus::Spawn(s)),
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            Some(TaskStatus::Wait) => Some(TaskStatus::Wait),
            Some(TaskStatus::Depends(signal)) => Some(TaskStatus::Depends(signal)),
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

impl<I> TaskIterator for TCountAll<I>
where
    I: TaskIterator,
{
    type Ready = usize;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.done {
            return None;
        }

        match self.inner.next_status(ctx) {
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            Some(
                TaskStatus::Ready(_)
                | TaskStatus::Pending(_)
                | TaskStatus::Delayed(_)
                | TaskStatus::Init
                | TaskStatus::Spawn(_)
                | TaskStatus::Wait
                | TaskStatus::Spread(_)
                | TaskStatus::Depends(_),
            ) => {
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
    circuit: BoxedTaskCircuit<I>,
    stopped: bool,
    _phantom: std::marker::PhantomData<I>,
}

impl<I> TaskIterator for TMapCircuit<I>
where
    I: TaskIterator + 'static,
    I::Ready: 'static,
    I::Pending: 'static,
{
    type Ready = I::Ready;
    type Pending = I::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        // If we've stopped, return None permanently
        if self.stopped {
            return None;
        }

        // Get the next item from the inner iterator
        let status = self.inner.next_status(ctx)?;

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

// ============================================================================
// WakeableStream impls (Feature 02) — stream observers default to NotSupported.
// Their backing Pipe already bridges via RecvFuture, not the stream wake path.
// ============================================================================

use crate::valtron::WakeableStream;

impl<D, P> WakeableStream for CollectorStreamIterator<D, P>
where
    D: Clone + 'static,
    P: Clone + 'static,
{
}

impl<D, P> WakeableStream for SplitUntilObserver<D, P>
where
    D: Clone + 'static,
    P: Clone + 'static,
{
}

impl<D, P> WakeableStream for SplitUntilObserverMap<D, P>
where
    D: Clone + 'static,
    P: Clone + 'static,
{
}

impl<M, P> WakeableStream for SplitCollectorMapObserver<M, P>
where
    M: Clone + 'static,
    P: Clone + 'static,
{
}
