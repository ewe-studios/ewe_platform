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
use std::sync::{Arc, Mutex};
use std::task::{RawWaker, RawWakerVTable, Wake, Waker};

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
            live: None,
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

/// A value that any thread may own, but only its owner thread may use.
///
/// WHY (GitHub issue #423): a [`Waker`] is `Send` and `Sync` by type, so the
/// data behind its vtable has to be usable from any thread. In the non-`multi`
/// build a [`WakeCallback`](crate::valtron::WakeCallback) has no `Send` bound
/// and may capture an `Rc`. This type makes that callback safe behind a
/// thread-safe waker: the callback stays reachable on the thread that
/// registered it, and no other thread can run it or drop it.
///
/// HOW: [`Self::get`] returns the value only on the owner thread. When the last
/// owner drops the value on another thread, the value leaks. A leak is safe;
/// running or dropping it there would break its thread affinity.
///
/// # Safety
/// `Send` and `Sync` hold for every `T` because the value never moves between
/// threads. A receiver of the owned value can only leak it or hand it back; a
/// sharer can only read the owner thread id. Both are safe for any `T`.
#[cfg(not(feature = "multi"))]
mod confined {
    use std::mem::ManuallyDrop;
    use std::thread::{self, ThreadId};

    pub(super) struct ThreadBound<T> {
        owner: ThreadId,
        value: ManuallyDrop<T>,
    }

    impl<T> ThreadBound<T> {
        pub(super) fn new(value: T) -> Self {
            Self {
                owner: thread::current().id(),
                value: ManuallyDrop::new(value),
            }
        }

        /// The value, on the thread that made it.
        pub(super) fn get(&self) -> Option<&T> {
            self.is_local().then(|| &*self.value)
        }

        /// Whether the calling thread made the value.
        pub(super) fn is_local(&self) -> bool {
            thread::current().id() == self.owner
        }
    }

    impl<T> Drop for ThreadBound<T> {
        fn drop(&mut self) {
            if self.is_local() {
                // SAFETY: the owner thread drops the value once. No reference
                // to it survives this drop.
                unsafe { ManuallyDrop::drop(&mut self.value) };
            }
        }
    }

    // SAFETY: see the type comment. No thread other than the owner reads or
    // drops `value`; `Drop` skips it off the owner thread.
    unsafe impl<T> Send for ThreadBound<T> {}
    // SAFETY: `get` yields a reference only on the owner thread, so sharing
    // `&ThreadBound<T>` never gives another thread access to `value`.
    unsafe impl<T> Sync for ThreadBound<T> {}
}

/// The callback slot of a bridged waker.
///
/// Without `multi` the callback may capture non-`Send` data (issue #423), so it
/// lives in a [`confined::ThreadBound`]. A wake from another thread drops the
/// wake instead of running the callback there, and the callback is dropped only
/// on its own thread.
#[cfg(not(feature = "multi"))]
struct CallbackSlot(confined::ThreadBound<Mutex<Option<crate::valtron::WakeCallback>>>);

#[cfg(not(feature = "multi"))]
impl CallbackSlot {
    fn new(callback: crate::valtron::WakeCallback) -> Self {
        Self(confined::ThreadBound::new(Mutex::new(Some(callback))))
    }

    /// Take the callback, on its own thread. `None` on any other thread.
    fn take(&self) -> Option<crate::valtron::WakeCallback> {
        self.0
            .get()?
            .lock()
            .unwrap_or_else(crate::compati::recover_poison)
            .take()
    }
}

/// The callback slot of a bridged waker in the `multi` build.
///
/// The callback is `Send` here, so any thread may take it.
#[cfg(feature = "multi")]
struct CallbackSlot(Mutex<Option<crate::valtron::WakeCallback>>);

#[cfg(feature = "multi")]
impl CallbackSlot {
    fn new(callback: crate::valtron::WakeCallback) -> Self {
        Self(Mutex::new(Some(callback)))
    }

    /// Every thread may take the callback in this build.
    fn take(&self) -> Option<crate::valtron::WakeCallback> {
        self.0
            .lock()
            .unwrap_or_else(crate::compati::recover_poison)
            .take()
    }
}

