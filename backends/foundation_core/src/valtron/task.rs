use crate::valtron::{Ctx, ReadinessNotifier, ReadinessWaker, Stream};
use concurrent_queue::ConcurrentQueue;
use derive_more::From;

use std::{
    any::Any,
    cell::{self, RefCell},
    marker::PhantomData,
    rc::{self, Rc},
    sync::Arc,
    time,
};

use crate::compati::Mutex;
use foundation_compact::rng::ChaCha8Rng;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{
    synca::Entry,
    valtron::{AnyResult, GenericResult},
};

/// A trait for types that can answer "is this task ready to run?"
///
/// Under `multi` the pointer type used by `State::Depends` requires
/// `Send + Sync` on the trait object; under single-threaded / wasm it
/// does not.  Concrete impls are always `Send + Sync` under `multi`
/// (they already were) and the trait itself carries no supertraits so
/// it can be implemented for `!Send` payload types under `not(multi)`.
pub trait EventReadiness {
    /// Check if the task is ready to run.
    ///
    /// `dur`: Optional timeout. If `None`, check immediately without blocking.
    /// If `Some(dur)`, may block up to `dur` waiting for readiness.
    ///
    /// Returns `true` if ready, `false` otherwise.
    fn is_ready(&self, dur: Option<time::Duration>) -> bool;

    /// Forget any latched readiness so the next edge is observed fresh.
    ///
    /// Defaults to a no-op: most signals (queues, atomics, deadlines) have
    /// nothing to clear. An edge-triggered readiness source overrides this to
    /// clear its latch before the next read, so an edge that arrives after a
    /// `WouldBlock` still wakes the parked task (see
    /// `foundation_reactor::FdReadiness`).
    fn clear(&self) {}

    /// The instant after which this signal becomes ready on its own, if it has
    /// one.
    ///
    /// WHY: an executor that parks with nothing runnable must wake at the
    /// nearest deadline of its sleepers. A signal such as "socket ready OR
    /// timer passed" is only re-checked when something else wakes the
    /// executor, so without this hint the task sleeps past its timer.
    ///
    /// Defaults to `None`: most signals (queues, atomics, sockets) become ready
    /// only by an outside event and have no instant of their own. A time-based
    /// signal returns its instant; a signal that combines others returns the
    /// earliest of theirs.
    fn deadline(&self) -> Option<foundation_compact::Instant> {
        None
    }

    /// Register the waker of the task that parks on this readiness.
    ///
    /// WHY: the executor must run the task again when the readiness becomes ready.
    /// Instead of every source calling an executor function, the executor hands the
    /// readiness a waker that belongs to **that task**, and the source only calls
    /// [`ReadinessWaker::wake`] (the `core::task::Waker` pattern).
    ///
    /// WHAT: the readiness keeps `waker` (a [`ReadinessNotifier`] does this for
    /// it) and wakes it when it becomes ready or closes. A second `register` for the
    /// same task must not add a second waker, and one for a new park replaces the
    /// old. The wake reaches only the executor that owns the task.
    ///
    /// Returns `true` when the readiness wakes the waker on **every** change to
    /// ready (or is ready only at its [`Self::deadline`]). The executor then does not
    /// re-check it on a timer and sleeps until the wake or the deadline.
    ///
    /// Returns `false` by default (safe): a readiness that cannot wake is still
    /// re-checked on the executor's readiness quantum (1 ms), so a custom
    /// implementation outside valtron keeps working until it opts in. A readiness
    /// that combines others returns `true` only if all of them do.
    fn register(&self, waker: &ReadinessWaker) -> bool {
        let _ = waker;
        false
    }
}

/// A readiness flag that wakes the tasks parked on it when it is set.
///
/// WHY: unlike [`BoolSignal`], it has no raw access to its flag, so every change
/// wakes the parked tasks (through the wakers they registered). The executor
/// then never has to re-check it on a timer ([`EventReadiness::register`] returns
/// `true`).
#[derive(Clone, Default)]
pub struct ResumeSignal(Arc<ResumeSignalInner>);

#[derive(Default)]
struct ResumeSignalInner {
    flag: AtomicBool,
    notifier: ReadinessNotifier,
}

impl ResumeSignal {
    /// A new signal with the given value.
    #[must_use]
    pub fn new(value: bool) -> Self {
        Self(Arc::new(ResumeSignalInner {
            flag: AtomicBool::new(value),
            notifier: ReadinessNotifier::new(),
        }))
    }

    /// Set the value. Setting it to `true` wakes the tasks parked on it.
    pub fn set(&self, value: bool) {
        self.0.flag.store(value, Ordering::SeqCst);
        if value {
            self.0.notifier.notify();
        }
    }

    /// The current value.
    #[must_use]
    pub fn get(&self) -> bool {
        self.0.flag.load(Ordering::SeqCst)
    }

    /// The number of wakers registered on this signal.
    ///
    /// This is a test hook: a test proves that finished waits do not pile up
    /// wakers. It is not part of the stable API.
    #[doc(hidden)]
    #[must_use]
    pub fn waiter_count(&self) -> usize {
        self.0.notifier.waiter_count()
    }
}

impl EventReadiness for ResumeSignal {
    fn is_ready(&self, _dur: Option<time::Duration>) -> bool {
        self.get()
    }

    fn register(&self, waker: &ReadinessWaker) -> bool {
        self.0.notifier.register(waker)
    }
}

/// The pointer type that [`State::Depends`] and [`TaskStatus::Depends`] carry.
///
/// Under `multi` the trait object requires `Send + Sync` so that `State`
/// itself is `Send` (the global task queue needs it).  Under
/// `not(multi)` those bounds are dropped — `State` is `!Send`, but the
/// single-threaded executor never crosses threads anyway.
#[cfg(feature = "multi")]
pub type EventReadinessPtr = Arc<dyn EventReadiness + Send + Sync>;

#[cfg(not(feature = "multi"))]
pub type EventReadinessPtr = Arc<dyn EventReadiness>;

/// A simple readiness signal backed by an `Arc<AtomicBool>`.
///
/// The task considers itself ready when the atomic bool is `true`.
///
/// [`BoolSignal::set`] wakes the tasks parked on the signal, so a signal that is only
/// changed through `set` is not re-checked on a timer. A signal built from a raw
/// atomic ([`BoolSignal::from_atomic`], [`BoolSignal::clone_inner`]) may be stored to
/// by code that wakes nobody, so it stays on the timed re-check.
#[derive(Clone)]
pub struct BoolSignal(Arc<BoolSignalInner>);

struct BoolSignalInner {
    flag: Arc<AtomicBool>,
    notifier: ReadinessNotifier,
    /// A raw writer exists: a store may not wake the tasks parked on the signal.
    raw: AtomicBool,
}

impl BoolSignal {
    /// Create a new `BoolSignal` with the given initial value.
    #[must_use]
    pub fn new(value: bool) -> Self {
        Self(Arc::new(BoolSignalInner {
            flag: Arc::new(AtomicBool::new(value)),
            notifier: ReadinessNotifier::new(),
            raw: AtomicBool::new(false),
        }))
    }

    /// Create a `BoolSignal` from an existing `Arc<AtomicBool>`.
    ///
    /// Whoever holds the atomic can store to it without waking a parked task, so
    /// the signal is re-checked on a timer.
    pub fn from_atomic(inner: Arc<AtomicBool>) -> Self {
        Self(Arc::new(BoolSignalInner {
            flag: inner,
            notifier: ReadinessNotifier::new(),
            raw: AtomicBool::new(true),
        }))
    }

    /// Get a clone of the underlying `Arc<AtomicBool>` for external mutation.
    ///
    /// Prefer [`BoolSignal::set`]: a raw store wakes nobody. Taking the atomic
    /// puts the signal on the timed re-check, so a raw store is still seen.
    #[must_use]
    pub fn clone_inner(&self) -> Arc<AtomicBool> {
        self.0.raw.store(true, Ordering::SeqCst);
        self.0.flag.clone()
    }

    /// Set the signal. Turning it on wakes the tasks parked on it, and on a JS wasm
    /// build asks the host to run the executor again, so a task parked on it is
    /// re-polled even when the writer is JS-side async work.
    pub fn set(&self, value: bool) {
        self.0.flag.store(value, Ordering::SeqCst);
        if value {
            self.0.notifier.notify();
            if self.0.raw.load(Ordering::SeqCst) {
                // A timed re-check task (no waker) parked on a raw signal: nudge
                // the executors of the process.
                crate::valtron::request_executor_resume();
            }
        }
    }
}

impl EventReadiness for BoolSignal {
    fn is_ready(&self, _dur: Option<time::Duration>) -> bool {
        self.0.flag.load(Ordering::SeqCst)
    }

    /// Takes the waker only while no raw writer exists (see the type docs).
    fn register(&self, waker: &ReadinessWaker) -> bool {
        let registered = self.0.notifier.register(waker);
        registered && !self.0.raw.load(Ordering::SeqCst)
    }
}

/// A readiness signal flipped by a [`WakeableStream`] wake callback.
///
/// WHY: a task that pulls from a [`WakeableStream`] and gets `Stream::Wait` should
/// park (`TaskStatus::Depends`) rather than re-poll, and be woken exactly when the
/// stream has data. This ties the two together.
///
/// HOW: [`WakeSignal::callback`] builds a one-shot [`WakeCallback`] that sets the
/// flag and wakes the task parked on the signal (so a JS wasm executor that has
/// handed control back to the event loop is run again). Register it with
/// `stream.wake(signal.callback())`; if that returns `WakeResult::Successful`,
/// return `TaskStatus::Depends(Arc::new(signal))`.
///
/// [`WakeableStream`]: crate::valtron::WakeableStream
/// [`WakeCallback`]: crate::valtron::WakeCallback
#[derive(Clone, Default)]
pub struct WakeSignal(Arc<WakeSignalInner>);

