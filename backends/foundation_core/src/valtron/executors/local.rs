#![allow(clippy::type_complexity)]
#![allow(clippy::return_self_not_must_use)]

use std::{
    any::Any,
    cell,
    collections::{HashMap, VecDeque},
    rc,
    sync::{
        self,
        atomic::{self, AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time,
};

use crate::{
    synca::{mpp, DurationWaker, Entry, EntryList, IdleMan, OnSignal, Sleepers, Waiter},
    valtron::{
        AnyResult, Ctx, EventReadinessPtr, ExecutionEngine, ExecutionIterator, State,
        TaskIterator, TaskStatus,
    },
};
use crate::{
    synca::{Timeable, Timing},
    valtron::{DualSequeunceChildAndParentLinkedTask, FinishChildBeforeParentTask},
};
use foundation_compact::rng::ChaCha8Rng;
use foundation_compact::rng::SeedableRng;

use concurrent_queue::{ConcurrentQueue, PopError, PushError};

#[allow(unused)]
use crate::compati::Mutex;

use crate::valtron::{WakeCallback, WakeRegistry, WakeResult, WakeableStream};

use crate::valtron::{
    BoxedExecutionEngine, BoxedExecutionIterator, ExecutorError, GlobalTask,
    ProcessController, SharedTaskQueue, SpawnInfo, SpawnType, TaskReadyResolver, TaskSpawnConfig,
};

use crate::valtron::executors::constants::{
    DEFAULT_KILL_SIGNAL_CHECK_INTERVAL, DEFAULT_MAX_YIELD_DURATION,
    DEFAULT_NOTIFY_QUEUE_MAX_SPINS, DEFAULT_READINESS_POLL_QUANTUM, DEFAULT_READINESS_WAIT,
    DEFAULT_WAIT_CHECK_INTERVAL,
};

/// Task that wraps an iterator of plain values and yields items as `TaskStatus::Ready`.
///
/// WHY: Allows standard iterators of plain values to be used as `TaskIterators`.
/// WHAT: Each `next()` call wraps the iterator's item in `TaskStatus::Ready`.
pub struct WrapTask<I, T>
where
    I: Iterator<Item = T>,
{
    iter: I,
}

impl<I, T> WrapTask<I, T>
where
    I: Iterator<Item = T>,
{
    pub fn new(iter: I) -> Self {
        Self { iter }
    }
}

impl<I, T> TaskIterator for WrapTask<I, T>
where
    I: Iterator<Item = T>,
{
    type Pending = ();
    type Ready = T;

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.iter.next().map(TaskStatus::Ready)
    }
}

#[cfg(all(
    target_family = "wasm",
    any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
))]
std::thread_local! {
    static RUN_UNTIL_DEPTH: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// Counts the live `run_until` frames on this thread (JS wasm), so a nested
/// call — a sync helper reached from inside a task — can tell its callers'
/// still-active slots from runnable work.
#[cfg(all(
    target_family = "wasm",
    any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
))]
struct RunUntilDepth {
    outer: usize,
}

#[cfg(all(
    target_family = "wasm",
    any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
))]
impl RunUntilDepth {
    fn enter() -> Self {
        let outer = RUN_UNTIL_DEPTH.with(|d| {
            let outer = d.get();
            d.set(outer + 1);
            outer
        });
        Self { outer }
    }

    /// How many `run_until` frames enclose this one.
    fn outer_frames(&self) -> usize {
        self.outer
    }
}

#[cfg(all(
    target_family = "wasm",
    any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
))]
impl Drop for RunUntilDepth {
    fn drop(&mut self) {
        RUN_UNTIL_DEPTH.with(|d| d.set(self.outer));
    }
}

/// Identifies a thread executor instance.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ThreadId(Entry, String);

impl ThreadId {
    #[must_use]
    pub fn new(entry: Entry, name: String) -> Self {
        Self(entry, name)
    }

    pub fn get_mut(&mut self) -> &mut Entry {
        &mut self.0
    }

    #[must_use]
    pub fn get_ref(&self) -> &Entry {
        &self.0
    }

    #[must_use]
    pub fn get_cloned(&self) -> Entry {
        self.0
    }

    #[must_use]
    pub fn get_name(&self) -> &String {
        &self.1
    }
}

/// Events emitted by thread executors for activity monitoring.
pub enum ThreadActivity {
    Started(ThreadId),
    Stopped(ThreadId),
    Blocked(ThreadId),
    Unblocked(ThreadId),
    Parked(ThreadId),
    Unparked(ThreadId),
    Panicked(ThreadId, Box<dyn Any + Send>),
    BroadcastedTask,
}

impl core::fmt::Display for ThreadActivity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThreadActivity::Panicked(id, _) => write!(f, "ThreadActivity::Panicked({id:?})"),
            ThreadActivity::Started(id) => write!(f, "ThreadActivity::Started({id:?})"),
            ThreadActivity::Stopped(id) => write!(f, "ThreadActivity::Stopped({id:?})"),
            ThreadActivity::Blocked(id) => write!(f, "ThreadActivity::Blocked({id:?})"),
            ThreadActivity::Unblocked(id) => write!(f, "ThreadActivity::Unblocked({id:?})"),
            ThreadActivity::Parked(id) => write!(f, "ThreadActivity::Parked({id:?})"),
            ThreadActivity::Unparked(id) => write!(f, "ThreadActivity::Unparked({id:?})"),
            ThreadActivity::BroadcastedTask => write!(f, "ThreadActivity::BroadcastedTask"),
        }
    }
}

impl core::fmt::Debug for ThreadActivity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThreadActivity::Panicked(id, _) => write!(f, "ThreadActivity::Panicked({id:?})"),
            ThreadActivity::Started(id) => write!(f, "ThreadActivity::Started({id:?})"),
            ThreadActivity::Stopped(id) => write!(f, "ThreadActivity::Stopped({id:?})"),
            ThreadActivity::Blocked(id) => write!(f, "ThreadActivity::Blocked({id:?})"),
            ThreadActivity::Unblocked(id) => write!(f, "ThreadActivity::Unblocked({id:?})"),
            ThreadActivity::Parked(id) => write!(f, "ThreadActivity::Parked({id:?})"),
            ThreadActivity::Unparked(id) => write!(f, "ThreadActivity::Unparked({id:?})"),
            ThreadActivity::BroadcastedTask => write!(f, "ThreadActivity::BroadcastedTask"),
        }
    }
}

/// `PriorityOrder` defines how wake up tasks should placed once woken up.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PriorityOrder {
    Top,
    Bottom,
}

/// `Sleepable` defines specific holders that help
/// indicate the readiness of a task via it's entry.
pub enum Sleepable {
    /// Timable are tasks that can sleep via duration and
    /// will communicate their readiness using when a duration
    /// and time is expired/over.
    Timable(DurationWaker<Entry>),

    /// Flag represents a task that connect a task with a `AtomicBool`
    /// signal will communicate when a giving task is ready.
    Atomic(sync::Arc<AtomicBool>, Entry),

    /// Readiness represents a task waiting on an arbitrary `EventReadiness`
    /// signal. The executor calls `is_ready(None)` to check without blocking.
    Readiness(EventReadinessPtr, Entry),
}

impl core::fmt::Debug for Sleepable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timable(inner) => f.debug_tuple("Timable").field(inner).finish(),
            Self::Atomic(_, entry) => f.debug_tuple("Atomic").field(entry).finish(),
            Self::Readiness(_, entry) => f.debug_tuple("Readiness").field(entry).finish(),
        }
    }
}

impl Sleepable {
    /// Returns the deadline when this sleeper will be ready.
    /// For Timable sleepers, returns the Instant when duration expires.
    /// For Atomic and Readiness sleepers, returns None (no specific deadline).
    pub fn deadline(&self) -> Option<time::Instant> {
        match self {
            Sleepable::Timable(inner) => Some(inner.deadline()),
            Sleepable::Atomic(_, _) | Sleepable::Readiness(_, _) => None,
        }
    }
}

impl Timeable for Sleepable {
    fn remaining_duration(&self) -> Option<time::Duration> {
        match self {
            Sleepable::Timable(inner) => inner.remaining(),
            Sleepable::Atomic(_, _) | Sleepable::Readiness(_, _) => None,
        }
    }
}

impl Waiter for Sleepable {
    fn is_ready(&self, dur: Option<time::Duration>) -> bool {
        match self {
            Sleepable::Timable(inner) => inner.is_ready(dur),
            Sleepable::Atomic(inner, _) => inner.load(atomic::Ordering::SeqCst),
            Sleepable::Readiness(signal, _) => signal.is_ready(dur),
        }
    }
}

// ============================================================================
// Feature 04: Notification-Based Waiting
// ============================================================================

/// `NotifyQueue` wraps a `ConcurrentQueue` with CondVar-based notification.
///
/// Return type for `wait_for_item` that lets callers decide how to handle "nothing yet".
/// `Ready(value)` means an item was received. `None` means the queue is open but empty
/// after exhausting `max_spins` retries — caller decides whether to retry, yield, or stop.
/// When the queue is actually closed, `wait_for_item` returns `None` (Option) to end iteration.
#[derive(Debug, Clone, PartialEq)]
pub enum NotificationItem<T> {
    /// An item was received successfully.
    Ready(T),
    /// No item available after max_spins retries, queue still open.
    /// Caller maps this upward (e.g., to Stream::Wait or TaskStatus::Wait).
    None,
}

/// Configuration for [`NotifyQueue`].
#[derive(Debug, Clone, Copy)]
pub struct NotifyQueueConfig {
    /// Maximum CondVar wait retries before yielding back to the executor.
    /// Default [`DEFAULT_NOTIFY_QUEUE_MAX_SPINS`].
    pub max_spins: usize,
}

impl Default for NotifyQueueConfig {
    fn default() -> Self {
        Self {
            max_spins: DEFAULT_NOTIFY_QUEUE_MAX_SPINS,
        }
    }
}

impl NotifyQueueConfig {
    /// Set [`Self::max_spins`].
    #[must_use]
    pub fn with_max_spins(mut self, max_spins: usize) -> Self {
        self.max_spins = max_spins;
        self
    }
}

/// When `push()` is called, it notifies one waiting consumer via CondVar.
/// Consumers use `wait_for_item()` to block efficiently until an item is available.
///
/// This replaces spin-sleep polling with proper blocking synchronization,
/// significantly reducing CPU usage and latency in multi-threaded scenarios.
#[derive(Debug)]
pub struct NotifyQueue<T> {
    queue: Arc<ConcurrentQueue<T>>,
    /// CondVar for notifying waiting consumers
    condvar: Arc<foundation_nostd::comp::condvar_comp::CondVar>,
    /// Mutex for CondVar wait condition
    mutex: Arc<foundation_nostd::comp::condvar_comp::Mutex<bool>>,
    /// Maximum number of CondVar wait retries before yielding back.
    max_spins: AtomicUsize,
    /// One-shot stream-wake callbacks, fired on `push` and on `close`. Shared
    /// across clones so a consumer's registration is visible to any producer.
    wake: Arc<WakeRegistry>,
    /// One-shot callbacks for a producer waiting on a full queue, fired when
    /// an item is taken through this queue and on `close`.
    space: Arc<WakeRegistry>,
}

impl<T> Clone for NotifyQueue<T> {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            condvar: self.condvar.clone(),
            mutex: self.mutex.clone(),
            max_spins: AtomicUsize::new(self.max_spins.load(Ordering::Relaxed)),
            wake: self.wake.clone(),
            space: self.space.clone(),
        }
    }
}

impl<T> NotifyQueue<T> {
    /// Creates a new NotifyQueue with an unbounded ConcurrentQueue.
    pub fn unbounded() -> Self {
        Self::unbounded_with_config(NotifyQueueConfig::default())
    }

    /// Creates a new unbounded NotifyQueue with the given [`NotifyQueueConfig`].
    pub fn unbounded_with_config(config: NotifyQueueConfig) -> Self {
        Self {
            queue: Arc::new(ConcurrentQueue::unbounded()),
            condvar: Arc::new(foundation_nostd::comp::condvar_comp::CondVar::new()),
            mutex: Arc::new(foundation_nostd::comp::condvar_comp::Mutex::new(false)),
            max_spins: AtomicUsize::new(config.max_spins),
            wake: Arc::new(WakeRegistry::new()),
            space: Arc::new(WakeRegistry::new()),
        }
    }

    /// Creates a new NotifyQueue with a bounded ConcurrentQueue.
    pub fn bounded(capacity: usize) -> Self {
        Self::bounded_with_config(capacity, NotifyQueueConfig::default())
    }

    /// Creates a new bounded NotifyQueue with the given [`NotifyQueueConfig`].
    pub fn bounded_with_config(capacity: usize, config: NotifyQueueConfig) -> Self {
        Self {
            queue: Arc::new(ConcurrentQueue::bounded(capacity)),
            condvar: Arc::new(foundation_nostd::comp::condvar_comp::CondVar::new()),
            mutex: Arc::new(foundation_nostd::comp::condvar_comp::Mutex::new(false)),
            max_spins: AtomicUsize::new(config.max_spins),
            wake: Arc::new(WakeRegistry::new()),
            space: Arc::new(WakeRegistry::new()),
        }
    }

    /// Set the maximum number of CondVar wait retries before yielding back to executor.
    pub fn set_max_spins(&self, max_spins: usize) {
        self.max_spins.store(max_spins, Ordering::Relaxed);
    }

    /// Pushes an item to the queue and notifies one waiting consumer.
    pub fn push(&self, item: T) -> Result<(), PushError<T>> {
        self.queue.push(item)?;
        // Fire any registered stream-wake callbacks. This is level-triggered: an
        // item is now available, so a parked consumer (stream bridge) re-polls.
        // Independent of the CondVar notify below, which serves the thread-blocking
        // `wait_for_item` consumers.
        self.wake.fire();
        // Set notification flag and notify one waiting consumer
        let mut guard = self.mutex.lock().unwrap();
        *guard = true;
        self.condvar.notify_one();
        Ok(())
    }

