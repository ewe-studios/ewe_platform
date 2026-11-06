//! Per-task wake for parked readiness (spec 70 feature 80).
//!
//! WHY: a task that parks on a readiness (`TaskStatus::Depends`) has to run
//! again when the readiness becomes ready. Until feature 80 every source did
//! that by calling a process-wide function (`request_executor_resume`) that
//! interrupted every idle executor thread. A wake meant for one task cost every
//! other executor a pass over its parked tasks (a "wake storm"), and a source
//! that forgot the call was a lost wake.
//!
//! WHAT: the same idea as [`std::task::Waker`]. When a task parks, its executor
//! gives the readiness a [`ReadinessWaker`] that belongs to **that task**
//! ([`EventReadiness::register`](crate::valtron::EventReadiness::register)).
//! The source keeps the waker and calls [`ReadinessWaker::wake`] when it becomes
//! ready or closes. That is all a source ever does: it never calls an executor
//! function. The wake marks that one task runnable and interrupts only the
//! executor thread that owns it.
//!
//! HOW: [`ReadinessNotifier`] is the small construct a source holds to keep the
//! wakers registered on it and to wake them all. [`EventQueue`] is a queue that
//! owns its notifiers, so a `push`, a `pop` and a `close` wake the right side on
//! their own.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Wake, Waker};

use concurrent_queue::{ConcurrentQueue, ForcePushError, PopError, PushError};

use crate::valtron::{QueueClosedReadiness, QueueReadiness, QueueVacancyReadiness};

/// The executor a [`ReadinessWaker`] wakes: the thread that parked the task.
#[cfg(not(target_family = "wasm"))]
type Target = Arc<crate::valtron::executors::idle_wake::ExecutorWake>;
/// On wasm one executor runs everything: the target is the host event loop.
#[cfg(target_family = "wasm")]
type Target = ();

#[cfg(not(target_family = "wasm"))]
fn current_target() -> Target {
    crate::valtron::executors::idle_wake::current()
}

#[cfg(target_family = "wasm")]
fn current_target() -> Target {}

#[cfg(not(target_family = "wasm"))]
fn wake_target(target: &Target) {
    target.wake();
}

#[cfg(target_family = "wasm")]
fn wake_target(_target: &Target) {
    crate::valtron::request_executor_resume();
}

/// The state one parked task shares with the wakers it handed out.
///
/// The executor makes one when a task parks and drops it when the task runs
/// again, so a waker that a source keeps for too long is a cheap no-op.
pub(crate) struct ParkState {
    /// The task is still parked on this registration.
    parked: AtomicBool,
    /// A wake arrived since the executor last looked at this task.
    woken: AtomicBool,
    /// The executor that owns the task.
    target: Target,
}

impl ParkState {
    /// A new registration for a task parking on the calling thread's executor.
    ///
    /// `woken` starts `true`: the executor checks the readiness once after it
    /// registers, so a change that happened before the registration is seen.
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            parked: AtomicBool::new(true),
            woken: AtomicBool::new(true),
            target: current_target(),
        })
    }

    /// The waker to hand to the readiness.
    pub(crate) fn waker(self: &Arc<Self>) -> ReadinessWaker {
        ReadinessWaker {
            waker: Waker::from(Arc::clone(self)),
            park: Some(Arc::clone(self)),
        }
    }

    /// Whether a wake arrived since the last call (and forget it).
    pub(crate) fn take_woken(&self) -> bool {
        self.woken.swap(false, Ordering::SeqCst)
    }

    /// The task is not parked on this registration any more.
    pub(crate) fn release(&self) {
        self.parked.store(false, Ordering::SeqCst);
    }

    fn is_parked(&self) -> bool {
        self.parked.load(Ordering::SeqCst)
    }
}

impl Wake for ParkState {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        if self.is_parked() {
            self.woken.store(true, Ordering::SeqCst);
            wake_target(&self.target);
        }
    }
}

/// Releases a [`ParkState`] when the sleeper that holds it goes away.
pub struct ParkGuard(pub(crate) Arc<ParkState>);

impl Drop for ParkGuard {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// A waker for one parked task. See the module docs.
///
/// A source stores it (see [`ReadinessNotifier`]) and calls [`Self::wake`] when
/// its readiness becomes ready or closes. It is cheap to clone, `Send` and
/// `Sync`, and it is built on [`std::task::Waker`].
#[derive(Clone)]
pub struct ReadinessWaker {
    waker: Waker,
    /// The parked task this waker belongs to, when the executor made it.
    park: Option<Arc<ParkState>>,
}

impl std::fmt::Debug for ReadinessWaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadinessWaker")
            .field("live", &self.is_live())
            .finish()
    }
}