#[derive(Default)]
struct WakeSignalInner {
    flag: AtomicBool,
    notifier: ReadinessNotifier,
}

impl WakeSignal {
    /// A new, not-yet-ready signal.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The wake callback to register on the stream: marks the signal ready and
    /// wakes the task parked on it.
    pub fn callback(&self) -> crate::valtron::WakeCallback {
        let inner = Arc::clone(&self.0);
        Box::new(move || {
            inner.flag.store(true, Ordering::SeqCst);
            inner.notifier.notify();
        })
    }
}

impl EventReadiness for WakeSignal {
    fn register(&self, waker: &ReadinessWaker) -> bool {
        self.0.notifier.register(waker)
    }

    fn is_ready(&self, _dur: Option<time::Duration>) -> bool {
        self.0.flag.load(Ordering::SeqCst)
    }
}

/// The status a task returns when the stream it is draining has no item yet
/// (`Stream::Wait`): park until the stream signals readiness.
///
/// Registers a [`WakeSignal`] on `stream` and returns
/// [`TaskStatus::Depends`] on it, so the executor stops polling the task until
/// the stream has an item (or closes) and then resumes it. A stream that
/// cannot signal readiness ([`WakeResult::NotSupported`]) gets
/// [`TaskStatus::Wait`], the plain re-poll.
///
/// ```
/// use std::sync::Arc;
/// use std::time::Duration;
///
/// use foundation_core::valtron::{
///     park_on, EventReadiness, NotifyQueue, NotifyQueueStreamIterator, Stream, TaskStatus,
/// };
///
/// let queue: Arc<NotifyQueue<Stream<u32, ()>>> = Arc::new(NotifyQueue::unbounded());
/// let mut stream = NotifyQueueStreamIterator::new(queue.clone(), 1, Duration::from_millis(1));
///
/// // An empty stream parks the task on a signal instead of re-polling it.
/// let status: TaskStatus<u32, ()> = park_on(&mut stream);
/// let TaskStatus::Depends(signal) = status else {
///     panic!("a waking stream parks on Depends");
/// };
/// assert!(!signal.is_ready(None));
///
/// // A push flips the signal, so the executor resumes the task.
/// queue.push(Stream::Next(7)).expect("push");
/// assert!(signal.is_ready(None));
/// ```
///
/// [`WakeResult::NotSupported`]: crate::valtron::WakeResult::NotSupported
pub fn park_on<S, D, P>(stream: &mut S) -> TaskStatus<D, P>
where
    S: crate::valtron::WakeableStream + ?Sized,
{
    let signal = WakeSignal::new();
    match stream.wake(signal.callback()) {
        crate::valtron::WakeResult::Successful => TaskStatus::Depends(Arc::new(signal)),
        crate::valtron::WakeResult::NotSupported => TaskStatus::Wait,
    }
}

impl<D, P> TaskStatus<D, P> {
    /// Pull the next item of `stream` and return the status a task reports
    /// for it — the whole "drain a stream inside a task" match in one call.
    ///
    /// | stream item | status |
    /// |---|---|
    /// | `Next(d)` | `Ready(d)` |
    /// | `Pending(p)` | `Pending(p)` |
    /// | `Spread(items)` | `Spread(items)` |
    /// | `Delayed(d)` | `Delayed(d)` |
    /// | `Init` / `Ignore` | `Init` / `Ignore` |
    /// | `Wait` | parks on the stream ([`park_on`]) |
    /// | end of stream | `None` |
    ///
    /// A task that needs the value handles `Ready` and returns everything
    /// else as-is:
    ///
    /// ```
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// use foundation_core::valtron::{
    ///     EventReadiness, NotifyQueue, NotifyQueueStreamIterator, Stream, TaskStatus,
    /// };
    ///
    /// let queue: Arc<NotifyQueue<Stream<u32, ()>>> = Arc::new(NotifyQueue::unbounded());
    /// let mut stream = NotifyQueueStreamIterator::new(queue.clone(), 1, Duration::from_millis(1));
    ///
    /// // With no item yet, the task parks on the stream's wake signal.
    /// let status: Option<TaskStatus<u32, ()>> = TaskStatus::from_stream(&mut stream);
    /// let Some(TaskStatus::Depends(signal)) = status else {
    ///     panic!("an empty, open stream parks on Depends");
    /// };
    /// assert!(!signal.is_ready(None));
    ///
    /// // An item turns the next pull into Ready.
    /// queue.push(Stream::Next(7)).expect("push");
    /// let status: Option<TaskStatus<u32, ()>> = TaskStatus::from_stream(&mut stream);
    /// assert!(matches!(status, Some(TaskStatus::Ready(7))));
    ///
    /// // A closed, drained stream ends with None.
    /// queue.close();
    /// assert!(TaskStatus::<u32, ()>::from_stream(&mut stream).is_none());
    /// ```
    pub fn from_stream<S>(stream: &mut S) -> Option<Self>
    where
        S: crate::valtron::WakeableStream<D = D, P = P> + ?Sized,
    {
        // Announce the parking consumer before pulling, so a queue-backed
        // stream answers an empty pull with `Wait` instead of blocking the
        // executor thread for a timed wait first. `announce` is idempotent and
        // registers no callback, so it cannot leak on repeated polls.
        stream.announce();
        Some(match stream.next()? {
            Stream::Next(value) => TaskStatus::Ready(value),
            Stream::Pending(progress) => TaskStatus::Pending(progress),
            Stream::Delayed(duration) => TaskStatus::Delayed(duration),
            Stream::Init => TaskStatus::Init,
            Stream::Ignore => TaskStatus::Ignore,
            Stream::Wait => park_on(stream),
            Stream::Spread(items) => TaskStatus::Spread(
                items
                    .into_iter()
                    .map(|item| match item {
                        crate::valtron::streams::StreamSpread::Done(d) => TaskSpread::Ready(d),
                        crate::valtron::streams::StreamSpread::Pending(p) => TaskSpread::Pending(p),
                    })
                    .collect(),
            ),
        })
    }
}

/// A readiness signal backed by a `ConcurrentQueue<T>`.
///
/// The task considers itself ready when the queue is **non-empty** — i.e.,
/// one or more messages are waiting to be consumed.
///
/// This is the preferred signal for any `TaskIterator` that waits on a
/// producer→consumer queue (file events, build results, SSE reload signals).
/// The executor parks the task entirely and only wakes it when a message
/// arrives. Zero CPU spinning.
///
/// # Example
///
/// ```
/// use foundation_core::valtron::{EventQueue, EventReadiness};
///
/// let queue = EventQueue::<u32>::unbounded();
/// let readiness = queue.readiness();
///
/// // Empty: the task parks on the readiness instead of taking a message.
/// assert!(!readiness.is_ready(None));
///
/// // Non-empty: the task can take the message now.
/// queue.push(7).expect("push");
/// assert!(readiness.is_ready(None));
/// assert_eq!(queue.pop().expect("item"), 7);
/// assert!(!readiness.is_ready(None));
/// ```
pub struct QueueReadiness<T> {
    queue: Arc<ConcurrentQueue<T>>,
    /// Woken after a push and on close, so the parked task runs again at once.
    notifier: Arc<ReadinessNotifier>,
}

// Manual Clone — Arc::clone doesn't need T: Clone, derive would add that bound.
impl<T> Clone for QueueReadiness<T> {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            notifier: self.notifier.clone(),
        }
    }
}

impl<T> QueueReadiness<T> {
    /// A readiness over a queue whose pushes and close call `notifier`.
    pub(crate) fn with_notifier(
        queue: Arc<ConcurrentQueue<T>>,
        notifier: Arc<ReadinessNotifier>,
    ) -> Self {
        Self { queue, notifier }
    }

    /// Get a clone of the underlying queue.
    ///
    /// Push with [`QueueReadiness::push`] (or through the [`EventQueue`] that
    /// made this readiness) so the parked consumer is woken. A raw push wakes
    /// nobody.
    #[must_use]
    pub fn queue(&self) -> Arc<ConcurrentQueue<T>> {
        self.queue.clone()
    }

    fn changed(&self) {
        self.notifier.notify();
    }

    /// Push `item` and wake the consumer parked on this readiness.
    ///
    /// # Errors
    /// The queue is full or closed; the item is handed back.
    pub fn push(&self, item: T) -> Result<(), concurrent_queue::PushError<T>> {
        self.queue.push(item)?;
        self.changed();
        Ok(())
    }

    /// Close the queue and wake the parked consumer, so it observes the end.
    /// Returns `true` if this call closed it.
    #[must_use]
    pub fn close(&self) -> bool {
        let closed = self.queue.close();
        self.changed();
        closed
    }
}

#[cfg(feature = "multi")]
impl<T: Send> EventReadiness for QueueReadiness<T> {
    fn is_ready(&self, _dur: Option<time::Duration>) -> bool {
        // Closed-aware: a closed queue is "ready" so a parked consumer unparks to
        // observe end-of-stream (its next `pop` drains any backlog then returns
        // `Closed`) instead of sleeping forever.
        !self.queue.is_empty() || self.queue.is_closed()
    }

    fn register(&self, waker: &ReadinessWaker) -> bool {
        self.notifier.register(waker)
    }
}

#[cfg(not(feature = "multi"))]
impl<T> EventReadiness for QueueReadiness<T> {
    fn is_ready(&self, _dur: Option<time::Duration>) -> bool {
        !self.queue.is_empty() || self.queue.is_closed()
    }

