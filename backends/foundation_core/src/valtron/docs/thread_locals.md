# Threads Local

We can use this way to fetch the current thread id.

All taking from the Smol async runtime

```rust
#[inline]
fn thread_id() -> ThreadId {
    std::thread_local! {
        static ID: ThreadId = thread::current().id();
    }
    ID.try_with(|id| *id)
        .unwrap_or_else(|_| thread::current().id())
}

```

An intersting use of thread local

```rust
thread_local! {
     // A queue that holds scheduled tasks.
     static QUEUE: (Sender<Runnable>, Receiver<Runnable>) = flume::unbounded();
 }

 // Make a non-Send future.
 let msg: Rc<str> = "Hello, world!".into();
 let future = async move {
     println!("{}", msg);
 };

 // A function that schedules the task when it gets woken up.
 let s = QUEUE.with(|(s, _)| s.clone());
 let schedule = move |runnable| s.send(runnable).unwrap();
```


```rust
#[cfg(not(feature = "wasm_runtime"))]
use std::sync;

#[cfg(feature = "wasm_runtime")]
use wasm_sync as sync;

use self::registry::{CustomSpawn, DefaultSpawn, ThreadSpawn};

/// Number of bits used for the thread counters.
#[cfg(target_pointer_width = "64")]
const THREADS_BITS: usize = 16;

#[cfg(target_pointer_width = "32")]
const THREADS_BITS: usize = 8;

/// Bits to shift to select the sleeping threads
/// (used with `select_bits`).
#[allow(clippy::erasing_op)]
const SLEEPING_SHIFT: usize = 0 * THREADS_BITS;

/// Bits to shift to select the inactive threads
/// (used with `select_bits`).
#[allow(clippy::identity_op)]
const INACTIVE_SHIFT: usize = 1 * THREADS_BITS;

/// Bits to shift to select the JEC
/// (use JOBS_BITS).
const JEC_SHIFT: usize = 2 * THREADS_BITS;

/// Max value for the thread counters.
pub(crate) const THREADS_MAX: usize = (1 << THREADS_BITS) - 1;


/// The type for a panic handling closure. Note that this same closure
/// may be invoked multiple times in parallel.
type PanicHandler = dyn Fn(Box<dyn Any + Send>) + Send + Sync;

/// The type for a closure that gets invoked when a thread starts. The
/// closure is passed the index of the thread on which it is invoked.
/// Note that this same closure may be invoked multiple times in parallel.
type StartHandler = dyn Fn(usize) + Send + Sync;

/// The type for a closure that gets invoked when a thread exits. The
/// closure is passed the index of the thread on which it is invoked.
/// Note that this same closure may be invoked multiple times in parallel.
type ExitHandler = dyn Fn(usize) + Send + Sync;

/// Used to create a new [`ThreadPool`] or to configure the global rayon thread pool.
/// ## Creating a ThreadPool
/// The following creates a thread pool with 22 threads.
///
/// `rust
/// # use rayon_core as rayon;
/// let pool = rayon::ThreadPoolBuilder::new().num_threads(22).build().unwrap();
/// `
///
/// To instead configure the global thread pool, use [`build_global()`]:
///
/// `rust
/// # use rayon_core as rayon;
/// rayon::ThreadPoolBuilder::new().num_threads(22).build_global().unwrap();
/// `
///
/// [`ThreadPool`]: struct.ThreadPool.html
/// [`build_global()`]: struct.ThreadPoolBuilder.html#method.build_global
pub struct ThreadPoolBuilder<S = DefaultSpawn> {
    /// The number of threads in the rayon thread pool.
    /// If zero will use the RAYON_NUM_THREADS environment variable.
    /// If RAYON_NUM_THREADS is invalid or zero will use the default.
    num_threads: usize,

    /// The thread we're building *from* will also be part of the pool.
    use_current_thread: bool,

    /// Custom closure, if any, to handle a panic that we cannot propagate
    /// anywhere else.
    panic_handler: Option<Box<PanicHandler>>,

    /// Closure to compute the name of a thread.
    get_thread_name: Option<Box<dyn FnMut(usize) -> String>>,

    /// The stack size for the created worker threads
    stack_size: Option<usize>,

    /// Closure invoked on worker thread start.
    start_handler: Option<Box<StartHandler>>,

    /// Closure invoked on worker thread exit.
    exit_handler: Option<Box<ExitHandler>>,

    /// Closure invoked to spawn threads.
    spawn_handler: S,

    /// If false, worker threads will execute spawned jobs in a
    /// "depth-first" fashion. If true, they will do a "breadth-first"
    /// fashion. Depth-first is the default.
    breadth_first: bool,
}