    /// Pops an item from the queue without waiting. Taking an item wakes a
    /// producer waiting for space ([`Self::register_space_wake`]) — but only
    /// when a producer actually waits, so a pop with no waiting producer stays
    /// lock-free on the wake side.
    pub fn pop(&self) -> Result<T, PopError> {
        let item = self.queue.pop()?;
        if self.space.has_waiters() {
            self.space.fire();
        }
        Ok(item)
    }

    /// Registers a one-shot callback for a producer whose `push` found the
    /// queue full: it fires when an item is taken through this queue or the
    /// queue closes — at once if there is already space or it is closed.
    /// Items taken straight from [`Self::queue`] do not fire it.
    ///
    /// The callback replaces any earlier space wake rather than joining it: a
    /// single producer re-registers on each retry of a full push, so keeping
    /// only the latest stops stale callbacks from piling up during a stall.
    pub fn register_space_wake(&self, cb: WakeCallback) {
        self.space.replace(cb);
        if !self.queue.is_full() || self.queue.is_closed() {
            self.space.fire();
        }
    }

    /// Waits for an item, blocking efficiently via CondVar.
    /// Returns `Some(NotificationItem::Ready(value))` if item available,
    /// `Some(NotificationItem::None)` if queue open but empty after max_spins,
    /// or `None` (Option) if queue is closed — iteration should end.
    pub fn wait_for_item(&self, timeout: time::Duration) -> Option<NotificationItem<T>> {
        // First try non-blocking pop
        match self.pop() {
            Ok(item) => return Some(NotificationItem::Ready(item)),
            Err(PopError::Closed) => return None,
            Err(PopError::Empty) => {}
        }

        let max_spins = self.max_spins.load(Ordering::Relaxed);
        let mut spins = 0;
        let mut guard = self.mutex.lock().unwrap();

        loop {
            if spins >= max_spins {
                *guard = false;
                return Some(NotificationItem::None);
            }

            // Check queue after acquiring lock
            match self.pop() {
                Ok(item) => {
                    *guard = false;
                    return Some(NotificationItem::Ready(item));
                }
                Err(PopError::Closed) => {
                    *guard = false;
                    return None;
                }
                Err(PopError::Empty) => {}
            }

            // Wait with timeout - returns guard when notified or timeout
            let result = self.condvar.wait_timeout(guard, timeout).unwrap();
            guard = result.0;
            spins += 1;

            // Check if item available after waking up
            match self.pop() {
                Ok(item) => {
                    *guard = false;
                    return Some(NotificationItem::Ready(item));
                }
                Err(PopError::Closed) => {
                    *guard = false;
                    return None;
                }
                Err(PopError::Empty) => {}
            }

            // Reset notification flag and loop again
            *guard = false;
        }
    }

    /// Returns true if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Returns true if the queue is closed.
    pub fn is_closed(&self) -> bool {
        self.queue.is_closed()
    }

    /// Returns the current length of the queue.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Closes the queue.
    pub fn close(&self) -> bool {
        let closed = self.queue.close();
        if closed {
            // Wake a consumer parked on an empty-but-open queue so it observes the
            // close. No-op if the queue was already closed (nothing new to observe).
            self.wake.fire();
            // And a producer waiting for space, which will now see `Closed`.
            self.space.fire();
        }
        closed
    }

    /// Registers a one-shot wake callback, fired on the next successful `push` or on
    /// the open→closed transition.
    ///
    /// If the queue already holds an item or is already closed, the callback fires
    /// immediately. A consumer registers after `next()` hands it a status it isn't
    /// waiting for (`Pending`, `Delayed`, `Init`, or a producer-pushed `Wait`), and
    /// that status says nothing about what is queued behind it. When the producer
    /// has already pushed everything and closed, no later `push` or `close` would
    /// ever fire, so the wake has to be delivered here or it is lost.
    pub fn register_wake(&self, cb: WakeCallback) {
        self.wake.register(cb);
        // A push or close racing this check fires the registry itself; firing it
        // again here only finds it empty.
        if !self.queue.is_empty() || self.queue.is_closed() {
            self.wake.fire();
        }
    }

    /// Returns a reference to the underlying queue.
    pub fn queue(&self) -> &ConcurrentQueue<T> {
        &self.queue
    }

    /// Returns a clone of the `Arc` to the underlying queue.
    ///
    /// WHY: A producer parking on a full bounded delivery queue needs to hand the
    /// executor a [`QueueVacancyReadiness`](crate::valtron::QueueVacancyReadiness)
    /// over this same queue (Decision 00 §L1b / Feature 45 Part A), which owns an
    /// `Arc<ConcurrentQueue<T>>` — not a borrow.
    pub fn queue_arc(&self) -> Arc<ConcurrentQueue<T>> {
        self.queue.clone()
    }

    /// Number of registered consumer wakes ([`Self::register_wake`]) not yet
    /// fired. Tests use it to pin that repeated registrations do not grow.
    pub fn pending_wakes(&self) -> usize {
        self.wake.pending_len()
    }

    /// Number of registered producer space wakes ([`Self::register_space_wake`])
    /// not yet fired. Tests use it to pin that repeated registrations do not grow.
    pub fn pending_space_wakes(&self) -> usize {
        self.space.pending_len()
    }
}

/// `NotifyRecvIter` provides notification-based receiving for `NotifyQueue`.
///
/// This is the notification-aware counterpart to `RecvIter` in `mpp.rs`.
/// Instead of spin-polling with park_timeout, it uses CondVar notification
/// for efficient blocking when the queue is empty.
#[derive(Debug)]
pub struct NotifyRecvIter<T> {
    queue: Arc<NotifyQueue<T>>,
}

impl<T> Clone for NotifyRecvIter<T> {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
        }
    }
}

impl<T> NotifyRecvIter<T> {
    pub fn new(queue: Arc<NotifyQueue<T>>) -> Self {
        Self { queue }
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.queue.is_closed()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Blocks efficiently until an item is available or timeout.
    /// Returns `Some(NotificationItem::Ready(value))` if item available,
    /// `Some(NotificationItem::None)` if queue open but empty,
    /// or `None` (Option) if queue is closed.
    pub fn block_recv(&self, timeout: time::Duration) -> Option<NotificationItem<T>> {
        self.queue.wait_for_item(timeout)
    }
}

/// `NotifyRecvIterator` provides an Iterator interface over `NotifyRecvIter`.
///
/// This is the notification-aware counterpart to `RecvIterator` in `mpp.rs`.
/// It uses CondVar notification instead of spin-polling, significantly
/// reducing CPU usage when waiting for values.
#[derive(Debug)]
pub struct NotifyRecvIterator<T>(NotifyRecvIter<T>, time::Duration);

impl<T> NotifyRecvIterator<T> {
    pub fn from_notify_queue(item: Arc<NotifyQueue<T>>, dur: time::Duration) -> Self {
        Self::new(NotifyRecvIter::new(item), dur)
    }

    #[must_use]
    pub fn new(item: NotifyRecvIter<T>, dur: time::Duration) -> Self {
        Self(item, dur)
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl<T> Iterator for NotifyRecvIterator<T> {
    type Item = NotificationItem<T>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.block_recv(self.1) {
            Some(NotificationItem::Ready(value)) => Some(NotificationItem::Ready(value)),
            Some(NotificationItem::None) => Some(NotificationItem::None),
            None => None, // Queue closed, iteration ends
        }
    }
}

// ============================================================================
// NotifyQueueStreamIterator - Notification-based stream iterator
// ============================================================================

use crate::valtron::iterators::Stream;

/// `NotifyQueueStreamIterator` provides notification-based iteration over a `NotifyQueue<Stream<D, P>>`.
///
/// It uses CondVar notification instead of busy polling, significantly reducing CPU
/// usage when waiting for stream values.
///
/// The iterator polls the queue up to `max_turns` times, yielding `Stream::Wait` if no
/// value is available. Between polls, it efficiently waits using CondVar notification.
#[derive(Debug)]
pub struct NotifyQueueStreamIterator<D, P> {
    chan: Arc<NotifyQueue<Stream<D, P>>>,
    max_turns: usize,
    park_duration: time::Duration,
    /// Set once a consumer registers a wake callback: it parks on the wake
    /// instead of blocking, so `next` must not wait on an empty queue.
    woken_consumer: bool,
}

impl<D, P> NotifyQueueStreamIterator<D, P> {
    /// Creates a new NotifyQueueStreamIterator.
    ///
    /// ## Arguments
    ///
    /// * `chan` - The notification queue to poll
    /// * `max_turns` - Number of poll attempts before yielding Stream::Ignore
    /// * `park_duration` - Duration to wait between polls (used as timeout for wait_for_item)
    ///
    /// ## Panics
    ///
    /// Panics if `max_turns` is 0 (must be at least 1)
    pub fn new(
        chan: Arc<NotifyQueue<Stream<D, P>>>,
        max_turns: usize,
        park_duration: time::Duration,
    ) -> Self {
        assert!(max_turns > 0, "max_turns must be greater than 0");
        Self {
            chan,
            max_turns,
            park_duration,
            woken_consumer: false,
        }
    }

    /// Returns a reference to the underlying channel.
    #[must_use]
    pub fn chan(&self) -> &Arc<NotifyQueue<Stream<D, P>>> {
        &self.chan
    }

    /// Returns the configured max_turns value.
    #[must_use]
    pub fn max_turns(&self) -> usize {
        self.max_turns
    }

    /// Returns the configured park_duration value.
    #[must_use]
    pub fn park_duration(&self) -> time::Duration {
        self.park_duration
    }

    /// Returns true if the underlying queue is closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.chan.is_closed()
    }

    /// The next item without waiting: `Some(Some(item))`, `Some(None)` once
    /// the queue is closed and drained, `None` when it is empty for now.
    pub fn try_next(&mut self) -> Option<Option<Stream<D, P>>> {
        match self.chan.pop() {
            Ok(value) => Some(Some(value)),
            Err(PopError::Closed) => Some(None),
            Err(PopError::Empty) => None,
        }
    }

    /// Returns the current number of items in the queue.
    #[must_use]
    pub fn len(&self) -> usize {
        self.chan.len()
    }

    /// Returns true if the queue is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chan.is_empty()
    }
}

impl<D, P> Iterator for NotifyQueueStreamIterator<D, P> {
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        // First try a quick non-blocking pop
        match self.chan.pop() {
            Ok(value) => {
                return Some(value);
            }
            Err(PopError::Closed) => {
                return None;
            }
            Err(PopError::Empty) => {}
        }

        // An async consumer (one that registered a wake) parks on that wake:
        // blocking here would stall its executor thread for up to
        // `max_spins` timed waits per park.
        if self.woken_consumer {
            return Some(Stream::Wait);
        }

        // Queue is empty - block efficiently until item is available
        match self.chan.wait_for_item(self.park_duration) {
            Some(NotificationItem::Ready(value)) => Some(value),
            Some(NotificationItem::None) => {
                // Queue open but empty after max_spins — signal executor to yield
                Some(Stream::Wait)
            }
            None => {
                // Queue closed, iteration ends
                None
            }
        }
    }
}

#[cfg(feature = "multi")]
impl<D, P> WakeableStream for NotifyQueueStreamIterator<D, P>
where
    D: Send + 'static,
    P: Send + 'static,
{
    fn wake(&mut self, cb: WakeCallback) -> WakeResult {
        self.woken_consumer = true;
        self.chan.register_wake(cb);
        WakeResult::Successful
    }

    fn announce(&mut self) {
        self.woken_consumer = true;
    }
}

#[cfg(not(feature = "multi"))]
impl<D, P> WakeableStream for NotifyQueueStreamIterator<D, P>
where
    D: 'static,
    P: 'static,
{
    fn wake(&mut self, cb: WakeCallback) -> WakeResult {
        self.woken_consumer = true;
        self.chan.register_wake(cb);
        WakeResult::Successful
    }

    fn announce(&mut self) {
        self.woken_consumer = true;
    }
}

/// Underlying state of an executor.
/// It is the inner structure used by a `LocalExecutorEngine` to manage
/// all relevant tasks it takes on.
///
/// It's important to note that if a task panic, so will the `ExecutorState`
/// which can't be restored and all tasks owned will be lost, so its important
/// you always handle panic in the actual task itself by using either the `DoNext`,
/// `CollectNext` and `OnNext` and if you implement your own `ExecutionIterator` to
/// ensure to follow the pattern demonstrated in those implementations to correctly
/// handle panic from tasks even at the trade of some runtime cost from Mutex which
/// allow you use [`std::panic::catch_unwind`].
pub struct ExecutorState {
    pub state_owner: String,

    /// what priority should waking task be placed.
    pub wakeup_priority: PriorityOrder,

    /// `global_tasks` are the shared tasks coming from the main thread
    /// they generally will always come in fifo order and will be processed
    /// in the order received.
    pub global_tasks: SharedTaskQueue,

    /// indicates the current task currently being handled for
    /// safety checks.
    pub current_task: rc::Rc<cell::RefCell<Option<Entry>>>,

    /// tasks owned by the executor which should be processed first
    /// before taking on the next task from the global queue,
    /// task generally come in here from the current task taking
    /// from the global queue.
    /// This allows us keep the overall execution of any async
    /// iterator within the same thread and keep a one thread
    /// execution.
    pub local_tasks: rc::Rc<cell::RefCell<EntryList<BoxedExecutionIterator>>>,

    /// `task_graph` provides dependency mapping of a Task (Key) lifted
    /// by another Task (Value) allowing us to go from lifted task
    /// to it's tree of dependents.
    pub task_graph: rc::Rc<cell::RefCell<HashMap<Entry, Entry>>>,

    /// map used to identify that a task was packed.
    pub packed_tasks: rc::Rc<cell::RefCell<HashMap<Entry, bool>>>,

    /// the queue used for performing perioritization, usually
    /// the global task taking will first be stored in the local
    /// tasks list and have it's related Entry pointer added
    /// into the queue for prioritization, and we will never
    /// take any new tasks from the global queue until all
    /// the entries in this queue is empty, this ensures
    /// we can consistently complete every received tasks
    /// till it finishes.
    pub processing: rc::Rc<cell::RefCell<VecDeque<Entry>>>,

