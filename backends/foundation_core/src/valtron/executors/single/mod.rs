#![allow(clippy::type_complexity)]

mod pool_guard;

pub use pool_guard::PoolGuard;

use std::cell::OnceCell;
use std::sync::Arc;

use crate::valtron::{
    executors::constants::{
        BACK_OFF_JITER, BACK_OFF_MAX_DURATION, BACK_OFF_MIN_DURATION, BACK_OFF_THREAD_FACTOR,
        DEFAULT_YIELD_WAIT_TIME, MAX_ROUNDS_IDLE_COUNT, MAX_ROUNDS_WHEN_SLEEPING_ENDS,
    },
    Ctx, LocalThreadExecutor, PriorityOrder, ProcessController, ProgressIndicator,
    SharedTaskQueue, TaskIterator, TaskReadyResolver, TaskSpawnConfig,
};

use crate::{
    retries::ExponentialBackoffDecider,
    synca::{IdleMan, SleepyMan},
};
use concurrent_queue::ConcurrentQueue;
#[cfg(target_family = "wasm")]
use foundation_nostd::primitives::SpinWaiter;

/// `NoThreadController` is the single-threaded executor's `ProcessController`:
/// what the engine does while it waits for its next timer or wake.
///
/// - **wasm**: no OS sleep is available, so it waits with an iteration-counted
///   `SpinWaiter` (bounded and interruptible).
/// - **native**: it parks the thread with `std::thread::park_timeout` for the
///   requested duration. That honours the engine's timer deadlines exactly,
///   gives the CPU back while idle, and returns early if the thread is
///   unparked. (It used to spin through `SpinWaiter::embedded()`, whose fixed
///   1,000,000-iterations-per-ms estimate made a 100 ms wait spin for seconds
///   on a desktop CPU and rounded sub-millisecond waits down to nothing.)
#[derive(Clone)]
pub struct NoThreadController {
    #[cfg(target_family = "wasm")]
    waiter: SpinWaiter,
}

impl NoThreadController {
    /// Creates a new NoThreadController with platform-appropriate settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            #[cfg(target_family = "wasm")]
            waiter: SpinWaiter::wasm(),
        }
    }
}

impl Default for NoThreadController {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessController for NoThreadController {
    type YieldSignal = ();

    fn yield_for(&self, dur: std::time::Duration) {
        #[cfg(target_family = "wasm")]
        self.waiter.wait(dur);

        #[cfg(not(target_family = "wasm"))]
        std::thread::park_timeout(dur);
    }
}

// Default controller: JSThreadYielder on wasm32 (JS event loop aware),
// NoThreadController on native (iteration-based spinning). The JS yielder is
// wasm-only, so the selection is gated on the target too — a native build with a
// `js-*` feature still uses NoThreadController.
#[cfg(all(
    target_family = "wasm",
    any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
))]
use crate::valtron::wasm::JSThreadYielder as DefaultController;

// NoThreadController is defined in this same file below — no import needed for native.
#[cfg(not(all(
    target_family = "wasm",
    any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
)))]
type DefaultController = NoThreadController;

thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static GLOBAL_LOCAL_EXECUTOR_ENGINE: OnceCell<LocalThreadExecutor<DefaultController>> = OnceCell::new();
}

/// Optional singleton guard backing [`SingleExecutorSingleton`].
static VALTRON_SINGLETON: std::sync::Mutex<Option<PoolGuard>> = std::sync::Mutex::new(None);

/// Optional convenience wrapper for initializing the single-threaded valtron executor.
///
/// Provides "init once, get guard" semantics similar to `CfHttpAppSingleton`.
/// Only available on wasm32/wasm64 targets (where single-threaded mode is the norm).
///
/// The existing free functions (`initialize_pool`, `spawn`, `run_until_complete`, etc.)
/// continue to work independently — use this when you want explicit guard ownership.
pub struct SingleExecutorSingleton;

impl SingleExecutorSingleton {
    /// Initialize the executor and return a PoolGuard.
    ///
    /// On first call: calls `initialize_pool(seed)`, stores a PoolGuard,
    /// runs the setup closure. On subsequent calls: returns a cloned guard
    /// (PoolGuard is zero-sized in single mode).
    pub fn get_or_init<F: FnOnce(&PoolGuard)>(seed: u64, setup: F) -> PoolGuard {
        initialize_pool(seed);
        let mut guard = VALTRON_SINGLETON.lock().unwrap();
        if guard.is_none() {
            *guard = Some(PoolGuard);
        }
        setup(guard.as_ref().unwrap());
        guard.clone().unwrap()
    }