    fn register(&self, waker: &ReadinessWaker) -> bool {
        self.notifier.register(waker)
    }
}

/// The **producer-side** readiness signal for a bounded `ConcurrentQueue<T>`.
///
/// WHY: `QueueReadiness` only covers the consumer direction (ready when
/// non-empty). A producer parked on a *full* bounded pipe needs the symmetric
/// signal, otherwise a full pipe degrades to bare `Pending` re-polling — the
/// busy-poll Decision 00 exists to remove (Decision 00 Level 1b).
///
/// WHAT: An [`EventReadiness`] that is ready when the queue has spare capacity
/// (i.e. is **not full**), so the executor unparks a producer task as soon as the
/// consumer's `pop` frees a slot.
///
/// HOW: Wraps the same `Arc<ConcurrentQueue<T>>` the pipe owns and reports
/// `!queue.is_full()`, plus a closed-queue override so a producer parked on a
/// full-then-closed pipe is unparked (its next `push` observes `Closed` and
/// errors out) rather than sleeping forever.
///
/// # Example
///
/// ```
/// use foundation_core::valtron::{EventReadiness, Pipe};
///
/// let (tx, rx) = Pipe::<u32>::with_depth(1);
/// let vacancy = tx.vacancy();
///
/// // An empty pipe has room for the producer.
/// assert!(vacancy.is_ready(None));
///
/// // A full pipe has none: the producer parks until a pop frees a slot.
/// tx.try_send(1).expect("fill the pipe");
/// assert!(!vacancy.is_ready(None));
///
/// assert_eq!(rx.try_recv().expect("item"), 1);
/// assert!(vacancy.is_ready(None));
/// ```
pub struct QueueVacancyReadiness<T> {
    queue: Arc<ConcurrentQueue<T>>,
    /// Woken after a pop and on close, so the parked producer runs again at once.
    notifier: Arc<ReadinessNotifier>,
}

// Manual Clone — Arc::clone doesn't need T: Clone, derive would add that bound.
impl<T> Clone for QueueVacancyReadiness<T> {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            notifier: self.notifier.clone(),
        }
    }
}

impl<T> QueueVacancyReadiness<T> {
    /// A readiness over a queue whose pops and close call `notifier`.
    pub(crate) fn with_notifier(
        queue: Arc<ConcurrentQueue<T>>,
        notifier: Arc<ReadinessNotifier>,
    ) -> Self {
        Self { queue, notifier }
    }

    /// Get a clone of the underlying queue.
    ///
    /// Pop with [`QueueVacancyReadiness::pop`] (or through the [`EventQueue`] that
    /// made this readiness) so the parked producer is woken. A raw pop wakes
    /// nobody.
    #[must_use]
    pub fn queue(&self) -> Arc<ConcurrentQueue<T>> {
        self.queue.clone()
    }

    fn changed(&self) {
        self.notifier.notify();
    }

    /// Pop an item and wake the producer parked on the full queue.
    ///
    /// # Errors
    /// The queue is empty or closed.
    pub fn pop(&self) -> Result<T, concurrent_queue::PopError> {
        let item = self.queue.pop()?;
        self.changed();
        Ok(item)
    }

    /// Close the queue and wake the parked producer, so it observes the close.
    /// Returns `true` if this call closed it.
    #[must_use]
    pub fn close(&self) -> bool {
        let closed = self.queue.close();
        self.changed();
        closed
    }
}

#[cfg(feature = "multi")]
impl<T: Send> EventReadiness for QueueVacancyReadiness<T> {
    fn is_ready(&self, _dur: Option<time::Duration>) -> bool {
        // Closed-aware: report ready when the queue closes so a producer parked on
        // a full-then-closed pipe unparks to observe the close on its next push.
        self.queue.is_closed() || !self.queue.is_full()
    }

    fn register(&self, waker: &ReadinessWaker) -> bool {
        self.notifier.register(waker)
    }
}

#[cfg(not(feature = "multi"))]
impl<T> EventReadiness for QueueVacancyReadiness<T> {
    fn is_ready(&self, _dur: Option<time::Duration>) -> bool {
        self.queue.is_closed() || !self.queue.is_full()
    }

    fn register(&self, waker: &ReadinessWaker) -> bool {
        self.notifier.register(waker)
    }
}

/// The **close-only** readiness signal for a bounded `ConcurrentQueue<T>`.
///
/// WHY: a producer that must learn "the consumer dropped the queue" has no
/// exact signal. [`QueueVacancyReadiness`] reports "not full", which is true for
/// every queue with room, so a producer parked on it wakes on each consumer pop.
/// This signal is ready only after the queue closes, and its notifier fires only
/// on the close.
///
/// WHAT: An [`EventReadiness`] that is ready when the queue **is closed**, so a
/// producer parks on it with `TaskStatus::Depends` and runs again once the
/// consumer goes away.
///
/// HOW: Wraps the same `Arc<ConcurrentQueue<T>>` the pipe owns and reports
/// `queue.is_closed()`. The notifier is fired by the pipe's close funnel
/// (`PipeInner::close_queue`), never by a push or a pop, and only on the first
/// close.
///
/// # Example
///
/// ```
/// use foundation_core::valtron::{EventReadiness, Pipe};
///
/// let (tx, rx) = Pipe::<u32>::with_depth(1);
/// let closed = tx.closed_readiness();
///
/// // The receiver is alive: the close has not happened, even with room.
/// assert!(!closed.is_ready(None));
///
/// drop(rx);
/// assert!(closed.is_ready(None));
/// ```
pub struct QueueClosedReadiness<T> {
    queue: Arc<ConcurrentQueue<T>>,
    /// Woken only when the queue closes, never by a push or a pop.
    notifier: Arc<ReadinessNotifier>,
}

// Manual Clone — Arc::clone doesn't need T: Clone, derive would add that bound.
impl<T> Clone for QueueClosedReadiness<T> {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            notifier: self.notifier.clone(),
        }
    }
}

impl<T> QueueClosedReadiness<T> {
    /// A readiness over a queue whose close calls `notifier`.
    pub(crate) fn with_notifier(
        queue: Arc<ConcurrentQueue<T>>,
        notifier: Arc<ReadinessNotifier>,
    ) -> Self {
        Self { queue, notifier }
    }

    /// Get a clone of the underlying queue.
    #[must_use]
    pub fn queue(&self) -> Arc<ConcurrentQueue<T>> {
        self.queue.clone()
    }
}

#[cfg(feature = "multi")]
impl<T: Send> EventReadiness for QueueClosedReadiness<T> {
    fn is_ready(&self, _dur: Option<time::Duration>) -> bool {
        self.queue.is_closed()
    }

    fn register(&self, waker: &ReadinessWaker) -> bool {
        self.notifier.register(waker)
    }
}

#[cfg(not(feature = "multi"))]
impl<T> EventReadiness for QueueClosedReadiness<T> {
    fn is_ready(&self, _dur: Option<time::Duration>) -> bool {
        self.queue.is_closed()
    }

    fn register(&self, waker: &ReadinessWaker) -> bool {
        self.notifier.register(waker)
    }
}

/// A composite [`EventReadiness`] that is ready when **any** child signal is ready.
///
/// WHY: A parked task returns exactly one `Depends`, but real waits are often
/// disjunctive — "response pipe has vacancy **or** the call was cancelled"
/// (Decision 00 Level 1b, Decision 11 §Cancellation). Composition happens inside
/// the readiness object so the task still parks on a single signal.
///
/// WHAT: Holds a set of `Arc<dyn EventReadiness>` children and reports ready as
/// soon as one of them is ready.
///
/// HOW: `is_ready` short-circuits on the first ready child. Children are checked
/// in order, so cheaper/more-likely signals can be listed first.
///
/// # Panics
/// Never panics.
///
/// # Example
///
/// ```
/// use std::sync::Arc;
///
/// use foundation_core::valtron::{
///     AnyReadiness, EventReadiness, EventReadinessPtr, Pipe, ResumeSignal,
/// };
///
/// // Park on "the pipe has vacancy OR the call was cancelled".
/// let (tx, _rx) = Pipe::<u32>::with_depth(1);
/// tx.try_send(1).expect("fill the pipe");
/// let vacancy: EventReadinessPtr = Arc::new(tx.vacancy());
/// let cancelled = ResumeSignal::new(false);
/// let signal = AnyReadiness::new(vec![
///     vacancy,
///     Arc::new(cancelled.clone()) as EventReadinessPtr,
/// ]);
///
/// // The pipe is full and the call is not cancelled: neither child is ready.
/// assert!(!signal.is_ready(None));
///
/// // One ready child is enough.
/// cancelled.set(true);
/// assert!(signal.is_ready(None));
/// ```
pub struct AnyReadiness(Vec<EventReadinessPtr>);

impl AnyReadiness {
    /// Create an `AnyReadiness` over the given child signals.
    #[must_use]
    pub fn new(children: Vec<EventReadinessPtr>) -> Self {
        Self(children)
    }

    /// Build from exactly two children (the common "event OR cancel" case).
    pub fn either(a: EventReadinessPtr, b: EventReadinessPtr) -> Self {
        Self(vec![a, b])
    }

    /// Add another child signal, returning `self` for chaining.
    #[must_use]
    pub fn with(mut self, child: EventReadinessPtr) -> Self {
        self.0.push(child);
        self
    }
}

impl EventReadiness for AnyReadiness {
    fn is_ready(&self, dur: Option<time::Duration>) -> bool {
        self.0.iter().any(|child| child.is_ready(dur))
    }