/// The state behind a one-shot waker built from a [`WakeCallback`].
///
/// WHY: the async stream bridge registers a closure, not a `Waker`, on a
/// queue-backed stream. This adapter turns that closure into a waker that a
/// [`ReadinessNotifier`] can hold, so the bridge parks on the queue instead of
/// waking itself and polling again. The old behaviour burned an executor
/// worker until the producer pushed a value (issue #329).
///
/// WHAT: fires the callback at most once. It sets its liveness token false when
/// it fires or when its last clone is dropped, so the notifier prunes a
/// registration that nobody waits on any more.
///
/// A wake that arrives on another thread does not run the callback there
/// (GitHub issue #423). It clears the liveness token, so the registration is
/// pruned on the next notify. The callback may capture non-`Send` data, and a
/// `Waker` may cross threads.
///
/// [`WakeCallback`]: crate::valtron::WakeCallback
struct CallbackWakeState {
    callback: CallbackSlot,
    live: Arc<AtomicBool>,
}

impl CallbackWakeState {
    fn new(callback: crate::valtron::WakeCallback, live: Arc<AtomicBool>) -> Self {
        Self {
            callback: CallbackSlot::new(callback),
            live,
        }
    }

    fn fire(&self) {
        // `None` off the callback's own thread: such a wake must not run a
        // callback that may hold values which are not `Send`, and must not move
        // it. Drop the wake; the registration ends now. The waiting side must
        // not register a non-`Send` callback on a waker that it then moves to
        // another thread.
        let callback = self.callback.take();
        self.live.store(false, Ordering::SeqCst);
        if let Some(callback) = callback {
            callback();
        }
    }
}

impl Drop for CallbackWakeState {
    fn drop(&mut self) {
        self.live.store(false, Ordering::SeqCst);
        // The slot drops its callback here, on its own thread only (see
        // `CallbackSlot`).
    }
}

static CALLBACK_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    callback_waker_clone,
    callback_waker_wake,
    callback_waker_wake_by_ref,
    callback_waker_drop,
);

/// Clone: bump the `Arc<CallbackWakeState>` strong count, reuse the pointer.
///
/// # Safety
/// `data` originates from `Arc::into_raw` of an `Arc<CallbackWakeState>` and is
/// still live (the owning `Waker` holds a strong reference across this call).
unsafe fn callback_waker_clone(data: *const ()) -> RawWaker {
    Arc::increment_strong_count(data.cast::<CallbackWakeState>());
    RawWaker::new(data, &CALLBACK_WAKER_VTABLE)
}

/// Wake (by value): fire the callback once, then drop the owned strong ref.
///
/// # Safety
/// `data` originates from `Arc::into_raw` of an `Arc<CallbackWakeState>`;
/// `wake` consumes the `Waker`, so reclaiming ownership here balances the
/// original `into_raw`.
unsafe fn callback_waker_wake(data: *const ()) {
    let state = Arc::from_raw(data.cast::<CallbackWakeState>());
    state.fire();
}

/// Wake by reference: fire the callback without consuming the strong ref.
///
/// # Safety
/// `data` originates from `Arc::into_raw` of an `Arc<CallbackWakeState>` and is
/// still live; `ManuallyDrop` prevents this borrowed reconstruction from
/// decrementing the count owned by the caller's `Waker`.
unsafe fn callback_waker_wake_by_ref(data: *const ()) {
    let state = core::mem::ManuallyDrop::new(Arc::from_raw(data.cast::<CallbackWakeState>()));
    state.fire();
}