    /// this provides consistent and repeatable random number generation
    /// this is exposed by the executor to it's callers via scopes
    /// or direct use that lets you borrow a random number that
    /// always produces the same sets of values when given the
    /// same seed for repeatability.
    pub rng: rc::Rc<cell::RefCell<ChaCha8Rng>>,

    /// sleepers contain a means to register tasks currently being executed
    /// as sleeping, forcing the executing to either in a single threaded environment
    /// like WebAssembly spin endlessly until one of them is ready for
    /// progress once the sleeper indicate the task is ready for waking up.
    ///
    /// Each executor may treat these differently in that they may process
    /// one task at a time until completion or concurrently process
    /// multiple tasks at a time with allotted time slot between them.
    pub sleepers: Sleepers<Sleepable>,

    /// sleepy provides a managed indicator of how many times we've been idle
    /// and recommends how much sleep should the executor take next.
    pub idler: rc::Rc<cell::RefCell<IdleMan>>,

    /// Track consecutive `State::Depends(true)` violations per entry.
    /// After `DEPENDS_TRUE_PANIC_THRESHOLD` consecutive violations, the task panics.
    pub depends_true_violations: rc::Rc<cell::RefCell<HashMap<Entry, u32>>>,

    /// The engine-owned root [`Ctx`]. Forked once per `ExecutionIterator` when a
    /// task enters the engine; each fork is stored in `task_ctx` and shared (by
    /// value clone) with the task on every poll.
    pub root: Ctx,

    /// Per-entry [`Ctx`] fork: one per `ExecutionIterator`, keyed by its `Entry`.
    pub task_ctx: rc::Rc<cell::RefCell<HashMap<Entry, Ctx>>>,
}

// --- constructors

static DEQUEUE_CAPACITY: usize = 10;

/// Maximum consecutive `State::Depends(true)` violations before panic.
///
/// "Parked on an already-ready signal" also occurs benignly: a pipe consumer
/// drains, returns `Depends(queue readiness)`, and the producer refills the
/// queue before the executor inspects the signal. Under sustained throughput
/// (first seen streaming a multi-MB BuildKit tar export) that race can repeat
/// back-to-back, so the streak alone is weak evidence of a bug. The counter
/// resets on every genuine (not-ready) park, the executor degrades gracefully
/// either way (re-queue at the back), and a real livelock — a task that
/// returns `Depends` without ever consuming the signal — repeats indefinitely.
/// The threshold therefore only needs to be finite, not small.
const DEPENDS_TRUE_PANIC_THRESHOLD: u32 = 4096;

impl ExecutorState {
    pub fn new(
        state_owner: String,
        global_tasks: SharedTaskQueue,
        wakeup_priority: PriorityOrder,
        rng: ChaCha8Rng,
        idler: IdleMan,
        readiness_wait: time::Duration,
    ) -> Self {
        Self {
            state_owner,
            wakeup_priority,
            global_tasks,
            rng: rc::Rc::new(cell::RefCell::new(rng)),
            idler: rc::Rc::new(cell::RefCell::new(idler)),
            current_task: rc::Rc::new(cell::RefCell::new(None)),
            task_graph: rc::Rc::new(cell::RefCell::new(HashMap::new())),
            packed_tasks: rc::Rc::new(cell::RefCell::new(HashMap::new())),
            local_tasks: rc::Rc::new(cell::RefCell::new(EntryList::new())),
            sleepers: Sleepers::with_max_readiness_wait(readiness_wait),
            processing: rc::Rc::new(cell::RefCell::new(VecDeque::with_capacity(
                DEQUEUE_CAPACITY,
            ))),
            depends_true_violations: rc::Rc::new(cell::RefCell::new(HashMap::new())),
            root: Ctx::new(),
            task_ctx: rc::Rc::new(cell::RefCell::new(HashMap::new())),
        }
    }
}

// --- implementations

/// `ScheduleOutcome` communicates the outcome of an attempt to
/// schedule a task from the global queue.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScheduleOutcome {
    /// Indicates we successfully acquired a global task
    /// from the global queue.
    GlobalTaskAcquired,

    /// Indicate no global task was acquired and no task
    /// is being processed.
    NoTaskRunningOrAcquired,

    /// `LocalTaskRunning` indicates local task are still
    /// running or pending hence no attempt will be made
    /// to go to the global queue yet.
    LocalTaskRunning,
}

/// `ProgressIndicator` communicates the potential status of
/// an `ExecutorState` and whether it can make progress in it's
/// work, this allows the manager of the executor to smartly
/// manage the overall management of the executor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgressIndicator {
    /// Indicates no work is available for progression.
    NoWork,

    /// Indicates work is available and can be progressed
    CanProgress(Option<State>),

    /// Indicates it needs to wait for some period of time
    /// before progress can be made.
    SpinWait(time::Duration),

    /// Queue empty, nothing available yet. Signals executor to yield
    /// and re-check. On JS, maps to a short setTimeout (~4ms).
    Wait,
}

// --- Task Dependences, Rng and Helper methods

impl Clone for ExecutorState {
    fn clone(&self) -> Self {
        Self {
            state_owner: self.state_owner.clone(),
            rng: self.rng.clone(),
            idler: self.idler.clone(),
            sleepers: self.sleepers.clone(),
            wakeup_priority: self.wakeup_priority.clone(),
            current_task: self.current_task.clone(),
            global_tasks: self.global_tasks.clone(),
            local_tasks: self.local_tasks.clone(),
            task_graph: self.task_graph.clone(),
            packed_tasks: self.packed_tasks.clone(),
            processing: self.processing.clone(),
            depends_true_violations: rc::Rc::new(cell::RefCell::new(HashMap::new())),
            root: self.root.clone(),
            task_ctx: rc::Rc::new(cell::RefCell::new(HashMap::new())),
        }
    }
}

impl core::fmt::Debug for ExecutorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutorState")
            .field("priority", &self.wakeup_priority)
            .field("global_tasks", &self.global_tasks)
            .field("current_task", &self.current_task)
            .field("local_tasks", &self.local_tasks.borrow().allocated_slots())
            .field("task_graph", &self.task_graph)
            .field("packed_tasks", &self.packed_tasks)
            .field("processing", &self.processing)
            .field("rng", &self.rng)
            .field("sleepers", &self.sleepers)
            .field("idler", &self.idler.borrow().count())
            .finish()
    }
}

impl ExecutorState {
    /// Returns a borrowed immutable
    #[must_use]
    pub fn get_rng(&self) -> rc::Rc<cell::RefCell<ChaCha8Rng>> {
        self.rng.clone()
    }

    fn number_of_local_tasks(&self) -> usize {
        self.local_tasks.borrow().active_slots()
    }

    fn number_of_inprocess(&self) -> usize {
        self.processing.borrow().len()
    }

    /// Returns the list of other task dependent on this giving tasks
    /// and their dependents in order.
    #[inline]
    #[must_use]
    pub fn get_task_dependents(&self, target: Entry) -> Vec<Entry> {
        let mut deps = Vec::new();

        let mut previous = Some(target);

        while previous.is_some() {
            let key = previous.unwrap();
            match self.task_graph.borrow().get(&key) {
                Some(dep) => {
                    deps.push(*dep);
                    previous = Some(*dep);
                }
                None => break,
            }
        }

        deps
    }

    /// Returns true/false indicative if the provided task entry
    /// is considered packed.
    #[inline]
    #[must_use]
    pub fn is_packed(&self, target: &Entry) -> bool {
        *self.packed_tasks.borrow().get(target).unwrap_or(&false)
    }

    /// `wake_up` adds the entry into the list of wakers
    /// that should be woken up by the executor.
    ///
    /// # Design Note (Feature 01: Sleeper Lifecycle Safety)
    /// Includes defensive check to skip stale entries that no longer exist in
    /// local_tasks. This prevents panics when a sleeper matures after its
    /// associated task was combined/removed. See `remove_sleepers_for_entry`
    /// for the full stale sleeper lifecycle explanation.
    #[inline]
    pub fn wake_up(&self, target: Entry) {
        // Check if entry still exists in local_tasks
        if !self.local_tasks.borrow_mut().has(&target) {
            // Expected race, not a fault: a sleeper can mature after its task
            // was combined or removed. The skip is the designed behaviour.
            tracing::debug!(
                "[SLEEPER] task {:?} no longer in local_tasks, skipping wake_up",
                target
            );
            return;
        }

        // remove packed registry
        self.packed_tasks.borrow_mut().remove(&target);

        match self.wakeup_priority {
            PriorityOrder::Top => {
                self.processing.borrow_mut().push_front(target);
            }
            PriorityOrder::Bottom => {
                self.processing.borrow_mut().push_back(target);
            }
        }
    }

    /// `next_wakeup` checks all registered sleepers to see if they
    /// matured to the age of being woken up and placed back into
    /// the processing queue.
    #[inline]
    pub fn wakeup_ready_sleepers(&self) {
        for matured in self.sleepers.get_matured() {
            match matured {
                Sleepable::Timable(wakeable) => self.wake_up(wakeable.handle),
                Sleepable::Atomic(_, entry) => self.wake_up(entry),
                Sleepable::Readiness(_, entry) => self.wake_up(entry),
            }
        }
    }

    #[inline]
    #[must_use]
    pub fn number_of_sleepers(&self) -> usize {
        self.sleepers.count()
    }

    /// The maximum wait a readiness check may block before the executor gives
    /// up and re-polls (the readiness-signal wait). See [`DEFAULT_READINESS_WAIT`].
    #[inline]
    #[must_use]
    pub fn readiness_wait(&self) -> time::Duration {
        self.sleepers.max_readiness_wait()
    }

    /// Set the readiness-signal wait on this state's sleepers.
    #[inline]
    pub fn set_readiness_wait(&mut self, dur: time::Duration) {
        self.sleepers.set_max_readiness_wait(dur);
    }

    /// How many tasks currently have a streak of `Depends` on an already-ready
    /// signal (see `DEPENDS_TRUE_PANIC_THRESHOLD`).
    #[inline]
    #[must_use]
    pub fn number_of_depends_true_violations(&self) -> usize {
        self.depends_true_violations.borrow().len()
    }

    #[inline]
    #[must_use]
    pub fn has_sleeping_tasks(&self) -> bool {
        self.sleepers.has_pending_tasks()
    }

    /// Returns the duration until the next sleeper wakes up.
    ///
    /// Used for sleeper-aware yielding - when no work is available,
    /// threads can sleep until the earliest sleeper deadline instead
    /// of using a fixed yield duration.
    ///
    /// Returns None if no sleepers are registered.
    /// Returns Duration::ZERO if a sleeper is already past its deadline.
    #[inline]
    #[must_use]
    pub fn time_until_next_wakeup(&self) -> Option<time::Duration> {
        self.sleepers.time_until_next()
    }

    /// Returns True/False indicative if the executor has any local
    /// task still processing
    #[inline]
    #[must_use]
    pub fn has_local_tasks(&self) -> bool {
        self.local_tasks.borrow().active_slots() > 0
    }

    /// Returns if there is any local task is active
    /// which means:
    ///
    /// 1. Not sleeping
    /// 2. In local task queue
    ///
    #[must_use]
    pub fn has_active_tasks(&self) -> bool {
        self.total_active_tasks() > 0
    }

    /// Returns true/false if processing queue has task.
    #[must_use]
    pub fn has_inflight_task(&self) -> bool {
        !self.processing.borrow().is_empty()
    }

    #[must_use]
    pub fn has_incoming_global_tasks(&self) -> bool {
        !self.global_tasks.is_empty()
    }

    /// Whether this worker has real work to run on the current tick — an active
    /// task, an in-flight (processing-queue) task, or a global task waiting to be
    /// acquired.
    ///
    /// When this is `false` the worker is kept alive solely by sleeping tasks and
    /// is busy-spinning `schedule_and_do_work` until one of them becomes ready.
    #[inline]
    #[must_use]
    pub fn has_runnable_work(&self) -> bool {
        self.has_active_tasks() || self.has_inflight_task() || self.has_incoming_global_tasks()
    }

    /// Returns totla
    #[must_use]
    pub fn total_inprocess_tasks(&self) -> usize {
        self.number_of_inprocess()
    }

    #[must_use]
    pub fn total_sleeping_tasks(&self) -> usize {
        self.sleepers.count()
    }

    /// Returns the total in queue regardless of sleeping or in-progress.
    #[must_use]
    pub fn total_tasks(&self) -> usize {
        self.local_tasks.borrow().active_slots()
    }

    /// Returns the total remaining tasks that are active and not sleeping.
    ///
    /// # Design Note
    /// Uses `saturating_sub` to prevent underflow panic when sleepers reference
    /// entries that have been removed (stale sleeper entries). This can happen when
    /// a task sleeps, then spawns a child via SpawnFinished and gets combined into
    /// a new task. The old entry is removed but its sleeper remains until cleaned up.
    pub fn total_active_tasks(&self) -> usize {
        let local_task_count = self.local_tasks.borrow().active_slots();
        let sleeping_task_count = self.sleepers.count();
        // Use saturating_sub to prevent underflow when sleepers reference
        // entries that have been removed (stale sleeper entries).
        local_task_count.saturating_sub(sleeping_task_count)
    }

    /// Removes any sleepers referencing the given entry.
    ///
    /// # Design Note (Feature 01: Sleeper Lifecycle Safety)
    /// Must be called when a task is removed to prevent stale sleeper entries.
    /// Stale entries occur when:
    /// 1. Task A sleeps (registered in sleepers with entry E)
    /// 2. Task A spawns child via SpawnFinished, gets combined into new task
    /// 3. Old entry E is removed from local_tasks via take()
    /// 4. Sleeper for E remains in sleepers -> stale entry
    /// 5. When sleeper matures, wake_up tries to process non-existent entry -> panic
    ///
    /// Calling this before take() prevents the stale entry issue.
    /// This was the root cause of the "entry must always have a task attached" panic.
    #[inline]
    pub fn remove_sleepers_for_entry(&self, entry: &Entry) {
        let _ = self.sleepers.remove(entry);
    }