/// Returns the maximum number of threads that Rayon supports in a single thread-pool.
///
/// If a higher thread count is requested by calling `ThreadPoolBuilder::num_threads` or by setting
/// the `RAYON_NUM_THREADS` environment variable, then it will be reduced to this maximum.
///
/// The value may vary between different targets, and is subject to change in new Rayon versions.
pub fn max_num_threads() -> usize {
    // We are limited by the bits available in the sleep counter's `AtomicUsize`.
    crate::sleep::THREADS_MAX
}

/// Push a job into each thread's own "external jobs" queue; it will be
/// executed only on that thread, when it has nothing else to do locally,
/// before it tries to steal other work.
///
/// **Panics** if not given exactly as many jobs as there are threads.
pub(super) fn inject_broadcast(&self, injected_jobs: impl ExactSizeIterator<Item = JobRef>) {
    assert_eq!(self.num_threads(), injected_jobs.len());
    {
        let broadcasts = self.broadcasts.lock().unwrap();

        // It should not be possible for `state.terminate` to be true
        // here. It is only set to true when the user creates (and
        // drops) a `ThreadPool`; and, in that case, they cannot be
        // calling `inject_broadcast()` later, since they dropped their
        // `ThreadPool`.
        debug_assert_ne!(
            self.terminate_count.load(Ordering::Acquire),
            0,
            "inject_broadcast() sees state.terminate as true"
        );

        assert_eq!(broadcasts.len(), injected_jobs.len());
        for (worker, job_ref) in broadcasts.iter().zip(injected_jobs) {
            worker.push(job_ref);
        }
    }
    for i in 0..self.num_threads() {
        self.sleep.notify_worker_latch_is_set(i);
    }
}

/// If already in a worker-thread of this registry, just execute `op`.
/// Otherwise, inject `op` in this thread-pool. Either way, block until `op`
/// completes and return its return value. If `op` panics, that panic will
/// be propagated as well.  The second argument indicates `true` if injection
/// was performed, `false` if executed directly.
pub(super) fn in_worker<OP, R>(&self, op: OP) -> R
where
    OP: FnOnce(&WorkerThread, bool) -> R + Send,
    R: Send,
{
    unsafe {
        let worker_thread = WorkerThread::current();
        if worker_thread.is_null() {
            self.in_worker_cold(op)
        } else if (*worker_thread).registry().id() != self.id() {
            self.in_worker_cross(&*worker_thread, op)
        } else {
            // Perfectly valid to give them a `&T`: this is the
            // current thread, so we know the data structure won't be
            // invalidated until we return.
            op(&*worker_thread, false)
        }
    }
}

#[cold]
unsafe fn in_worker_cold<OP, R>(&self, op: OP) -> R
where
    OP: FnOnce(&WorkerThread, bool) -> R + Send,
    R: Send,
{
    thread_local!(static LOCK_LATCH: LockLatch = LockLatch::new());

    LOCK_LATCH.with(|l| {
        // This thread isn't a member of *any* thread pool, so just block.
        debug_assert!(WorkerThread::current().is_null());
        let job = StackJob::new(
            |injected| {
                let worker_thread = WorkerThread::current();
                assert!(injected && !worker_thread.is_null());
                op(&*worker_thread, true)
            },
            LatchRef::new(l),
        );
        self.inject(job.as_job_ref());
        job.latch.wait_and_reset(); // Make sure we can use the same latch again next time.

        job.into_result()
    })
}


const ROUNDS_UNTIL_SLEEPY: u32 = 32;
const ROUNDS_UNTIL_SLEEPING: u32 = ROUNDS_UNTIL_SLEEPY + 1;

#[inline]
pub(super) fn no_work_found(
    &self,
    idle_state: &mut IdleState,
    latch: &CoreLatch,
    has_injected_jobs: impl FnOnce() -> bool,
) {
    if idle_state.rounds < ROUNDS_UNTIL_SLEEPY {
        thread::yield_now();
        idle_state.rounds += 1;
    } else if idle_state.rounds == ROUNDS_UNTIL_SLEEPY {
        idle_state.jobs_counter = self.announce_sleepy();
        idle_state.rounds += 1;
        thread::yield_now();
    } else if idle_state.rounds < ROUNDS_UNTIL_SLEEPING {
        idle_state.rounds += 1;
        thread::yield_now();
    } else {
        debug_assert_eq!(idle_state.rounds, ROUNDS_UNTIL_SLEEPING);
        self.sleep(idle_state, latch, has_injected_jobs);
    }
}

```



```rust

use std::future::Future;
use std::panic::catch_unwind;
use std::thread;

use async_executor::{Executor, Task};
use async_io::block_on;
use async_lock::OnceCell;
use futures_lite::future;

