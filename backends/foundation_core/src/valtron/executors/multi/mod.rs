#![cfg(feature = "multi")]
#![allow(clippy::new_ret_no_self)]
#![allow(clippy::type_complexity)]

use std::{
    any::Any,
    collections::HashMap,
    env,
    marker::PhantomData,
    panic,
    sync::{
        self,
        atomic::{self, AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{self},
};

use crate::valtron::{Stream, DEFAULT_YIELD_WAIT_TIME};
use concurrent_queue::{ConcurrentQueue, PushError};
use derive_more::derive::From;
use foundation_compact::rng::ChaCha8Rng;
use foundation_compact::rng::Rng;
use foundation_compact::rng::SeedableRng;
use std::str::FromStr;

use crate::{
    extensions::result_ext::SendableBoxedError,
    retries::ExponentialBackoffDecider,
    synca::{
        mpp::{self},
        EntryList, IdleMan, LockSignal, OnSignal, SleepyMan, WaitGroup,
    },
    valtron::{
        AnyResult, LocalThreadExecutor, NotifyQueue, NotifyQueueStreamIterator, NotifyRecvIterator,
    },
};

use crate::valtron::{
    BoxedExecutionEngine, BoxedPanicHandler, BoxedSendExecutionIterator, ExecutionAction,
    ExecutorError, FnMutReady, FnReady, OnNext, PriorityOrder, ProcessController,
    ReadyConsumingIter, SharedTaskQueue, TaskIterator, TaskReadyResolver, TaskStatus,
    TaskStatusMapper,
};

use crate::valtron::{ThreadActivity, ThreadId};

use crate::valtron::{
    executors::constants::{
        BACK_OFF_JITER, BACK_OFF_MAX_DURATION, BACK_OFF_MIN_DURATION, BACK_OFF_THREAD_FACTOR,
        DEFAULT_OP_READ_TIME, MAX_ROUNDS_IDLE_COUNT, MAX_ROUNDS_WHEN_SLEEPING_ENDS,
    },
    ConsumingIter, DoNext, StreamConsumingIter,
};

use crate::compati::{Condvar, CondVarMutex, Mutex, RwLock};
use foundation_nostd::comp::condvar_comp::{CondVar, CondVarMutex as CvMutex};

use crate::valtron::{split_thread_count, BackgroundJobRegistry, GenericResult};

use crate::valtron::executors::background::DEFAULT_BG_YIELD_DURATION;

/// Global registry - stores the `ThreadRegistry` Arc.
/// Uses Mutex<Option> to allow resetting between tests.
static REGISTRY: Mutex<Option<Arc<ThreadRegistry>>> = Mutex::new(None);

/// Global background job registry.
/// Uses Mutex<Option> to allow resetting between tests.
static BG_REGISTRY: Mutex<Option<Arc<BackgroundJobRegistry>>> = Mutex::new(None);

/// Process-wide FIFO-fair serialization gate for pool lifecycle. Because the
/// global `REGISTRY`/`BG_REGISTRY` are singletons, only ONE pool may be alive
/// at a time. `initialize_pool` acquires this gate and the returned `PoolGuard`
/// holds it until drop — so concurrent pool users (across test files, with or
/// without `#[serial]`) run one-at-a-time. This makes a generation counter
/// unnecessary: a stale guard can never coexist with a newer pool.
///
/// We use a ticket gate rather than a plain `Mutex<()>` because `std::sync::Mutex`
/// is UNFAIR: under heavy parallel contention (cargo runs the test binary on many
/// threads, all sharing this one global pool) a waiter can be starved indefinitely
/// — surfacing as "test running for over 60 seconds" even though each test is fast.
/// The ticket gate hands out FIFO tickets and parks waiters on a condvar, so every
/// caller is served in arrival order with no busy-waiting.
static POOL_LIFECYCLE_GATE: FairGate = FairGate::new();

/// A FIFO-fair, blocking serialization gate built from a ticket counter and a
/// condvar. `acquire()` returns a guard that releases the gate on drop.
pub struct FairGate {
    /// Next ticket to hand out.
    next_ticket: AtomicUsize,
    /// Ticket currently being served (allowed to proceed).
    now_serving: AtomicUsize,
    /// Mutex + condvar used to park/wake waiters without busy-spinning.
    /// `CondVarMutex` (not the plain `compati::Mutex`) because a condvar needs a
    /// guard-compatible mutex on every target — `compati::Mutex` is a Noop/Spin
    /// lock on wasm whose guard `Condvar::wait` can't accept.
    inner: CondVarMutex<()>,
    cond: Condvar,
}

impl FairGate {
    const fn new() -> Self {
        Self {
            next_ticket: AtomicUsize::new(0),
            now_serving: AtomicUsize::new(0),
            inner: CondVarMutex::new(()),
            cond: Condvar::new(),
        }
    }

    /// Acquire the gate, blocking in FIFO order until this caller's ticket is up.
    fn acquire(&'static self) -> FairGateGuard {
        let my_ticket = self.next_ticket.fetch_add(1, Ordering::SeqCst);
        let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        while self.now_serving.load(Ordering::SeqCst) != my_ticket {
            guard = self.cond.wait(guard).unwrap_or_else(|p| p.into_inner());
        }
        FairGateGuard { gate: self }
    }
}

/// Releases the `FairGate` on drop, advancing to the next ticket and waking
/// all parked waiters (each re-checks its ticket; exactly one proceeds).
pub struct FairGateGuard {
    gate: &'static FairGate,
}

impl Drop for FairGateGuard {
    fn drop(&mut self) {
        let _guard = self.gate.inner.lock().unwrap_or_else(|p| p.into_inner());
        self.gate.now_serving.fetch_add(1, Ordering::SeqCst);
        self.gate.cond.notify_all();
    }
}

pub(crate) fn clear_global_registries() {
    let mut reg = REGISTRY.lock().unwrap_or_else(|p| p.into_inner());
    *reg = None;
    drop(reg);
    let mut bg = BG_REGISTRY.lock().unwrap_or_else(|p| p.into_inner());
    *bg = None;
}

/// Handle for spawning tasks into the shared queue.
///
/// This is the new return type of `get_pool()`. It provides the same
/// spawn API surface as the old `&ThreadPool` by constructing
/// `ThreadPoolTaskBuilder` directly with the shared queue and latch.
#[derive(Clone)]
pub struct LocalPoolHandle {
    shared_tasks: SharedTaskQueue,
    latch: Arc<LockSignal>,
    kill_signal: Arc<OnSignal>,
    yielders: SharedThreadYielders,
}

impl LocalPoolHandle {
    fn new(registry: &Arc<ThreadRegistry>) -> Self {
        Self {
            shared_tasks: registry.shared_tasks(),
            latch: registry.latch(),
            kill_signal: registry.kill_signal(),
            yielders: registry.yielders(),
        }
    }

    /// Create a task builder for scheduling work.
    #[must_use]
    pub fn spawn<Task, Action>(
        &self,
    ) -> ThreadPoolTaskBuilder<
        Task::Ready,
        Task::Pending,
        Task::Spawner,
        Box<dyn TaskStatusMapper<Task::Ready, Task::Pending, Task::Spawner> + Send + 'static>,
        Box<dyn TaskReadyResolver<Task::Spawner, Task::Ready, Task::Pending> + Send + 'static>,
        Task,
    >
    where
        Task::Ready: Send + 'static,
        Task::Pending: Send + 'static,
        Task: TaskIterator<Spawner = Action> + Send + 'static,
        Action: ExecutionAction + Send + 'static,
    {
        ThreadPoolTaskBuilder::new(self.shared_tasks.clone(), self.latch.clone())
            .with_yielders(self.yielders.clone())
    }

    /// Create a task builder with explicit Mapper and Resolver types.
    #[must_use]
    pub fn spawn2<Task, Action, Mapper, Resolver>(
        &self,
    ) -> ThreadPoolTaskBuilder<Task::Ready, Task::Pending, Action, Mapper, Resolver, Task>
    where
        Task::Ready: Send + 'static,
        Task::Pending: Send + 'static,
        Task: TaskIterator<Spawner = Action> + Send + 'static,
        Action: ExecutionAction + Send + 'static,
        Mapper: TaskStatusMapper<Task::Ready, Task::Pending, Action> + Send + 'static,
        Resolver: TaskReadyResolver<Action, Task::Ready, Task::Pending> + Send + 'static,
    {
        ThreadPoolTaskBuilder::new(self.shared_tasks.clone(), self.latch.clone())
            .with_yielders(self.yielders.clone())
    }

    /// Signal all threads to die.
    pub fn kill(&self) {
        self.kill_signal.turn_on();
        self.latch.signal_all();
    }
}

/// [`get_pool`] returns a handle for spawning tasks.
///
/// # Panics
///
/// Panics if the thread pool has not been initialized (ensure to call `block_on` or
/// `initialize_pool` first), or if the internal registry mutex is poisoned.
pub fn get_pool() -> LocalPoolHandle {
    let registry = REGISTRY
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .expect("Thread pool not initialized, ensure to call block_on or initialize_pool first");
    LocalPoolHandle::new(&registry)
}

/// [`block_on`] instantiates the thread registry, spawns worker threads,
/// runs the setup function, then blocks until all tasks complete.
///
/// Returns a `PoolGuard` - when dropped, kills all threads and blocks
/// until they're cleaned up. No Ctrl-C handler is registered automatically.
pub fn block_on<F>(seed_from_rng: u64, thread_num: Option<usize>, setup: F) -> PoolGuard
where
    F: FnOnce(LocalPoolHandle),
{
    let guard = initialize_pool(seed_from_rng, thread_num);
    let handle = get_pool();
    setup(handle);
    tracing::debug!("Initialize and call WaitGroup::wait");
    guard.waitgroup().wait();
    guard
}

/// `initialize_pool` creates a `ThreadRegistry` and `BackgroundJobRegistry`,
/// splitting the available threads between them.
///
/// Returns a `PoolGuard` — when dropped, signals all threads to die,
/// waits for them to complete via `WaitGroup`, joins all handles,
/// and clears the global registries to prevent test interference.
///
/// The caller is responsible for signal handling via the `PoolGuard`.
pub fn initialize_pool(seed_for_rng: u64, user_thread_num: Option<usize>) -> PoolGuard {
    // Acquire the process-wide lifecycle gate FIRST — blocks (in FIFO order)
    // until any previous pool's PoolGuard has dropped. This serializes all pool
    // users (across test files, regardless of #[serial]) since the global
    // registries are singletons, and the FIFO fairness prevents starvation when
    // cargo runs many test threads contending for the one global pool.
    let lifecycle = POOL_LIFECYCLE_GATE.acquire();

    // Clear any stale state now that we hold the lock exclusively. With the
    // lifecycle lock held, the previous pool is guaranteed fully torn down.
    clear_global_registries();

    let thread_num = match user_thread_num {
        None => get_allocatable_thread_count(),
        // The multi pool splits threads into background + task workers and needs
        // ≥ 2 TASK workers (`ThreadRegistry`), which means ≥ 3 total once the
        // background worker is carved off. Clamp a smaller request up rather than
        // panic — callers (and the `#[valtron]` macros) pass a plain count.
        Some(num) => num.max(3),
    };

    let (task_threads, bg_threads) = split_thread_count(thread_num);

    tracing::debug!(
        "Splitting {thread_num} threads: {task_threads} for tasks, {bg_threads} for background jobs"
    );

    let registry = Arc::new(ThreadRegistry::with_seed_and_threads(
        seed_for_rng,
        task_threads,
    ));

    // Spawn task worker threads
    for _ in 0..task_threads {
        registry
            .spawn_worker()
            .expect("Failed to spawn worker thread");
    }

    // Create background job registry sharing the kill signal
    let bg_registry = BackgroundJobRegistry::new(
        bg_threads,
        registry.kill_signal(),
        DEFAULT_BG_YIELD_DURATION,
    );

    let mut reg_guard = REGISTRY.lock().unwrap_or_else(|p| p.into_inner());
    *reg_guard = Some(registry.clone());
    let mut bg_guard = BG_REGISTRY.lock().unwrap_or_else(|p| p.into_inner());
    *bg_guard = Some(bg_registry.clone());
    drop(reg_guard);
    drop(bg_guard);

    PoolGuard::with_bg_registry(registry, bg_registry, lifecycle)
}

/// Submit a blocking closure for execution on the background job pool.
///
/// WHY: Provides managed background thread execution without spawning new OS threads
/// WHAT: Pushes the closure onto the `BackgroundJobRegistry` queue
/// HOW: Acquires the global `BG_REGISTRY` and calls `submit()`
///
/// # Errors
///
/// Returns an error if the background registry is not initialized or the queue is closed.
///
/// # Panics
///
/// Panics if the background job pool has not been initialized.
pub fn run_background_job(job: impl FnOnce() + Send + 'static) -> GenericResult<()> {
    let bg_registry = BG_REGISTRY
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .expect("Background job pool not initialized, ensure to call initialize_pool first");
    bg_registry.submit(job)
}

/// [`spawn`] provides a builder which allows you to build out
/// the underlying tasks to be scheduled into the global queue.
///
/// Uses the global registry's shared queue and latch.
#[must_use]
pub fn spawn<Task, Action>() -> ThreadPoolTaskBuilder<
    Task::Ready,
    Task::Pending,
    Task::Spawner,
    Box<dyn TaskStatusMapper<Task::Ready, Task::Pending, Task::Spawner> + Send + 'static>,
    Box<dyn TaskReadyResolver<Task::Spawner, Task::Ready, Task::Pending> + Send + 'static>,
    Task,
>
where
    Task::Ready: Send + 'static,
    Task::Pending: Send + 'static,
    Task: TaskIterator<Spawner = Action> + Send + 'static,
    Action: ExecutionAction + Send + 'static,
{
    get_pool().spawn::<Task, Action>()
}

/// [`spawn2`] provides a builder which allows you to build out
/// the underlying tasks with explicit Mapper and Resolver types.
#[must_use]
pub fn spawn2<Task, Action, Mapper, Resolver>(
) -> ThreadPoolTaskBuilder<Task::Ready, Task::Pending, Action, Mapper, Resolver, Task>
where
    Task::Ready: Send + 'static,
    Task::Pending: Send + 'static,
    Task: TaskIterator<Spawner = Action> + Send + 'static,
    Action: ExecutionAction + Send + 'static,
    Mapper: TaskStatusMapper<Task::Ready, Task::Pending, Action> + Send + 'static,
    Resolver: TaskReadyResolver<Action, Task::Ready, Task::Pending> + Send + 'static,
{
    get_pool().spawn2::<Task, Action, Mapper, Resolver>()
}

// ============================================================================
// PoolGuard - Drop-based Lifecycle Handle
// ============================================================================

// NOTE: PoolCleanupFn and the `cleanup_fn` field were removed. They were never
// wired up by any caller across the entire workspace, yet the presence of
// `Box<dyn FnOnce() + Send>` made PoolGuard non-Sync — breaking uses like
// `static POOL_GUARD: OnceLock<PoolGuard>`.
//
// Whoever adds cleanup_fn back needs to either:
//   - Use a Sync-compatible type (e.g. Arc<dyn Fn() + Send + Sync>)
//   - Or add `unsafe impl Sync for PoolGuard {}` with a safety justification
//     that cleanup_fn is only consumed during Drop and never accessed concurrently.

/// Returned by `initialize_pool`. Provides deterministic cleanup.
///
/// On drop (or explicit `shutdown()` call):
/// 1. `OnSignal::turn_on()` — broadcast kill to all threads
/// 2. `LockSignal::signal_all()` — wake any blocked/parked threads
/// 3. `BackgroundJobRegistry::shutdown()` — wait for background workers
/// 4. `WaitGroup::wait()` — block until all task threads report death
/// 5. Join all `JoinHandle`s
/// 6. Clear global registries ONLY if generation still matches
///
/// This replaces the old pattern of spawning a thread to call `get_pool().kill()`.
pub struct PoolGuard {
    registry: Arc<ThreadRegistry>,
    bg_registry: Option<Arc<crate::valtron::BackgroundJobRegistry>>,
    shut_down: AtomicBool,
    /// Process-wide lifecycle gate guard — held for the lifetime of this guard
    /// so no other pool can be initialized concurrently. Released on drop, after
    /// `shutdown()` has torn down the pool and cleared the global registries.
    _lifecycle: Option<FairGateGuard>,
}

impl PoolGuard {
    /// Create a new `PoolGuard` with the given `ThreadRegistry`.
    #[must_use]
    pub fn new(registry: Arc<ThreadRegistry>) -> Self {
        Self {
            registry,
            bg_registry: None,
            shut_down: AtomicBool::new(false),
            _lifecycle: None,
        }
    }

    /// Create a new `PoolGuard` with both registries and the process-wide
    /// lifecycle lock guard.
    #[must_use]
    pub fn with_bg_registry(
        registry: Arc<ThreadRegistry>,
        bg_registry: Arc<crate::valtron::BackgroundJobRegistry>,
        lifecycle: FairGateGuard,
    ) -> Self {
        Self {
            registry,
            bg_registry: Some(bg_registry),
            shut_down: AtomicBool::new(false),
            _lifecycle: Some(lifecycle),
        }
    }

    /// Create a dummy `PoolGuard` that does nothing on drop.
    /// Used for single-threaded/wasm builds where no thread pool is created.
    #[must_use]
    pub fn dummy() -> Self {
        Self {
            registry: Arc::new(ThreadRegistry::with_seed_and_threads(0, 2)),
            bg_registry: None,
            shut_down: AtomicBool::new(true),
            _lifecycle: None,
        }
    }

    /// Explicit shutdown. Idempotent — safe to call multiple times.
    /// Clears the global registries; the lifecycle lock (held until this guard
    /// drops) guarantees no other pool can be running concurrently.
    pub fn shutdown(&self) {
        tracing::warn!("PoolGuard::shutdown() called - pool shutting down");
        if self
            .shut_down
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
        {
            tracing::warn!("PoolGuard::shutdown() - signaling kill to all threads");
            // Shut down background workers first (they share the kill signal)
            if let Some(ref bg) = self.bg_registry {
                tracing::warn!("PoolGuard::shutdown() - shutting down background workers");
                bg.shutdown();
            }

            self.registry.shutdown();
            tracing::warn!("Clearing global pool registery");
            clear_global_registries();
            tracing::warn!("Cleared global pool registery");
        } else {
            tracing::warn!("PoolGuard::shutdown() - already shut down, skipping");
        }
    }

    /// Get the `WaitGroup` for this pool.
    #[must_use]
    pub fn waitgroup(&self) -> &WaitGroup {
        self.registry.waitgroup()
    }
}

impl Drop for PoolGuard {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Number of bits used for the thread counters.
#[cfg(target_pointer_width = "64")]
const THREADS_BITS: usize = 16;

#[cfg(target_pointer_width = "32")]
const THREADS_BITS: usize = 8;

/// Max value for the thread counters.
const THREADS_MAX: usize = (1 << THREADS_BITS) - 1;

/// [`get_allocatable_thread_count`] ensures we allocate enough of threads as requested
/// less 1 as the 1 thread will be used for waiting for kill signal and the remaining
/// for task execution in situations where on 2 threads can be created apart from the current
/// process.
///
/// # Panics
///
/// Panics if the desired thread count is greater than the maximum allowed threads.
/// Panics if the desired thread count is zero.
pub fn get_allocatable_thread_count() -> usize {
    let max_threads = get_max_threads();
    tracing::debug!("Max available threads: {max_threads:}");
    let desired_threads = get_num_threads();
    tracing::debug!("Desired thread count: {desired_threads:}");

    assert!(
        (desired_threads <= max_threads),
        "Desired thread count cant be greater than maximum allowed threads {max_threads}"
    );

    assert!((desired_threads != 0), "Desired thread count cant be zero");

    assert!((max_threads >= 2), "Available thread cant be less than 2 we use 1 thread for the service kill signal, and 1 for tasks");

    assert!((desired_threads >= 2), "Requested thread cant be less than 2 we use 1 thread for the service kill signal, and 1 for tasks");

    // return size of threads with us keeping 2 for our purposes.
    if desired_threads == max_threads {
        return max_threads - 1;
    }

    let rem_threads = max_threads - desired_threads;
    tracing::debug!(
        "Remaining threads {} from desired: {} and max: {}",
        rem_threads,
        desired_threads,
        max_threads
    );

    desired_threads
}

#[cfg(test)]
mod test_allocatable_threads {
    use serial_test::serial;
    use tracing_test::traced_test;

    use super::*;

    #[test]
    #[traced_test]
    #[serial]
    fn get_allocatable_thread_count_as_far_as_1_remains() {
        let max_threads = get_max_threads();
        assert!(max_threads > 3);

        env::remove_var("VALTRON_NUM_THREADS");

        let new_thread_count = max_threads - 2;
        tracing::debug!("Setting thread count to: {new_thread_count:}");

        let new_thread_count_str = format!("{new_thread_count:}");
        env::set_var("VALTRON_NUM_THREADS", new_thread_count_str.clone());
        assert_eq!(
            env::var("VALTRON_NUM_THREADS").unwrap(),
            new_thread_count_str
        );

        // Very flaky, for now asset non-zero
        // assert!(get_allocatable_thread_count() != 0);
        //
        assert_eq!(get_allocatable_thread_count(), max_threads - 2);
    }
}

/// [`get_max_threads`] returns the max threads allowed
/// in the current system.
pub fn get_max_threads() -> usize {
    let system_value = std::thread::available_parallelism()
        .ok()
        .map_or(1, std::num::NonZero::get);
    tracing::debug!("thread::available_parallelism() reported: {}", system_value);
    system_value
}

/// [`get_num_threads`] will attempt to fetch the desired thread we want
/// from the either the environment variable `VALTRON_NUM_THREADS`
/// or gets the maximum allowed thread from the platform
/// via [`std::thread::available_parallelism`];
pub fn get_num_threads() -> usize {
    let thread_num = match env::var("VALTRON_NUM_THREADS")
        .ok()
        .and_then(|s| usize::from_str(&s).ok())
    {
        Some(x) => {
            tracing::debug!("Retrieved thread_number: {x:} from VALTRON_NUM_THREADS");
            x
        }
        _ => get_max_threads(),
    };

    tracing::debug!("Reporting Thread available for use: {}", thread_num);

    thread_num
}

/// `ThreadYielders` manages the collection of thread yielders for interruption.
///
/// This allows any code with access to the Arc<ThreadYielders> to interrupt
/// all sleeping threads, useful when new work arrives that needs immediate attention.
pub struct ThreadYielders {
    yielders: RwLock<Vec<Arc<ThreadYielder>>>,
}

impl ThreadYielders {
    /// Creates a new ThreadYielders registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            yielders: RwLock::new(Vec::new()),
        }
    }

    /// Register a yielder for interrupt tracking.
    #[tracing::instrument(skip(self))]
    pub fn register(&self, yielder: Arc<ThreadYielder>) {
        tracing::trace!("ThreadYielders::register() - registering yielder");
        let mut yielders = self.yielders.write().unwrap();
        yielders.push(yielder);
        tracing::trace!(
            "ThreadYielders::register() - done, {} yielders registered",
            yielders.len()
        );
    }

    /// Interrupt all registered yielders.
    /// Call this when new work arrives to wake threads waiting in yield_for.
    #[tracing::instrument(skip(self))]
    pub fn interrupt_all(&self) {
        tracing::trace!("ThreadYielders::interrupt_all() - acquiring read lock");
        let yielders = self.yielders.read().unwrap();
        tracing::trace!(
            "ThreadYielders::interrupt_all() - interrupting {} yielders",
            yielders.len()
        );
        for (i, yielder) in yielders.iter().enumerate() {
            tracing::trace!(
                "ThreadYielders::interrupt_all() - interrupting yielder {}",
                i
            );
            yielder.interrupt();
        }
        tracing::trace!("ThreadYielders::interrupt_all() - done");
    }
}

impl Default for ThreadYielders {
    fn default() -> Self {
        Self::new()
    }
}

pub type SharedThreadYielders = sync::Arc<ThreadYielders>;

pub struct ThreadYielder {
    thread_id: ThreadId,
    latch: Arc<LockSignal>,
    sender: mpp::Sender<ThreadActivity>,
    /// CondVar for interruptible waiting - replaces park_timeout
    condvar: Arc<CondVar>,
    /// State for CondVar waiting
    wait_state: Arc<CvMutex<WaitState>>,
}

impl std::fmt::Debug for ThreadYielder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreadYielder")
            .field("thread_id", &self.thread_id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WaitState {
    Waiting,
    Notified,
}

impl ThreadYielder {
    pub fn new(
        thread_id: ThreadId,
        latch: Arc<LockSignal>,
        sender: mpp::Sender<ThreadActivity>,
    ) -> Self {
        Self {
            thread_id,
            latch,
            sender,
            condvar: Arc::new(CondVar::new()),
            wait_state: Arc::new(CvMutex::new(WaitState::Waiting)),
        }
    }

    /// Interrupt the current wait by notifying the CondVar.
    /// This wakes the thread waiting in `wait_timeout_while` immediately.
    ///
    /// WHY: Called during shutdown (via interrupt_all_yielders) or when new work arrives
    /// WHAT: Sets state to Notified and calls notify_all() on the CondVar
    /// HOW:
    ///   1. Acquire lock on wait_state
    ///   2. Set state to Notified (causes predicate in wait_timeout_while to return false)
    ///   3. Call notify_all() to wake any waiting threads
    ///
    /// This wakes threads waiting in yield_for() even if they're in the middle
    /// of a long sleep. The wait_timeout_while will check the predicate again
    /// after waking and return immediately since state != Waiting.
    #[tracing::instrument(skip(self))]
    pub fn interrupt(&self) {
        tracing::trace!("ThreadYielder::interrupt() - setting state to Notified");
        if let Ok(mut guard) = self.wait_state.lock() {
            *guard = WaitState::Notified;
            tracing::trace!("ThreadYielder::interrupt() - state set to Notified");
        }
        tracing::trace!("ThreadYielder::interrupt() - calling condvar.notify_all()");
        self.condvar.notify_all();
        tracing::trace!("ThreadYielder::interrupt() - done");
    }
}

impl Clone for ThreadYielder {
    fn clone(&self) -> Self {
        Self {
            thread_id: self.thread_id.clone(),
            latch: self.latch.clone(),
            sender: self.sender.clone(),
            condvar: self.condvar.clone(),
            wait_state: self.wait_state.clone(),
        }
    }
}

impl ProcessController for ThreadYielder {
    type YieldSignal = ();

    /// WHY: Uses CondVar::wait_timeout_while for interruptible waiting
    /// WHAT: Blocks thread with timeout, but can be woken early via interrupt()
    /// HOW:
    ///   1. Acquire lock on wait_state
    ///   2. Call wait_timeout_while which atomically:
    ///      - Checks predicate (returns immediately if false)
    ///      - If true, unlocks mutex and waits for timeout or notification
    ///      - On wakeup, re-locks mutex and re-checks predicate
    ///   3. Returns when either:
    ///      - Duration expires (timed_out=true)
    ///      - interrupt() sets state to Notified and calls notify_all()
    ///
    /// CRITICAL FIX: wait_timeout_while returns the guard still locked.
    /// We MUST use that returned guard directly to reset state.
    /// Attempting to acquire the lock again causes deadlock.
    fn yield_for(&self, dur: std::time::Duration) {
        tracing::trace!("ThreadYielder::yield_for() - START, duration={:?}", dur);
        // Send parked notification
        self.sender
            .send(ThreadActivity::Parked(self.thread_id.clone()))
            .expect("should send event");

        tracing::trace!("ThreadYielder::yield_for() - acquiring wait_state lock");
        let guard = self.wait_state.lock().unwrap();

        tracing::trace!("ThreadYielder::yield_for() - calling wait_timeout_while");
        // Wait while not notified (spurious wakeups are handled by re-checking)
        // The wait_timeout_while atomically:
        // 1. Checks the predicate (returns immediately if false)
        // 2. If predicate is true, unlocks mutex and waits
        // 3. On wakeup, re-locks mutex and re-checks predicate
        // This ensures we never miss a notification that arrives between check and wait
        let (mut guard, result) = self
            .condvar
            .wait_timeout_while(guard, dur, |state| *state == WaitState::Waiting)
            .unwrap();
        let _timed_out = result.timed_out();
        tracing::trace!("ThreadYielder::yield_for() - wait_timeout_while returned");

        // CRITICAL: Reset state using the guard returned by wait_timeout_while.
        // The guard is still locked - don't try to acquire again or deadlock!
        *guard = WaitState::Waiting;

        self.sender
            .send(ThreadActivity::Unparked(self.thread_id.clone()))
            .expect("should send event");
        tracing::trace!("ThreadYielder::yield_for() - END");
    }
}

#[derive(Clone)]
pub struct SharedThreadRegistry(sync::Arc<RwLock<ThreadPoolRegistryInner>>);

impl SharedThreadRegistry {
    pub fn new(inner: sync::Arc<RwLock<ThreadPoolRegistryInner>>) -> Self {
        Self(inner)
    }

    #[must_use]
    pub fn executor_count(&self) -> usize {
        let registry = self.0.read().unwrap_or_else(|p| p.into_inner());
        registry.threads.active_slots()
    }

    #[must_use]
    pub fn get_thread(&self, thread: ThreadId) -> ThreadRef {
        let registry = self.0.read().unwrap_or_else(|p| p.into_inner());

        match registry.threads.get(thread.get_ref()) {
            Some(thread_ref) => thread_ref.clone(),
            None => unreachable!("We should never get to a point where a ThreadId is invalid"),
        }
    }

    pub fn register_thread(
        &self,
        name: String,
        thread: ThreadRef,
        latch: Arc<LockSignal>,
        sender: mpp::Sender<ThreadActivity>,
    ) -> ThreadId {
        let mut registry = self.0.write().unwrap_or_else(|p| p.into_inner());

        // insert thread and get thread key.
        let entry = registry.threads.insert(thread);

        // update the key for the thread in place within the
        // entry and in the same lock.
        match registry.threads.get_mut(&entry) {
            Some(mutable_thread_ref) => {
                let thread_id = ThreadId::new(entry, name);
                mutable_thread_ref.registry_id = Some(thread_id.clone());
                mutable_thread_ref.process =
                    Some(ThreadYielder::new(thread_id.clone(), latch, sender));
                thread_id
            }
            None => unreachable!("Thread must be registered at the entry key"),
        }
    }

    /// Remove a thread entry from the registry when the thread stops.
    /// This prevents stale entries from accumulating after shutdown/restart cycles.
    pub fn unregister_thread(&self, thread_id: ThreadId) {
        let mut registry = self.0.write().unwrap_or_else(|p| p.into_inner());
        registry.threads.vacate(thread_id.get_ref());
    }

    /// Clear all thread entries from the registry. Called during shutdown
    /// to ensure a clean state for the next test.
    pub fn clear(&self) {
        let mut registry = self.0.write().unwrap_or_else(|p| p.into_inner());
        registry.threads.clear();
    }
}

pub type SharedActivityQueue = sync::Arc<ConcurrentQueue<ThreadActivity>>;
pub type SharedThreadYielder = sync::Arc<ThreadYielder>;

#[derive(Clone)]
pub struct ThreadRef {
    /// name of thread ref
    pub name: String,

    /// seed for the giving thread ref
    pub seed: u64,

    /// the queue for tasks
    pub tasks: Arc<ConcurrentQueue<BoxedSendExecutionIterator>>,

    /// the relevant key used by the thread in
    /// the core thread registry.
    pub registry_id: Option<ThreadId>,

    /// the register this thread is related to.
    pub register: SharedThreadRegistry,

    /// global signal used for communicating death
    /// to all threads.
    pub global_kill_signal: sync::Arc<OnSignal>,

    /// process manager for controlling process yielding.
    pub process: Option<ThreadYielder>,
}

impl ThreadRef {
    pub fn new(
        seed: u64,
        name: String,
        queue: Arc<ConcurrentQueue<BoxedSendExecutionIterator>>,
        registry_id: Option<ThreadId>,
        register: SharedThreadRegistry,
        global_kill_signal: sync::Arc<OnSignal>,
    ) -> ThreadRef {
        Self {
            seed,
            name,
            tasks: queue,
            register,
            registry_id,
            global_kill_signal,
            process: None,
        }
    }
}

pub struct ThreadPoolRegistryInner {
    threads: EntryList<ThreadRef>,
}

impl ThreadPoolRegistryInner {
    #[must_use]
    pub fn new() -> SharedThreadRegistry {
        SharedThreadRegistry::new(sync::Arc::new(RwLock::new(ThreadPoolRegistryInner {
            threads: EntryList::new(),
        })))
    }
}

// -- ThreadExecutionError

pub type ThreadExecutionResult<T> = AnyResult<T, ThreadExecutionError>;

#[derive(From, Debug)]
pub enum ThreadExecutionError {
    #[from(ignore)]
    FailedStart(SendableBoxedError),

    #[from(ignore)]
    Panicked(Box<dyn Any + Send>),
}

impl core::error::Error for ThreadExecutionError {}

impl core::fmt::Display for ThreadExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Panicked(_) => write!(f, "ThreadExecutionError(_)"),
            _ => write!(f, "{self:?}"),
        }
    }
}