    /// Insert a task into the local task list and fork a dedicated [`Ctx`] for it.
    ///
    /// Each `ExecutionIterator` gets its own fork of [`ExecutorState::root`] so a
    /// task's `ctx.insert(..)` never collides with a sibling's; the fork is then
    /// shared (cloned) with the task on every poll.
    #[inline]
    fn insert_local_task(&self, task: BoxedExecutionIterator) -> Entry {
        let entry = self.local_tasks.borrow_mut().insert(task);
        self.task_ctx.borrow_mut().insert(entry, self.root.fork());
        entry
    }

    /// Drop a task's per-entry [`Ctx`] fork once the task is permanently removed
    /// from the local task list.
    #[inline]
    fn remove_task_ctx(&self, entry: &Entry) {
        self.task_ctx.borrow_mut().remove(entry);
    }

    /// `schedule_next` will attempt to pull a new task from the
    /// global queue if no task is pending on the local queue
    /// and if so returns true to indicate success else a false
    /// to indicate no task was taking from the global queue
    /// as the local queue had a task or no task was found.
    #[inline]
    pub fn schedule_next(&self) -> ScheduleOutcome {
        if self.local_tasks.borrow().active_slots() > 0 && !self.processing.borrow().is_empty() {
            return ScheduleOutcome::LocalTaskRunning;
        }

        match self.global_tasks.pop() {
            Ok(task) => {
                let task_entry = self.insert_local_task(task);
                self.processing.borrow_mut().push_front(task_entry);
                ScheduleOutcome::GlobalTaskAcquired
            }
            Err(_) => ScheduleOutcome::NoTaskRunningOrAcquired,
        }
    }

    #[inline]
    pub fn request_global_task(&self) -> ProgressIndicator {
        if self.has_active_tasks() {
            return ProgressIndicator::CanProgress(None);
        }

        match self.schedule_next() {
            ScheduleOutcome::GlobalTaskAcquired => ProgressIndicator::CanProgress(None),
            ScheduleOutcome::NoTaskRunningOrAcquired => {
                if self.has_sleeping_tasks() {
                    return ProgressIndicator::CanProgress(None);
                }
                ProgressIndicator::NoWork
            }
            ScheduleOutcome::LocalTaskRunning => {
                unreachable!("No local task should be running at this point")
            }
        }
    }

    #[inline]
    pub fn schedule_and_do_work(&self, engine: BoxedExecutionEngine) -> ProgressIndicator {
        match self.request_global_task() {
            ProgressIndicator::CanProgress(_) => {}
            ProgressIndicator::NoWork => {
                return ProgressIndicator::NoWork;
            }
            ProgressIndicator::SpinWait(_) => {
                unreachable!("Requesting global task should never spin wait")
            }
            ProgressIndicator::Wait => {
                return ProgressIndicator::Wait;
            }
        }

        self.wakeup_ready_sleepers();

        match self.do_work(engine) {
            ProgressIndicator::CanProgress(state) => ProgressIndicator::CanProgress(state),
            ProgressIndicator::NoWork => {
                if self.has_sleeping_tasks() {
                    // Sleep until the NEAREST deadline. Sleeping to the
                    // furthest (`max_duration`, the previous behavior) made
                    // every shorter timer fire late by the gap to the longest
                    // sleeper — a 10ms-polling I/O pump next to a 1s sleeper
                    // woke ~1s late, collapsing transport throughput.
                    if let Some(min_sleep_dur) = self.sleepers.min_duration() {
                        return ProgressIndicator::SpinWait(min_sleep_dur);
                    }

                    return ProgressIndicator::CanProgress(None);
                }

                // empty the current task marker
                self.current_task.borrow_mut().take();

                match self.idler.borrow_mut().increment() {
                    Some(next_dur) => ProgressIndicator::SpinWait(next_dur),
                    None => ProgressIndicator::NoWork,
                }
            }
            ProgressIndicator::SpinWait(duration) => {
                if self.has_inflight_task() {
                    return ProgressIndicator::CanProgress(None);
                }

                // if there is other work to be done, lets
                // not waste CPU time, we can return CanProgress here
                // to allow process work on other tasks
                // since our process will always wakeup sleepers
                // first anyway.
                if self.has_incoming_global_tasks() {
                    return ProgressIndicator::CanProgress(None);
                }

                // empty the current task marker
                self.current_task.borrow_mut().take();

                // if the current task indicates it wants to spin wait,
                // attempt to get global task else return
                // duration as is.
                match self.schedule_next() {
                    ScheduleOutcome::GlobalTaskAcquired => ProgressIndicator::CanProgress(None),
                    ScheduleOutcome::NoTaskRunningOrAcquired => {
                        ProgressIndicator::SpinWait(duration)
                    }
                    ScheduleOutcome::LocalTaskRunning => {
                        unreachable!("global task never spinWaits")
                    }
                }
            }
            ProgressIndicator::Wait => ProgressIndicator::Wait,
        }
    }

    pub fn check_processing_queue(&self) -> Option<ProgressIndicator> {
        let has_sleeping_tasks = self.has_sleeping_tasks();
        let handle = self.processing.borrow_mut();

        // if after wake up, no task still enters
        // the processing queue then no work is available
        if handle.is_empty() {
            if has_sleeping_tasks {
                return Some(ProgressIndicator::CanProgress(None));
            }
            return Some(ProgressIndicator::NoWork);
        }
        None
    }

    /// `do_work` attempts to call the current iterator to progress
    /// executing the next operation internally till it's ready for
    /// work to begin.
    ///
    /// # Panics
    ///
    /// 1. Will panic if when popping from the processing queue, we fail to get the top entry
    ///    as at this point work should be in queue before `do_work` is called.
    ///
    #[inline]
    pub fn do_work(&self, engine: BoxedExecutionEngine) -> ProgressIndicator {
        // if after wake up, no task still enters
        // the processing queue then no work is available
        if let Some(inner) = self.check_processing_queue() {
            match inner {
                ProgressIndicator::NoWork => {
                    return ProgressIndicator::NoWork;
                }
                ProgressIndicator::CanProgress(inner) => {
                    return ProgressIndicator::CanProgress(inner);
                }
                ProgressIndicator::SpinWait(_) => {
                    unreachable!("check_processing_queue should never reach here")
                }
                ProgressIndicator::Wait => {
                    unreachable!("check_processing_queue should never return Wait")
                }
            }
        }

        let top_entry = self.processing.borrow_mut().pop_front().unwrap();

        let remaining_tasks = self.processing.borrow().len();

        if self.is_packed(&top_entry) {
            return ProgressIndicator::CanProgress(None);
        }

        self.current_task.borrow_mut().replace(top_entry);

        let iter_container = self.local_tasks.borrow_mut().park(&top_entry);
        assert!(
            iter_container.is_some(),
            "An entry must always have a task attached"
        );

        let mut iter = iter_container.unwrap();
        let ctx = self
            .task_ctx
            .borrow()
            .get(&top_entry)
            .cloned()
            .unwrap_or_else(|| self.root.fork());
        let next_result = iter.next(top_entry, engine, ctx);

        // if result is none
        if next_result.is_none() {
            // remove task from queue
            self.local_tasks.borrow_mut().unpark(&top_entry, iter);

            // Clean up sleepers before removing the task to prevent stale entries
            self.remove_sleepers_for_entry(&top_entry);

            self.local_tasks.borrow_mut().take(&top_entry);
            self.remove_task_ctx(&top_entry);

            // Task Iterator is really done
            if remaining_tasks == 0 {
                return ProgressIndicator::NoWork;
            }

            return ProgressIndicator::CanProgress(None);
        }

        // Any state other than `Depends` is a healthy run and ends the task's
        // "already-ready Depends" violation streak — the counter must measure
        // CONSECUTIVE violations only (a task alternating progress and racy
        // parks is fine; one that returns Depends-on-ready every single run is
        // the livelock the tripwire exists for). The `Depends` arm manages the
        // counter itself: increment on an already-ready signal, reset on a
        // genuine park.
        if !matches!(next_result.as_ref().unwrap(), State::Depends(_)) {
            self.depends_true_violations.borrow_mut().remove(&top_entry);
        }

        match next_result.unwrap() {
            State::SpawnFailed(parent_key) => {
                // unpack the entry in the task list
                self.local_tasks.borrow_mut().unpark(&top_entry, iter);
                // Clean up sleepers before removing the task
                self.remove_sleepers_for_entry(&top_entry);
                self.local_tasks.borrow_mut().take(&top_entry);
                self.remove_task_ctx(&top_entry);

                // no need to push entry since it must have

                // Task Iterator is really done
                if remaining_tasks == 0 {
                    ProgressIndicator::NoWork
                } else {
                    ProgressIndicator::CanProgress(Some(State::SpawnFailed(parent_key)))
                }
            }
            State::Panicked => {
                // unpack the entry in the task list
                self.local_tasks.borrow_mut().unpark(&top_entry, iter);
                // Clean up sleepers before removing the task
                self.remove_sleepers_for_entry(&top_entry);
                self.local_tasks.borrow_mut().take(&top_entry);
                self.remove_task_ctx(&top_entry);

                // no need to push entry since it must have

                // Task Iterator is really done
                if remaining_tasks == 0 {
                    ProgressIndicator::NoWork
                } else {
                    ProgressIndicator::CanProgress(Some(State::Panicked))
                }
            }
            State::SpawnFinished(info) => {
                // unpack the entry in the task list
                self.local_tasks.borrow_mut().unpark(&top_entry, iter);

                // create a new task for spawn information where both parent and child tag exists.

                if !info.has_parent_and_child() {
                    return match info.spawn_type() {
                        SpawnType::Lifted => {
                            let child_entry = info.parent().unwrap();

                            // get the parent task - i.e top_entry
                            let parent_task = self
                                .local_tasks
                                .borrow_mut()
                                .take(&top_entry)
                                .expect("get parent task");

                            let child_task = self
                                .local_tasks
                                .borrow_mut()
                                .take(&child_entry)
                                .expect("get child task");

                            // The parent and child are recombined into a single task
                            // below; their individual forks are no longer polled.
                            self.remove_task_ctx(&top_entry);
                            self.remove_task_ctx(&child_entry);

                            let task =
                                FinishChildBeforeParentTask::new(info, parent_task, child_task);

                            let new_task_id = self.insert_local_task(Box::new(task));

                            // push entry back into processing mut
                            self.processing.borrow_mut().push_front(new_task_id);

                            // no need to push entry since it must have
                            ProgressIndicator::CanProgress(Some(State::SpawnFinished(
                                SpawnInfo::new(SpawnType::Lifted, Some(new_task_id), None),
                            )))
                        }
                        SpawnType::Broadcasted | SpawnType::None | SpawnType::Scheduled => {
                            tracing::trace!("spawned process without parent, requeueing: {:?} - id: {:?}", info, top_entry);

                            // push entry back into processing mut
                            self.processing.borrow_mut().push_front(top_entry);

                            // no need to push entry since it must have
                            ProgressIndicator::CanProgress(Some(State::SpawnFinished(info)))
                        }
                        SpawnType::LiftedWithParent | SpawnType::Sequenced => {
                            unreachable!("This state must always come with a parent");
                        }
                    };
                }

                match info.spawn_type() {
                    SpawnType::LiftedWithParent => {
                        assert!(
                            top_entry == info.parent().unwrap(),
                            "Parent entry id must be the same as top entry"
                        );

                        let parent_entry = info.parent().unwrap();
                        let child_entry = info.child().unwrap();

                        // get the parent task - i.e top_entry
                        let parent_task = self
                            .local_tasks
                            .borrow_mut()
                            .take(&parent_entry)
                            .expect("get parent task");

                        let child_task = self
                            .local_tasks
                            .borrow_mut()
                            .take(&child_entry)
                            .expect("get child task");

                        // The parent and child are recombined into a single task
                        // below; their individual forks are no longer polled.
                        self.remove_task_ctx(&parent_entry);
                        self.remove_task_ctx(&child_entry);

                        let task = FinishChildBeforeParentTask::new(info, parent_task, child_task);

                        let new_task_id = self.insert_local_task(Box::new(task));

                        // push entry back into processing mut
                        self.processing.borrow_mut().push_front(new_task_id);

                        // no need to push entry since it must have
                        ProgressIndicator::CanProgress(Some(State::SpawnFinished(SpawnInfo::new(
                            SpawnType::Lifted,
                            Some(new_task_id),
                            None,
                        ))))
                    }
                    SpawnType::Sequenced => {
                        assert!(
                            top_entry == info.parent().unwrap(),
                            "Parent entry id must be the same as top entry"
                        );

                        let parent_entry = info.parent().unwrap();
                        let child_entry = info.child().unwrap();

                        // get the parent task - i.e top_entry
                        let parent_task = self
                            .local_tasks
                            .borrow_mut()
                            .take(&parent_entry)
                            .expect("get parent task");

                        let child_task = self
                            .local_tasks
                            .borrow_mut()
                            .take(&child_entry)
                            .expect("get child task");

                        // The parent and child are recombined into a single task
                        // below; their individual forks are no longer polled.
                        self.remove_task_ctx(&parent_entry);
                        self.remove_task_ctx(&child_entry);

                        let task = DualSequeunceChildAndParentLinkedTask::new(
                            info,
                            parent_task,
                            child_task,
                        );

                        let new_task_id = self.insert_local_task(Box::new(task));

                        // push entry back into processing mut
                        self.processing.borrow_mut().push_front(new_task_id);

                        // no need to push entry since it must have
                        ProgressIndicator::CanProgress(Some(State::SpawnFinished(SpawnInfo::new(
                            SpawnType::Lifted,
                            Some(new_task_id),
                            None,
                        ))))
                    }
                    _ => {
                        tracing::trace!(
                            "spawned process without parent, requeueing: {:?} - id: {:?}",
                            info,
                            top_entry
                        );

                        // push entry back into processing mut
                        self.processing.borrow_mut().push_front(top_entry);

                        // no need to push entry since it must have
                        ProgressIndicator::CanProgress(Some(State::SpawnFinished(info)))
                    }
                }
            }
            State::Done => {
                // now unpack and take entry out of local tasks
                self.local_tasks.borrow_mut().unpark(&top_entry, iter);
                // Clean up sleepers before removing the task
                self.remove_sleepers_for_entry(&top_entry);

                self.local_tasks.borrow_mut().take(&top_entry);
                self.remove_task_ctx(&top_entry);

                // Task Iterator is really done
                if remaining_tasks == 0 {
                    ProgressIndicator::NoWork
                } else {
                    ProgressIndicator::CanProgress(Some(State::Done))
                }
            }
            State::ReadyValue(entry_id) => {
                // unpack the entry in the task list
                self.local_tasks.borrow_mut().unpark(&top_entry, iter);

                // push entry back into processing mut
                self.processing.borrow_mut().push_front(top_entry);
                ProgressIndicator::CanProgress(Some(State::ReadyValue(entry_id)))
            }
            State::Progressed => {
                // unpack the entry in the task list
                self.local_tasks.borrow_mut().unpark(&top_entry, iter);

                // push entry back into processing mut
                self.processing.borrow_mut().push_front(top_entry);
                ProgressIndicator::CanProgress(Some(State::Progressed))
            }
            State::Pending(duration) => {
                // unpack the entry in the task list
                self.local_tasks.borrow_mut().unpark(&top_entry, iter);

                // if we have a duration then we check if
                // we have other tasks pending, and if so
                // we indicate we can make progress else we
                // tell the executor to pause us for that duration
                // till the task is ready.
                //
                // The task that wants to sleep gets removed from
                // the processing queue and gets registered with the
                // sleepers (which monitors task that are sleeping).
                let final_state = if let Some(inner) = duration {
                    // Store sleeper keyed by the task entry to avoid ID collisions
                    self.sleepers.insert(
                        top_entry,
                        Sleepable::Timable(DurationWaker::from_now(top_entry, inner)),
                    );

                    if !self.processing.borrow().is_empty() {
                        return ProgressIndicator::CanProgress(Some(State::Pending(duration)));
                    }

                    // We will send SpinWait but the engine will decide
                    // if it really wants to spin wait or work on other
                    // tasks if CPU thread becomes free.
                    ProgressIndicator::SpinWait(inner)
                } else {
                    // push back to top
                    self.processing.borrow_mut().push_front(top_entry);
                    ProgressIndicator::CanProgress(None)
                };

                final_state
            }
            State::Reschedule => {
                // unpack the entry in the task list
                self.local_tasks.borrow_mut().unpark(&top_entry, iter);

                // add back task into queue
                self.processing.borrow_mut().push_back(top_entry);

                ProgressIndicator::CanProgress(Some(State::Reschedule))
            }
            State::Wait => {
                // unpack the entry in the task list
                self.local_tasks.borrow_mut().unpark(&top_entry, iter);

                // push entry back into processing queue
                self.processing.borrow_mut().push_front(top_entry);

                ProgressIndicator::Wait
            }
            State::Depends(signal) => {
                // 1. Check if task already has a sleeper record -> PANIC
                //    A sleeping task should never reach this point.
                //    It must be woken and its sleeper record removed before
                //    it can send Depends again.
                if self.sleepers.contains(&top_entry) {
                    panic!(
                        "Task {:?} returned State::Depends while already registered as a sleeper. \
                         A sleeping task must be woken before it can send Depends again.",
                        top_entry
                    );
                }

                // 2. If signal is already true -> treat as Pending(None)
                //    Push to BACK of processing queue (not front).
                //    Track consecutive violations -- after 3 -> PANIC
                if signal.is_ready(None) {
                    self.depends_true_violations
                        .borrow_mut()
                        .entry(top_entry)
                        .and_modify(|c| *c += 1)
                        .or_insert(1);

                    let violations = *self
                        .depends_true_violations
                        .borrow()
                        .get(&top_entry)
                        .unwrap();
                    if violations >= DEPENDS_TRUE_PANIC_THRESHOLD {
                        panic!(
                            "Task {:?} returned State::Depends with an already-ready signal {} \
                             consecutive times. This is a task bug, not an executor issue.",
                            top_entry, violations
                        );
                    }

                    // Rate-limit: under sustained pipe throughput every park can
                    // legitimately hit this (see DEPENDS_TRUE_PANIC_THRESHOLD),
                    // and one warn per violation floods the log at wire speed.
                    if violations <= 3 || violations.is_multiple_of(256) {
                        tracing::warn!(
                            "[DEPENDS] Task {:?} returned already-ready signal (violation {}/{})",
                            top_entry,
                            violations,
                            DEPENDS_TRUE_PANIC_THRESHOLD
                        );
                    }

                    self.local_tasks.borrow_mut().unpark(&top_entry, iter);
                    self.processing.borrow_mut().push_back(top_entry);
                    return ProgressIndicator::CanProgress(Some(State::Pending(None)));
                }

                // 3. Signal is false -> register as Sleepable::Readiness(signal, entry)
                // A genuine park also ends any "already-ready" violation streak —
                // the counter tracks CONSECUTIVE violations, so a well-behaved
                // park must clear it or the count accumulates over the task's
                // whole lifetime and eventually panics a healthy hot task.
                self.depends_true_violations.borrow_mut().remove(&top_entry);
                self.local_tasks.borrow_mut().unpark(&top_entry, iter);
                self.sleepers
                    .insert(top_entry, Sleepable::Readiness(signal, top_entry));

                if self.has_incoming_global_tasks() || self.has_inflight_task() {
                    return ProgressIndicator::CanProgress(None);
                }

                // If this task was the only one, spin wait
                match self.idler.borrow_mut().increment() {
                    Some(next_dur) => ProgressIndicator::SpinWait(next_dur),
                    None => ProgressIndicator::NoWork,
                }
            }
        }
    }
}