/// Spawns a task onto the global executor (single-threaded by default).
///
/// There is a global executor that gets lazily initialized on first use. It is included in this
/// library for convenience when writing unit tests and small programs, but it is otherwise
/// more advisable to create your own [`Executor`].
///
/// By default, the global executor is run by a single background thread, but you can also
/// configure the number of threads by setting the `SMOL_THREADS` environment variable.
///
/// Since the executor is kept around forever, `drop` is not called for tasks when the program
/// exits.
///
/// # Examples
///
/// ``
/// let task = smol::spawn(async {
///     1 + 2
/// });
///
/// smol::block_on(async {
///     assert_eq!(task.await, 3);
/// });
/// ``
pub fn spawn<T: Send + 'static>(future: impl Future<Output = T> + Send + 'static) -> Task<T> {
    static GLOBAL: OnceCell<Executor<'_>> = OnceCell::new();

    fn global() -> &'static Executor<'static> {
        GLOBAL.get_or_init_blocking(|| {
            let num_threads = {
                // Parse SMOL_THREADS or default to 1.
                std::env::var("SMOL_THREADS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1)
            };

            for n in 1..=num_threads {
                thread::Builder::new()
                    .name(format!("smol-{}", n))
                    .spawn(|| loop {
                        catch_unwind(|| block_on(global().run(future::pending::<()>()))).ok();
                    })
                    .expect("cannot spawn executor thread");
            }

            // Prevent spawning another thread by running the process driver on this thread.
            let ex = Executor::new();
            #[cfg(not(target_os = "espidf"))]
            ex.spawn(async_process::driver()).detach();
            ex
        })
    }

    global().spawn(future)
}

/// Number of currently active `block_on()` invocations.
static BLOCK_ON_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Unparker for the "async-io" thread.
fn unparker() -> &'static parking::Unparker {
    static UNPARKER: OnceCell<parking::Unparker> = OnceCell::new();

    UNPARKER.get_or_init_blocking(|| {
        let (parker, unparker) = parking::pair();

        // Spawn a helper thread driving the reactor.
        //
        // Note that this thread is not exactly necessary, it's only here to help push things
        // forward if there are no `Parker`s around or if `Parker`s are just idling and never
        // parking.
        thread::Builder::new()
            .name("async-io".to_string())
            .spawn(move || main_loop(parker))
            .expect("cannot spawn async-io thread");

        unparker
    })
}


A thread registry

```rust
pub(super) struct Registry {
    thread_infos: Vec<ThreadInfo>,
    sleep: Sleep,
    injected_jobs: Injector<JobRef>,
    broadcasts: Mutex<Vec<Worker<JobRef>>>,
    panic_handler: Option<Box<PanicHandler>>,
    start_handler: Option<Box<StartHandler>>,
    exit_handler: Option<Box<ExitHandler>>,

    // When this latch reaches 0, it means that all work on this
    // registry must be complete. This is ensured in the following ways:
    //
    // - if this is the global registry, there is a ref-count that never
    //   gets released.
    // - if this is a user-created thread-pool, then so long as the thread-pool
    //   exists, it holds a reference.
    // - when we inject a "blocking job" into the registry with `ThreadPool::install()`,
    //   no adjustment is needed; the `ThreadPool` holds the reference, and since we won't
    //   return until the blocking job is complete, that ref will continue to be held.
    // - when `join()` or `scope()` is invoked, similarly, no adjustments are needed.
    //   These are always owned by some other job (e.g., one injected by `ThreadPool::install()`)
    //   and that job will keep the pool alive.
    terminate_count: AtomicUsize,
}

/// //////////////

/// ////////////////////////////////////////////////////////////////////////
/// Initialization

static mut THE_REGISTRY: Option<Arc<Registry>> = None;
static THE_REGISTRY_SET: Once = Once::new();

/// Starts the worker threads (if that has not already happened). If
/// initialization has not already occurred, use the default
/// configuration.
pub(super) fn global_registry() -> &'static Arc<Registry> {
    set_global_registry(default_global_registry)
        .or_else(|err| unsafe { THE_REGISTRY.as_ref().ok_or(err) })
        .expect("The global thread pool has not been initialized.")
}

/// Starts the worker threads (if that has not already happened) with
/// the given builder.
pub(super) fn init_global_registry<S>(
    builder: ThreadPoolBuilder<S>,
) -> Result<&'static Arc<Registry>, ThreadPoolBuildError>
where
    S: ThreadSpawn,
{
    set_global_registry(|| Registry::new(builder))
}

/// Starts the worker threads (if that has not already happened)
/// by creating a registry with the given callback.
fn set_global_registry<F>(registry: F) -> Result<&'static Arc<Registry>, ThreadPoolBuildError>
where
    F: FnOnce() -> Result<Arc<Registry>, ThreadPoolBuildError>,
{
    let mut result = Err(ThreadPoolBuildError::new(
        ErrorKind::GlobalPoolAlreadyInitialized,
    ));

    THE_REGISTRY_SET.call_once(|| {
        result = registry()
            .map(|registry: Arc<Registry>| unsafe { &*THE_REGISTRY.get_or_insert(registry) })
    });

    result
}