pub struct ThreadPoolTaskBuilder<
    Done: Send + 'static,
    Pending: Send + 'static,
    Action: ExecutionAction + Send + 'static,
    Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
    Resolver: TaskReadyResolver<Action, Done, Pending> + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
> {
    tasks: SharedTaskQueue,
    latch: Arc<LockSignal>,
    /// Yielders registry to interrupt when new work arrives
    yielders: Option<SharedThreadYielders>,
    task: Option<Task>,
    resolver: Option<Resolver>,
    mappers: Option<Vec<Mapper>>,
    panic_handler: Option<BoxedPanicHandler>,
    /// Channel capacity for bounded queues (None = unbounded)
    channel_capacity: Option<usize>,
    _marker: PhantomData<(Done, Pending, Action)>,
}

impl<
        Done: Send + 'static,
        Pending: Send + 'static,
        Action: ExecutionAction + Send + 'static,
        Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
        Resolver: TaskReadyResolver<Action, Done, Pending> + Send + 'static,
        Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
    > ThreadPoolTaskBuilder<Done, Pending, Action, Mapper, Resolver, Task>
{
    pub fn new(tasks: SharedTaskQueue, latch: Arc<LockSignal>) -> Self {
        Self {
            tasks,
            latch,
            yielders: None,
            task: None,
            mappers: None,
            resolver: None,
            panic_handler: None,
            channel_capacity: None,
            _marker: PhantomData,
        }
    }

    /// Set the yielders registry to interrupt when new work arrives.
    #[must_use]
    pub fn with_yielders(mut self, yielders: SharedThreadYielders) -> Self {
        self.yielders = Some(yielders);
        self
    }

    #[allow(clippy::return_self_not_must_use)]
    pub fn with_mappers(mut self, mapper: Mapper) -> Self {
        let mut mappers = if self.mappers.is_some() {
            self.mappers.take().unwrap()
        } else {
            Vec::new()
        };

        mappers.push(mapper);
        self.mappers = Some(mappers);
        self
    }

    #[must_use]
    pub fn with_task(mut self, task: Task) -> Self {
        self.task = Some(task);
        self
    }

    #[allow(clippy::return_self_not_must_use)]
    pub fn with_panic_handler<T>(mut self, handler: T) -> Self
    where
        T: Fn(Box<dyn Any + Send>) + Send + Sync + 'static,
    {
        self.panic_handler = Some(Box::new(handler));
        self
    }

    #[allow(clippy::return_self_not_must_use)]
    pub fn with_resolver(mut self, resolver: Resolver) -> Self {
        self.resolver = Some(resolver);
        self
    }

    /// Sets the channel capacity for bounded queues.
    ///
    /// When set, `ready_iter`, `stream_iter`, and `schedule_iter` will use
    /// bounded channels with the specified capacity. When the channel is full,
    /// producers will apply backpressure (return Pending/Reschedule).
    ///
    /// Default is unbounded (None).
    #[allow(clippy::return_self_not_must_use)]
    pub fn with_channel_capacity(mut self, capacity: usize) -> Self {
        self.channel_capacity = Some(capacity);
        self
    }

    /// `ready_iter` adds a task into execution queue but instead of depending
    /// on a [`TaskReadyResolver`] to process the final state instead allows you
    /// to get back a wrapping iterator that allows you synchronously receive those
    /// values from a `RecvIterator` which are only the [`TaskStatus::Ready`].
    /// values of the task.
    ///
    /// This makes it possible to build synchronous experiences in a async world.
    ///
    /// This will deliver task to deliver the bottom of the thread-local execution queue.
    pub fn ready_iter(
        self,
        wait_cycle: time::Duration,
    ) -> AnyResult<NotifyRecvIterator<TaskStatus<Done, Pending, Action>>, ExecutorError> {
        let iter_chan: Arc<NotifyQueue<TaskStatus<Done, Pending, Action>>> =
            match self.channel_capacity {
                Some(capacity) => Arc::new(NotifyQueue::bounded(capacity)),
                None => Arc::new(NotifyQueue::unbounded()),
            };

        let boxed_task = match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (None, Some(mappers)) => ReadyConsumingIter::new(task, mappers, iter_chan.clone()),
                (None, None) => ReadyConsumingIter::new(task, Vec::new(), iter_chan.clone()),
                (_, _) => return Err(ExecutorError::NotSupported),
            },
            None => return Err(ExecutorError::TaskRequired),
        };

        match self.tasks.push(boxed_task.into()) {
            Ok(()) => {
                if let Some(ref yielders) = self.yielders {
                    yielders.interrupt_all();
                }

                match self.tasks.len() {
                    1 => self.latch.signal_one(),
                    _ => self.latch.signal_all(),
                }

                Ok(NotifyRecvIterator::from_notify_queue(iter_chan, wait_cycle))
            }
            Err(err) => match err {
                PushError::Full(_) => Err(ExecutorError::QueueFull),
                PushError::Closed(_) => Err(ExecutorError::QueueClosed),
            },
        }
    }

    /// `stream_iter` adds a task into execution queue but instead of depending
    /// on a [`TaskReadyResolver`] to process the state streams instead allows you
    /// to get back a wrapper iterator that allows you synchronously receive those
    /// values from a `ConcurrentQueueStreamIterator` that implements the [`Iterator`] trait.
    ///
    /// But unlike `schedule_iter` returns [`Stream`] values that hide the underlying
    /// value types of `TaskStatus` which simplifies the trait types your usage
    /// requires.
    ///
    /// This makes it possible to build synchronous experiences in a async world.
    ///
    /// Uses default configuration: `DEFAULT_PARK_DURATION` for park duration and `DEFAULT_MAX_TURNS` for max turns.
    ///
    /// This will deliver task to deliver the bottom of the thread-local execution queue.
    pub fn stream_iter(
        self,
        wait_cycle: time::Duration,
    ) -> AnyResult<NotifyQueueStreamIterator<Done, Pending>, ExecutorError> {
        self.stream_iter_with_config(wait_cycle, crate::valtron::executors::DEFAULT_MAX_TURNS)
    }

    /// `stream_iter_with_config` adds a task into execution queue and returns
    /// a `ConcurrentQueueStreamIterator` with configurable polling behavior.
    ///
    /// But unlike `schedule_iter` returns [`Stream`] values that hide the underlying
    /// value types of `TaskStatus` which simplifies the trait types your usage
    /// requires.
    ///
    /// This makes it possible to build synchronous experiences in an async world,
    /// with fine-grained control over iterator polling strategy.
    ///
    /// # Arguments
    ///
    /// * `wait_cycle` - Thread park duration when queue is empty (passed to `ConcurrentQueueStreamIterator`)
    /// * `max_turns` - Max poll attempts before yielding `Stream::Ignore`
    ///
    /// # Returns
    ///
    /// Returns a `ConcurrentQueueStreamIterator` that yields `Stream<Done, Pending>` items.
    pub fn stream_iter_with_config(
        self,
        wait_cycle: time::Duration,
        max_turns: usize,
    ) -> AnyResult<NotifyQueueStreamIterator<Done, Pending>, ExecutorError> {
        let iter_chan: Arc<NotifyQueue<Stream<Done, Pending>>> = match self.channel_capacity {
            Some(capacity) => Arc::new(NotifyQueue::bounded(capacity)),
            None => Arc::new(NotifyQueue::unbounded()),
        };

        let boxed_task = match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (None, Some(mappers)) => StreamConsumingIter::new(task, mappers, iter_chan.clone()),
                (None, None) => StreamConsumingIter::new(task, Vec::new(), iter_chan.clone()),
                (_, _) => return Err(ExecutorError::NotSupported),
            },
            None => return Err(ExecutorError::TaskRequired),
        };

        match self.tasks.push(boxed_task.into()) {
            Ok(()) => {
                if let Some(ref yielders) = self.yielders {
                    yielders.interrupt_all();
                }

                match self.tasks.len() {
                    1 => self.latch.signal_one(),
                    _ => self.latch.signal_all(),
                }

                Ok(NotifyQueueStreamIterator::new(
                    iter_chan, max_turns, wait_cycle,
                ))
            }
            Err(err) => match err {
                PushError::Full(_) => Err(ExecutorError::QueueFull),
                PushError::Closed(_) => Err(ExecutorError::QueueClosed),
            },
        }
    }

    /// `schedule_iter` adds a task into execution queue but instead of depending
    /// on a [`TaskReadyResolver`] to process the final state instead allows you
    /// to get back a wrapper iterator that allows you synchronously receive those
    /// values from a `RecvIterator` that implements the [`Iterator`] trait.
    ///
    /// This makes it possible to build synchronous experiences in a async world.
    ///
    /// This will deliver task to deliver the bottom of the thread-local execution queue.
    pub fn schedule_iter(
        self,
        wait_cycle: time::Duration,
    ) -> AnyResult<NotifyRecvIterator<TaskStatus<Done, Pending, Action>>, ExecutorError> {
        let iter_chan: Arc<NotifyQueue<TaskStatus<Done, Pending, Action>>> =
            match self.channel_capacity {
                Some(capacity) => Arc::new(NotifyQueue::bounded(capacity)),
                None => Arc::new(NotifyQueue::unbounded()),
            };

        let boxed_task = match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (None, Some(mappers)) => ConsumingIter::new(task, mappers, iter_chan.clone()),
                (None, None) => ConsumingIter::new(task, Vec::new(), iter_chan.clone()),
                (_, _) => return Err(ExecutorError::NotSupported),
            },
            None => return Err(ExecutorError::TaskRequired),
        };

        match self.tasks.push(boxed_task.into()) {
            Ok(()) => {
                if let Some(ref yielders) = self.yielders {
                    yielders.interrupt_all();
                }

                match self.tasks.len() {
                    1 => self.latch.signal_one(),
                    _ => self.latch.signal_all(),
                }

                Ok(NotifyRecvIterator::from_notify_queue(iter_chan, wait_cycle))
            }
            Err(err) => match err {
                PushError::Full(_) => Err(ExecutorError::QueueFull),
                PushError::Closed(_) => Err(ExecutorError::QueueClosed),
            },
        }
    }

    /// `schedule` adds a task into the global execution queue which should
    /// be processed by the underlying thread pool.
    pub fn schedule(self) -> AnyResult<(), ExecutorError> {
        let task: BoxedSendExecutionIterator = match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (Some(resolver), Some(mappers)) => {
                    let mut task_iter = OnNext::new(task, resolver, mappers);
                    if let Some(panic_handler) = self.panic_handler {
                        task_iter = task_iter.with_panic_handler(panic_handler);
                    }
                    Box::new(task_iter)
                }
                (Some(resolver), None) => {
                    let mut task_iter = OnNext::new(task, resolver, Vec::<Mapper>::new());
                    if let Some(panic_handler) = self.panic_handler {
                        task_iter = task_iter.with_panic_handler(panic_handler);
                    }
                    Box::new(task_iter)
                }
                (None, None) => {
                    let mut task_iter = DoNext::new(task);
                    if let Some(panic_handler) = self.panic_handler {
                        task_iter = task_iter.with_panic_handler(panic_handler);
                    }
                    Box::new(task_iter)
                }
                (None, Some(_)) => return Err(ExecutorError::FailedToCreate),
            },
            None => return Err(ExecutorError::TaskRequired),
        };

        match self.tasks.push(task) {
            Ok(()) => {
                // Interrupt any sleeping yielders so they pick up new work immediately
                if let Some(ref yielders) = self.yielders {
                    yielders.interrupt_all();
                }

                match self.tasks.len() {
                    1 => self.latch.signal_one(),
                    _ => self.latch.signal_all(),
                }
                Ok(())
            }
            Err(err) => match err {
                PushError::Full(_) => Err(ExecutorError::QueueFull),
                PushError::Closed(_) => Err(ExecutorError::QueueClosed),
            },
        }
    }
}