// --- End of: Task Dependences, Rng and Helper methods

// --- Task spawn methods: Lift, Schedule & Broadcast

impl ExecutorState {
    /// sequenced defers from the [`ExecutorState::lift`] method
    /// where instead we create a co-linked sequential loop between
    /// the parent (via its entry id) creating this task and itself.
    ///
    /// Where the parent will be pulled and used to create a [`DualSequeunceChildAndParentLinkedTask`]
    /// executor which will trigger the child then the parent in sequence.
    ///
    /// This means the currently processed task that might have
    /// called [`ExecutorState::lift`] since the executor will only
    /// ever execute a singular task each time (even if it concurrently)
    /// processes them (based on their operating semantics).
    ///
    /// But even if its from outside a task, understand the new task
    /// will take priority till it's done.
    ///
    /// Returns:
    ///
    /// The result of the spawn operation with the [`SpawnInfo`] containing the
    /// key details of parent and child and what type of operation was performed.
    #[inline]
    pub fn sequenced(
        &self,
        task: BoxedExecutionIterator,
        parent: Entry,
    ) -> AnyResult<SpawnInfo, ExecutorError> {
        // if there is a parent then you need to be
        // the top of the executing set.
        match *self.current_task.borrow() {
            Some(current_task) => {
                if !current_task.eq(&parent) {
                    return Err(ExecutorError::ParentMustBeExecutingToLift);
                }
            }
            None => unreachable!("Parent task should never truly be empty at this point"),
        }

        let task_entry = self.insert_local_task(task);

        Ok(SpawnInfo::new(
            SpawnType::Sequenced,
            Some(task_entry),
            Some(parent),
        ))
    }

    /// lift defers from the [`ExecutorState::schedule`] method
    /// where instead when adding the task into the local queue
    /// but also lifts the task as the highest priority task
    /// to be processed.
    ///
    /// This if the parent is supplied should create a [`FinishChildBeforeParentTask`]
    /// executor which will process the child until its done then move to
    /// the parent in sequence.
    ///
    /// This means the currently processed task that might have
    /// called [`ExecutorState::lift`] since the executor will only
    /// ever execute a singular task each time (even if it concurrently)
    /// processes them (based on their operating semantics).
    ///
    /// But even if its from outside a task, understand the new task
    /// will take priority till it's done.
    #[inline]
    pub fn lift(
        &self,
        task: BoxedExecutionIterator,
        parent: Option<Entry>,
    ) -> AnyResult<SpawnInfo, ExecutorError> {
        // if there is a parent then you need to be
        // the top of the executing set.
        if let Some(parent_handle) = &parent {
            match *self.current_task.borrow() {
                Some(current_task) => {
                    if !current_task.eq(parent_handle) {
                        return Err(ExecutorError::ParentMustBeExecutingToLift);
                    }
                }
                None => unreachable!("Current task should never truly be empty at this point"),
            }
        }

        let task_entry = self.insert_local_task(task);

        Ok(if parent.is_some() {
            SpawnInfo::new(SpawnType::LiftedWithParent, Some(task_entry), parent)
        } else {
            self.processing.borrow_mut().push_front(task_entry);

            SpawnInfo::new(SpawnType::Lifted, Some(task_entry), parent)
        })
    }

    /// schedule appends the provided task into the
    /// end of the local task queue which then gets
    /// processed accordingly when its turns comes up
    /// once all previous tasks and it's sub-tasks have
    /// been resolved.
    ///
    /// Scheduled tasks do not create relationships at all
    /// any task can schedule new task to the executor
    /// and it never affects its execution or priority.
    #[inline]
    pub fn schedule(&self, task: BoxedExecutionIterator) -> AnyResult<SpawnInfo, ExecutorError> {
        let task_entry = self.insert_local_task(task);
        self.processing.borrow_mut().push_back(task_entry);
        Ok(SpawnInfo::new(SpawnType::Scheduled, Some(task_entry), None))
    }
}

impl ExecutorState {
    /// Delivers a task (Iterator) type to the global execution queue
    /// and in such a case you do not have an handle to the task as we
    /// no more have control as to where it gets allocated.
    pub fn broadcast(&self, task: GlobalTask) -> AnyResult<SpawnInfo, ExecutorError> {
        match self.global_tasks.push(task) {
            Ok(()) => Ok(SpawnInfo::new(SpawnType::Broadcasted, None, None)),
            Err(err) => match err {
                PushError::Full(_) => Err(ExecutorError::QueueFull),
                PushError::Closed(_) => Err(ExecutorError::QueueClosed),
            },
        }
    }
}

// --- End of: Task spawn methods: Lift, Schedule & Broadcast

pub struct ReferencedExecutorState {
    inner: rc::Rc<ExecutorState>,
    activities: Option<mpp::Sender<ThreadActivity>>,
}

impl From<rc::Rc<ExecutorState>> for ReferencedExecutorState {
    fn from(value: rc::Rc<ExecutorState>) -> Self {
        ReferencedExecutorState {
            inner: value,
            activities: None,
        }
    }
}

impl Clone for ReferencedExecutorState {
    fn clone(&self) -> Self {
        ReferencedExecutorState {
            inner: rc::Rc::clone(&self.inner),
            activities: self.activities.clone(),
        }
    }
}

#[allow(unused)]
impl ReferencedExecutorState {
    #[must_use]
    pub fn new(
        inner: rc::Rc<ExecutorState>,
        activities: Option<mpp::Sender<ThreadActivity>>,
    ) -> Self {
        Self { inner, activities }
    }

    #[must_use]
    pub fn clone_queue(&self) -> SharedTaskQueue {
        self.inner.global_tasks.clone()
    }

    #[must_use]
    pub fn clone_state(&self) -> rc::Rc<ExecutorState> {
        self.inner.clone()
    }

    fn use_activities(&mut self, activities: mpp::Sender<ThreadActivity>) {
        self.activities = Some(activities);
    }

    fn get_ref(&self) -> &rc::Rc<ExecutorState> {
        &self.inner
    }

    fn get_ref_mut(&mut self) -> &mut rc::Rc<ExecutorState> {
        &mut self.inner
    }

    fn number_of_local_tasks(&self) -> usize {
        self.inner.number_of_local_tasks()
    }

    fn number_of_inprocess(&self) -> usize {
        self.inner.number_of_inprocess()
    }
}

impl ReferencedExecutorState {
    #[inline]
    #[must_use]
    pub fn get_rng(&self) -> rc::Rc<cell::RefCell<ChaCha8Rng>> {
        self.inner.get_rng()
    }

    /// Fork a fresh [`Ctx`] from this engine's stable root.
    ///
    /// The engine owns one root [`Ctx`] seeded at construction; every task it
    /// drives gets a fork of that root (see [`ExecutorState::insert_local_task`]).
    /// This exposes the same fork to code that drives a task alongside the engine
    /// (e.g. [`super::non_sendables::DrivenTaskIterator`]) so it never mints an
    /// unrelated `Ctx::new()`.
    #[inline]
    #[must_use]
    pub fn fork_ctx(&self) -> Ctx {
        self.inner.root.fork()
    }

    #[inline]
    #[must_use]
    pub fn number_of_sleepers(&self) -> usize {
        self.inner.number_of_sleepers()
    }

    /// See [`ExecutorState::readiness_wait`].
    #[inline]
    #[must_use]
    pub fn readiness_wait(&self) -> time::Duration {
        self.inner.readiness_wait()
    }

    /// See [`ExecutorState::set_readiness_wait`].
    ///
    /// # Panics
    /// Panics if the underlying [`ExecutorState`] is shared (the executor was
    /// cloned). Builders run on a fresh executor before it is shared.
    #[inline]
    pub fn set_readiness_wait(&mut self, dur: time::Duration) {
        rc::Rc::get_mut(&mut self.inner)
            .expect("cannot set readiness wait on a shared executor state")
            .set_readiness_wait(dur);
    }

    /// See [`ExecutorState::number_of_depends_true_violations`].
    #[inline]
    #[must_use]
    pub fn number_of_depends_true_violations(&self) -> usize {
        self.inner.number_of_depends_true_violations()
    }

    /// Returns the duration until the next sleeper wakes up.
    /// Used for sleeper-aware yielding in worker threads.
    #[inline]
    #[must_use]
    pub fn time_until_next_wakeup(&self) -> Option<time::Duration> {
        self.inner.time_until_next_wakeup()
    }