    /// The earliest deadline of the children: the signal is ready as soon as
    /// any child is, so it can be ready at the first of their instants. `None`
    /// when no child has one.
    fn deadline(&self) -> Option<foundation_compact::Instant> {
        self.0.iter().filter_map(|child| child.deadline()).min()
    }

    /// Registers `waker` with every child (no short-circuit: each child must
    /// hold it). Returns `true` only if every child wakes on change, so a child
    /// that cannot wake keeps the whole signal on the timed re-check.
    fn register(&self, waker: &ReadinessWaker) -> bool {
        let mut all = true;
        for child in &self.0 {
            all &= child.register(waker);
        }
        all
    }
}

/// The type for a panic handling closure. Note that this same closure
/// may be invoked multiple times in parallel.
pub type PanicHandler = dyn Fn(Box<dyn Any + Send>) + Send + Sync;

pub type BoxedPanicHandler = Box<PanicHandler>;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SpawnType {
    Sequenced,
    Lifted,
    LiftedWithParent,
    Broadcasted,
    Scheduled,
    None,
}

/// [`SpawnInfo`] returns detailed information about
/// the request to spawn a given task with the
/// parent of said tasks the last entry if present.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SpawnInfo(SpawnType, Option<Entry>, Option<Entry>);

impl SpawnInfo {
    #[must_use]
    pub fn new(st: SpawnType, task: Option<Entry>, parent: Option<Entry>) -> Self {
        Self(st, task, parent)
    }

    #[must_use]
    pub fn spawn_type(&self) -> SpawnType {
        self.0
    }

    #[must_use]
    pub fn child(&self) -> Option<Entry> {
        self.1
    }

    #[must_use]
    pub fn parent(&self) -> Option<Entry> {
        self.2
    }

    #[must_use]
    pub fn has_parent_and_child(&self) -> bool {
        self.1.is_some() && self.2.is_some()
    }
}

#[derive(Clone)]
pub enum TaskSpread<D, P> {
    Pending(P),
    Ready(D),
}

impl<D: core::fmt::Debug, P: core::fmt::Debug> core::fmt::Display for TaskSpread<D, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending(p) => write!(f, "Pending({p:?})"),
            Self::Ready(d) => write!(f, "Next({d:?})"),
        }
    }
}

impl<D: core::fmt::Debug, P: core::fmt::Debug> core::fmt::Debug for TaskSpread<D, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending(p) => f.debug_tuple("Pending").field(p).finish(),
            Self::Ready(d) => f.debug_tuple("Ready").field(d).finish(),
        }
    }
}

impl<D: PartialEq, P: PartialEq> PartialEq for TaskSpread<D, P> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Ready(me), Self::Ready(them)) => me == them,
            (Self::Pending(me), Self::Pending(them)) => me == them,
            _ => false,
        }
    }
}

/// completed and delivered from the iterator.
pub enum TaskStatus<D, P> {
    /// Allows a task status to communicate a delay
    /// to continued operation.
    Delayed(time::Duration),

    /// Allows you from within an executor
    /// spawn a new task that either will take
    /// priority on the same thread or be handled
    /// by an open thread from the global task queue
    /// when there is any available to process work
    ///
    /// The action is boxed — `Send + 'static` under `multi`, the plain
    /// non-`Send` box otherwise — so no concrete spawner type is threaded.
    #[cfg(feature = "multi")]
    Spawn(BoxedSendExecutionAction),
    #[cfg(not(feature = "multi"))]
    Spawn(BoxedExecutionAction),

    /// Pending is a state indicative of the status
    /// of still awaiting the readiness of some operations
    /// this can be the an underlying process waiting for
    /// some timeout to expire or a response to be received
    /// over IO or the network.
    ///
    /// Generally you send this to indicate the task as still
    /// being in a state of processing.
    Pending(P),

    /// Init represents a middle point state where the process
    /// may not immediately move into a ready state e.g reconnect
    /// to some remote endpoint or to trigger some actual underlying
    /// processes that get us into a ready state with
    /// the relevant result .
    Init,

    /// Ready is the final state where we consider the task
    /// has finished/ended with relevant result.
    Ready(D),

    /// Ignore signals that this item should be skipped.
    /// Used by filtering combinators to indicate filtered-out items
    /// without blocking the iterator.
    Ignore,

    /// No data available, queue still open.
    /// Propagates to executor as a yield signal.
    Wait,

    /// Emit multiple ready values at once.
    /// Each element is delivered individually as `TaskStatus::Ready(value)` at the delivery point.
    Spread(Vec<TaskSpread<D, P>>),

    /// Wait for an external readiness signal.
    ///
    /// The signal must return `false` on first receipt. If `true`, treated as `Pending`.
    /// Any type implementing `EventReadiness` can be used.
    Depends(EventReadinessPtr),
}

impl<D, P> From<TaskStatus<D, P>> for Stream<D, P> {
    fn from(val: TaskStatus<D, P>) -> Self {
        match val {
            TaskStatus::Init => Stream::Init,
            TaskStatus::Spawn(_) | TaskStatus::Ignore | TaskStatus::Depends(_) => Stream::Ignore,
            TaskStatus::Ready(inner) => Stream::Next(inner),
            TaskStatus::Delayed(inner) => Stream::Delayed(inner),
            TaskStatus::Pending(inner) => Stream::Pending(inner),
            TaskStatus::Wait => Stream::Wait,
            TaskStatus::Spread(items) => Stream::Spread(
                items
                    .into_iter()
                    .map(|item| match item {
                        TaskSpread::Ready(d) => crate::valtron::streams::StreamSpread::Done(d),
                        TaskSpread::Pending(p) => crate::valtron::streams::StreamSpread::Pending(p),
                    })
                    .collect(),
            ),
        }
    }
}

impl<D: PartialEq, P: PartialEq> Eq for TaskStatus<D, P> {}

impl<D: PartialEq, P: PartialEq> PartialEq for TaskStatus<D, P> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TaskStatus::Delayed(me), TaskStatus::Delayed(them)) => me == them,
            (TaskStatus::Pending(me), TaskStatus::Pending(them)) => me == them,
            (TaskStatus::Ready(me), TaskStatus::Ready(them)) => me == them,
            (TaskStatus::Spawn(_), TaskStatus::Spawn(_))
            | (TaskStatus::Init, TaskStatus::Init)
            | (TaskStatus::Ignore, TaskStatus::Ignore)
            | (TaskStatus::Wait, TaskStatus::Wait)
            | (TaskStatus::Depends(_), TaskStatus::Depends(_)) => true,
            (TaskStatus::Spread(me), TaskStatus::Spread(them)) => me == them,
            _ => false,
        }
    }
}

impl<D: core::fmt::Debug, P: core::fmt::Debug> core::fmt::Display for TaskStatus<D, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[allow(unused)]
        #[derive(Debug)]
        enum TStatus<'a, D, P> {
            Delayed(time::Duration),
            Pending(&'a P),
            Ready(&'a D),
            Init,
            Spawn,
            Ignore,
            Wait,
            Spread(&'a [TaskSpread<D, P>]),
            Depends,
        }

        let debug_item = match self {
            TaskStatus::Delayed(duration) => TStatus::Delayed(*duration),
            TaskStatus::Pending(inner) => TStatus::Pending(inner),
            TaskStatus::Ready(inner) => TStatus::Ready(inner),
            TaskStatus::Spawn(_) => TStatus::Spawn,
            TaskStatus::Init => TStatus::Init,
            TaskStatus::Ignore => TStatus::Ignore,
            TaskStatus::Wait => TStatus::Wait,
            TaskStatus::Spread(items) => TStatus::Spread(items.as_slice()),
            TaskStatus::Depends(_) => TStatus::Depends,
        };

        write!(f, "{debug_item:?}")
    }
}

impl<D: core::fmt::Debug, P: core::fmt::Debug> core::fmt::Debug for TaskStatus<D, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[allow(unused)]
        #[derive(Debug)]
        enum TStatus<'a, D, P> {
            Delayed(time::Duration),
            Pending(&'a P),
            Ready(&'a D),
            Init,
            Spawn,
            Ignore,
            Wait,
            Spread(&'a [TaskSpread<D, P>]),
            Depends,
        }

        let debug_item = match self {
            TaskStatus::Delayed(duration) => TStatus::Delayed(*duration),
            TaskStatus::Pending(inner) => TStatus::Pending(inner),
            TaskStatus::Ready(inner) => TStatus::Ready(inner),
            TaskStatus::Spawn(_) => TStatus::Spawn,
            TaskStatus::Init => TStatus::Init,
            TaskStatus::Ignore => TStatus::Ignore,
            TaskStatus::Wait => TStatus::Wait,
            TaskStatus::Spread(items) => TStatus::Spread(items.as_slice()),
            TaskStatus::Depends(_) => TStatus::Depends,
        };

        write!(f, "{debug_item:?}")
    }
}

/// `AsTaskIterator` represents a type for an iterator with
/// the underlying output of the iterator to be `TaskStatus`
/// and it's relevant semantics.
pub type AsTaskIterator<D, P> = dyn Iterator<Item = TaskStatus<D, P>>;

/// `BoxedTaskIterator` provides a Boxed (heap-allocated) Iterator based on a `TaskIterator`.
pub type BoxedTaskIterator<D, P> = Box<dyn Iterator<Item = TaskStatus<D, P>>>;