impl<
        F,
        Done: Send + 'static,
        Pending: Send + 'static,
        Action: ExecutionAction + Send + 'static,
        Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
        Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
    > ThreadPoolTaskBuilder<Done, Pending, Action, Mapper, FnReady<F, Action>, Task>
where
    F: Fn(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + Send + 'static,
{
    pub fn on_next(self, action: F) -> Self {
        self.with_resolver(FnReady::new(action))
    }
}

impl<
        F,
        Done: Send + 'static,
        Pending: Send + 'static,
        Action: ExecutionAction + Send + 'static,
        Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
        Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
    > ThreadPoolTaskBuilder<Done, Pending, Action, Mapper, FnMutReady<F, Action>, Task>
where
    F: FnMut(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + Send + 'static,
{
    pub fn on_next_mut(self, action: F) -> Self {
        self.with_resolver(FnMutReady::new(action))
    }
}

// ============================================================================
// ThreadRegistry - Central coordination replacing ThreadPool
// ============================================================================

/// `ThreadRegistry` replaces `ThreadPool` as the coordination point.
/// It holds the shared task queue, signals, activity channel, and thread handles.
pub struct ThreadRegistry {
    // Shared work distribution
    pub(crate) shared_tasks: Arc<ConcurrentQueue<BoxedSendExecutionIterator>>,
    pub(crate) latch: Arc<LockSignal>,

    // Kill coordination
    pub(crate) kill_signal: Arc<OnSignal>,
    pub(crate) kill_latch: Arc<LockSignal>,

    // Activity monitoring
    activity_sender: mpp::Sender<ThreadActivity>,
    pub(crate) activity_receiver: mpp::Receiver<ThreadActivity>,

    // Shutdown coordination
    pub(crate) waitgroup: WaitGroup,

    // Thread management
    rng: Mutex<ChaCha8Rng>,
    pub(crate) max_threads: usize,
    pub(crate) live_threads: Arc<AtomicUsize>,
    idle_threads: Arc<AtomicUsize>,

    // Worker thread config
    priority: PriorityOrder,
    op_read_time: time::Duration,
    yield_wait_time: time::Duration,
    thread_stack_size: Option<usize>,
    thread_max_idle_count: u32,
    thread_max_sleep_before_end: u32,
    thread_back_off_factor: u32,
    thread_back_off_jitter: f32,
    thread_back_min_duration: time::Duration,
    thread_back_max_duration: time::Duration,

    // Thread tracking
    registry: SharedThreadRegistry,
    thread_handles: RwLock<HashMap<ThreadId, JoinHandle<ThreadExecutionResult<()>>>>,

    // Yielder tracking for interrupt_all
    yielders: SharedThreadYielders,
}

impl std::fmt::Debug for ThreadRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreadRegistry")
            .field("shared_tasks", &"<SharedTaskQueue>")
            .field("latch", &"<LockSignal>")
            .field("kill_signal", &"<OnSignal>")
            .field("kill_latch", &"<LockSignal>")
            .field("activity_sender", &"<Sender<ThreadActivity>>")
            .field("activity_receiver", &"<Receiver<ThreadActivity>>")
            .field("waitgroup", &"<WaitGroup>")
            .field("rng", &"<Mutex<ChaCha8Rng>>")
            .field("max_threads", &self.max_threads)
            .field("live_threads", &self.live_threads)
            .field("idle_threads", &self.idle_threads)
            .field("priority", &self.priority)
            .field("op_read_time", &self.op_read_time)
            .field("yield_wait_time", &self.yield_wait_time)
            .field("thread_stack_size", &self.thread_stack_size)
            .field("thread_max_idle_count", &self.thread_max_idle_count)
            .field(
                "thread_max_sleep_before_end",
                &self.thread_max_sleep_before_end,
            )
            .field("thread_back_off_factor", &self.thread_back_off_factor)
            .field("thread_back_off_jitter", &self.thread_back_off_jitter)
            .field("thread_back_min_duration", &self.thread_back_min_duration)
            .field("thread_back_max_duration", &self.thread_back_max_duration)
            .field("registry", &"<SharedThreadRegistry>")
            .field("thread_handles", &"<RwLock<HashMap>>")
            .finish()
    }
}