    #[inline]
    #[must_use]
    pub fn total_tasks(&self) -> usize {
        self.inner.total_tasks()
    }

    /// Returns true/false if processing queue has task.
    #[must_use]
    pub fn has_inflight_task(&self) -> bool {
        self.inner.has_inflight_task()
    }

    /// See [`ExecutorState::has_runnable_work`].
    #[inline]
    #[must_use]
    pub fn has_runnable_work(&self) -> bool {
        self.inner.has_runnable_work()
    }

    /// See [`ExecutorState::has_incoming_global_tasks`].
    #[must_use]
    pub fn has_incoming_global_tasks(&self) -> bool {
        self.inner.has_incoming_global_tasks()
    }

    /// See [`ExecutorState::total_active_tasks`].
    #[must_use]
    pub fn total_active_tasks(&self) -> usize {
        self.inner.total_active_tasks()
    }

    #[must_use]
    pub fn schedule_and_do_work(&self, engine: BoxedExecutionEngine) -> ProgressIndicator {
        self.inner.schedule_and_do_work(engine)
    }
}

// --- LocalExecutor for ExecutionIterator::Executor

pub struct LocalExecutionEngine {
    inner: rc::Rc<ExecutorState>,
    activities: Option<mpp::Sender<ThreadActivity>>,
}

impl Clone for LocalExecutionEngine {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            activities: self.activities.clone(),
        }
    }
}

impl LocalExecutionEngine {
    #[must_use]
    pub fn new(
        inner: rc::Rc<ExecutorState>,
        activities: Option<mpp::Sender<ThreadActivity>>,
    ) -> Self {
        Self { inner, activities }
    }

    #[allow(unused)]
    fn use_activities(&mut self, activities: mpp::Sender<ThreadActivity>) {
        self.activities = Some(activities);
    }
}

#[allow(clippy::needless_lifetimes)]
impl<'a> ExecutionEngine for Box<&'a LocalExecutionEngine> {
    fn lift(
        &self,
        task: BoxedExecutionIterator,
        parent: Option<Entry>,
    ) -> AnyResult<SpawnInfo, ExecutorError> {
        (**self).lift(task, parent)
    }

    fn sequenced(
        &self,
        task: BoxedExecutionIterator,
        parent: Entry,
    ) -> AnyResult<SpawnInfo, ExecutorError> {
        (**self).sequenced(task, parent)
    }

    fn schedule(&self, task: BoxedExecutionIterator) -> AnyResult<SpawnInfo, ExecutorError> {
        (**self).schedule(task)
    }

    fn broadcast(&self, task: GlobalTask) -> AnyResult<SpawnInfo, ExecutorError> {
        (**self).broadcast(task)
    }

    fn shared_queue(&self) -> SharedTaskQueue {
        (**self).shared_queue()
    }

    fn boxed_engine(&self) -> BoxedExecutionEngine {
        (**self).boxed_engine()
    }

    fn rng(&self) -> rc::Rc<cell::RefCell<ChaCha8Rng>> {
        (**self).rng()
    }
}

impl ExecutionEngine for Box<LocalExecutionEngine> {
    fn sequenced(
        &self,
        task: BoxedExecutionIterator,
        parent: Entry,
    ) -> AnyResult<SpawnInfo, ExecutorError> {
        (**self).sequenced(task, parent)
    }

    fn lift(
        &self,
        task: BoxedExecutionIterator,
        parent: Option<Entry>,
    ) -> AnyResult<SpawnInfo, ExecutorError> {
        (**self).lift(task, parent)
    }

    fn schedule(&self, task: BoxedExecutionIterator) -> AnyResult<SpawnInfo, ExecutorError> {
        (**self).schedule(task)
    }

    fn broadcast(&self, task: GlobalTask) -> AnyResult<SpawnInfo, ExecutorError> {
        (**self).broadcast(task)
    }

    fn shared_queue(&self) -> SharedTaskQueue {
        (**self).shared_queue()
    }

    fn boxed_engine(&self) -> BoxedExecutionEngine {
        (**self).boxed_engine()
    }

    fn rng(&self) -> rc::Rc<cell::RefCell<ChaCha8Rng>> {
        (**self).rng()
    }
}

impl ExecutionEngine for LocalExecutionEngine {
    fn sequenced(
        &self,
        task: BoxedExecutionIterator,
        parent: Entry,
    ) -> AnyResult<SpawnInfo, ExecutorError> {
        let entry = self.inner.sequenced(task, parent)?;
        Ok(entry)
    }

    fn lift(
        &self,
        task: BoxedExecutionIterator,
        parent: Option<Entry>,
    ) -> AnyResult<SpawnInfo, ExecutorError> {
        let entry = self.inner.lift(task, parent)?;
        Ok(entry)
    }

    fn schedule(&self, task: BoxedExecutionIterator) -> AnyResult<SpawnInfo, ExecutorError> {
        let entry = self.inner.schedule(task)?;
        Ok(entry)
    }

    fn broadcast(&self, task: GlobalTask) -> AnyResult<SpawnInfo, ExecutorError> {
        let info = self.inner.broadcast(task)?;
        if let Some(sender) = &self.activities {
            if let Err(err) = sender.send(ThreadActivity::BroadcastedTask) {
                tracing::error!("Failure in sending thread activity occurred: {:?}", err);
                return Err(ExecutorError::FailedToSendThreadActivity);
            }
        }
        Ok(info)
    }

    fn boxed_engine(&self) -> BoxedExecutionEngine {
        Box::new(self.clone())
    }

    /// `shared_queue` returns access to the global queue.
    fn shared_queue(&self) -> SharedTaskQueue {
        self.inner.global_tasks.clone()
    }

    /// rng returns the local executor random number generator.
    fn rng(&self) -> rc::Rc<cell::RefCell<ChaCha8Rng>> {
        self.inner.get_rng()
    }
}

impl ReferencedExecutorState {
    #[inline]
    #[must_use]
    pub fn local_engine(&self) -> LocalExecutionEngine {
        LocalExecutionEngine::new(self.inner.clone(), self.activities.clone())
    }

    #[inline]
    #[must_use]
    pub fn boxed_engine(&self) -> BoxedExecutionEngine {
        Box::new(self.local_engine())
    }
}

// --- End of: LocalExecutor for ExecutionIterator::Executor

/// # `LocalThreadExecutor`
///
/// Detailed is the overall design decision selected in the design and implementation of the executor and
/// it's variants
/// where each provides a specific feature, behavior of each type of executor.
///
/// Executors have two core concepts:
///
/// ### Tasks
/// This describes an execution represented by a rust iterator which provides an adequate means of representing an
/// infinite and asynchronous work that needs to be managed unto completion. Tasks generally can have what you would
/// consider one direct dependent task.
///
/// Generally Tasks are Iterators with callbacks that allow tasks to supply their result when the `Iter::next()` is
/// called, which means each result is delivered as received to the callback for processing, but generally you can also
/// do blocking calls like `collect()` that indicate to the executor you are looking to collecting all the result at
/// surface points (entry and exit calls).
///
/// Since all Tasks are really Iterators, most times your implementations will really work at the Iterator level,
/// where you implement iterators or have functions wrapped as iterators from other iterators which means
/// generally asynchronous, stream like behaviour is backed into your implementation by design.
///
///  This does not mean a task can trigger another, in the concept of the executors describe below,
/// Task A can generate a
/// Task B but the relationship between these tasks can only ever be in two modes
///
///  #### Lift
///  This describes the relationship where a task basically says I am dependent on this task completing
/// before I
/// continue, this means when such a command is received, an executed will:
///
///  1. Take the provided task and allocate it to the top of the executing queue
///  2. Take the current task and move it below the newly lifted task
///  3. Create a direct connection from Task A to Task B to directly correlate that a task that was paused
///     or asleep with a direct upward dependency should also be moved into sleep till
///     the task it lifted is done.
///
///  Generally the iron clad rule is a task can never lift up more than one task because its evidently
/// indicating to the executor it wants to prioritize the lifted task above itself,
/// given it the remaining execution slot till that task completes.
///
///
///  Which might seem limiting since then we loose the ability of mutual progress where we want to progress in the same
/// accordance and order (e.g managing memory by iterator just from next to next which allows a more refined memory
/// management than waiting for a list to be completely collected) but  with such a paradigm we simplify the underlying
/// core operations and lift such ability to a higher level above the core behaviour e.g custom iterators
/// that allow such behaviour, though the default callback system we wish to define also allows this with ease.
///
///  #### Schedule
///
///  This describes the ability of tasks to schedule other tasks after them which might not be immediately after them /// (but you get the idea). In such a case, a task is not saying its dependent on this task for its operation even if
/// technically it might be, but rather its clear indicating this is deferrable work it can leave
/// for later handling by the executor and is not urgent to it's executing
/// operation and hence this can be left for later.
///
/// #### Distribute
///
/// This describes the ability of tasks to opt out to instead of executing a task in the local queue of their own
/// executor but instead to send tasks to the global tasks queue which moves those tasks to a new thread which will own
/// them for execution.
///
/// This allows you intentionally opt out of local queue execution, which then requires that task to be `Send`
/// without `Sync` which means what the given task
/// can't keep a reference to any local values in that thread else must copy them.
///
/// But local tasks cant wait for such tasks to finish to continue execution in anyway, its no different
/// from `Schedule` with the difference being explicitly knowing this task will likely
/// not execute in the same thread (depending on what
/// executor is running).
///
/// ### Sleeps
/// This describe the fact that tasks have the ability to communicate their desire to be put to sleep at some point in
/// time until their sleep cycle is over upon which they should be awoken to continue operation.
///
/// ## Executors
///
/// ### `LocalThreadExecutor`
///
/// A Non threaded foundation executor that executes in the thread it is created in, generally most more complicated
/// executors might use this executor underneath or the `ConcurrentExecutor` underneath. The core behaviour this executor
/// outlines is that it will only ever execute a singular task unto completion. Now to be clear this does not mean the
/// executor waste cycles by doing nothing when a task in the queue is sleeping, delayed or communicates its unreadiness
/// which then allows the executor prioritize other pending tasks but generally a executing task that can make progress
/// will be executed all the way to completion.
///
/// One core things we must note here is no task should ever block the queue else there will be a deadlock and no work
/// can be done.
///
/// *I see benefit for this type of executor in environments like WebAssembly.*
///
/// Outline below are different scenarios their related expectations for
/// how this executor should behave in implemented:
///
/// #### Scenario Concepts (You will meet)
///
/// Below are concepts you will meet and should keep in mind as you reason about these scenarios:
///
/// - [`PriorityOrder`]: means the executor will ensure to maintain existing priority of a task even if it goes to sleep,
///   when the sleep period has expired no matter if another task is executing that task will be demoted for the previous
///   task to become priority.
///
/// - Task Graph: internally the executor should keep a graph (`HashMap` really) that maps Task to it's
///   Dependents (the lifter) in this case, this allows us to do the following:
///   1. Task A lifts Task B so we store in Map: {`B`: Vec<`A`>}
///   2. Task B lifts Task C so we store in Map: {`C`: Vec<`B`>, `B`: Vec<`A`>}
///   3. With Above we can identify the dependency tree by going Task C -> Task B -> Task A to
///      understand the relationship graph and understand which tasks we need to move out of
///      processing since Task C is now sleeping for some period of time.
///
/// #### Scenario 1: Task A to Completion
/// A scenario where a task can execute to completion.
///
/// 1. Task A gets scheduled by executor and can make progress.
/// 2. Executor executes Task A `next()` checking returned `State` and continues to execute till completion.
///
/// #### Scenario 2: Task A Goes to Sleep (Only task in queue)
/// In such a scenario an executing task can indicate it wishes to go to sleep for some period of time with other tasks
/// taking it place to utilize resources better.
///
/// 1. Task A gets scheduled by executor and can make progress
/// 2. Task A wants to sleep for some duration of time
/// 3. Executor removes Task A from queue and puts it to sleep (register sleep waker)
/// 4. Executor pulls new task from global queue and queues it for execution and progress (with: `CoExecutionAllowed`).
/// 5. When Task A sleep expires, executor lift Task A as priority with no dependency
///    and continues executing Task A ///(with: `PriorityOrder`).
///
/// #### Scenario 3: Task A Goes to Sleep (`PriorityOrder`: On)
/// In such a scenario an executing task can indicate it wishes to go to sleep for some period of time with other tasks
/// taking it place to utilize resources better.
///
/// - `PriorityOrder`: means the executor will ensure to maintain existing priority of a task even if it goes to sleep,
///   when the sleep period has expired no matter if another task is executing that task will be demoted for the previous
///   task to become priority.
///
/// 1. Task A gets scheduled by executor and can make progress
/// 2. Task A wants to sleep for some duration of time
/// 3. Executor removes Task A from queue and puts it to sleep (register sleep waker)
/// 4. Executor pulls new task from global queue and queues it for execution and progress (with: `CoExecutionAllowed`).
/// 5. When Task A sleep expires, executor lift Task A as priority with no dependency and
///    continues executing Task A ///(with: `PriorityOrder`).
///
/// #### Scenario 4:  Task A Goes to Sleep (`PriorityOrder`: Off)
/// In such a scenario an executing task can indicate it wishes to go to sleep for some period of time with other tasks
/// taking it place to utilize resources better.
///
/// 1. Task A gets scheduled by executor and can make progress
/// 2. Task A wants to sleep for some duration of time
/// 3. Executor removes Task A from queue and puts it to sleep (register sleep waker)
/// 4. Executor pulls new task from global queue and queues it for execution and progress (with: `CoExecutionAllowed`)
/// 5. When Task A sleep expires, executor schedules Task A to bottom of queue to wait its turn again.
///
/// #### Scenario 5:  Task A spawns Task B which then wants to go to sleep
/// In such a scenario an executing spawns a task as priority that spawns another task as priority which spawns a final
/// one that wishes to sleep.
///
/// 1. Task A gets scheduled by executor and can make progress
/// 2. Task A spawns Task B as priority
/// 2. Task B wants to sleep for some duration of time
/// 3. Executor removes Task B to Task A due to task graph (Task A -> (depends) Task B) from queue
///    and puts Task B to sleep (register sleep waker) and moves Task A from queue till Task B returns from sleep.
/// 4. Executor goes on to execute other tasks and depending on state of `PriorityOrder` will either add Task C to Task A
///    back to end of queue or start of queue.
///
/// ##### Dev Notes
/// I am skeptical if this really is of value but for now it will be supported.
///
/// ### Concurrent Executor
///
/// `ConcurrentExecutor` generally will be executed in a multi-threaded environment which allows more larger control and
/// performance has it can spread tasks to dedicated threads who really are each
/// managing a `LocalThreadExecutor` in a given
/// thread with `CoExecution` turned on.
///
/// One thing must be clear, executors can never be sent across threads, they are designed that way, they exists in the
/// thread they were created in which means their overall operations are serialized as you can't have some other thread
/// do something with the task executor that requires `Sync`.
///
/// `ConcurrentExecutors` have the exact same scenarios as the `LocalThreadExecutor` but it instead coordinates multiple
/// instances of them in as many OS threads as you want.
///
/// ## Higher Order Iterator (HOI) Tasks
///
/// ### Grouped Iterator
///
/// Grouped a HOI task type that sits on the provided executor in that it allows us group a series of tasks in that they
/// make progress together. Unlike `TimeSlice` iterator where each task in the group gets
/// a `TimeSlice`, the Grouped iterator simply just calls `Iter::next()` on all sub-tasks
///
/// ### `TimeSlice` Iterator
///
/// `TimeSlice` iterator implements a variant that sits on the provided executor in that it allows us create a
/// series of sub-tasks that can make progress for a given time slice,
/// noting that it internally manages this tasks in it's `next()` call.
///
/// It achieves this by wrapping each task (basically are type `Iterator`) in a `TimeSlice` structure that grant each
/// iterator a giving time slice within which the task `next()` keeps getting called till the `TimeSlice` issues a
/// State::Reschedule` event that indicates that the executor should move the `TimeSlice` to the bottom of the queue.
///
/// The executor will keep executing those tasks concurrently for their time slice until they reach completion at which
/// point the completed task is removed from the queue and another task is added in.
///
/// #### Scenario 1:  Task A spawns a `TimeSlice` Iterator type Task with Task [B, C, D]
///
/// In such a scenario an executing spawns the `TimeSlice` Iterator as a task making progress in the queue and calls
/// it's `next()` which in turn calls the `next()` method of each sub-tasks (B, C, D)
/// till its `TimeSlice` wrapper indicates to be
/// rescheduled, moving said task (either B, C, D) to the end of it's internal queue till it completes.
///
/// The Executor just keeps receiving `State::Progress` from `TimeSlice` iterator indicating its making progress.
///
/// #### Scenario 2:  Task A spawns a `TimeSlice` Iterator type Task with Task [B, C, D] but C wants to reschedule
///
/// In such a scenario an executing task spawns the `TimeSlice` Iterator as a task making progress in the queue
/// and calls it's `next()` which in turn calls the `next()` method of each sub-tasks (B, C, D)
/// till its `TimeSlice` wrapper indicates
/// to be rescheduled, moving said task (either B, C, D) to the end of it's internal queue till it completes .
///
/// When sub-task C indicates it wants to reschedule, `TimeSlice` iterator moves C out into sleep register with a waker
/// and continues executing task (B, D) within their time slices, and till C indicates its
/// ready to make progress ///`State::Progress` to the `TimeSlice` iterator else keeps skipping C.
///
/// If All sub-task exclaim `State::Reschedule` then `TimeSlice` forwards that to Executor to reschedule `TimeSlice` as a
/// whole.
///
/// The Executor just keeps receiving `State::Progress` from `TimeSlice` iterator indicating its making progress.
///
/// #### Scenario 3:  Task A spawns a `TimeSlice` Iterator type Task with Task [B, C, D] but C wants to sleep
///
/// In such a scenario an executing task spawns the `TimeSlice` Iterator as a task making progress in the queue and calls
/// it's `next()` which in turn calls the `next()` method of each sub-tasks (B, C, D) till
/// its `TimeSlice` wrapper indicates
/// to be rescheduled, moving said task (either B, C, D) to the end of it's internal queue till it completes .
///
/// When sub-task C indicates it wants to sleep, `TimeSlice` iterator moves C out into sleep register with a waker and
/// continues executing task (B, D) within their time slices, and till C wakes up, it keeps sending `State::Progress` to
/// executor.
///
/// If sub-task (B, D) finishes before C wakes up then `TimeSlice` iterator now yields `State::Pending(time::Duration)` to
/// indicate it also should be put to sleep for that given duration time upon which when it
/// wakes will check if it has any
/// sleepers who are ready to be woken and if so moves them into its internal execution queue for progress else issues
/// another `State::Pending(time::Duration)`.
///
/// #### Scenario 4:  Task A spawns a `TimeSlice` Iterator type Task with Task [B, C, D] and all sub-tasks wants to sleep
///
/// In such a scenario an executing task spawns the `TimeSlice` Iterator as a task making progress in
/// the queue and calls it's `next()` which in turn calls the `next()` method of each
/// sub-tasks (B, C, D) till all task indicates they wish to sleep.
///
/// The `TimeSlice` iterator then puts all into its internal sleeping tracker and issues
/// `State::Reschedule` until one of the task is ready to make progress.
///
/// #### Scenario 5:  Task A spawns a `TimeSlice` Iterator type Task with Task [B, C, D] and B wants to lift sub-task
///
/// In such a scenario an executing task spawns the `TimeSlice` Iterator as a task making progress in the queue and which
/// one of the tasks (Task B) would like to lift up another task as priority at which point the `TimeSlice` iterator will
/// replace Task B positionally with new lifted Task (with the same time slice settings as Task B)  in essence
/// putting Task B to sleep till it's lifted Task is done at which point only will
/// Task B enters the group again to continue execution.
///
/// It continues executing the other tasks in their time slice till their completion, applying same logic to them if
/// they wish to lift their own sub-tasks.
///
pub struct LocalThreadExecutor<T: ProcessController + Clone> {
    state_owner: String,
    kill_signal: Option<Arc<OnSignal>>,
    state: ReferencedExecutorState,
    no_work_yield: time::Duration,
    /// Maximum duration to yield even when sleeper deadline is longer.
    /// Ensures threads periodically wake to check for new work.
    max_yield_duration: time::Duration,
    /// Re-poll interval when only readiness-parked tasks remain (see
    /// [`DEFAULT_READINESS_POLL_QUANTUM`]).
    readiness_poll_quantum: time::Duration,
    /// JS event-loop yield interval for an empty queue (see
    /// [`DEFAULT_WAIT_CHECK_INTERVAL`]).
    wait_check_interval: time::Duration,
    yielder: T,