/// `TaskIterator` is an iterator engineered around the concept of asynchronouse
/// task that have 3 states: PENDING, INIT (Initializing) and READY.
///
/// This follows the Iterators in that when the iterator returns None then it's
/// finished, else we can assume its always producing results that might be in any of
/// those states both for either a singular result or multiple elements.
///
/// This means this can keep producing elements as and the caller simply takes the
/// `TaskStatus::Done` state results (if they only care about the final value)
/// else use the other state for whatever is sensible.
///
/// One thing to note is, the same restrictions apply with these iterators, you do
/// not want to block the thread where this is executed or other processes and its always
/// advised to finish early by moving long processes into other threads that allow you
/// notify once next is called to notify if the task is done or still pending.
pub trait TaskIterator {
    /// The type to indicate pending operation, either can be a `time::Duration`
    /// to communicate time to completion or some other type you wish
    /// to allow communication of the state of things before the `Done` signal
    /// is received.
    type Pending;

    /// The type that indicates when a single or partial sequence
    /// is what can be considered a intermediate and final result is.
    ///
    /// In the case of a stream this is the type you wish to produce every single
    /// time that is actual data.
    type Ready;

    /// Advances the iterator and returns the next value.
    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>>;

    /// Wraps this task in the contextual orchestrator, [`TaskAsIterator`], seeding
    /// it with `ctx`.
    ///
    /// This is the single bridge from a bare [`TaskIterator`] to the contextual
    /// orchestration surface. The returned orchestrator holds `ctx`, and every
    /// terminal drain on it (e.g. [`TaskAsIterator::collect_ready`]) pulls that
    /// stored ctx — `Ctx` is never minted internally. Combinators remain available
    /// on the task itself via [`TaskIteratorExt`] before calling `with_ctx`, since
    /// they only forward `ctx` through `next_status` at drive time rather than
    /// needing a ctx of their own.
    fn with_ctx(self, ctx: Ctx) -> TaskAsIterator<Self::Ready, Self::Pending>
    where
        Self: Sized + 'static,
        Self::Ready: 'static,
        Self::Pending: 'static,
    {
        TaskAsIterator::from_impl(self, ctx)
    }
}

/// Newtype wrapper that exposes **only** the [`TaskIterator`] and
/// [`TaskIteratorExt`] surface — standard `Iterator` methods (`.map`,
/// `.filter`, `.collect`, `.next`) are intentionally absent so callers
/// use the valtron-native combinators (`.map_ready`, `.map_pending`,
/// `.filter_ready`, etc.) without ambiguity.
///
/// Construct via [`TaskIteratorExt::into_task_iter`].
///
/// # Why no `Iterator` impl?
///
/// The blanket `TaskIterator` impl (line 594) gives `TaskIterator` to
/// anything that is `Iterator<Item = TaskStatus<…>>`.  That means every
/// `TaskIterator` automatically carries the full `Iterator` method set.
/// By wrapping in `TaskIter<T>` and implementing `TaskIterator`
/// explicitly (coherence: a non-blanket impl on a concrete struct
/// outranks the universal blanket), the chain is broken — no `Iterator`
/// impl is generated.
pub struct TaskIter<T: TaskIterator>(pub T);

impl<T: TaskIterator> TaskIterator for TaskIter<T> {
    type Ready = T::Ready;
    type Pending = T::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.0.next_status(ctx)
    }
}

// Boxed trait objects keep the `TaskIterator` surface so they stay usable with
// `TaskIteratorExt` combinators (e.g. returning `impl TaskIterator + Send`).
// The inner `dyn TaskIterator` forwards its `ctx` by value.
impl<D: 'static, P: 'static> TaskIterator for Box<dyn TaskIterator<Ready = D, Pending = P>> {
    type Ready = D;
    type Pending = P;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        (**self).next_status(ctx)
    }
}

impl<D: 'static, P: 'static> TaskIterator for Box<dyn TaskIterator<Ready = D, Pending = P> + Send> {
    type Ready = D;
    type Pending = P;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        (**self).next_status(ctx)
    }
}

// NOTE: there is intentionally no `impl<M: Iterator> TaskIterator for M` blanket
// here. `TaskIterator` is valtron's internal driver trait and every task must
// implement it explicitly (forwarding its `Ctx`). The only `Iterator`-based
// surface is `StreamIterator`, so callers never bridge from a raw `Iterator`
// into `TaskIterator` implicitly.

pub struct TaskAsIterator<D, P>(Box<dyn TaskIterator<Ready = D, Pending = P>>, Ctx);

impl<D, P> TaskAsIterator<D, P> {
    pub fn from_impl(t: impl TaskIterator<Ready = D, Pending = P> + 'static, ctx: Ctx) -> Self {
        Self(Box::new(t), ctx)
    }

    #[must_use]
    pub fn new(t: Box<dyn TaskIterator<Ready = D, Pending = P>>, ctx: Ctx) -> Self {
        Self(t, ctx)
    }
}

impl<D: 'static, P: 'static> Iterator for TaskAsIterator<D, P> {
    type Item = TaskStatus<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next_status(self.1.clone())
    }
}

/// Terminal (draining) methods for the contextual orchestrator.
///
/// These drive the wrapped task to completion, always threading the `ctx` this
/// orchestrator was seeded with — never minting a fresh `Ctx::new()`.
impl<D: 'static, P: 'static> TaskAsIterator<D, P> {
    /// Drives the task, returning the first `Ready` value (if any).
    #[must_use]
    pub fn first_ready(mut self) -> Option<D> {
        while let Some(status) = self.0.next_status(self.1.clone()) {
            if let TaskStatus::Ready(value) = status {
                return Some(value);
            }
        }
        None
    }

    /// Drives the task, returning the first `Pending` value (if any).
    #[must_use]
    pub fn first_pending(mut self) -> Option<P> {
        while let Some(status) = self.0.next_status(self.1.clone()) {
            if let TaskStatus::Pending(value) = status {
                return Some(value);
            }
        }
        None
    }

    /// Drains the task, collecting every `Ready` value into a `Vec`.
    #[must_use]
    pub fn collect_ready(mut self) -> Vec<D> {
        let mut out = Vec::new();
        while let Some(status) = self.0.next_status(self.1.clone()) {
            if let TaskStatus::Ready(value) = status {
                out.push(value);
            }
        }
        out
    }

    /// Drains the task, returning the first `Ready` value (if any).
    #[must_use]
    pub fn collect_one_ready(mut self) -> Option<D> {
        while let Some(status) = self.0.next_status(self.1.clone()) {
            if let TaskStatus::Ready(value) = status {
                return Some(value);
            }
        }
        None
    }
}

#[derive(Clone)]
pub enum State {
    /// Pending indicates the underlying process to be
    /// still waiting progress to it's next state with
    /// a communicated indicator of how long possibly that
    /// state might be. Its an optional value that the
    /// underlying process could communicate to the executor
    /// that allows the executor to be smarter about how it
    /// polls for progress.
    Pending(Option<time::Duration>),

    /// The state indicating a underlying task Panicked.
    Panicked,

    /// The state is sent out when there was an attempt to spawn
    /// a task from another and that failed which is not a desired
    /// or wanted state to be in ever.
    SpawnFailed(Entry),

    /// The state indicating a spawn action succeeded.
    SpawnFinished(SpawnInfo),

    /// Reschedule indicates we want to reschedule the underlying
    /// task leaving the performance of that to the underlying
    /// process that receives this.
    Reschedule,

    /// Progressed simply indicates the underlying iterator
    /// has progressed in it's state. This lets the executor
    /// perform whatever tracking/progress logic it needs to do
    /// in relation to this.
    Progressed,

    /// Indicate that the state saw a ready value from one of the
    /// task executors, and provides the entry id for the task.
    ReadyValue(Entry),

    /// Done indicates that the iterator has finished can also be when it returns None
    /// but generally this means we should not in anyway process it further.
    Done,

    /// Queue empty, no item yet. Signals executor to yield
    /// and re-check. On JS, maps to a short setTimeout (~4ms).
    Wait,

    /// Wait for an external readiness signal.
    ///
    /// The executor registers this as a `Sleepable::Readiness` sleeper.
    /// The signal must return `false` on first receipt; if `true`, treated as `Pending(None)`.
    Depends(EventReadinessPtr),
}

impl core::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending(d) => f.debug_tuple("Pending").field(d).finish(),
            Self::Panicked => write!(f, "Panicked"),
            Self::SpawnFailed(e) => f.debug_tuple("SpawnFailed").field(e).finish(),
            Self::SpawnFinished(s) => f.debug_tuple("SpawnFinished").field(s).finish(),
            Self::Reschedule => write!(f, "Reschedule"),
            Self::Progressed => write!(f, "Progressed"),
            Self::ReadyValue(e) => f.debug_tuple("ReadyValue").field(e).finish(),
            Self::Done => write!(f, "Done"),
            Self::Wait => write!(f, "Wait"),
            Self::Depends(_) => write!(f, "Depends"),
        }
    }
}

impl PartialEq for State {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Pending(a), Self::Pending(b)) => a == b,
            (Self::SpawnFailed(a), Self::SpawnFailed(b))
            | (Self::ReadyValue(a), Self::ReadyValue(b)) => a == b,
            (Self::Panicked, Self::Panicked)
            | (Self::SpawnFinished(_), Self::SpawnFinished(_))
            | (Self::Reschedule, Self::Reschedule)
            | (Self::Progressed, Self::Progressed)
            | (Self::Done, Self::Done)
            | (Self::Wait, Self::Wait)
            | (Self::Depends(_), Self::Depends(_)) => true,
            _ => false,
        }
    }
}

impl Eq for State {}

pub type BoxedStateIterator = Box<dyn Iterator<Item = State>>;
pub type BoxedSendStateIterator = Box<dyn Iterator<Item = State> + Send>;
pub type BoxedSendSyncStateIterator = Box<dyn Iterator<Item = State> + Send + Sync + 'static>;