    /// Get a PoolGuard. Panics if `get_or_init` has not been called.
    pub fn guard() -> PoolGuard {
        VALTRON_SINGLETON
            .lock()
            .unwrap()
            .clone()
            .expect("Valtron not initialized — call SingleExecutorSingleton::get_or_init() first")
    }

    /// Check if the singleton has been initialized.
    pub fn is_initialized() -> bool {
        VALTRON_SINGLETON.lock().unwrap().is_some()
    }
}

/// `initialize` initializes the local single-threaded
/// execution engine, and is required to call this as your
/// first call when using this in WebAssembly or `SingleThreaded`
/// environment.
pub fn initialize_pool(seed_for_rng: u64) {
    GLOBAL_LOCAL_EXECUTOR_ENGINE.with(|pool| {
        let _ = pool.get_or_init(|| {
            // `SharedTaskQueue` is `Arc<…>` in BOTH cfgs so one alias serves the
            // multi-threaded (Send) and single-threaded (non-Send) executors; in
            // this thread-local engine the Arc never crosses a thread, so the
            // non-Send contents are fine.
            #[allow(clippy::arc_with_non_send_sync)]
            let tasks: SharedTaskQueue = Arc::new(ConcurrentQueue::unbounded());
            LocalThreadExecutor::from_seed(
                seed_for_rng,
                "single-thread-pool".to_string(),
                tasks,
                IdleMan::new(
                    MAX_ROUNDS_IDLE_COUNT,
                    None,
                    SleepyMan::new(
                        MAX_ROUNDS_WHEN_SLEEPING_ENDS,
                        ExponentialBackoffDecider::new(
                            BACK_OFF_THREAD_FACTOR,
                            BACK_OFF_JITER,
                            BACK_OFF_MIN_DURATION,
                            Some(BACK_OFF_MAX_DURATION),
                        ),
                    ),
                ),
                PriorityOrder::Top,
                DefaultController::new(),
                DEFAULT_YIELD_WAIT_TIME,
                None,
                None,
            )
        });
    });
}

/// Whether this thread's engine has work it can run right now: an active or
/// in-flight task, or a queued one waiting to be picked up. `false` means every
/// remaining task (if any) is parked, waiting on a timer or a readiness signal.
///
/// Returns `false` when the engine has not been initialized.
#[must_use]
pub fn has_runnable_work() -> bool {
    GLOBAL_LOCAL_EXECUTOR_ENGINE.with(|pool| {
        pool.get()
            .is_some_and(LocalThreadExecutor::has_runnable_work)
    })
}

/// Whether this thread's engine has been initialized ([`initialize_pool`]).
///
/// Host callbacks that resume the executor (a JS timer or microtask queued by a
/// yield) check this first: such a callback can fire on a thread that never
/// created a pool, and with nothing to resume it must be a no-op rather than
/// [`run_until`]'s panic.
#[must_use]
pub fn is_initialized() -> bool {
    GLOBAL_LOCAL_EXECUTOR_ENGINE.with(|pool| pool.get().is_some())
}

/// `run_until` calls the `LocalExecution` queue and processes
/// the next pending message by moving it forward just until
/// the provided function returns true indicating it should stop.
///
/// This lets you review the state or some internal conditions
/// upon which you wish to stop processing.
///
/// It works like [`run_once`] but not just executed once but till
/// the condition is met.
///
/// # Panics
///
/// 1. Will panic if no pool exists.
///
/// # Returns
///
/// The progress indicator from the execution
pub fn run_until<S>(checker: S)
where
    S: Fn(ProgressIndicator) -> bool,
{
    GLOBAL_LOCAL_EXECUTOR_ENGINE.with(|pool| match pool.get() {
        Some(pool) => pool.run_until(checker),
        None => panic!("Thread pool not initialized, ensure to call initialize() first"),
    });
}