    /// Feature 04: How often to check kill signal in block_on() inner loop.
    /// Default is every 16 iterations (checked within the 200-iteration inner loop).
    kill_signal_check_interval: usize,
}

// --- constructors

impl<T: ProcessController + Clone> Clone for LocalThreadExecutor<T> {
    fn clone(&self) -> Self {
        LocalThreadExecutor {
            state_owner: self.state_owner.clone(),
            state: self.state.clone(),
            yielder: self.yielder.clone(),
            no_work_yield: self.no_work_yield,
            max_yield_duration: self.max_yield_duration,
            readiness_poll_quantum: self.readiness_poll_quantum,
            wait_check_interval: self.wait_check_interval,
            kill_signal: self.kill_signal.clone(),
            kill_signal_check_interval: self.kill_signal_check_interval,
        }
    }
}

// -- Constructors

#[allow(unused)]
#[allow(clippy::too_many_arguments)]
impl<T: ProcessController + Clone> LocalThreadExecutor<T> {
    pub fn new(
        state_owner: String,
        tasks: SharedTaskQueue,
        rng: ChaCha8Rng,
        idler: IdleMan,
        priority: PriorityOrder,
        yielder: T,
        no_work_yield: time::Duration,
        kill_signal: Option<Arc<OnSignal>>,
        activities: Option<mpp::Sender<ThreadActivity>>,
        kill_signal_check_interval: usize,
    ) -> Self {
        Self {
            state_owner: state_owner.clone(),
            yielder,
            kill_signal,
            no_work_yield,
            max_yield_duration: DEFAULT_MAX_YIELD_DURATION,
            readiness_poll_quantum: DEFAULT_READINESS_POLL_QUANTUM,
            wait_check_interval: DEFAULT_WAIT_CHECK_INTERVAL,
            kill_signal_check_interval,
            state: ReferencedExecutorState::new(
                rc::Rc::new(ExecutorState::new(
                    state_owner,
                    tasks,
                    priority,
                    rng,
                    idler,
                    DEFAULT_READINESS_WAIT,
                )),
                activities,
            ),
        }
    }

    /// Set the longest this executor yields between steps, even when a sleeper's
    /// deadline is further out (the idle re-check cap). Default
    /// [`DEFAULT_MAX_YIELD_DURATION`].
    #[must_use]
    pub fn with_max_yield_duration(mut self, duration: time::Duration) -> Self {
        self.max_yield_duration = duration;
        self
    }

    /// Set the re-poll interval when only readiness-parked tasks remain (the
    /// busy-spin guard). Default [`DEFAULT_READINESS_POLL_QUANTUM`].
    #[must_use]
    pub fn with_readiness_poll_quantum(mut self, duration: time::Duration) -> Self {
        self.readiness_poll_quantum = duration;
        self
    }

    /// Set the maximum wait a readiness check may block before the executor
    /// re-polls (the readiness-signal wait). Default [`DEFAULT_READINESS_WAIT`].
    #[must_use]
    pub fn with_readiness_wait(mut self, duration: time::Duration) -> Self {
        self.state.set_readiness_wait(duration);
        self
    }

    /// The current readiness-signal wait (see [`Self::with_readiness_wait`]).
    #[must_use]
    pub fn readiness_wait(&self) -> time::Duration {
        self.state.readiness_wait()
    }

    /// Set the JS event-loop yield interval for an empty queue. Default
    /// [`DEFAULT_WAIT_CHECK_INTERVAL`].
    #[must_use]
    pub fn with_wait_check_interval(mut self, duration: time::Duration) -> Self {
        self.wait_check_interval = duration;
        self
    }

    /// Creates a new local executor which uses the provided
    /// seed for `ChaCha8Rng` generator.
    pub fn from_seed(
        seed: u64,
        state_owner: String,
        tasks: SharedTaskQueue,
        idler: IdleMan,
        priority: PriorityOrder,
        yielder: T,
        no_work_yield: time::Duration,
        kill_signal: Option<Arc<OnSignal>>,
        activities: Option<mpp::Sender<ThreadActivity>>,
    ) -> Self {
        Self::new(
            state_owner,
            tasks,
            ChaCha8Rng::seed_from_u64(seed),
            idler,
            priority,
            yielder,
            no_work_yield,
            kill_signal,
            activities,
            DEFAULT_KILL_SIGNAL_CHECK_INTERVAL,
        )
    }

    /// Allows supplying a custom Rng generator for creating the initial
    /// `ChaCha8Rng` seed.
    pub fn from_rng<R: foundation_compact::rng::Rng>(
        state_owner: String,
        tasks: SharedTaskQueue,
        rng: &mut R,
        idler: IdleMan,
        priority: PriorityOrder,
        yielder: T,
        no_work_yield: time::Duration,
        kill_signal: Option<Arc<OnSignal>>,
        activities: Option<mpp::Sender<ThreadActivity>>,
    ) -> Self {
        Self::from_seed(
            rng.next_u64(),
            state_owner,
            tasks,
            idler,
            priority,
            yielder,
            no_work_yield,
            kill_signal,
            activities,
        )
    }

    /// Feature 04: Sets the kill signal check interval for block_on() inner loop.
    /// Every N iterations, the kill signal is checked within the 200-iteration inner loop.
    /// Default is 16. Lower values check more frequently but add overhead.
    ///
    /// # Example
    /// ```ignore
    /// let executor = LocalThreadExecutor::from_seed(...)
    ///     .with_kill_signal_check_interval(32);
    /// ```
    pub fn with_kill_signal_check_interval(mut self, interval: usize) -> Self {
        self.kill_signal_check_interval = interval;
        self
    }
}

// -- LocalExecutor builder

#[allow(unused)]
impl<T: ProcessController + Clone> LocalThreadExecutor<T> {
    #[inline]
    pub fn local_engine(&self) -> LocalExecutionEngine {
        self.state.local_engine()
    }

    #[inline]
    pub fn boxed_engine(&self) -> BoxedExecutionEngine {
        self.state.boxed_engine()
    }

    /// Fork a fresh [`Ctx`] from this engine's stable root.
    #[inline]
    #[must_use]
    pub fn fork_ctx(&self) -> Ctx {
        self.state.fork_ctx()
    }

    #[inline]
    pub fn number_of_sleepers(&self) -> usize {
        self.state.number_of_sleepers()
    }

    /// See [`ExecutorState::has_runnable_work`].
    #[inline]
    #[must_use]
    pub fn has_runnable_work(&self) -> bool {
        self.state.has_runnable_work()
    }

    /// See [`ExecutorState::number_of_depends_true_violations`].
    #[inline]
    #[must_use]
    pub fn number_of_depends_true_violations(&self) -> usize {
        self.state.number_of_depends_true_violations()
    }

    #[inline]
    pub fn number_of_local_tasks(&self) -> usize {
        self.state.number_of_local_tasks()
    }

    #[inline]
    pub fn number_of_inprocess(&self) -> usize {
        self.state.number_of_inprocess()
    }
}

// --- LocalExecutor rng and run methods

#[allow(unused)]
impl<T: ProcessController + Clone> LocalThreadExecutor<T> {
    /// `get_rng` returns the `LocalThreadExecutor` random number
    /// generator that allows you generate predictable and repeatable
    /// random numbers consistently where such a property is very useful
    /// e.g When doing [DST](https://docs.tigerbeetle.com/about/vopr/).
    #[inline]
    pub fn get_rng(&self) -> rc::Rc<cell::RefCell<ChaCha8Rng>> {
        self.state.get_rng()
    }