/// Shared task queue — `Send` when multi-threaded execution is enabled,
/// `!Send` for single-threaded / wasm32 runtimes.
#[cfg(feature = "multi")]
pub type SharedTaskQueue = Arc<ConcurrentQueue<BoxedSendExecutionIterator>>;

#[cfg(not(feature = "multi"))]
pub type SharedTaskQueue = Arc<ConcurrentQueue<BoxedExecutionIterator>>;

/// Task type for `broadcast` — must match `SharedTaskQueue`'s element type.
#[cfg(feature = "multi")]
pub type GlobalTask = BoxedSendExecutionIterator;

#[cfg(not(feature = "multi"))]
pub type GlobalTask = BoxedExecutionIterator;

pub type BoxedExecutionEngine = Box<dyn ExecutionEngine>;

/// [`ExecutionIterator`] is a type of Iterator that
/// uniquely always just returns the State of
/// it's internal processes and never
/// an actual value of the internal calculation
/// it performs.
///
/// It provides a clean way for an execution engine to
/// progressively generate progress for task only based on
/// the underlying state information it returns.
pub trait ExecutionIterator {
    fn next(&mut self, parent_id: Entry, engine: BoxedExecutionEngine, ctx: Ctx) -> Option<State>;

    /// A short name for this task, used by executor diagnostics.
    ///
    /// WHY: the slow-poll watchdog (feature 139) names the task that blocked a
    /// worker in its warning. The default returns the concrete type's name,
    /// which is a compile-time constant, so it costs nothing to produce.
    fn name(&self) -> &'static str {
        core::any::type_name::<Self>()
    }
}

pub trait IntoBoxedExecutionIterator {
    fn into_box_execution_iterator(self) -> Box<dyn ExecutionIterator>;
}

pub trait IntoBoxedSendExecutionIterator {
    fn into_box_send_execution_iterator(self) -> Box<dyn ExecutionIterator + Send + 'static>;
}

impl<F> IntoBoxedSendExecutionIterator for F
where
    F: ExecutionIterator + Send + 'static,
{
    fn into_box_send_execution_iterator(self) -> Box<dyn ExecutionIterator + Send + 'static> {
        Box::new(self)
    }
}

impl<F> IntoBoxedExecutionIterator for F
where
    F: ExecutionIterator + 'static,
{
    fn into_box_execution_iterator(self) -> Box<dyn ExecutionIterator> {
        Box::new(self)
    }
}

/// [`BoxedExecutionIterator`] defines a [`Box`] version of an [`ExecutionIterator`]
/// which allows us allocate an [`ExecutionIterator`] on the heap.
pub type BoxedExecutionIterator = Box<dyn ExecutionIterator + 'static>;

/// [`BoxedSendExecutionIterator`] defines a [`Box`] version of an [`ExecutionIterator`]
/// which allows us allocate an [`ExecutionIterator`] on the heap that is [`Send`].
pub type BoxedSendExecutionIterator = Box<dyn ExecutionIterator + Send + 'static>;

pub trait CloneableExecutionIterator: ExecutionIterator {
    fn clone_execution_iterator(&self) -> Box<dyn CloneableExecutionIterator>;
}

pub trait IntoRawExecutionIterator: CloneableExecutionIterator {
    fn into_execution_iterator(self) -> Box<dyn ExecutionIterator + Send + 'static>;
}

impl<M> IntoRawExecutionIterator for M
where
    M: CloneableExecutionIterator + Send + 'static,
{
    fn into_execution_iterator(self) -> Box<dyn ExecutionIterator + Send + 'static> {
        Box::new(self)
    }
}

impl<M> CloneableExecutionIterator for M
where
    M: ExecutionIterator + Clone + 'static,
{
    fn clone_execution_iterator(&self) -> Box<dyn CloneableExecutionIterator> {
        Box::new(self.clone())
    }
}

pub struct CanCloneExecutionIterator(Box<dyn CloneableExecutionIterator>);

impl CanCloneExecutionIterator {
    #[must_use]
    pub fn new(elem: Box<dyn CloneableExecutionIterator>) -> Self {
        Self(elem)
    }
}

impl Clone for CanCloneExecutionIterator {
    fn clone(&self) -> Self {
        Self(self.0.clone_execution_iterator())
    }
}

impl ExecutionIterator for CanCloneExecutionIterator {
    fn next(&mut self, entry: Entry, engine: BoxedExecutionEngine, ctx: Ctx) -> Option<State> {
        self.0.next(entry, engine, ctx)
    }

    fn name(&self) -> &'static str {
        self.0.name()
    }
}

impl<M> ExecutionIterator for &mut M
where
    M: ExecutionIterator,
{
    fn next(&mut self, entry: Entry, engine: BoxedExecutionEngine, ctx: Ctx) -> Option<State> {
        (**self).next(entry, engine, ctx)
    }

    fn name(&self) -> &'static str {
        (**self).name()
    }
}

impl<M> ExecutionIterator for RefCell<Box<M>>
where
    M: ExecutionIterator + ?Sized,
{
    fn next(&mut self, entry: Entry, engine: BoxedExecutionEngine, ctx: Ctx) -> Option<State> {
        self.get_mut().next(entry, engine, ctx)
    }

    fn name(&self) -> &'static str {
        self.borrow().name()
    }
}

impl<M> ExecutionIterator for Rc<RefCell<Box<M>>>
where
    M: ExecutionIterator + ?Sized,
{
    fn next(&mut self, entry: Entry, engine: BoxedExecutionEngine, ctx: Ctx) -> Option<State> {
        self.borrow_mut().next(entry, engine, ctx)
    }

    fn name(&self) -> &'static str {
        self.borrow().name()
    }
}

impl<M> ExecutionIterator for Mutex<Box<M>>
where
    M: ExecutionIterator + ?Sized,
{
    fn next(&mut self, entry: Entry, engine: BoxedExecutionEngine, ctx: Ctx) -> Option<State> {
        self.get_mut().unwrap().next(entry, engine, ctx)
    }

    fn name(&self) -> &'static str {
        self.lock().unwrap().name()
    }
}

impl<M> ExecutionIterator for Arc<Mutex<Box<M>>>
where
    M: ExecutionIterator + ?Sized,
{
    fn next(&mut self, entry: Entry, engine: BoxedExecutionEngine, ctx: Ctx) -> Option<State> {
        self.lock().unwrap().next(entry, engine, ctx)
    }

    fn name(&self) -> &'static str {
        self.lock().unwrap().name()
    }
}

impl<M> ExecutionIterator for Box<M>
where
    M: ExecutionIterator + ?Sized,
{
    fn next(&mut self, entry: Entry, engine: BoxedExecutionEngine, ctx: Ctx) -> Option<State> {
        (**self).next(entry, engine, ctx)
    }

    fn name(&self) -> &'static str {
        (**self).name()
    }
}

#[derive(Debug, From)]
pub enum ExecutorError {
    /// Executor failed to lift a new task as priority.
    FailedToLift,

    /// Executor cant schedule new task as provided.
    FailedToSchedule,

    /// Indicates a task has already lifted another
    /// task as priority and cant lift another since by
    /// definition it should not at all be able to do so
    /// since its not being executed at such periods of time.
    AlreadyLiftedTask,

    /// Is issued when a task not currently being processed by the executor
    /// attempts to lift a task as priority, only currently executing tasks
    /// can do that.
    ParentMustBeExecutingToLift,

    /// Issued when queue is closed and we are unable to deliver tasks anymore.
    QueueClosed,

    /// Issued when the queue is a bounded queue and execution of new tasks is
    /// impossible.
    QueueFull,

    /// We failed to create the relevant executor task.
    FailedToCreate,

    /// We failed to create the relevant executor task due to configuration not
    /// being supported.
    NotSupported,

    /// A given executor has no provided task in the case of a [`TaskIterator`]
    /// based `ExecutorIterator`.
    TaskRequired,

    #[from(ignore)]
    FailedToSendThreadActivity,
    ParentMustBeSupplied,
}

impl std::error::Error for ExecutorError {}

impl core::fmt::Display for ExecutorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// `ExecutorEngine` is the backbone of the valtron execution model
/// they can be spawned within threads or be the singular owner
/// of a thread which the user/caller create to manage execution within the
/// thread.
pub trait ExecutionEngine {
    /// `sequenced` prioritizes an incoming task to the top of the local
    /// execution queue as a sequential operation that links both this task
    /// and parent into a execution loop where one execution of the task
    /// must lead to the execution of the parent as well.
    ///
    /// If `parent` is provided then a dependency connection is made
    /// with the relevant parent's identified by the `Entry` key.
    ///
    /// # Errors
    /// Returns `ExecutorError` when the engine cannot accept the task.
    fn sequenced(
        &self,
        task: BoxedExecutionIterator,
        parent: Entry,
    ) -> AnyResult<SpawnInfo, ExecutorError>;

    /// `lift` prioritizes an incoming task to the top of the local
    /// execution queue which pauses all processing task till that
    /// point till the new task is done or goes to sleep (dependent on
    /// the internals of the `ExecutionEngine`).
    ///
    /// If `parent` is provided then a dependency connection is made
    /// with the relevant parent's identified by the `Entry` key.
    /// We expect that such connection will have the task executed
    /// to completion before the parent is scheduled immediately
    /// to process the result or continue its opration.
    ///
    /// # Errors
    /// Returns `ExecutorError` when the engine cannot accept the task.
    fn lift(
        &self,
        task: BoxedExecutionIterator,
        parent: Option<Entry>,
    ) -> AnyResult<SpawnInfo, ExecutorError>;