impl ReadinessWaker {
    /// Wake the task: mark it runnable in its own executor and interrupt only
    /// that executor's thread. Idempotent; a no-op once the task runs again.
    pub fn wake(&self) {
        self.waker.wake_by_ref();
    }

    /// Whether `other` wakes the same task (so a re-register can skip a clone).
    #[must_use]
    pub fn will_wake(&self, other: &Self) -> bool {
        self.waker.will_wake(&other.waker)
    }

    /// Whether the task is still parked on this waker. A source may drop a
    /// waker that is not live.
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.park.as_ref().is_none_or(|park| park.is_parked())
    }

    /// A waker built on a standard waker, for a source that bridges to a
    /// `Future` (waking it wakes that waker).
    #[must_use]
    pub fn from_waker(waker: Waker) -> Self {
        Self { waker, park: None }
    }

    /// The standard waker behind this one, for a source that needs a
    /// [`std::task::Waker`] (the reactor's drain thread, for example).
    #[must_use]
    pub fn as_waker(&self) -> &Waker {
        &self.waker
    }

    /// A waker that does nothing. For code that must pass one to
    /// `EventReadiness::register` outside an executor (tests).
    #[must_use]
    pub fn noop() -> Self {
        Self {
            waker: Waker::noop().clone(),
            park: None,
        }
    }
}

/// The construct a source holds to keep the wakers registered on it and to wake
/// them all.
///
/// A readiness that becomes ready by an event implements
/// `EventReadiness::register` with [`Self::register`] and calls [`Self::notify`]
/// where the event happens. Several tasks may park on one readiness (clones of
/// one signal), so the notifier keeps one waker per task. A re-register of the
/// same task does not add a second waker, and wakers of tasks that are not
/// parked any more are dropped on the next register.
#[derive(Default)]
pub struct ReadinessNotifier {
    wakers: Mutex<Vec<ReadinessWaker>>,
    /// `wakers.len()`, readable without the lock so a `notify` with nobody
    /// registered costs one load (a hot queue pushes millions of items).
    count: AtomicUsize,
}

impl std::fmt::Debug for ReadinessNotifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadinessNotifier")
            .field("registered", &self.count.load(Ordering::SeqCst))
            .finish()
    }
}

impl ReadinessNotifier {
    /// A notifier with no wakers.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Keep `waker` and wake it on every [`Self::notify`]. Always returns
    /// `true`: it is what `EventReadiness::register` returns for a readiness
    /// that calls `notify` on every change.
    pub fn register(&self, waker: &ReadinessWaker) -> bool {
        let mut wakers = self.wakers.lock().unwrap_or_else(PoisonError::into_inner);
        wakers.retain(ReadinessWaker::is_live);
        if !wakers.iter().any(|kept| kept.will_wake(waker)) {
            wakers.push(waker.clone());
        }
        self.count.store(wakers.len(), Ordering::SeqCst);
        true
    }

    /// Wake every registered task. Call it after the state changed (an item
    /// pushed, a flag set, a queue closed).
    pub fn notify(&self) {
        if self.count.load(Ordering::SeqCst) == 0 {
            return;
        }
        let wakers: Vec<ReadinessWaker> = {
            let mut wakers = self.wakers.lock().unwrap_or_else(PoisonError::into_inner);
            wakers.retain(ReadinessWaker::is_live);
            self.count.store(wakers.len(), Ordering::SeqCst);
            wakers.clone()
        };
        // Outside the lock: a wake may run code that registers again.
        for waker in wakers {
            waker.wake();
        }
    }

    /// Whether any task is registered.
    #[must_use]
    pub fn has_waiters(&self) -> bool {
        self.count.load(Ordering::SeqCst) > 0
    }
}

struct EventQueueInner<T> {
    queue: Arc<ConcurrentQueue<T>>,
    /// Woken after a push and on close: tasks that wait for an item.
    items: Arc<ReadinessNotifier>,
    /// Woken after a pop and on close: tasks that wait for room.
    space: Arc<ReadinessNotifier>,
    /// Woken only on close: tasks that wait for the consumer to go away.
    closed: Arc<ReadinessNotifier>,
}