/// Drop: reclaim and release one `Arc<CallbackWakeState>` strong reference.
///
/// # Safety
/// `data` originates from `Arc::into_raw` of an `Arc<CallbackWakeState>` and
/// this call balances exactly one outstanding strong reference.
unsafe fn callback_waker_drop(data: *const ()) {
    drop(Arc::from_raw(data.cast::<CallbackWakeState>()));
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
    /// The liveness token of the wait behind a bridged waker. The bridge owner
    /// stores `false` when its `Future` goes away, so the notifier can drop the
    /// waker.
    live: Option<Arc<AtomicBool>>,
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

    /// Whether this waker and `other` belong to the same wait.
    ///
    /// An executor waker has no liveness token, so `will_wake` alone identifies
    /// it. A bridged waker carries one token per wait. Two waits polled by one
    /// task wake the same task, but each must keep its own registration: if the
    /// notifier kept one and that wait ended first, the other would never wake.
    #[must_use]
    fn same_wait(&self, other: &Self) -> bool {
        match (&self.live, &other.live) {
            (Some(mine), Some(theirs)) => Arc::ptr_eq(mine, theirs),
            (None, None) => true,
            _ => false,
        }
    }

    /// Whether the task is still parked on this waker. A source may drop a
    /// waker that is not live.
    ///
    /// A waker the executor made checks its [`ParkState`]. A bridged waker
    /// checks the liveness token its owner gave it, so a `Future` that is gone
    /// stops holding a registration.
    #[must_use]
    pub fn is_live(&self) -> bool {
        if let Some(live) = &self.live {
            live.load(Ordering::SeqCst)
        } else {
            self.park.as_ref().is_none_or(|park| park.is_parked())
        }
    }

    /// A waker built on a standard waker, for a source that bridges to a
    /// `Future` (waking it wakes that waker).
    ///
    /// This waker reports live for as long as it exists. A bridge whose wait can
    /// end before the source does uses [`Self::from_waker_with_liveness`].
    #[must_use]
    pub fn from_waker(waker: Waker) -> Self {
        Self {
            waker,
            park: None,
            live: None,
        }
    }

    /// A bridged waker whose liveness the caller controls.
    ///
    /// The caller stores `false` in `live` when the wait behind this waker is
    /// gone (a `Future` that is dropped). The notifier's next `register` or
    /// `notify` then drops the waker instead of keeping it until the source
    /// fires.
    #[must_use]
    pub fn from_waker_with_liveness(waker: Waker, live: Arc<AtomicBool>) -> Self {
        Self {
            waker,
            park: None,
            live: Some(live),
        }
    }

    /// A waker that runs `callback` once, for a queue-backed stream whose async
    /// bridge registers a closure instead of a waker.
    ///
    /// The waker reports live until it fires or every clone is dropped, so the
    /// notifier drops a registration that nobody waits on any more.
    ///
    /// The callback has no `Send` bound in the non-`multi` build. A wake from
    /// another thread drops the wake instead of running the callback there, and
    /// the callback is dropped only on its own thread (issue #423).
    #[must_use]
    pub(crate) fn from_callback(callback: crate::valtron::WakeCallback) -> Self {
        let live = Arc::new(AtomicBool::new(true));
        let state = Arc::new(CallbackWakeState::new(callback, Arc::clone(&live)));
        let raw = RawWaker::new(Arc::into_raw(state).cast::<()>(), &CALLBACK_WAKER_VTABLE);
        // SAFETY: `raw` is built from a fresh `Arc<CallbackWakeState>` and the
        // vtable functions uphold the `Arc` clone/drop contract described on
        // each one.
        let waker = unsafe { Waker::from_raw(raw) };
        Self {
            waker,
            park: None,
            live: Some(live),
        }
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
            live: None,
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
            .finish_non_exhaustive()
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
        let mut wakers = self
            .wakers
            .lock()
            .unwrap_or_else(crate::compati::recover_poison);
        wakers.retain(ReadinessWaker::is_live);
        if !wakers
            .iter()
            .any(|kept| kept.will_wake(waker) && kept.same_wait(waker))
        {
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
            let mut wakers = self
                .wakers
                .lock()
                .unwrap_or_else(crate::compati::recover_poison);
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

    /// The number of registered wakers.
    ///
    /// This is a test hook: a test proves that a dead bridged waker is pruned.
    /// It is not part of the stable API.
    #[doc(hidden)]
    #[must_use]
    pub fn waiter_count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
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
    #[must_use]
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