    /// `schedule` adds provided incoming task to the bottom of the local
    /// execution queue which pauses all processing task till that
    /// point till the new task is done or goes to sleep (dependent on
    /// the internals of the `ExecutionEngine`).
    ///
    /// # Errors
    /// Returns `ExecutorError` when the engine cannot accept the task.
    fn schedule(&self, task: BoxedExecutionIterator) -> AnyResult<SpawnInfo, ExecutorError>;

    /// `broadcast` allows you to deliver a task to the global execution queue
    /// which then lets the giving task to be sent of to the same or another
    /// executor in another thread for processing, which requires the type to be
    /// `Send` safe.
    ///
    /// # Errors
    /// Returns `ExecutorError::QueueFull` or `ExecutorError::QueueClosed` from
    /// the global queue.
    fn broadcast(&self, task: GlobalTask) -> AnyResult<SpawnInfo, ExecutorError>;

    /// `broadcast_or_lift` dispatches a task based on the execution mode:
    /// - `multi=on`: broadcasts the task to the global queue so another worker picks it up
    /// - `multi=off`: lifts the task to the top of the local queue for priority execution
    ///
    /// # Errors
    /// Returns the error from the chosen dispatch call: [`Self::broadcast`] in
    /// multi mode, [`Self::lift`] otherwise.
    fn broadcast_or_lift(
        &self,
        task: GlobalTask,
        parent: Option<Entry>,
    ) -> AnyResult<SpawnInfo, ExecutorError> {
        #[cfg(feature = "multi")]
        {
            let _ = parent;
            self.broadcast(task)
        }
        #[cfg(not(feature = "multi"))]
        {
            self.lift(task, parent)
        }
    }

    /// `broadcast_or_sequence` dispatches a task based on the execution mode:
    /// - `multi=on`: broadcasts the task to the global queue so another worker picks it up
    /// - `multi=off`: sequences the task with the parent for single-threaded execution
    ///
    /// # Errors
    /// Returns the error from the chosen dispatch call: [`Self::broadcast`] in
    /// multi mode, [`Self::sequenced`] otherwise.
    fn broadcast_or_sequence(
        &self,
        task: GlobalTask,
        parent: Entry,
    ) -> AnyResult<SpawnInfo, ExecutorError> {
        #[cfg(feature = "multi")]
        {
            let _ = parent;
            self.broadcast(task)
        }
        #[cfg(not(feature = "multi"))]
        {
            self.sequenced(task, parent)
        }
    }

    /// `boxed_engine` returns a instance of the engine as a [`BoxedExecutionEngine`].
    fn boxed_engine(&self) -> BoxedExecutionEngine;

    /// `shared_queue` returns access to the global queue.
    fn shared_queue(&self) -> SharedTaskQueue;

    /// rng returns a shared thread-safe `ChaCha8Rng` random generation
    /// managed by the executor.
    fn rng(&self) -> rc::Rc<cell::RefCell<ChaCha8Rng>>;
}

pub type BoxedExecutionAction = Box<dyn ExecutionAction>;
pub type BoxedSendExecutionAction = Box<dyn ExecutionAction + Send + 'static>;

/// [`ExecutionAction`] represents a underlying action definition that can
/// spawn some other task by using the provided execution engine and parent key.
///
/// Returns:
///     A result indicating success or failure with the error that caused said failure.
pub trait ExecutionAction {
    /// Applies the action through `engine`, with `key` as the parent entry.
    ///
    /// # Errors
    /// Returns the error from the engine call that cannot accept the task.
    fn apply(
        &mut self,
        key: Option<Entry>,
        engine: BoxedExecutionEngine,
    ) -> GenericResult<SpawnInfo>;
}

impl<M> ExecutionAction for Box<M>
where
    M: ExecutionAction + ?Sized,
{
    fn apply(
        &mut self,
        key: Option<Entry>,
        engine: BoxedExecutionEngine,
    ) -> GenericResult<SpawnInfo> {
        (**self).apply(key, engine)
    }
}

pub trait IntoBoxedSendExecutionAction {
    fn into_box_send_execution_action(self) -> Box<dyn ExecutionAction + Send + 'static>;
}

impl<F> IntoBoxedSendExecutionAction for F
where
    F: ExecutionAction + Send + 'static,
{
    fn into_box_send_execution_action(self) -> Box<dyn ExecutionAction + Send + 'static> {
        Box::new(self)
    }
}

pub trait IntoBoxedExecutionAction {
    fn into_box_execution_action(self) -> Box<dyn ExecutionAction>;
}

impl<F> IntoBoxedExecutionAction for F
where
    F: ExecutionAction + 'static,
{
    fn into_box_execution_action(self) -> Box<dyn ExecutionAction> {
        Box::new(self)
    }
}

#[derive(Default, Clone, Debug)]
pub struct NoAction;

impl ExecutionAction for NoAction {
    fn apply(
        &mut self,
        _entry: Option<Entry>,
        _engine: BoxedExecutionEngine,
    ) -> GenericResult<SpawnInfo> {
        // do nothing
        Ok(SpawnInfo::new(SpawnType::None, None, None))
    }
}

pub type BoxedTaskReadyResolver<D, P> = Box<dyn TaskReadyResolver<D, P>>;

pub type BoxedSendTaskReadyResolver<D, P> = Box<dyn TaskReadyResolver<D, P> + Send + 'static>;

/// `TaskResolver` are types implementing this trait to
/// perform final resolution of a task when the task emits
/// the relevant `TaskStatus::Ready` enum state.
///
/// Unlike a per-status stream transform (use `TaskIteratorExt` combinators for
/// that), these implementing types do
/// not care about the varying states of a `TaskIterator`
/// but about the final state of the task when it signals
/// it's readiness via the `TaskStatus::Ready` state.
pub trait TaskReadyResolver<D, P> {
    fn handle(&self, item: TaskStatus<D, P>, engine: BoxedExecutionEngine);
}

pub trait IntoBoxedSendTaskReadyResolver<D, P> {
    fn into_box_send_task_ready_resolver(self)
        -> Box<dyn TaskReadyResolver<D, P> + Send + 'static>;
}

impl<F, D, P> IntoBoxedSendTaskReadyResolver<D, P> for F
where
    F: TaskReadyResolver<D, P> + Send + 'static,
{
    fn into_box_send_task_ready_resolver(
        self,
    ) -> Box<dyn TaskReadyResolver<D, P> + Send + 'static> {
        Box::new(self)
    }
}

pub trait IntoBoxedTaskReadyResolver<D, P> {
    fn into_box_task_ready_resolver(self) -> Box<dyn TaskReadyResolver<D, P>>;
}

impl<F, D, P> IntoBoxedTaskReadyResolver<D, P> for F
where
    F: TaskReadyResolver<D, P> + Send + 'static,
{
    fn into_box_task_ready_resolver(self) -> Box<dyn TaskReadyResolver<D, P>> {
        Box::new(self)
    }
}

pub type NoResolver<Done, Pending> = NoResolving<Done, Pending>;

pub struct NoResolving<Done, Pending>(PhantomData<(Done, Pending)>);