    /// `run_once` provides a fine-grained control at what you can
    /// think of as a single tick of progress by the executor.
    /// It will ask the executor to make one singular move in work time
    /// allowing the top task to make progress, this is super useful when
    /// you an an environment where fine-grained control is super important
    /// and you do not care about the `ProcessController` or wish to call
    /// and handle that portion yourself unlike in `block_on`.
    ///
    /// `block_on` and `block_until_finished` call `run_once` internally
    /// as well.
    #[inline]
    pub fn run_once(&self) -> ProgressIndicator {
        let local_executor = self.state.local_engine();
        self.state.schedule_and_do_work(Box::new(local_executor))
    }

    /// Every remaining task is parked on a readiness signal and none on a timer:
    /// nothing can run until the signal's owner flips it.
    ///
    /// On JS wasm the drivers return to the event loop in this state instead of
    /// spinning (which would starve the async work being waited on) or polling on
    /// a timer; the owner calls `request_executor_resume` when the signal flips.
    #[cfg(all(
        target_family = "wasm",
        any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
    ))]
    fn parked_on_readiness_only(&self) -> bool {
        !self.state.has_runnable_work()
            && self.state.number_of_sleepers() > 0
            && self.state.time_until_next_wakeup().is_none()
    }

    /// JS: whether nothing can run but the `outer_frames` tasks that are
    /// blocked inside a nested `run_until` (see [`RunUntilDepth`]).
    #[cfg(all(
        target_family = "wasm",
        any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
    ))]
    fn only_callers_remain(&self, outer_frames: usize) -> bool {
        outer_frames > 0
            && !self.state.has_inflight_task()
            && !self.state.has_incoming_global_tasks()
            && self.state.total_active_tasks() <= outer_frames
            && self.state.time_until_next_wakeup().is_none()
    }

    /// `run_until` provides a fine-grained of the executor which allows
    /// you to loop the executor until it reaches a giving [`ProgressIndicator`]
    /// state upon which you wish it to stop.
    ///
    /// This keeps executing the `schedule_and_do_work` until the condition
    /// with the function is true.
    #[inline]
    pub fn run_until<S>(&self, checker: S)
    where
        S: Fn(ProgressIndicator) -> bool,
    {
        // How many `run_until` frames are live on this thread, this one
        // included: more than one means we were called from inside a task
        // (a sync "block on" helper), whose slot stays active while it is
        // being polled.
        #[cfg(all(
            target_family = "wasm",
            any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
        ))]
        let _depth = RunUntilDepth::enter();
        loop {
            #[cfg(not(target_family = "wasm"))]
            crate::valtron::executors::idle_wake::checking();
            // Check kill_signal if provided, otherwise continue
            if let Some(ref kill_signal) = self.kill_signal {
                if kill_signal.probe() {
                    return;
                }
            }

            let local_executor = self.state.local_engine();

            let response = self.state.schedule_and_do_work(Box::new(local_executor));

            // JS: every remaining task is parked on a readiness signal and none on
            // a timer. Nothing can make progress until JS-side async work (a fetch,
            // a stream read) completes, so hand the thread back to the event loop.
            // Neither spin here (that starves the very work we wait on and freezes
            // the isolate) nor poll on a timer: whoever flips the signal asks for a
            // run via `request_executor_resume` (`WakeSignal` does it for you).
            #[cfg(all(
                target_family = "wasm",
                any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
            ))]
            if self.parked_on_readiness_only() {
                break;
            }

            // JS, nested: the only tasks left active are the ones whose
            // `next_status` called into us. They cannot progress until we
            // return, and nothing else will run while we hold the thread, so
            // looping here would freeze the isolate. Return to the caller.
            #[cfg(all(
                target_family = "wasm",
                any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
            ))]
            if self.only_callers_remain(_depth.outer_frames()) {
                break;
            }

            if checker(response.clone()) {
                break;
            }

            // Nothing left to run: with zero tasks, an idle back-off
            // (`SpinWait` from the idler) or `NoWork` only means the caller
            // has not scheduled anything else yet. Waiting out the back-off
            // (up to a second) would stall a caller that drives the engine
            // between its own steps, so hand control back instead.
            if matches!(
                response,
                ProgressIndicator::NoWork | ProgressIndicator::SpinWait(_)
            ) && self.state.total_tasks() == 0
            {
                break;
            }

            // JS: intercept NoWork → stop and return to JS
            #[cfg(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))]
            if matches!(&response, ProgressIndicator::NoWork) {
                break;
            }

            // JS: intercept Reschedule → yield, let JS do other work
            #[cfg(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))]
            if matches!(
                &response,
                ProgressIndicator::CanProgress(Some(State::Reschedule))
            ) {
                break;
            }

            // if we get recommended spin duration then lets spin that long
            if let ProgressIndicator::SpinWait(spin_duration) = response {
                #[cfg(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))]
                {
                    let signal = self.yielder.yield_for(spin_duration);
                    if self.yielder.should_stop(&signal) {
                        break;
                    }
                }
                #[cfg(not(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")))]
                self.yielder.yield_for(spin_duration);
                continue;
            }

            // JS: intercept Wait → yield to event loop with short timer
            #[cfg(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))]
            if matches!(&response, ProgressIndicator::Wait) {
                let signal = self
                    .yielder
                    .yield_for(self.wait_check_interval);
                if self.yielder.should_stop(&signal) {
                    break;
                }
            }

            // The step made progress and there is still work to run: go
            // straight to the next step. Yielding here (as this loop used to
            // after every step) stalled each step by up to
            // `max_yield_duration`. `CanProgress(None)` is also what the engine
            // reports when only parked tasks remain, hence the runnable check.
            if matches!(response, ProgressIndicator::CanProgress(_))
                && self.state.has_runnable_work()
            {
                continue;
            }

            // Native: a task's `Wait` is a yield ("re-poll me"), and the caller's
            // checker has already had the chance to stop on it. Keep driving rather
            // than sleeping out the idle back-off: the task is still runnable, and
            // native async work progresses on other threads.
            #[cfg(not(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")))]
            if matches!(response, ProgressIndicator::Wait) && self.state.has_runnable_work() {
                continue;
            }

            // Use sleeper-aware yielding: sleep until the next sleeper wakes up
            // or fall back to the default yield duration if no sleepers exist.
            // Cap at `max_yield_duration` to ensure threads periodically check
            // for new work.
            let yield_duration = self
                .state
                .time_until_next_wakeup()
                .unwrap_or(self.no_work_yield)
                .min(self.max_yield_duration);
            self.yielder.yield_for(yield_duration);
        }
    }

    /// `block_until_finished` defers from `block_on` in that it will
    /// continue to execute the executor till the task is ideally finished
    /// and no more work remains in this task.
    ///
    /// This exists for environments like WebAssembly where spawning multiple
    /// threads do not exists but still need a way to make progress for tasks ]
    /// added till they are completed.
    ///
    /// More so, this provides fine grained control to the user to decide at what
    /// point in time they will call the executor to begin progressing on tasks
    /// which allows them to add one or more tasks into the queue before triggering
    /// processing via `block_until_finished`.
    #[inline]
    pub fn block_until_finished(&self) {
        'main_loop: loop {
            #[cfg(not(target_family = "wasm"))]
            crate::valtron::executors::idle_wake::checking();
            for _ in 0..200 {
                let progress = self.run_once();

                // JS: every remaining task is parked on a readiness signal (none
                // on a timer). Return to the event loop; the signal's owner
                // requests a resume (`request_executor_resume`) when it flips.
                // Looping here would spin without ever letting JS run.
                #[cfg(all(
                    target_family = "wasm",
                    any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
                ))]
                if self.parked_on_readiness_only() {
                    break 'main_loop;
                }

                match progress {
                    ProgressIndicator::NoWork => {
                        #[cfg(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))]
                        {
                            break 'main_loop; // Nothing to do, return to JS
                        }
                        #[cfg(not(any(
                            feature = "js-wasmbindgen",
                            feature = "js-foundation-wasm"
                        )))]
                        break 'main_loop;
                    }
                    ProgressIndicator::SpinWait(duration) => {
                        #[cfg(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))]
                        {
                            let signal = self.yielder.yield_for(duration);
                            if self.yielder.should_stop(&signal) {
                                break 'main_loop; // JS event loop will resume us
                            }
                        }
                        #[cfg(not(any(
                            feature = "js-wasmbindgen",
                            feature = "js-foundation-wasm"
                        )))]
                        self.yielder.yield_for(duration);
                    }
                    ProgressIndicator::CanProgress(_) => {}
                    ProgressIndicator::Wait => {
                        #[cfg(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))]
                        {
                            let signal = self
                                .yielder
                                .yield_for(self.wait_check_interval);
                            if self.yielder.should_stop(&signal) {
                                break 'main_loop; // JS event loop will check for new tasks
                            }
                        }
                        #[cfg(not(any(
                            feature = "js-wasmbindgen",
                            feature = "js-foundation-wasm"
                        )))]
                        self.yielder.yield_for(self.no_work_yield);
                    }
                }
            }
            // Use sleeper-aware yielding when no immediate work
            let yield_duration = self
                .state
                .time_until_next_wakeup()
                .unwrap_or(self.no_work_yield);
            self.yielder.yield_for(yield_duration);
        }
    }

    /// `block_on` usually should in a separate thread where it will block
    /// the thread until either a panic occurs or the kill signal is sent
    /// to the thread.
    ///
    /// # Panics
    ///
    /// 1. Will panic if `self.kill_signal` is not instantiated or set.
    ///
    #[inline]
    pub fn block_on(&self) {
        /// panics: if [`kill_signal`] is not set or instantiated
        /// require `kill_signal` to be provided.
        let kill_signal = self.kill_signal
            .clone()
            .expect("Calling LocalThreadExecutor::block_on requires kill_signal to be provided for termination");

        loop {
            #[cfg(not(target_family = "wasm"))]
            crate::valtron::executors::idle_wake::checking();
            if kill_signal.probe() {
                return;
            }

            // Feature 04: Check kill signal every N iterations instead of every iteration
            // This reduces overhead while still ensuring timely shutdown
            for i in 0..200 {
                // Check kill signal at configured interval (default: every 16 iterations)
                if i % self.kill_signal_check_interval == 0 && kill_signal.probe() {
                    return;
                }

                match self.run_once() {
                    ProgressIndicator::CanProgress(_) => {
                        // The step reported progress, but when nothing is actually
                        // runnable — only readiness-parked sleepers remain — sleep
                        // the readiness quantum instead of busy-spinning until one
                        // is woken. The reactor (or a bridged future's waker) calls
                        // `request_executor_resume`, which interrupts this wait.
                        if !self.state.has_runnable_work()
                            && self.state.number_of_sleepers() > 0
                        {
                            self.yielder.yield_for(self.readiness_poll_quantum);
                        }
                    }
                    ProgressIndicator::NoWork => {
                        // Also check kill signal when no work
                        if kill_signal.probe() {
                            return;
                        }

                        // Use sleeper-aware yielding: sleep until next sleeper wakes.
                        // Readiness-parked tasks (pipe waits) have NO deadline and
                        // their cross-thread wake cannot interrupt a plain sleep, so
                        // a worker hosting them must poll at a short quantum — a
                        // blind `no_work_yield` (seconds) here turned every
                        // cross-worker pipe handoff into a multi-second stall
                        // (first seen collapsing BuildKit tar-export throughput).
                        let yield_duration =
                            self.state.time_until_next_wakeup().unwrap_or_else(|| {
                                if self.state.number_of_sleepers() > 0 {
                                    self.readiness_poll_quantum
                                } else {
                                    self.no_work_yield
                                }
                            });
                        self.yielder.yield_for(yield_duration);
                    }
                    ProgressIndicator::SpinWait(duration) => {
                        // Also check kill signal during spin wait
                        if kill_signal.probe() {
                            return;
                        }

                        self.yielder.yield_for(duration);
                    }
                    ProgressIndicator::Wait => {
                        // A task said "re-poll me" (no deadline). Cap the wait
                        // at `max_yield_duration` so the worker re-polls it and
                        // looks for new work promptly, instead of sleeping the
                        // full `no_work_yield` (seconds) between polls.
                        if kill_signal.probe() {
                            return;
                        }

                        self.yielder
                            .yield_for(self.no_work_yield.min(self.max_yield_duration));
                    }
                }
            }
        }
    }
}

// --- Access Methods run once

/// A task builder like [`any_task`], for a task (and its values) that are
/// `Send` — what the multi-threaded executor requires. Where the task is
/// scheduled is chosen on the [`TaskSpawnConfig`] (`as_scheduled` for the
/// local queue, `as_broadcast` for the global one, `as_lifted`, ...), not by
/// this builder.
#[must_use]
pub fn send_any_task<Task>(
    engine: BoxedExecutionEngine,
) -> TaskSpawnConfig<
    Task::Ready,
    Task::Pending,
    Box<dyn TaskReadyResolver<Task::Ready, Task::Pending> + Send + 'static>,
    Task,
>
where
    Task::Ready: Send,
    Task::Pending: Send,
    Task: TaskIterator + Send + 'static,
{
    TaskSpawnConfig::new(engine)
}

/// A task builder like `typed_task`, for a task, its values and its resolver
/// that are `Send` — what the multi-threaded executor requires. Where the task
/// is scheduled is chosen on the [`TaskSpawnConfig`] (`as_scheduled`,
/// `as_broadcast`, `as_lifted`, ...), not by this builder.
#[must_use]
pub fn send_typed_task<Task, Resolver>(
    engine: BoxedExecutionEngine,
) -> TaskSpawnConfig<Task::Ready, Task::Pending, Resolver, Task>
where
    Task::Ready: Send + 'static,
    Task::Pending: Send + 'static,
    Task: TaskIterator + Send + 'static,
    Resolver: TaskReadyResolver<Task::Ready, Task::Pending> + Send + 'static,
{
    TaskSpawnConfig::new(engine)
}