impl ThreadRegistry {
    /// Create a new `ThreadRegistry` with the given seed and thread count.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        seed_for_rng: u64,
        num_threads: usize,
        priority: PriorityOrder,
        op_read_time: time::Duration,
        yield_wait: time::Duration,
        thread_stack_size: Option<usize>,
        thread_max_idle_count: u32,
        thread_max_sleep_before_end: u32,
        thread_back_off_factor: u32,
        thread_back_off_jitter: f32,
        thread_back_min_duration: time::Duration,
        thread_back_max_duration: time::Duration,
    ) -> Self {
        assert!(
            num_threads >= 2,
            "Unable to create ThreadRegistry with < 2 threads"
        );

        assert!(
            num_threads <= THREADS_MAX,
            "Thread count must not exceed {THREADS_MAX}"
        );

        let latch = Arc::new(LockSignal::new());
        let kill_latch = Arc::new(LockSignal::new());
        let shared_tasks: Arc<ConcurrentQueue<BoxedSendExecutionIterator>> =
            Arc::new(ConcurrentQueue::unbounded());
        let (activity_sender, activity_receiver) = mpp::unbounded::<ThreadActivity>();

        Self {
            shared_tasks: shared_tasks.clone(),
            latch: latch.clone(),
            kill_signal: Arc::new(OnSignal::new()),
            kill_latch,
            activity_sender: activity_sender.clone(),
            activity_receiver,
            waitgroup: WaitGroup::new(),
            rng: Mutex::new(ChaCha8Rng::seed_from_u64(seed_for_rng)),
            max_threads: num_threads,
            live_threads: Arc::new(AtomicUsize::new(0)),
            idle_threads: Arc::new(AtomicUsize::new(0)),
            priority,
            op_read_time,
            yield_wait_time: yield_wait,
            thread_stack_size,
            thread_max_idle_count,
            thread_max_sleep_before_end,
            thread_back_off_factor,
            thread_back_off_jitter,
            thread_back_min_duration,
            thread_back_max_duration,
            registry: SharedThreadRegistry::new(Arc::new(RwLock::new(ThreadPoolRegistryInner {
                threads: EntryList::new(),
            }))),
            thread_handles: RwLock::new(HashMap::new()),
            yielders: SharedThreadYielders::new(ThreadYielders::new()),
        }
    }

    /// Create a `ThreadRegistry` with default configuration.
    #[must_use]
    pub fn with_seed_and_threads(seed_from_rng: u64, num_threads: usize) -> Self {
        Self::new(
            seed_from_rng,
            num_threads,
            PriorityOrder::Top,
            DEFAULT_OP_READ_TIME,
            DEFAULT_YIELD_WAIT_TIME,
            None,
            MAX_ROUNDS_IDLE_COUNT,
            MAX_ROUNDS_WHEN_SLEEPING_ENDS,
            BACK_OFF_THREAD_FACTOR,
            BACK_OFF_JITER,
            BACK_OFF_MIN_DURATION,
            BACK_OFF_MAX_DURATION,
        )
    }

    /// Accessor for `shared_tasks` queue.
    #[must_use]
    pub fn shared_tasks(&self) -> Arc<ConcurrentQueue<BoxedSendExecutionIterator>> {
        self.shared_tasks.clone()
    }

    /// Accessor for latch.
    #[must_use]
    pub fn latch(&self) -> Arc<LockSignal> {
        self.latch.clone()
    }

    /// Accessor for `kill_signal`.
    #[must_use]
    pub fn kill_signal(&self) -> Arc<OnSignal> {
        self.kill_signal.clone()
    }

    /// Accessor for waitgroup.
    #[must_use]
    pub fn waitgroup(&self) -> &WaitGroup {
        &self.waitgroup
    }

    /// Register a yielder for interrupt_all tracking.
    #[tracing::instrument(skip(self))]
    pub fn register_yielder(&self, yielder: Arc<ThreadYielder>) {
        tracing::trace!("ThreadRegistry::register_yielder() - registering yielder");
        self.yielders.register(yielder);
        tracing::trace!("ThreadRegistry::register_yielder() - done");
    }

    /// Get the shared yielders registry.
    #[must_use]
    pub fn yielders(&self) -> SharedThreadYielders {
        Arc::clone(&self.yielders)
    }

    /// Interrupt all registered yielders.
    /// Call this during shutdown to wake threads waiting in yield_for.
    pub fn interrupt_all_yielders(&self) {
        tracing::trace!("ThreadRegistry::interrupt_all_yielders() - calling interrupt_all");
        self.yielders.interrupt_all();
        tracing::trace!("ThreadRegistry::interrupt_all_yielders() - done");
    }

    /// Shutdown the registry - signals kill and waits for all threads.
    pub fn shutdown(&self) {
        self.kill_signal.turn_on();
        // Interrupt all yielding threads first
        self.interrupt_all_yielders();
        self.latch.signal_all();
        self.waitgroup.wait();
        self.join_all_threads();
        // Clear all remaining thread entries from the registry
        self.registry.clear();
        self.kill_latch.signal_all();
    }

    /// Join all thread handles.
    pub fn join_all_threads(&self) {
        let handles = {
            let mut map = self.thread_handles.write().unwrap();
            std::mem::take(&mut *map)
        };

        for (_id, handle) in handles {
            let _ = handle.join();
        }
    }

    /// Block until all threads are done.
    pub fn block_until_done(&self) {
        loop {
            if self.listen_for_activity().is_none() {
                break;
            }
        }
        self.waitgroup.wait();
        self.join_all_threads();
        self.kill_latch.signal_all();
    }

    /// Listen for activity events from worker threads.
    fn listen_for_activity(&self) -> Option<()> {
        let remaining_live = self.live_threads.load(atomic::Ordering::Acquire);
        if remaining_live == 0 {
            tracing::debug!("No more live threads, returning");
            self.latch.signal_all();
            return None;
        }

        if self.kill_signal.probe() {
            tracing::debug!("Kill signal detected, waking up");
            self.latch.signal_all();
        }

        let thread_activity = match self.activity_receiver.recv_timeout(self.op_read_time) {
            Ok(activity) => activity,
            Err(err) => match err {
                mpp::ReceiverError::Empty => return Some(()),
                mpp::ReceiverError::Timeout => return Some(()),
                mpp::ReceiverError::Closed(_) => return None,
            },
        };

        match thread_activity {
            ThreadActivity::BroadcastedTask => {
                if self.shared_tasks.len() == 1 {
                    self.latch.signal_one();
                }
            }
            ThreadActivity::Started(_id) => {
                tracing::debug!("Thread started: {:?}", _id);
            }
            ThreadActivity::Stopped(id) => {
                tracing::debug!("Thread stopped: {:?}", id);
                self.live_threads.fetch_sub(1, atomic::Ordering::AcqRel);
                // Remove from thread registry entry list to prevent stale entries
                self.registry.unregister_thread(id.clone());
                self.remove_thread_id(id);
            }
            ThreadActivity::Panicked(id, err) => {
                tracing::debug!("Thread panicked: {:?}, err: {:?}", id, err);
                self.live_threads.fetch_sub(1, atomic::Ordering::AcqRel);
                // Remove from thread registry entry list to prevent stale entries
                self.registry.unregister_thread(id.clone());
                self.remove_thread_id(id);

                // Respawn if under max threads and not killed
                if self.live_threads.load(atomic::Ordering::Acquire) < self.max_threads
                    && !self.kill_signal.probe()
                {
                    tracing::debug!("Respawning thread after panic");
                    let _ = self.spawn_worker();
                }
            }
            ThreadActivity::Blocked(id) => {
                tracing::debug!("Thread blocked: {:?}", id);
                self.idle_threads.fetch_add(1, atomic::Ordering::AcqRel);
            }
            ThreadActivity::Unblocked(id) => {
                tracing::debug!("Thread unblocked: {:?}", id);
                self.idle_threads.fetch_sub(1, atomic::Ordering::AcqRel);
            }
            ThreadActivity::Parked(id) => {
                tracing::debug!("Thread parked: {:?}", id);
                self.idle_threads.fetch_add(1, atomic::Ordering::AcqRel);
            }
            ThreadActivity::Unparked(id) => {
                tracing::debug!("Thread unparked: {:?}", id);
                self.idle_threads.fetch_sub(1, atomic::Ordering::AcqRel);
            }
        }

        Some(())
    }

    fn remove_thread_id(&self, thread_id: ThreadId) {
        self.thread_handles.write().unwrap().remove(&thread_id);
    }

    /// Spawn a worker thread with `WaitGroup` tracking.
    pub fn spawn_worker(&self) -> ThreadExecutionResult<ThreadRef> {
        use std::thread::Builder;

        let mut rng = self.rng.lock().unwrap();
        let seed = rng.next_u64();
        drop(rng);

        let b = Builder::new();
        let b = if let Some(size) = self.thread_stack_size {
            b.stack_size(size)
        } else {
            b
        };

        // Register thread in registry first
        let worker_tag = format!("worker-{seed}");

        let thread_ref = ThreadRef::new(
            seed,
            worker_tag.clone(),
            self.shared_tasks.clone(),
            None,
            self.registry.clone(),
            self.kill_signal.clone(),
        );

        let thread_id = self.registry.register_thread(
            thread_ref.name.clone(),
            thread_ref.clone(),
            self.latch.clone(),
            self.activity_sender.clone(),
        );

        // Get updated thread_ref from registry (process field is now populated)
        let thread_ref = self.registry.get_thread(thread_id.clone());

        // Register the yielder for interrupt_all tracking
        let process_clone = thread_ref.process.clone().unwrap();
        self.register_yielder(Arc::new(process_clone.clone()));

        let priority = self.priority.clone();
        let seed_clone = thread_ref.seed;
        let task_clone = thread_ref.tasks.clone();
        let thread_kill_signal = thread_ref.global_kill_signal.clone();

        let sender_id = thread_id.clone();
        let sender = self.activity_sender.clone();

        let thread_yield_wait_duration = self.yield_wait_time;
        let thread_max_idle_count = self.thread_max_idle_count;
        let thread_max_sleep_before_end = self.thread_max_sleep_before_end;
        let thread_back_off_factor = self.thread_back_off_factor;
        let thread_back_off_jitter = self.thread_back_off_jitter;
        let thread_back_min_duration = self.thread_back_min_duration;
        let thread_back_max_duration = self.thread_back_max_duration;

        // Get WaitGroup guard BEFORE spawning
        let wg_guard = self.waitgroup.guard();
        self.waitgroup.add(1);

        // Capture the spawning thread's tracing dispatcher so events emitted on
        // this worker reach the SAME subscriber — including a test's thread-local
        // `#[traced_test]` subscriber, which otherwise only covers the test thread
        // (worker-thread `tracing::*` would silently go to the global no-op one).
        let dispatch = tracing::dispatcher::get_default(|d| d.clone());

        match b.spawn(move || {
            // Install the inherited dispatcher for this worker thread's lifetime.
            let _dispatch_guard = tracing::dispatcher::set_default(&dispatch);
            let span = tracing::trace_span!("ThreadRegistry::spawn_worker.local_executor.thread");
            let _enter = span.enter();

            tracing::info!("Worker thread({}) {} STARTED", &worker_tag, seed_clone);

            // Hold guard for lifetime of thread - dropped on exit (including panic)
            let _wg = wg_guard;

            match panic::catch_unwind(|| {
                sender
                    .send(ThreadActivity::Started(sender_id.clone()))
                    .expect("should send event");

                let thread_executor = LocalThreadExecutor::from_seed(
                    seed_clone,
                    worker_tag.clone(),
                    task_clone,
                    IdleMan::new(
                        thread_max_idle_count,
                        None,
                        SleepyMan::new(
                            thread_max_sleep_before_end,
                            ExponentialBackoffDecider::new(
                                thread_back_off_factor,
                                thread_back_off_jitter,
                                thread_back_min_duration,
                                Some(thread_back_max_duration),
                            ),
                        ),
                    ),
                    priority,
                    process_clone,
                    thread_yield_wait_duration,
                    Some(thread_kill_signal),
                    Some(sender.clone()),
                );

                thread_executor.block_on();

                sender
                    .send(ThreadActivity::Stopped(sender_id.clone()))
                    .expect("should send event");
                tracing::trace!(
                    "Worker thread({}) {} stopped blocking and shoiuld now stop (ok)",
                    &worker_tag,
                    seed_clone
                );
            }) {
                Ok(()) => {
                    tracing::warn!("Worker thread({}) {} STOPPED (ok)", &worker_tag, seed_clone);
                    Ok(())
                }
                Err(err) => {
                    tracing::warn!(
                        "Worker thread({}) {} STOPPED (panic: {:?})",
                        &worker_tag,
                        seed_clone,
                        err
                    );
                    sender
                        .send(ThreadActivity::Panicked(sender_id.clone(), err))
                        .expect("should send event");
                    sender
                        .send(ThreadActivity::Stopped(sender_id))
                        .expect("should send event");
                    Ok(())
                }
            }
            // _wg dropped here → WaitGroup::done()
        }) {
            Ok(handle) => {
                self.thread_handles
                    .write()
                    .unwrap()
                    .insert(thread_id.clone(), handle);

                self.live_threads.fetch_add(1, atomic::Ordering::AcqRel);

                Ok(thread_ref.clone())
            }
            Err(err) => Err(ThreadExecutionError::FailedStart(Box::new(err))),
        }
    }
}