impl<Done, Pending> NoResolving<Done, Pending> {
    #[must_use]
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<Done, Pending> Default for NoResolving<Done, Pending> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Done, Pending> TaskReadyResolver<Done, Pending> for NoResolving<Done, Pending> {
    fn handle(&self, _item: TaskStatus<Done, Pending>, _engine: BoxedExecutionEngine) {
        // do nothing
    }
}

impl<F, D, P> TaskReadyResolver<D, P> for &mut F
where
    F: TaskReadyResolver<D, P>,
{
    fn handle(&self, item: TaskStatus<D, P>, engine: BoxedExecutionEngine) {
        (**self).handle(item, engine);
    }
}

impl<F, D, P> TaskReadyResolver<D, P> for Box<F>
where
    F: TaskReadyResolver<D, P> + ?Sized,
{
    fn handle(&self, item: TaskStatus<D, P>, engine: BoxedExecutionEngine) {
        (**self).handle(item, engine);
    }
}

pub struct FnMutReady<F>(cell::RefCell<F>);

impl<F> FnMutReady<F> {
    pub fn new(f: F) -> Self {
        Self(cell::RefCell::new(f))
    }
}

impl<F, D, P> TaskReadyResolver<D, P> for FnMutReady<F>
where
    F: FnMut(TaskStatus<D, P>, BoxedExecutionEngine),
{
    fn handle(&self, item: TaskStatus<D, P>, engine: BoxedExecutionEngine) {
        let mut mut_fn = self.0.borrow_mut();
        (mut_fn)(item, engine);
    }
}

pub struct FnReady<F>(F);

impl<F> FnReady<F> {
    pub fn new(f: F) -> Self {
        Self(f)
    }
}

impl<F, D, P> TaskReadyResolver<D, P> for FnReady<F>
where
    F: Fn(TaskStatus<D, P>, BoxedExecutionEngine),
{
    fn handle(&self, item: TaskStatus<D, P>, engine: BoxedExecutionEngine) {
        self.0(item, engine);
    }
}

/// [`OnceCache`] implements a `TaskStatus` iterator that wraps
/// a provided iterator and provides a onetime read semantic
/// on the iterator, where it ends its operation once the first
/// value the iterator is received and returns None from then on.
///
/// It captures the value that allows you to retrieve it via it's
/// [`OnceCache::take`] method.
///
/// if you prefer an iterator that becomes re-usable again after the
/// value is taking look at the [`UntilTake`] iterator.
///
/// Usually yo use these types of iterator in instances where you control ownership
/// of them and can retrieve them after whatever runs them (calling their next)
/// consider it finished.
pub struct OnceCache<D, P, T: Iterator<Item = TaskStatus<D, P>>> {
    iter: T,
    used: Option<()>,
    cache: Option<TaskStatus<D, P>>,
}

impl<D, P, T> OnceCache<D, P, T>
where
    T: Iterator<Item = TaskStatus<D, P>>,
{
    pub fn new(item: T) -> Self {
        Self {
            iter: item,
            cache: None,
            used: None,
        }
    }

    pub fn take(&mut self) -> Option<TaskStatus<D, P>> {
        self.cache.take()
    }
}

impl<D, P, T> Iterator for OnceCache<D, P, T>
where
    T: Iterator<Item = TaskStatus<D, P>>,
{
    type Item = TaskStatus<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.used.is_some() {
            return None;
        }

        match self.iter.next() {
            Some(elem) => match elem {
                TaskStatus::Spawn(inner) => Some(TaskStatus::Spawn(inner)),
                TaskStatus::Delayed(dur) => Some(TaskStatus::Delayed(dur)),
                TaskStatus::Pending(dur) => Some(TaskStatus::Pending(dur)),
                TaskStatus::Init => Some(TaskStatus::Init),
                TaskStatus::Ignore => Some(TaskStatus::Ignore),
                TaskStatus::Wait => Some(TaskStatus::Wait),
                TaskStatus::Ready(item) => {
                    self.cache = Some(TaskStatus::Ready(item));
                    self.used = Some(());
                    None
                }
                TaskStatus::Spread(items) => Some(TaskStatus::Spread(items)),
                TaskStatus::Depends(signal) => Some(TaskStatus::Depends(signal)),
            },
            None => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReadyValue<P> {
    Skip,
    Inner(P),
}

impl<P> ReadyValue<P> {
    pub fn inner(self) -> Option<P> {
        match self {
            Self::Skip => None,
            Self::Inner(inner) => Some(inner),
        }
    }
}

pub struct ReadyValues<D, P, T: Iterator<Item = TaskStatus<D, P>>> {
    iter: T,
}

impl<D, P, T> ReadyValues<D, P, T>
where
    T: Iterator<Item = TaskStatus<D, P>>,
{
    pub fn new(item: T) -> Self {
        Self { iter: item }
    }
}

impl<D, P, T> Iterator for ReadyValues<D, P, T>
where
    T: Iterator<Item = TaskStatus<D, P>>,
{
    type Item = ReadyValue<D>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some(elem) => match elem {
                TaskStatus::Init
                | TaskStatus::Spawn(_)
                | TaskStatus::Delayed(_)
                | TaskStatus::Pending(_)
                | TaskStatus::Ignore
                | TaskStatus::Wait
                | TaskStatus::Depends(_)
                | TaskStatus::Spread(_) => Some(ReadyValue::Skip),
                TaskStatus::Ready(item) => Some(ReadyValue::Inner(item)),
            },
            None => None,
        }
    }
}

/// `UntilTake` implements an iterator that becomes temporarily finished/done
/// by always returning `None` until it's cached value is taken.
///
/// This allows you to allocate the iterator for only one cycle, get it
/// back and re-add it for another cycle later.
///
/// To be clear, the iterator never returns the actual value in next
/// you can use it to cache said value and only have a call to `UntilTake::take`
/// will it ever allow progress.
///
/// Usually you use these types of iterator in instances where you control ownership
/// of them and can retrieve them after whatever runs them (calling their next)
/// consider it finished for one that inverts this behaviour i.e yielding the
/// next value then being unusable till it's reset for reuse, see `UntilReset`.
pub struct UntilTake<D, P, T: Iterator<Item = TaskStatus<D, P>>> {
    iter: T,
    next: Option<TaskStatus<D, P>>,
}

impl<D, P, T> UntilTake<D, P, T>
where
    T: Iterator<Item = TaskStatus<D, P>>,
{
    pub fn new(item: T) -> Self {
        Self {
            iter: item,
            next: None,
        }
    }

    pub fn take(&mut self) -> Option<TaskStatus<D, P>> {
        self.next.take()
    }
}

impl<D, P, T> Iterator for UntilTake<D, P, T>
where
    T: Iterator<Item = TaskStatus<D, P>>,
{
    type Item = TaskStatus<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next.is_some() {
            return None;
        }

        match self.iter.next() {
            Some(elem) => match elem {
                TaskStatus::Delayed(dur) => Some(TaskStatus::Delayed(dur)),
                TaskStatus::Spawn(inner) => Some(TaskStatus::Spawn(inner)),
                TaskStatus::Pending(dur) => Some(TaskStatus::Pending(dur)),
                TaskStatus::Init => Some(TaskStatus::Init),
                TaskStatus::Ignore => Some(TaskStatus::Ignore),
                TaskStatus::Ready(item) => {
                    self.next = Some(TaskStatus::Ready(item));
                    None
                }
                TaskStatus::Wait => Some(TaskStatus::Wait),
                TaskStatus::Spread(items) => Some(TaskStatus::Spread(items)),
                TaskStatus::Depends(signal) => Some(TaskStatus::Depends(signal)),
            },
            None => None,
        }
    }
}

/// `UntilUnblocked` implements an iterator that yields the first received
/// value from a owned iterator after which it becomes blocked until
/// you call `UntilUnblocked::reset` method to be reusable again.
pub struct UntilUnblocked<D, P, T: Iterator<Item = TaskStatus<D, P>>> {
    iter: T,
    blocked: Option<()>,
}

impl<D, P, T> UntilUnblocked<D, P, T>
where
    T: Iterator<Item = TaskStatus<D, P>>,
{
    pub fn new(item: T) -> Self {
        Self {
            iter: item,
            blocked: None,
        }
    }

    pub fn reset(&mut self) {
        self.blocked.take();
    }
}

impl<D, P, T> Iterator for UntilUnblocked<D, P, T>
where
    T: Iterator<Item = TaskStatus<D, P>>,
{
    type Item = TaskStatus<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.blocked.is_some() {
            return None;
        }

        self.iter.next().map(|elem| match elem {
            TaskStatus::Delayed(dur) => TaskStatus::Delayed(dur),
            TaskStatus::Spawn(inner) => TaskStatus::Spawn(inner),
            TaskStatus::Pending(dur) => TaskStatus::Pending(dur),
            TaskStatus::Init => TaskStatus::Init,
            TaskStatus::Ignore => TaskStatus::Ignore,
            TaskStatus::Ready(item) => {
                self.blocked = Some(());
                TaskStatus::Ready(item)
            }
            TaskStatus::Wait => TaskStatus::Wait,
            TaskStatus::Spread(items) => TaskStatus::Spread(items),
            TaskStatus::Depends(signal) => TaskStatus::Depends(signal),
        })
    }
}

// ============================================================================
// TaskIterator Extension Trait and Combinators
// ============================================================================

/// Control enum for the `map_circuit` combinator on `TaskIterator`.
///
/// Allows short-circuiting task iterator chains with three possible outcomes:
/// - `Continue(item)` - Continue iteration with the transformed item
/// - `ReturnAndStop(item)` - Return this item and then stop iteration permanently
/// - `Stop` - Stop iteration without returning anything
///
/// This is useful for error handling patterns where you want to:
/// - Return an error value and immediately stop when an error is encountered
/// - Stop silently without returning anything in certain conditions
/// - Continue normal iteration otherwise
///
/// ## Type Parameters
///
/// - `D`: The Ready/done value type
/// - `P`: The Pending value type
/// - `S`: The ExecutionAction/Spawn type (must implement `ExecutionAction`)
///
/// ## Example
///
/// ```
/// use foundation_core::valtron::{
///     Ctx, TaskIterator, TaskIteratorExt, TaskShortCircuit, TaskStatus,
/// };
///
/// # #[derive(Clone, Copy, PartialEq, Debug)]
/// # enum Outcome {
/// #     Ok(u32),
/// #     Err(&'static str),
/// # }
/// #
/// # struct Steps {
/// #     items: Vec<Outcome>,
/// #     index: usize,
/// # }
/// #
/// # impl TaskIterator for Steps {
/// #     type Ready = Outcome;
/// #     type Pending = ();
/// #
/// #     fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Outcome, ()>> {
/// #         let item = self.items.get(self.index).copied();
/// #         self.index += 1;
/// #         item.map(TaskStatus::Ready)
/// #     }
/// # }
/// #
/// # let ctx = Ctx::new();
/// # let task = Steps {
/// #     items: vec![
/// #         Outcome::Ok(1),
/// #         Outcome::Ok(2),
/// #         Outcome::Err("boom"),
/// #         Outcome::Ok(3),
/// #     ],
/// #     index: 0,
/// # };
/// let mut task = task.map_circuit(|status| match status {
///     TaskStatus::Ready(Outcome::Err(err)) => {
///         TaskShortCircuit::ReturnAndStop(TaskStatus::Ready(Outcome::Err(err)))
///     }
///     TaskStatus::Pending(_) | TaskStatus::Ready(_) => {
///         TaskShortCircuit::Continue(status)
///     }
///     _ => TaskShortCircuit::Stop,
/// });
///
/// assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ready(Outcome::Ok(1))));
/// assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ready(Outcome::Ok(2))));
/// // The error is returned, then iteration stops; Ok(3) is never reached.
/// assert_eq!(
///     task.next_status(ctx.clone()),
///     Some(TaskStatus::Ready(Outcome::Err("boom")))
/// );
/// assert_eq!(task.next_status(ctx), None);
/// ```
pub enum TaskShortCircuit<D, P> {
    /// Continue iteration with the wrapped item
    Continue(TaskStatus<D, P>),
    /// Return this item and then stop iteration permanently
    ReturnAndStop(TaskStatus<D, P>),
    /// Stop iteration without returning anything
    Stop,
}