/// A queue that wakes the tasks parked on it by itself.
///
/// WHY: with a plain queue every producer had to remember to ask the executor
/// to run again after a push, a pop and a close (about 80 hand-placed calls); a
/// producer that forgot was a lost wake. Here the queue owns the wake, so there
/// is nothing to forget.
///
/// WHAT: the `ConcurrentQueue` operations a task needs, where `push` wakes the
/// tasks parked on [`Self::readiness`], `pop` wakes the tasks parked on
/// [`Self::vacancy`], and `close` wakes both. A close also wakes the tasks
/// parked on [`Self::closed_readiness`], which nothing else wakes. Clones share
/// the queue.
pub struct EventQueue<T>(Arc<EventQueueInner<T>>);

impl<T> Clone for EventQueue<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T> std::fmt::Debug for EventQueue<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventQueue")
            .field("len", &self.0.queue.len())
            .field("closed", &self.0.queue.is_closed())
            .finish()
    }
}

impl<T> EventQueue<T> {
    fn from_queue(queue: ConcurrentQueue<T>) -> Self {
        Self(Arc::new(EventQueueInner {
            queue: Arc::new(queue),
            items: Arc::new(ReadinessNotifier::new()),
            space: Arc::new(ReadinessNotifier::new()),
            closed: Arc::new(ReadinessNotifier::new()),
        }))
    }

    /// A queue with no capacity limit.
    #[must_use]
    pub fn unbounded() -> Self {
        Self::from_queue(ConcurrentQueue::unbounded())
    }

    /// A queue that holds at most `capacity` items.
    #[must_use]
    pub fn bounded(capacity: usize) -> Self {
        Self::from_queue(ConcurrentQueue::bounded(capacity))
    }

    /// Push an item and wake the tasks that wait for one.
    ///
    /// # Errors
    /// The queue is full or closed; the item is handed back.
    pub fn push(&self, item: T) -> Result<(), PushError<T>> {
        self.0.queue.push(item)?;
        self.0.items.notify();
        Ok(())
    }

    /// Push an item, evicting the oldest one if the queue is full, and wake the
    /// tasks that wait for an item. Returns the evicted item, if any.
    ///
    /// # Errors
    /// The queue is closed; the item is handed back.
    pub fn force_push(&self, item: T) -> Result<Option<T>, ForcePushError<T>> {
        let evicted = self.0.queue.force_push(item)?;
        self.0.items.notify();
        Ok(evicted)
    }

    /// Pop an item and wake the tasks that wait for room.
    ///
    /// # Errors
    /// The queue is empty or closed.
    pub fn pop(&self) -> Result<T, PopError> {
        let item = self.0.queue.pop()?;
        self.0.space.notify();
        Ok(item)
    }

    /// Close the queue and wake every task parked on it, so each sees the end.
    /// Returns `true` if this call closed it.
    ///
    /// The close-only waiters ([`Self::closed_readiness`]) wake once, on the
    /// first close. A later close of an already closed queue wakes nobody.
    pub fn close(&self) -> bool {
        let closed = self.0.queue.close();
        self.0.items.notify();
        self.0.space.notify();
        if closed {
            self.0.closed.notify();
        }
        closed
    }

    /// Whether the queue holds no item.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.queue.is_empty()
    }

    /// Whether the queue is at capacity.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.0.queue.is_full()
    }

    /// Whether the queue is closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.0.queue.is_closed()
    }

    /// The number of items in the queue.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.queue.len()
    }

    /// The capacity of the queue, or `None` when it is unbounded.
    #[must_use]
    pub fn capacity(&self) -> Option<usize> {
        self.0.queue.capacity()
    }

    /// A readiness that is ready when the queue has an item or is closed. A
    /// task parks on it with `TaskStatus::Depends`.
    #[must_use]
    pub fn readiness(&self) -> QueueReadiness<T> {
        QueueReadiness::with_notifier(Arc::clone(&self.0.queue), Arc::clone(&self.0.items))
    }

    /// A readiness that is ready when the queue has room or is closed. A
    /// producer that finds the queue full parks on it.
    #[must_use]
    pub fn vacancy(&self) -> QueueVacancyReadiness<T> {
        QueueVacancyReadiness::with_notifier(Arc::clone(&self.0.queue), Arc::clone(&self.0.space))
    }

    /// A readiness that is ready only once the queue closes. A push or a pop
    /// does not wake a task parked on it, so a producer that must learn "the
    /// consumer is gone" does not wake on every item the consumer takes.
    #[must_use]
    pub fn closed_readiness(&self) -> QueueClosedReadiness<T> {
        QueueClosedReadiness::with_notifier(Arc::clone(&self.0.queue), Arc::clone(&self.0.closed))
    }
}