/// Fork a fresh [`Ctx`] from the current engine's stable root.
///
/// The single-threaded engine owns one root [`Ctx`] seeded at construction and
/// forks it per task. This exposes that same root-fork to code that drives a
/// task alongside the engine (e.g. [`super::non_sendables::DrivenTaskIterator`])
/// so it never mints an unrelated `Ctx::new()`.
///
/// # Panics
///
/// Panics if the pool has not been initialized (mirrors [`run_until`]).
#[must_use]
pub fn fork_ctx() -> Ctx {
    GLOBAL_LOCAL_EXECUTOR_ENGINE.with(|pool| match pool.get() {
        Some(pool) => pool.fork_ctx(),
        None => panic!("Thread pool not initialized, ensure to call initialize() first"),
    })
}

/// `run_once` calls the `LocalExecution` queue and processes
/// the next pending message by moving it forward just once.
///
/// I rearly see you using this, but there might be situations
/// where more fine-grained control on how much work you wish
/// to do matters and you do not want a [`run_until_complete`]
/// where the underlying point of stopping is not controlled
/// by you.
///
/// # Panics
///
/// 1. Will panic if no pool exists.
///
/// # Returns
///
/// The progress indicator from the execution
#[must_use]
pub fn run_once() -> ProgressIndicator {
    GLOBAL_LOCAL_EXECUTOR_ENGINE.with(|pool| match pool.get() {
        Some(pool) => pool.run_once(),
        None => panic!("Thread pool not initialized, ensure to call initialize() first"),
    })
}

/// `run_until_complete` calls the `LocalExecution` queue and processes
/// all pending messages till they are completed.
///
/// # Panics
///
/// 1. Will panic if no pool exists.
///
pub fn run_until_complete() {
    GLOBAL_LOCAL_EXECUTOR_ENGINE.with(|pool| match pool.get() {
        Some(pool) => pool.block_until_finished(),
        None => panic!("Thread pool not initialized, ensure to call initialize() first"),
    });
}

/// `spawn` provides a builder which specifically allows you to build out
/// the underlying tasks to be scheduled into the global queue.
///
/// It expects you infer the type of `Task` and `Action` from the
/// type implementing `TaskIterator`.
///
/// # Panics
///
/// 1. Will panic if no pool exists.
///
/// # Returns
///
/// A builder for creating a task iterator
#[must_use]
pub fn spawn<Task>() -> TaskSpawnConfig<
    Task::Ready,
    Task::Pending,
    Box<dyn TaskReadyResolver<Task::Ready, Task::Pending> + 'static>,
    Task,
>
where
    Task::Ready: 'static,
    Task::Pending: 'static,
    Task: TaskIterator + 'static,
{
    GLOBAL_LOCAL_EXECUTOR_ENGINE.with(|pool| match pool.get() {
        Some(pool) => TaskSpawnConfig::new(pool.boxed_engine()),
        None => panic!("Thread pool not initialized, ensure to call initialize() first"),
    })
}

/// `spawn2` provides a builder which specifically allows you to build out
/// the underlying tasks to be scheduled into the global queue.
///
/// It expects you to provide the `Resolver` type explicitly.
///
/// # Panics
///
/// 1. Will panic if no pool exists.
///
/// # Returns
///
/// A builder for creating a task iterator
#[must_use]
pub fn spawn2<Task, Resolver>(
) -> TaskSpawnConfig<Task::Ready, Task::Pending, Resolver, Task>
where
    Task::Ready: 'static,
    Task::Pending: 'static,
    Task: TaskIterator + 'static,
    Resolver: TaskReadyResolver<Task::Ready, Task::Pending> + 'static,
{
    GLOBAL_LOCAL_EXECUTOR_ENGINE.with(|pool| match pool.get() {
        Some(pool) => TaskSpawnConfig::new(pool.boxed_engine()),
        None => panic!("Thread pool not initialized, ensure to call initialize() first"),
    })
}

/// Execute a blocking closure inline on the current thread.
///
/// WHY: Single-threaded environments have no background threads — run immediately
/// WHAT: Executes the closure with `catch_unwind` for consistency with multi behavior
/// HOW: Wraps in `AssertUnwindSafe` and calls `catch_unwind`
///
/// # Errors
///
/// Returns an error if the closure panics.
pub fn run_background_job(job: impl FnOnce() + 'static) -> crate::valtron::GenericResult<()> {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
    match result {
        Ok(()) => Ok(()),
        Err(panic_info) => {
            tracing::error!("Background job panicked (single-threaded): {panic_info:?}");
            Err("Background job panicked".into())
        }
    }
}
