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
#[cfg(not(feature = "web_spin_lock"))]
use std::sync;

#[cfg(feature = "web_spin_lock")]
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

/// Get the number of threads that will be used for the thread
/// pool. See `num_threads()` for more information.
fn get_num_threads(&self) -> usize {
    if self.num_threads > 0 {
        self.num_threads
    } else {
        let default = || {
            thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
        };

        match env::var("RAYON_NUM_THREADS")
            .ok()
            .and_then(|s| usize::from_str(&s).ok())
        {
            Some(x @ 1..) => return x,
            Some(0) => return default(),
            _ => {}
        }

        // Support for deprecated `RAYON_RS_NUM_CPUS`.
        match env::var("RAYON_RS_NUM_CPUS")
            .ok()
            .and_then(|s| usize::from_str(&s).ok())
        {
            Some(x @ 1..) => x,
            _ => default(),
        }
    }
}

```

```Rust
//! Package up unwind recovery. Note that if you are in some sensitive
//! place, you can use the `AbortIfPanic` helper to protect against
//! accidental panics in the rayon code itself.

use std::any::Any;
use std::panic::{self, AssertUnwindSafe};
use std::thread;

/// Executes `f` and captures any panic, translating that panic into a
/// `Err` result. The assumption is that any panic will be propagated
/// later with `resume_unwinding`, and hence `f` can be treated as
/// exception safe.
pub(super) fn halt_unwinding<F, R>(func: F) -> thread::Result<R>
where
    F: FnOnce() -> R,
{
    panic::catch_unwind(AssertUnwindSafe(func))
}

pub(super) fn resume_unwinding(payload: Box<dyn Any + Send>) -> ! {
    panic::resume_unwind(payload)
}

pub(super) struct AbortIfPanic;

impl Drop for AbortIfPanic {
    fn drop(&mut self) {
        eprintln!("Rayon: detected unexpected panic; aborting");
        ::std::process::abort();
    }
}

```

```Rust
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

/// Contains the rayon thread pool configuration. Use [`ThreadPoolBuilder`] instead.
///
/// [`ThreadPoolBuilder`]: struct.ThreadPoolBuilder.html
#[deprecated(note = "Use `ThreadPoolBuilder`")]
#[derive(Default)]
pub struct Configuration {
    builder: ThreadPoolBuilder,
}

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

// NB: We can't `#[derive(Default)]` because `S` is left ambiguous.
impl Default for ThreadPoolBuilder {
    fn default() -> Self {
        ThreadPoolBuilder {
            num_threads: 0,
            use_current_thread: false,
            panic_handler: None,
            get_thread_name: None,
            stack_size: None,
            start_handler: None,
            exit_handler: None,
            spawn_handler: DefaultSpawn,
            breadth_first: false,
        }
    }
}

```

```rust

/// Sets a custom function for spawning threads.
///
/// Note that the threads will not exit until after the pool is dropped. It
/// is up to the caller to wait for thread termination if that is important
/// for any invariants. For instance, threads created in [`std::thread::scope`]
/// will be joined before that scope returns, and this will block indefinitely
/// if the pool is leaked. Furthermore, the global thread pool doesn't terminate
/// until the entire process exits!
///
/// # Examples
///
/// A minimal spawn handler just needs to call `run()` from an independent thread.
///
/// `
/// # use rayon_core as rayon;
/// fn main() -> Result<(), rayon::ThreadPoolBuildError> {
///     let pool = rayon::ThreadPoolBuilder::new()
///         .spawn_handler(|thread| {
///             std::thread::spawn(|| thread.run());
///             Ok(())
///         })
///         .build()?;
///
///     pool.install(|| println!("Hello from my custom thread!"));
///     Ok(())
/// }
/// `
///
/// The default spawn handler sets the name and stack size if given, and propagates
/// any errors from the thread builder.
///
/// `
/// # use rayon_core as rayon;
/// fn main() -> Result<(), rayon::ThreadPoolBuildError> {
///     let pool = rayon::ThreadPoolBuilder::new()
///         .spawn_handler(|thread| {
///             let mut b = std::thread::Builder::new();
///             if let Some(name) = thread.name() {
///                 b = b.name(name.to_owned());
///             }
///             if let Some(stack_size) = thread.stack_size() {
///                 b = b.stack_size(stack_size);
///             }
///             b.spawn(|| thread.run())?;
///             Ok(())
///         })
///         .build()?;
///
///     pool.install(|| println!("Hello from my fully custom thread!"));
///     Ok(())
/// }
/// `
///
/// This can also be used for a pool of scoped threads like [`crossbeam::scope`],
/// or [`std::thread::scope`] introduced in Rust 1.63, which is encapsulated in
/// [`build_scoped`](#method.build_scoped).
///
/// [`crossbeam::scope`]: https://docs.rs/crossbeam/0.8/crossbeam/fn.scope.html
/// [`std::thread::scope`]: https://doc.rust-lang.org/std/thread/fn.scope.html
///
/// `
/// # use rayon_core as rayon;
/// fn main() -> Result<(), rayon::ThreadPoolBuildError> {
///     std::thread::scope(|scope| {
///         let pool = rayon::ThreadPoolBuilder::new()
///             .spawn_handler(|thread| {
///                 let mut builder = std::thread::Builder::new();
///                 if let Some(name) = thread.name() {
///                     builder = builder.name(name.to_string());
///                 }
///                 if let Some(size) = thread.stack_size() {
///                     builder = builder.stack_size(size);
///                 }
///                 builder.spawn_scoped(scope, || {
///                     // Add any scoped initialization here, then run!
///                     thread.run()
///                 })?;
///                 Ok(())
///             })
///             .build()?;
///
///         pool.install(|| println!("Hello from my custom scoped thread!"));
///         Ok(())
///     })
/// }
/// `
pub fn spawn_handler<F>(self, spawn: F) -> ThreadPoolBuilder<CustomSpawn<F>>
where
    F: FnMut(ThreadBuilder) -> io::Result<()>,
{
    ThreadPoolBuilder {
        spawn_handler: CustomSpawn::new(spawn),
        // ..self
        num_threads: self.num_threads,
        use_current_thread: self.use_current_thread,
        panic_handler: self.panic_handler,
        get_thread_name: self.get_thread_name,
        stack_size: self.stack_size,
        start_handler: self.start_handler,
        exit_handler: self.exit_handler,
        breadth_first: self.breadth_first,
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

```

```rust
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::Waker;
use std::task::{Context, Poll};
use std::thread;
use std::time::{Duration, Instant};

use async_lock::OnceCell;
use futures_lite::pin;
use parking::Parker;

use crate::reactor::Reactor;

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

/// Initializes the "async-io" thread.
pub(crate) fn init() {
    let _ = unparker();
}

/// The main loop for the "async-io" thread.
fn main_loop(parker: parking::Parker) {
    let span = tracing::trace_span!("async_io::main_loop");
    let _enter = span.enter();

    // The last observed reactor tick.
    let mut last_tick = 0;
    // Number of sleeps since this thread has called `react()`.
    let mut sleeps = 0u64;

    loop {
        let tick = Reactor::get().ticker();

        if last_tick == tick {
            let reactor_lock = if sleeps >= 10 {
                // If no new ticks have occurred for a while, stop sleeping and spinning in
                // this loop and just block on the reactor lock.
                Some(Reactor::get().lock())
            } else {
                Reactor::get().try_lock()
            };

            if let Some(mut reactor_lock) = reactor_lock {
                tracing::trace!("waiting on I/O");
                reactor_lock.react(None).ok();
                last_tick = Reactor::get().ticker();
                sleeps = 0;
            }
        } else {
            last_tick = tick;
        }

        if BLOCK_ON_COUNT.load(Ordering::SeqCst) > 0 {
            // Exponential backoff from 50us to 10ms.
            let delay_us = [50, 75, 100, 250, 500, 750, 1000, 2500, 5000]
                .get(sleeps as usize)
                .unwrap_or(&10_000);

            tracing::trace!("sleeping for {} us", delay_us);
            if parker.park_timeout(Duration::from_micros(*delay_us)) {
                tracing::trace!("notified");

                // If notified before timeout, reset the last tick and the sleep counter.
                last_tick = Reactor::get().ticker();
                sleeps = 0;
            } else {
                sleeps += 1;
            }
        }
    }
}

/// Blocks the current thread on a future, processing I/O events when idle.
///
/// # Examples
///
/// ``
/// use async_io::Timer;
/// use std::time::Duration;
///
/// async_io::block_on(async {
///     // This timer will likely be processed by the current
///     // thread rather than the fallback "async-io" thread.
///     Timer::after(Duration::from_millis(1)).await;
/// });
/// ``
pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    let span = tracing::trace_span!("async_io::block_on");
    let _enter = span.enter();

    // Increment `BLOCK_ON_COUNT` so that the "async-io" thread becomes less aggressive.
    BLOCK_ON_COUNT.fetch_add(1, Ordering::SeqCst);

    // Make sure to decrement `BLOCK_ON_COUNT` at the end and wake the "async-io" thread.
    let _guard = CallOnDrop(|| {
        BLOCK_ON_COUNT.fetch_sub(1, Ordering::SeqCst);
        unparker().unpark();
    });

    // Creates a parker and an associated waker that unparks it.
    fn parker_and_waker() -> (Parker, Waker, Arc<AtomicBool>) {
        // Parker and unparker for notifying the current thread.
        let (p, u) = parking::pair();

        // This boolean is set to `true` when the current thread is blocked on I/O.
        let io_blocked = Arc::new(AtomicBool::new(false));

        // Prepare the waker.
        let waker = BlockOnWaker::create(io_blocked.clone(), u);

        (p, waker, io_blocked)
    }

    thread_local! {
        // Cached parker and waker for efficiency.
        static CACHE: RefCell<(Parker, Waker, Arc<AtomicBool>)> = RefCell::new(parker_and_waker());

        // Indicates that the current thread is polling I/O, but not necessarily blocked on it.
        static IO_POLLING: Cell<bool> = const { Cell::new(false) };
    }

    struct BlockOnWaker {
        io_blocked: Arc<AtomicBool>,
        unparker: parking::Unparker,
    }

    impl BlockOnWaker {
        fn create(io_blocked: Arc<AtomicBool>, unparker: parking::Unparker) -> Waker {
            Waker::from(Arc::new(BlockOnWaker {
                io_blocked,
                unparker,
            }))
        }
    }

    impl std::task::Wake for BlockOnWaker {
        fn wake_by_ref(self: &Arc<Self>) {
            if self.unparker.unpark() {
                // Check if waking from another thread and if currently blocked on I/O.
                if !IO_POLLING.with(Cell::get) && self.io_blocked.load(Ordering::SeqCst) {
                    Reactor::get().notify();
                }
            }
        }

        fn wake(self: Arc<Self>) {
            self.wake_by_ref()
        }
    }

    CACHE.with(|cache| {
        // Try grabbing the cached parker and waker.
        let tmp_cached;
        let tmp_fresh;
        let (p, waker, io_blocked) = match cache.try_borrow_mut() {
            Ok(cache) => {
                // Use the cached parker and waker.
                tmp_cached = cache;
                &*tmp_cached
            }
            Err(_) => {
                // Looks like this is a recursive `block_on()` call.
                // Create a fresh parker and waker.
                tmp_fresh = parker_and_waker();
                &tmp_fresh
            }
        };

        pin!(future);

        let cx = &mut Context::from_waker(waker);

        loop {
            // Poll the future.
            if let Poll::Ready(t) = future.as_mut().poll(cx) {
                // Ensure the cached parker is reset to the unnotified state for future block_on calls,
                // in case this future called wake and then immediately returned Poll::Ready.
                p.park_timeout(Duration::from_secs(0));
                tracing::trace!("completed");
                return t;
            }

            // Check if a notification was received.
            if p.park_timeout(Duration::from_secs(0)) {
                tracing::trace!("notified");

                // Try grabbing a lock on the reactor to process I/O events.
                if let Some(mut reactor_lock) = Reactor::get().try_lock() {
                    // First let wakers know this parker is processing I/O events.
                    IO_POLLING.with(|io| io.set(true));
                    let _guard = CallOnDrop(|| {
                        IO_POLLING.with(|io| io.set(false));
                    });

                    // Process available I/O events.
                    reactor_lock.react(Some(Duration::from_secs(0))).ok();
                }
                continue;
            }

            // Try grabbing a lock on the reactor to wait on I/O.
            if let Some(mut reactor_lock) = Reactor::get().try_lock() {
                // Record the instant at which the lock was grabbed.
                let start = Instant::now();

                loop {
                    // First let wakers know this parker is blocked on I/O.
                    IO_POLLING.with(|io| io.set(true));
                    io_blocked.store(true, Ordering::SeqCst);
                    let _guard = CallOnDrop(|| {
                        IO_POLLING.with(|io| io.set(false));
                        io_blocked.store(false, Ordering::SeqCst);
                    });

                    // Check if a notification has been received before `io_blocked` was updated
                    // because in that case the reactor won't receive a wakeup.
                    if p.park_timeout(Duration::from_secs(0)) {
                        tracing::trace!("notified");
                        break;
                    }

                    // Wait for I/O events.
                    tracing::trace!("waiting on I/O");
                    reactor_lock.react(None).ok();

                    // Check if a notification has been received.
                    if p.park_timeout(Duration::from_secs(0)) {
                        tracing::trace!("notified");
                        break;
                    }

                    // Check if this thread been handling I/O events for a long time.
                    if start.elapsed() > Duration::from_micros(500) {
                        tracing::trace!("stops hogging the reactor");

                        // This thread is clearly processing I/O events for some other threads
                        // because it didn't get a notification yet. It's best to stop hogging the
                        // reactor and give other threads a chance to process I/O events for
                        // themselves.
                        drop(reactor_lock);

                        // Unpark the "async-io" thread in case no other thread is ready to start
                        // processing I/O events. This way we prevent a potential latency spike.
                        unparker().unpark();

                        // Wait for a notification.
                        p.park();
                        break;
                    }
                }
            } else {
                // Wait for an actual notification.
                tracing::trace!("sleep until notification");
                p.park();
            }
        }
    })
}

/// Runs a closure when dropped.
struct CallOnDrop<F: Fn()>(F);

impl<F: Fn()> Drop for CallOnDrop<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}
```


```rust

use crate::job::{JobFifo, JobRef, StackJob};
use crate::latch::{AsCoreLatch, CoreLatch, Latch, LatchRef, LockLatch, OnceLatch, SpinLatch};
use crate::sleep::Sleep;
use crate::sync::Mutex;
use crate::unwind;
use crate::{
    ErrorKind, ExitHandler, PanicHandler, StartHandler, ThreadPoolBuildError, ThreadPoolBuilder,
    Yield,
};
use crossbeam_deque::{Injector, Steal, Stealer, Worker};
use std::cell::Cell;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::Hasher;
use std::io;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Once};
use std::thread;

/// Thread builder used for customization via
/// [`ThreadPoolBuilder::spawn_handler`](struct.ThreadPoolBuilder.html#method.spawn_handler).
pub struct ThreadBuilder {
    name: Option<String>,
    stack_size: Option<usize>,
    worker: Worker<JobRef>,
    stealer: Stealer<JobRef>,
    registry: Arc<Registry>,
    index: usize,
}

impl ThreadBuilder {
    /// Gets the index of this thread in the pool, within `0..num_threads`.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Gets the string that was specified by `ThreadPoolBuilder::name()`.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Gets the value that was specified by `ThreadPoolBuilder::stack_size()`.
    pub fn stack_size(&self) -> Option<usize> {
        self.stack_size
    }

    /// Executes the main loop for this thread. This will not return until the
    /// thread pool is dropped.
    pub fn run(self) {
        unsafe { main_loop(self) }
    }
}

impl fmt::Debug for ThreadBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThreadBuilder")
            .field("pool", &self.registry.id())
            .field("index", &self.index)
            .field("name", &self.name)
            .field("stack_size", &self.stack_size)
            .finish()
    }
}

/// Generalized trait for spawning a thread in the `Registry`.
///
/// This trait is pub-in-private -- E0445 forces us to make it public,
/// but we don't actually want to expose these details in the API.
pub trait ThreadSpawn {
    private_decl! {}

    /// Spawn a thread with the `ThreadBuilder` parameters, and then
    /// call `ThreadBuilder::run()`.
    fn spawn(&mut self, thread: ThreadBuilder) -> io::Result<()>;
}

/// Spawns a thread in the "normal" way with `std::thread::Builder`.
///
/// This type is pub-in-private -- E0445 forces us to make it public,
/// but we don't actually want to expose these details in the API.
#[derive(Debug, Default)]
pub struct DefaultSpawn;

impl ThreadSpawn for DefaultSpawn {
    private_impl! {}

    fn spawn(&mut self, thread: ThreadBuilder) -> io::Result<()> {
        let mut b = thread::Builder::new();
        if let Some(name) = thread.name() {
            b = b.name(name.to_owned());
        }
        if let Some(stack_size) = thread.stack_size() {
            b = b.stack_size(stack_size);
        }
        b.spawn(|| thread.run())?;
        Ok(())
    }
}

/// Spawns a thread with a user's custom callback.
///
/// This type is pub-in-private -- E0445 forces us to make it public,
/// but we don't actually want to expose these details in the API.
#[derive(Debug)]
pub struct CustomSpawn<F>(F);

impl<F> CustomSpawn<F>
where
    F: FnMut(ThreadBuilder) -> io::Result<()>,
{
    pub(super) fn new(spawn: F) -> Self {
        CustomSpawn(spawn)
    }
}

impl<F> ThreadSpawn for CustomSpawn<F>
where
    F: FnMut(ThreadBuilder) -> io::Result<()>,
{
    private_impl! {}

    #[inline]
    fn spawn(&mut self, thread: ThreadBuilder) -> io::Result<()> {
        (self.0)(thread)
    }
}

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

fn default_global_registry() -> Result<Arc<Registry>, ThreadPoolBuildError> {
    let result = Registry::new(ThreadPoolBuilder::new());

    // If we're running in an environment that doesn't support threads at all, we can fall back to
    // using the current thread alone. This is crude, and probably won't work for non-blocking
    // calls like `spawn` or `broadcast_spawn`, but a lot of stuff does work fine.
    //
    // Notably, this allows current WebAssembly targets to work even though their threading support
    // is stubbed out, and we won't have to change anything if they do add real threading.
    let unsupported = matches!(&result, Err(e) if e.is_unsupported());
    if unsupported && WorkerThread::current().is_null() {
        let builder = ThreadPoolBuilder::new().num_threads(1).use_current_thread();
        let fallback_result = Registry::new(builder);
        if fallback_result.is_ok() {
            return fallback_result;
        }
    }

    result
}

struct Terminator<'a>(&'a Arc<Registry>);

impl<'a> Drop for Terminator<'a> {
    fn drop(&mut self) {
        self.0.terminate()
    }
}

impl Registry {
    pub(super) fn new<S>(
        mut builder: ThreadPoolBuilder<S>,
    ) -> Result<Arc<Self>, ThreadPoolBuildError>
    where
        S: ThreadSpawn,
    {
        // Soft-limit the number of threads that we can actually support.
        let n_threads = Ord::min(builder.get_num_threads(), crate::max_num_threads());

        let breadth_first = builder.get_breadth_first();

        let (workers, stealers): (Vec<_>, Vec<_>) = (0..n_threads)
            .map(|_| {
                let worker = if breadth_first {
                    Worker::new_fifo()
                } else {
                    Worker::new_lifo()
                };

                let stealer = worker.stealer();
                (worker, stealer)
            })
            .unzip();

        let (broadcasts, broadcast_stealers): (Vec<_>, Vec<_>) = (0..n_threads)
            .map(|_| {
                let worker = Worker::new_fifo();
                let stealer = worker.stealer();
                (worker, stealer)
            })
            .unzip();

        let registry = Arc::new(Registry {
            thread_infos: stealers.into_iter().map(ThreadInfo::new).collect(),
            sleep: Sleep::new(n_threads),
            injected_jobs: Injector::new(),
            broadcasts: Mutex::new(broadcasts),
            terminate_count: AtomicUsize::new(1),
            panic_handler: builder.take_panic_handler(),
            start_handler: builder.take_start_handler(),
            exit_handler: builder.take_exit_handler(),
        });

        // If we return early or panic, make sure to terminate existing threads.
        let t1000 = Terminator(&registry);

        for (index, (worker, stealer)) in workers.into_iter().zip(broadcast_stealers).enumerate() {
            let thread = ThreadBuilder {
                name: builder.get_thread_name(index),
                stack_size: builder.get_stack_size(),
                registry: Arc::clone(&registry),
                worker,
                stealer,
                index,
            };

            if index == 0 && builder.use_current_thread {
                if !WorkerThread::current().is_null() {
                    return Err(ThreadPoolBuildError::new(
                        ErrorKind::CurrentThreadAlreadyInPool,
                    ));
                }
                // Rather than starting a new thread, we're just taking over the current thread
                // *without* running the main loop, so we can still return from here.
                // The WorkerThread is leaked, but we never shutdown the global pool anyway.
                let worker_thread = Box::into_raw(Box::new(WorkerThread::from(thread)));

                unsafe {
                    WorkerThread::set_current(worker_thread);
                    Latch::set(&registry.thread_infos[index].primed);
                }
                continue;
            }

            if let Err(e) = builder.get_spawn_handler().spawn(thread) {
                return Err(ThreadPoolBuildError::new(ErrorKind::IOError(e)));
            }
        }

        // Returning normally now, without termination.
        mem::forget(t1000);

        Ok(registry)
    }

    pub(super) fn current() -> Arc<Registry> {
        unsafe {
            let worker_thread = WorkerThread::current();
            let registry = if worker_thread.is_null() {
                global_registry()
            } else {
                &(*worker_thread).registry
            };
            Arc::clone(registry)
        }
    }

    /// Returns the number of threads in the current registry.  This
    /// is better than `Registry::current().num_threads()` because it
    /// avoids incrementing the `Arc`.
    pub(super) fn current_num_threads() -> usize {
        unsafe {
            let worker_thread = WorkerThread::current();
            if worker_thread.is_null() {
                global_registry().num_threads()
            } else {
                (*worker_thread).registry.num_threads()
            }
        }
    }

    /// Returns the current `WorkerThread` if it's part of this `Registry`.
    pub(super) fn current_thread(&self) -> Option<&WorkerThread> {
        unsafe {
            let worker = WorkerThread::current().as_ref()?;
            if worker.registry().id() == self.id() {
                Some(worker)
            } else {
                None
            }
        }
    }

    /// Returns an opaque identifier for this registry.
    pub(super) fn id(&self) -> RegistryId {
        // We can rely on `self` not to change since we only ever create
        // registries that are boxed up in an `Arc` (see `new()` above).
        RegistryId {
            addr: self as *const Self as usize,
        }
    }

    pub(super) fn num_threads(&self) -> usize {
        self.thread_infos.len()
    }

    pub(super) fn catch_unwind(&self, f: impl FnOnce()) {
        if let Err(err) = unwind::halt_unwinding(f) {
            // If there is no handler, or if that handler itself panics, then we abort.
            let abort_guard = unwind::AbortIfPanic;
            if let Some(ref handler) = self.panic_handler {
                handler(err);
                mem::forget(abort_guard);
            }
        }
    }

    /// Waits for the worker threads to get up and running.  This is
    /// meant to be used for benchmarking purposes, primarily, so that
    /// you can get more consistent numbers by having everything
    /// "ready to go".
    pub(super) fn wait_until_primed(&self) {
        for info in &self.thread_infos {
            info.primed.wait();
        }
    }

    /// Waits for the worker threads to stop. This is used for testing
    /// -- so we can check that termination actually works.
    #[cfg(test)]
    pub(super) fn wait_until_stopped(&self) {
        for info in &self.thread_infos {
            info.stopped.wait();
        }
    }

    /// ////////////////////////////////////////////////////////////////////////
    /// MAIN LOOP
    ///
    /// So long as all of the worker threads are hanging out in their
    /// top-level loop, there is no work to be done.

    /// Push a job into the given `registry`. If we are running on a
    /// worker thread for the registry, this will push onto the
    /// deque. Else, it will inject from the outside (which is slower).
    pub(super) fn inject_or_push(&self, job_ref: JobRef) {
        let worker_thread = WorkerThread::current();
        unsafe {
            if !worker_thread.is_null() && (*worker_thread).registry().id() == self.id() {
                (*worker_thread).push(job_ref);
            } else {
                self.inject(job_ref);
            }
        }
    }

    /// Push a job into the "external jobs" queue; it will be taken by
    /// whatever worker has nothing to do. Use this if you know that
    /// you are not on a worker of this registry.
    pub(super) fn inject(&self, injected_job: JobRef) {
        // It should not be possible for `state.terminate` to be true
        // here. It is only set to true when the user creates (and
        // drops) a `ThreadPool`; and, in that case, they cannot be
        // calling `inject()` later, since they dropped their
        // `ThreadPool`.
        debug_assert_ne!(
            self.terminate_count.load(Ordering::Acquire),
            0,
            "inject() sees state.terminate as true"
        );

        let queue_was_empty = self.injected_jobs.is_empty();

        self.injected_jobs.push(injected_job);
        self.sleep.new_injected_jobs(1, queue_was_empty);
    }

    fn has_injected_job(&self) -> bool {
        !self.injected_jobs.is_empty()
    }

    fn pop_injected_job(&self) -> Option<JobRef> {
        loop {
            match self.injected_jobs.steal() {
                Steal::Success(job) => return Some(job),
                Steal::Empty => return None,
                Steal::Retry => {}
            }
        }
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

    #[cold]
    unsafe fn in_worker_cross<OP, R>(&self, current_thread: &WorkerThread, op: OP) -> R
    where
        OP: FnOnce(&WorkerThread, bool) -> R + Send,
        R: Send,
    {
        // This thread is a member of a different pool, so let it process
        // other work while waiting for this `op` to complete.
        debug_assert!(current_thread.registry().id() != self.id());
        let latch = SpinLatch::cross(current_thread);
        let job = StackJob::new(
            |injected| {
                let worker_thread = WorkerThread::current();
                assert!(injected && !worker_thread.is_null());
                op(&*worker_thread, true)
            },
            latch,
        );
        self.inject(job.as_job_ref());
        current_thread.wait_until(&job.latch);
        job.into_result()
    }

    /// Increments the terminate counter. This increment should be
    /// balanced by a call to `terminate`, which will decrement. This
    /// is used when spawning asynchronous work, which needs to
    /// prevent the registry from terminating so long as it is active.
    ///
    /// Note that blocking functions such as `join` and `scope` do not
    /// need to concern themselves with this fn; their context is
    /// responsible for ensuring the current thread-pool will not
    /// terminate until they return.
    ///
    /// The global thread-pool always has an outstanding reference
    /// (the initial one). Custom thread-pools have one outstanding
    /// reference that is dropped when the `ThreadPool` is dropped:
    /// since installing the thread-pool blocks until any joins/scopes
    /// complete, this ensures that joins/scopes are covered.
    ///
    /// The exception is `::spawn()`, which can create a job outside
    /// of any blocking scope. In that case, the job itself holds a
    /// terminate count and is responsible for invoking `terminate()`
    /// when finished.
    pub(super) fn increment_terminate_count(&self) {
        let previous = self.terminate_count.fetch_add(1, Ordering::AcqRel);
        debug_assert!(previous != 0, "registry ref count incremented from zero");
        assert!(previous != usize::MAX, "overflow in registry ref count");
    }

    /// Signals that the thread-pool which owns this registry has been
    /// dropped. The worker threads will gradually terminate, once any
    /// extant work is completed.
    pub(super) fn terminate(&self) {
        if self.terminate_count.fetch_sub(1, Ordering::AcqRel) == 1 {
            for (i, thread_info) in self.thread_infos.iter().enumerate() {
                unsafe { OnceLatch::set_and_tickle_one(&thread_info.terminate, self, i) };
            }
        }
    }

    /// Notify the worker that the latch they are sleeping on has been "set".
    pub(super) fn notify_worker_latch_is_set(&self, target_worker_index: usize) {
        self.sleep.notify_worker_latch_is_set(target_worker_index);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RegistryId {
    addr: usize,
}

struct ThreadInfo {
    /// Latch set once thread has started and we are entering into the
    /// main loop. Used to wait for worker threads to become primed,
    /// primarily of interest for benchmarking.
    primed: LockLatch,

    /// Latch is set once worker thread has completed. Used to wait
    /// until workers have stopped; only used for tests.
    stopped: LockLatch,

    /// The latch used to signal that terminated has been requested.
    /// This latch is *set* by the `terminate` method on the
    /// `Registry`, once the registry's main "terminate" counter
    /// reaches zero.
    terminate: OnceLatch,

    /// the "stealer" half of the worker's deque
    stealer: Stealer<JobRef>,
}

impl ThreadInfo {
    fn new(stealer: Stealer<JobRef>) -> ThreadInfo {
        ThreadInfo {
            primed: LockLatch::new(),
            stopped: LockLatch::new(),
            terminate: OnceLatch::new(),
            stealer,
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////
/// WorkerThread identifiers

pub(super) struct WorkerThread {
    /// the "worker" half of our local deque
    worker: Worker<JobRef>,

    /// the "stealer" half of the worker's broadcast deque
    stealer: Stealer<JobRef>,

    /// local queue used for `spawn_fifo` indirection
    fifo: JobFifo,

    index: usize,

    /// A weak random number generator.
    rng: XorShift64Star,

    registry: Arc<Registry>,
}

// This is a bit sketchy, but basically: the WorkerThread is
// allocated on the stack of the worker on entry and stored into this
// thread local variable. So it will remain valid at least until the
// worker is fully unwound. Using an unsafe pointer avoids the need
// for a RefCell<T> etc.
thread_local! {
    static WORKER_THREAD_STATE: Cell<*const WorkerThread> = const { Cell::new(ptr::null()) };
}

impl From<ThreadBuilder> for WorkerThread {
    fn from(thread: ThreadBuilder) -> Self {
        Self {
            worker: thread.worker,
            stealer: thread.stealer,
            fifo: JobFifo::new(),
            index: thread.index,
            rng: XorShift64Star::new(),
            registry: thread.registry,
        }
    }
}

impl Drop for WorkerThread {
    fn drop(&mut self) {
        // Undo `set_current`
        WORKER_THREAD_STATE.with(|t| {
            assert!(t.get().eq(&(self as *const _)));
            t.set(ptr::null());
        });
    }
}

impl WorkerThread {
    /// Gets the `WorkerThread` index for the current thread; returns
    /// NULL if this is not a worker thread. This pointer is valid
    /// anywhere on the current thread.
    #[inline]
    pub(super) fn current() -> *const WorkerThread {
        WORKER_THREAD_STATE.with(Cell::get)
    }

    /// Sets `self` as the worker thread index for the current thread.
    /// This is done during worker thread startup.
    unsafe fn set_current(thread: *const WorkerThread) {
        WORKER_THREAD_STATE.with(|t| {
            assert!(t.get().is_null());
            t.set(thread);
        });
    }

    /// Returns the registry that owns this worker thread.
    #[inline]
    pub(super) fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }

    /// Our index amongst the worker threads (ranges from `0..self.num_threads()`).
    #[inline]
    pub(super) fn index(&self) -> usize {
        self.index
    }

    #[inline]
    pub(super) unsafe fn push(&self, job: JobRef) {
        let queue_was_empty = self.worker.is_empty();
        self.worker.push(job);
        self.registry.sleep.new_internal_jobs(1, queue_was_empty);
    }

    #[inline]
    pub(super) unsafe fn push_fifo(&self, job: JobRef) {
        self.push(self.fifo.push(job));
    }

    #[inline]
    pub(super) fn local_deque_is_empty(&self) -> bool {
        self.worker.is_empty()
    }

    /// Attempts to obtain a "local" job -- typically this means
    /// popping from the top of the stack, though if we are configured
    /// for breadth-first execution, it would mean dequeuing from the
    /// bottom.
    #[inline]
    pub(super) fn take_local_job(&self) -> Option<JobRef> {
        let popped_job = self.worker.pop();

        if popped_job.is_some() {
            return popped_job;
        }

        loop {
            match self.stealer.steal() {
                Steal::Success(job) => return Some(job),
                Steal::Empty => return None,
                Steal::Retry => {}
            }
        }
    }

    fn has_injected_job(&self) -> bool {
        !self.stealer.is_empty() || self.registry.has_injected_job()
    }

    /// Wait until the latch is set. Try to keep busy by popping and
    /// stealing tasks as necessary.
    #[inline]
    pub(super) unsafe fn wait_until<L: AsCoreLatch + ?Sized>(&self, latch: &L) {
        let latch = latch.as_core_latch();
        if !latch.probe() {
            self.wait_until_cold(latch);
        }
    }

    #[cold]
    unsafe fn wait_until_cold(&self, latch: &CoreLatch) {
        // the code below should swallow all panics and hence never
        // unwind; but if something does wrong, we want to abort,
        // because otherwise other code in rayon may assume that the
        // latch has been signaled, and that can lead to random memory
        // accesses, which would be *very bad*
        let abort_guard = unwind::AbortIfPanic;

        'outer: while !latch.probe() {
            // Check for local work *before* we start marking ourself idle,
            // especially to avoid modifying shared sleep state.
            if let Some(job) = self.take_local_job() {
                self.execute(job);
                continue;
            }

            let mut idle_state = self.registry.sleep.start_looking(self.index);
            while !latch.probe() {
                if let Some(job) = self.find_work() {
                    self.registry.sleep.work_found();
                    self.execute(job);
                    // The job might have injected local work, so go back to the outer loop.
                    continue 'outer;
                } else {
                    self.registry
                        .sleep
                        .no_work_found(&mut idle_state, latch, || self.has_injected_job())
                }
            }

            // If we were sleepy, we are not anymore. We "found work" --
            // whatever the surrounding thread was doing before it had to wait.
            self.registry.sleep.work_found();
            break;
        }

        mem::forget(abort_guard); // successful execution, do not abort
    }

    unsafe fn wait_until_out_of_work(&self) {
        debug_assert_eq!(self as *const _, WorkerThread::current());
        let registry = &*self.registry;
        let index = self.index;

        self.wait_until(&registry.thread_infos[index].terminate);

        // Should not be any work left in our queue.
        debug_assert!(self.take_local_job().is_none());

        // Let registry know we are done
        Latch::set(&registry.thread_infos[index].stopped);
    }

    fn find_work(&self) -> Option<JobRef> {
        // Try to find some work to do. We give preference first
        // to things in our local deque, then in other workers
        // deques, and finally to injected jobs from the
        // outside. The idea is to finish what we started before
        // we take on something new.
        self.take_local_job()
            .or_else(|| self.steal())
            .or_else(|| self.registry.pop_injected_job())
    }

    pub(super) fn yield_now(&self) -> Yield {
        match self.find_work() {
            Some(job) => unsafe {
                self.execute(job);
                Yield::Executed
            },
            None => Yield::Idle,
        }
    }

    pub(super) fn yield_local(&self) -> Yield {
        match self.take_local_job() {
            Some(job) => unsafe {
                self.execute(job);
                Yield::Executed
            },
            None => Yield::Idle,
        }
    }

    #[inline]
    pub(super) unsafe fn execute(&self, job: JobRef) {
        job.execute();
    }

    /// Try to steal a single job and return it.
    ///
    /// This should only be done as a last resort, when there is no
    /// local work to do.
    fn steal(&self) -> Option<JobRef> {
        // we only steal when we don't have any work to do locally
        debug_assert!(self.local_deque_is_empty());

        // otherwise, try to steal
        let thread_infos = &self.registry.thread_infos.as_slice();
        let num_threads = thread_infos.len();
        if num_threads <= 1 {
            return None;
        }

        loop {
            let mut retry = false;
            let start = self.rng.next_usize(num_threads);
            let job = (start..num_threads)
                .chain(0..start)
                .filter(move |&i| i != self.index)
                .find_map(|victim_index| {
                    let victim = &thread_infos[victim_index];
                    match victim.stealer.steal() {
                        Steal::Success(job) => Some(job),
                        Steal::Empty => None,
                        Steal::Retry => {
                            retry = true;
                            None
                        }
                    }
                });
            if job.is_some() || !retry {
                return job;
            }
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////

unsafe fn main_loop(thread: ThreadBuilder) {
    let worker_thread = &WorkerThread::from(thread);
    WorkerThread::set_current(worker_thread);
    let registry = &*worker_thread.registry;
    let index = worker_thread.index;

    // let registry know we are ready to do work
    Latch::set(&registry.thread_infos[index].primed);

    // Worker threads should not panic. If they do, just abort, as the
    // internal state of the threadpool is corrupted. Note that if
    // **user code** panics, we should catch that and redirect.
    let abort_guard = unwind::AbortIfPanic;

    // Inform a user callback that we started a thread.
    if let Some(ref handler) = registry.start_handler {
        registry.catch_unwind(|| handler(index));
    }

    worker_thread.wait_until_out_of_work();

    // Normal termination, do not abort.
    mem::forget(abort_guard);

    // Inform a user callback that we exited a thread.
    if let Some(ref handler) = registry.exit_handler {
        registry.catch_unwind(|| handler(index));
        // We're already exiting the thread, there's nothing else to do.
    }
}

/// If already in a worker-thread, just execute `op`.  Otherwise,
/// execute `op` in the default thread-pool. Either way, block until
/// `op` completes and return its return value. If `op` panics, that
/// panic will be propagated as well.  The second argument indicates
/// `true` if injection was performed, `false` if executed directly.
pub(super) fn in_worker<OP, R>(op: OP) -> R
where
    OP: FnOnce(&WorkerThread, bool) -> R + Send,
    R: Send,
{
    unsafe {
        let owner_thread = WorkerThread::current();
        if !owner_thread.is_null() {
            // Perfectly valid to give them a `&T`: this is the
            // current thread, so we know the data structure won't be
            // invalidated until we return.
            op(&*owner_thread, false)
        } else {
            global_registry().in_worker(op)
        }
    }
}

/// [xorshift*] is a fast pseudorandom number generator which will
/// even tolerate weak seeding, as long as it's not zero.
///
/// [xorshift*]: https://en.wikipedia.org/wiki/Xorshift#xorshift*
struct XorShift64Star {
    state: Cell<u64>,
}

impl XorShift64Star {
    fn new() -> Self {
        // Any non-zero seed will do -- this uses the hash of a global counter.
        let mut seed = 0;
        while seed == 0 {
            let mut hasher = DefaultHasher::new();
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            hasher.write_usize(COUNTER.fetch_add(1, Ordering::Relaxed));
            seed = hasher.finish();
        }

        XorShift64Star {
            state: Cell::new(seed),
        }
    }

    fn next(&self) -> u64 {
        let mut x = self.state.get();
        debug_assert_ne!(x, 0);
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state.set(x);
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Return a value from `0..n`.
    fn next_usize(&self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}


```


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

fn default_global_registry() -> Result<Arc<Registry>, ThreadPoolBuildError> {
    let result = Registry::new(ThreadPoolBuilder::new());

    // If we're running in an environment that doesn't support threads at all, we can fall back to
    // using the current thread alone. This is crude, and probably won't work for non-blocking
    // calls like `spawn` or `broadcast_spawn`, but a lot of stuff does work fine.
    //
    // Notably, this allows current WebAssembly targets to work even though their threading support
    // is stubbed out, and we won't have to change anything if they do add real threading.
    let unsupported = matches!(&result, Err(e) if e.is_unsupported());
    if unsupported && WorkerThread::current().is_null() {
        let builder = ThreadPoolBuilder::new().num_threads(1).use_current_thread();
        let fallback_result = Registry::new(builder);
        if fallback_result.is_ok() {
            return fallback_result;
        }
    }

    result
}

struct Terminator<'a>(&'a Arc<Registry>);

impl<'a> Drop for Terminator<'a> {
    fn drop(&mut self) {
        self.0.terminate()
    }
}

impl Registry {
    pub(super) fn new<S>(
        mut builder: ThreadPoolBuilder<S>,
    ) -> Result<Arc<Self>, ThreadPoolBuildError>
    where
        S: ThreadSpawn,
    {
        // Soft-limit the number of threads that we can actually support.
        let n_threads = Ord::min(builder.get_num_threads(), crate::max_num_threads());

        let breadth_first = builder.get_breadth_first();

        let (workers, stealers): (Vec<_>, Vec<_>) = (0..n_threads)
            .map(|_| {
                let worker = if breadth_first {
                    Worker::new_fifo()
                } else {
                    Worker::new_lifo()
                };

                let stealer = worker.stealer();
                (worker, stealer)
            })
            .unzip();

        let (broadcasts, broadcast_stealers): (Vec<_>, Vec<_>) = (0..n_threads)
            .map(|_| {
                let worker = Worker::new_fifo();
                let stealer = worker.stealer();
                (worker, stealer)
            })
            .unzip();

        let registry = Arc::new(Registry {
            thread_infos: stealers.into_iter().map(ThreadInfo::new).collect(),
            sleep: Sleep::new(n_threads),
            injected_jobs: Injector::new(),
            broadcasts: Mutex::new(broadcasts),
            terminate_count: AtomicUsize::new(1),
            panic_handler: builder.take_panic_handler(),
            start_handler: builder.take_start_handler(),
            exit_handler: builder.take_exit_handler(),
        });

        // If we return early or panic, make sure to terminate existing threads.
        let t1000 = Terminator(&registry);

        for (index, (worker, stealer)) in workers.into_iter().zip(broadcast_stealers).enumerate() {
            let thread = ThreadBuilder {
                name: builder.get_thread_name(index),
                stack_size: builder.get_stack_size(),
                registry: Arc::clone(&registry),
                worker,
                stealer,
                index,
            };

            if index == 0 && builder.use_current_thread {
                if !WorkerThread::current().is_null() {
                    return Err(ThreadPoolBuildError::new(
                        ErrorKind::CurrentThreadAlreadyInPool,
                    ));
                }
                // Rather than starting a new thread, we're just taking over the current thread
                // *without* running the main loop, so we can still return from here.
                // The WorkerThread is leaked, but we never shutdown the global pool anyway.
                let worker_thread = Box::into_raw(Box::new(WorkerThread::from(thread)));

                unsafe {
                    WorkerThread::set_current(worker_thread);
                    Latch::set(&registry.thread_infos[index].primed);
                }
                continue;
            }

            if let Err(e) = builder.get_spawn_handler().spawn(thread) {
                return Err(ThreadPoolBuildError::new(ErrorKind::IOError(e)));
            }
        }

        // Returning normally now, without termination.
        mem::forget(t1000);

        Ok(registry)
    }

    pub(super) fn current() -> Arc<Registry> {
        unsafe {
            let worker_thread = WorkerThread::current();
            let registry = if worker_thread.is_null() {
                global_registry()
            } else {
                &(*worker_thread).registry
            };
            Arc::clone(registry)
        }
    }

    /// Returns the number of threads in the current registry.  This
    /// is better than `Registry::current().num_threads()` because it
    /// avoids incrementing the `Arc`.
    pub(super) fn current_num_threads() -> usize {
        unsafe {
            let worker_thread = WorkerThread::current();
            if worker_thread.is_null() {
                global_registry().num_threads()
            } else {
                (*worker_thread).registry.num_threads()
            }
        }
    }

    /// Returns the current `WorkerThread` if it's part of this `Registry`.
    pub(super) fn current_thread(&self) -> Option<&WorkerThread> {
        unsafe {
            let worker = WorkerThread::current().as_ref()?;
            if worker.registry().id() == self.id() {
                Some(worker)
            } else {
                None
            }
        }
    }

    /// Returns an opaque identifier for this registry.
    pub(super) fn id(&self) -> RegistryId {
        // We can rely on `self` not to change since we only ever create
        // registries that are boxed up in an `Arc` (see `new()` above).
        RegistryId {
            addr: self as *const Self as usize,
        }
    }

    pub(super) fn num_threads(&self) -> usize {
        self.thread_infos.len()
    }

    pub(super) fn catch_unwind(&self, f: impl FnOnce()) {
        if let Err(err) = unwind::halt_unwinding(f) {
            // If there is no handler, or if that handler itself panics, then we abort.
            let abort_guard = unwind::AbortIfPanic;
            if let Some(ref handler) = self.panic_handler {
                handler(err);
                mem::forget(abort_guard);
            }
        }
    }

    /// Waits for the worker threads to get up and running.  This is
    /// meant to be used for benchmarking purposes, primarily, so that
    /// you can get more consistent numbers by having everything
    /// "ready to go".
    pub(super) fn wait_until_primed(&self) {
        for info in &self.thread_infos {
            info.primed.wait();
        }
    }

    /// Waits for the worker threads to stop. This is used for testing
    /// -- so we can check that termination actually works.
    #[cfg(test)]
    pub(super) fn wait_until_stopped(&self) {
        for info in &self.thread_infos {
            info.stopped.wait();
        }
    }

    /// ////////////////////////////////////////////////////////////////////////
    /// MAIN LOOP
    ///
    /// So long as all of the worker threads are hanging out in their
    /// top-level loop, there is no work to be done.

    /// Push a job into the given `registry`. If we are running on a
    /// worker thread for the registry, this will push onto the
    /// deque. Else, it will inject from the outside (which is slower).
    pub(super) fn inject_or_push(&self, job_ref: JobRef) {
        let worker_thread = WorkerThread::current();
        unsafe {
            if !worker_thread.is_null() && (*worker_thread).registry().id() == self.id() {
                (*worker_thread).push(job_ref);
            } else {
                self.inject(job_ref);
            }
        }
    }

    /// Push a job into the "external jobs" queue; it will be taken by
    /// whatever worker has nothing to do. Use this if you know that
    /// you are not on a worker of this registry.
    pub(super) fn inject(&self, injected_job: JobRef) {
        // It should not be possible for `state.terminate` to be true
        // here. It is only set to true when the user creates (and
        // drops) a `ThreadPool`; and, in that case, they cannot be
        // calling `inject()` later, since they dropped their
        // `ThreadPool`.
        debug_assert_ne!(
            self.terminate_count.load(Ordering::Acquire),
            0,
            "inject() sees state.terminate as true"
        );

        let queue_was_empty = self.injected_jobs.is_empty();

        self.injected_jobs.push(injected_job);
        self.sleep.new_injected_jobs(1, queue_was_empty);
    }

    fn has_injected_job(&self) -> bool {
        !self.injected_jobs.is_empty()
    }

    fn pop_injected_job(&self) -> Option<JobRef> {
        loop {
            match self.injected_jobs.steal() {
                Steal::Success(job) => return Some(job),
                Steal::Empty => return None,
                Steal::Retry => {}
            }
        }
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

    #[cold]
    unsafe fn in_worker_cross<OP, R>(&self, current_thread: &WorkerThread, op: OP) -> R
    where
        OP: FnOnce(&WorkerThread, bool) -> R + Send,
        R: Send,
    {
        // This thread is a member of a different pool, so let it process
        // other work while waiting for this `op` to complete.
        debug_assert!(current_thread.registry().id() != self.id());
        let latch = SpinLatch::cross(current_thread);
        let job = StackJob::new(
            |injected| {
                let worker_thread = WorkerThread::current();
                assert!(injected && !worker_thread.is_null());
                op(&*worker_thread, true)
            },
            latch,
        );
        self.inject(job.as_job_ref());
        current_thread.wait_until(&job.latch);
        job.into_result()
    }

    /// Increments the terminate counter. This increment should be
    /// balanced by a call to `terminate`, which will decrement. This
    /// is used when spawning asynchronous work, which needs to
    /// prevent the registry from terminating so long as it is active.
    ///
    /// Note that blocking functions such as `join` and `scope` do not
    /// need to concern themselves with this fn; their context is
    /// responsible for ensuring the current thread-pool will not
    /// terminate until they return.
    ///
    /// The global thread-pool always has an outstanding reference
    /// (the initial one). Custom thread-pools have one outstanding
    /// reference that is dropped when the `ThreadPool` is dropped:
    /// since installing the thread-pool blocks until any joins/scopes
    /// complete, this ensures that joins/scopes are covered.
    ///
    /// The exception is `::spawn()`, which can create a job outside
    /// of any blocking scope. In that case, the job itself holds a
    /// terminate count and is responsible for invoking `terminate()`
    /// when finished.
    pub(super) fn increment_terminate_count(&self) {
        let previous = self.terminate_count.fetch_add(1, Ordering::AcqRel);
        debug_assert!(previous != 0, "registry ref count incremented from zero");
        assert!(previous != usize::MAX, "overflow in registry ref count");
    }

    /// Signals that the thread-pool which owns this registry has been
    /// dropped. The worker threads will gradually terminate, once any
    /// extant work is completed.
    pub(super) fn terminate(&self) {
        if self.terminate_count.fetch_sub(1, Ordering::AcqRel) == 1 {
            for (i, thread_info) in self.thread_infos.iter().enumerate() {
                unsafe { OnceLatch::set_and_tickle_one(&thread_info.terminate, self, i) };
            }
        }
    }

    /// Notify the worker that the latch they are sleeping on has been "set".
    pub(super) fn notify_worker_latch_is_set(&self, target_worker_index: usize) {
        self.sleep.notify_worker_latch_is_set(target_worker_index);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RegistryId {
    addr: usize,
}

struct ThreadInfo {
    /// Latch set once thread has started and we are entering into the
    /// main loop. Used to wait for worker threads to become primed,
    /// primarily of interest for benchmarking.
    primed: LockLatch,

    /// Latch is set once worker thread has completed. Used to wait
    /// until workers have stopped; only used for tests.
    stopped: LockLatch,

    /// The latch used to signal that terminated has been requested.
    /// This latch is *set* by the `terminate` method on the
    /// `Registry`, once the registry's main "terminate" counter
    /// reaches zero.
    terminate: OnceLatch,

    /// the "stealer" half of the worker's deque
    stealer: Stealer<JobRef>,
}

impl ThreadInfo {
    fn new(stealer: Stealer<JobRef>) -> ThreadInfo {
        ThreadInfo {
            primed: LockLatch::new(),
            stopped: LockLatch::new(),
            terminate: OnceLatch::new(),
            stealer,
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////
/// WorkerThread identifiers

pub(super) struct WorkerThread {
    /// the "worker" half of our local deque
    worker: Worker<JobRef>,

    /// the "stealer" half of the worker's broadcast deque
    stealer: Stealer<JobRef>,

    /// local queue used for `spawn_fifo` indirection
    fifo: JobFifo,

    index: usize,

    /// A weak random number generator.
    rng: XorShift64Star,

    registry: Arc<Registry>,
}

// This is a bit sketchy, but basically: the WorkerThread is
// allocated on the stack of the worker on entry and stored into this
// thread local variable. So it will remain valid at least until the
// worker is fully unwound. Using an unsafe pointer avoids the need
// for a RefCell<T> etc.
thread_local! {
    static WORKER_THREAD_STATE: Cell<*const WorkerThread> = const { Cell::new(ptr::null()) };
}

impl From<ThreadBuilder> for WorkerThread {
    fn from(thread: ThreadBuilder) -> Self {
        Self {
            worker: thread.worker,
            stealer: thread.stealer,
            fifo: JobFifo::new(),
            index: thread.index,
            rng: XorShift64Star::new(),
            registry: thread.registry,
        }
    }
}

impl Drop for WorkerThread {
    fn drop(&mut self) {
        // Undo `set_current`
        WORKER_THREAD_STATE.with(|t| {
            assert!(t.get().eq(&(self as *const _)));
            t.set(ptr::null());
        });
    }
}

impl WorkerThread {
    /// Gets the `WorkerThread` index for the current thread; returns
    /// NULL if this is not a worker thread. This pointer is valid
    /// anywhere on the current thread.
    #[inline]
    pub(super) fn current() -> *const WorkerThread {
        WORKER_THREAD_STATE.with(Cell::get)
    }

    /// Sets `self` as the worker thread index for the current thread.
    /// This is done during worker thread startup.
    unsafe fn set_current(thread: *const WorkerThread) {
        WORKER_THREAD_STATE.with(|t| {
            assert!(t.get().is_null());
            t.set(thread);
        });
    }

    /// Returns the registry that owns this worker thread.
    #[inline]
    pub(super) fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }

    /// Our index amongst the worker threads (ranges from `0..self.num_threads()`).
    #[inline]
    pub(super) fn index(&self) -> usize {
        self.index
    }

    #[inline]
    pub(super) unsafe fn push(&self, job: JobRef) {
        let queue_was_empty = self.worker.is_empty();
        self.worker.push(job);
        self.registry.sleep.new_internal_jobs(1, queue_was_empty);
    }

    #[inline]
    pub(super) unsafe fn push_fifo(&self, job: JobRef) {
        self.push(self.fifo.push(job));
    }

    #[inline]
    pub(super) fn local_deque_is_empty(&self) -> bool {
        self.worker.is_empty()
    }

    /// Attempts to obtain a "local" job -- typically this means
    /// popping from the top of the stack, though if we are configured
    /// for breadth-first execution, it would mean dequeuing from the
    /// bottom.
    #[inline]
    pub(super) fn take_local_job(&self) -> Option<JobRef> {
        let popped_job = self.worker.pop();

        if popped_job.is_some() {
            return popped_job;
        }

        loop {
            match self.stealer.steal() {
                Steal::Success(job) => return Some(job),
                Steal::Empty => return None,
                Steal::Retry => {}
            }
        }
    }

    fn has_injected_job(&self) -> bool {
        !self.stealer.is_empty() || self.registry.has_injected_job()
    }

    /// Wait until the latch is set. Try to keep busy by popping and
    /// stealing tasks as necessary.
    #[inline]
    pub(super) unsafe fn wait_until<L: AsCoreLatch + ?Sized>(&self, latch: &L) {
        let latch = latch.as_core_latch();
        if !latch.probe() {
            self.wait_until_cold(latch);
        }
    }

    #[cold]
    unsafe fn wait_until_cold(&self, latch: &CoreLatch) {
        // the code below should swallow all panics and hence never
        // unwind; but if something does wrong, we want to abort,
        // because otherwise other code in rayon may assume that the
        // latch has been signaled, and that can lead to random memory
        // accesses, which would be *very bad*
        let abort_guard = unwind::AbortIfPanic;

        'outer: while !latch.probe() {
            // Check for local work *before* we start marking ourself idle,
            // especially to avoid modifying shared sleep state.
            if let Some(job) = self.take_local_job() {
                self.execute(job);
                continue;
            }

            let mut idle_state = self.registry.sleep.start_looking(self.index);
            while !latch.probe() {
                if let Some(job) = self.find_work() {
                    self.registry.sleep.work_found();
                    self.execute(job);
                    // The job might have injected local work, so go back to the outer loop.
                    continue 'outer;
                } else {
                    self.registry
                        .sleep
                        .no_work_found(&mut idle_state, latch, || self.has_injected_job())
                }
            }

            // If we were sleepy, we are not anymore. We "found work" --
            // whatever the surrounding thread was doing before it had to wait.
            self.registry.sleep.work_found();
            break;
        }

        mem::forget(abort_guard); // successful execution, do not abort
    }

    unsafe fn wait_until_out_of_work(&self) {
        debug_assert_eq!(self as *const _, WorkerThread::current());
        let registry = &*self.registry;
        let index = self.index;

        self.wait_until(&registry.thread_infos[index].terminate);

        // Should not be any work left in our queue.
        debug_assert!(self.take_local_job().is_none());

        // Let registry know we are done
        Latch::set(&registry.thread_infos[index].stopped);
    }

    fn find_work(&self) -> Option<JobRef> {
        // Try to find some work to do. We give preference first
        // to things in our local deque, then in other workers
        // deques, and finally to injected jobs from the
        // outside. The idea is to finish what we started before
        // we take on something new.
        self.take_local_job()
            .or_else(|| self.steal())
            .or_else(|| self.registry.pop_injected_job())
    }

    pub(super) fn yield_now(&self) -> Yield {
        match self.find_work() {
            Some(job) => unsafe {
                self.execute(job);
                Yield::Executed
            },
            None => Yield::Idle,
        }
    }

    pub(super) fn yield_local(&self) -> Yield {
        match self.take_local_job() {
            Some(job) => unsafe {
                self.execute(job);
                Yield::Executed
            },
            None => Yield::Idle,
        }
    }

    #[inline]
    pub(super) unsafe fn execute(&self, job: JobRef) {
        job.execute();
    }

    /// Try to steal a single job and return it.
    ///
    /// This should only be done as a last resort, when there is no
    /// local work to do.
    fn steal(&self) -> Option<JobRef> {
        // we only steal when we don't have any work to do locally
        debug_assert!(self.local_deque_is_empty());

        // otherwise, try to steal
        let thread_infos = &self.registry.thread_infos.as_slice();
        let num_threads = thread_infos.len();
        if num_threads <= 1 {
            return None;
        }

        loop {
            let mut retry = false;
            let start = self.rng.next_usize(num_threads);
            let job = (start..num_threads)
                .chain(0..start)
                .filter(move |&i| i != self.index)
                .find_map(|victim_index| {
                    let victim = &thread_infos[victim_index];
                    match victim.stealer.steal() {
                        Steal::Success(job) => Some(job),
                        Steal::Empty => None,
                        Steal::Retry => {
                            retry = true;
                            None
                        }
                    }
                });
            if job.is_some() || !retry {
                return job;
            }
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////

unsafe fn main_loop(thread: ThreadBuilder) {
    let worker_thread = &WorkerThread::from(thread);
    WorkerThread::set_current(worker_thread);
    let registry = &*worker_thread.registry;
    let index = worker_thread.index;

    // let registry know we are ready to do work
    Latch::set(&registry.thread_infos[index].primed);

    // Worker threads should not panic. If they do, just abort, as the
    // internal state of the threadpool is corrupted. Note that if
    // **user code** panics, we should catch that and redirect.
    let abort_guard = unwind::AbortIfPanic;

    // Inform a user callback that we started a thread.
    if let Some(ref handler) = registry.start_handler {
        registry.catch_unwind(|| handler(index));
    }

    worker_thread.wait_until_out_of_work();

    // Normal termination, do not abort.
    mem::forget(abort_guard);

    // Inform a user callback that we exited a thread.
    if let Some(ref handler) = registry.exit_handler {
        registry.catch_unwind(|| handler(index));
        // We're already exiting the thread, there's nothing else to do.
    }
}

/// If already in a worker-thread, just execute `op`.  Otherwise,
/// execute `op` in the default thread-pool. Either way, block until
/// `op` completes and return its return value. If `op` panics, that
/// panic will be propagated as well.  The second argument indicates
/// `true` if injection was performed, `false` if executed directly.
pub(super) fn in_worker<OP, R>(op: OP) -> R
where
    OP: FnOnce(&WorkerThread, bool) -> R + Send,
    R: Send,
{
    unsafe {
        let owner_thread = WorkerThread::current();
        if !owner_thread.is_null() {
            // Perfectly valid to give them a `&T`: this is the
            // current thread, so we know the data structure won't be
            // invalidated until we return.
            op(&*owner_thread, false)
        } else {
            global_registry().in_worker(op)
        }
    }
}

/// [xorshift*] is a fast pseudorandom number generator which will
/// even tolerate weak seeding, as long as it's not zero.
///
/// [xorshift*]: https://en.wikipedia.org/wiki/Xorshift#xorshift*
struct XorShift64Star {
    state: Cell<u64>,
}

impl XorShift64Star {
    fn new() -> Self {
        // Any non-zero seed will do -- this uses the hash of a global counter.
        let mut seed = 0;
        while seed == 0 {
            let mut hasher = DefaultHasher::new();
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            hasher.write_usize(COUNTER.fetch_add(1, Ordering::Relaxed));
            seed = hasher.finish();
        }

        XorShift64Star {
            state: Cell::new(seed),
        }
    }

    fn next(&self) -> u64 {
        let mut x = self.state.get();
        debug_assert_ne!(x, 0);
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state.set(x);
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Return a value from `0..n`.
    fn next_usize(&self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

```

```rust
/// Note: the `S: ThreadSpawn` constraint is an internal implementation detail for the
/// default spawn and those set by [`spawn_handler`](#method.spawn_handler).
impl<S> ThreadPoolBuilder<S>
where
    S: ThreadSpawn,
{
    /// Creates a new `ThreadPool` initialized using this configuration.
    pub fn build(self) -> Result<ThreadPool, ThreadPoolBuildError> {
        ThreadPool::build(self)
    }

    /// Initializes the global thread pool. This initialization is
    /// **optional**.  If you do not call this function, the thread pool
    /// will be automatically initialized with the default
    /// configuration. Calling `build_global` is not recommended, except
    /// in two scenarios:
    ///
    /// - You wish to change the default configuration.
    /// - You are running a benchmark, in which case initializing may
    ///   yield slightly more consistent results, since the worker threads
    ///   will already be ready to go even in the first iteration.  But
    ///   this cost is minimal.
    ///
    /// Initialization of the global thread pool happens exactly
    /// once. Once started, the configuration cannot be
    /// changed. Therefore, if you call `build_global` a second time, it
    /// will return an error. An `Ok` result indicates that this
    /// is the first initialization of the thread pool.
    pub fn build_global(self) -> Result<(), ThreadPoolBuildError> {
        let registry = registry::init_global_registry(self)?;
        registry.wait_until_primed();
        Ok(())
    }
}

impl ThreadPoolBuilder {
    /// Creates a scoped `ThreadPool` initialized using this configuration.
    ///
    /// This is a convenience function for building a pool using [`std::thread::scope`]
    /// to spawn threads in a [`spawn_handler`](#method.spawn_handler).
    /// The threads in this pool will start by calling `wrapper`, which should
    /// do initialization and continue by calling `ThreadBuilder::run()`.
    ///
    /// [`std::thread::scope`]: https://doc.rust-lang.org/std/thread/fn.scope.html
    ///
    /// # Examples
    ///
    /// A scoped pool may be useful in combination with scoped thread-local variables.
    ///
    /// `
    /// # use rayon_core as rayon;
    ///
    /// scoped_tls::scoped_thread_local!(static POOL_DATA: Vec<i32>);
    ///
    /// fn main() -> Result<(), rayon::ThreadPoolBuildError> {
    ///     let pool_data = vec![1, 2, 3];
    ///
    ///     // We haven't assigned any TLS data yet.
    ///     assert!(!POOL_DATA.is_set());
    ///
    ///     rayon::ThreadPoolBuilder::new()
    ///         .build_scoped(
    ///             // Borrow `pool_data` in TLS for each thread.
    ///             |thread| POOL_DATA.set(&pool_data, || thread.run()),
    ///             // Do some work that needs the TLS data.
    ///             |pool| pool.install(|| assert!(POOL_DATA.is_set())),
    ///         )?;
    ///
    ///     // Once we've returned, `pool_data` is no longer borrowed.
    ///     drop(pool_data);
    ///     Ok(())
    /// }
    /// `
    pub fn build_scoped<W, F, R>(self, wrapper: W, with_pool: F) -> Result<R, ThreadPoolBuildError>
    where
        W: Fn(ThreadBuilder) + Sync, // expected to call `run()`
        F: FnOnce(&ThreadPool) -> R,
    {
        std::thread::scope(|scope| {
            let pool = self
                .spawn_handler(|thread| {
                    let mut builder = std::thread::Builder::new();
                    if let Some(name) = thread.name() {
                        builder = builder.name(name.to_string());
                    }
                    if let Some(size) = thread.stack_size() {
                        builder = builder.stack_size(size);
                    }
                    builder.spawn_scoped(scope, || wrapper(thread))?;
                    Ok(())
                })
                .build()?;
            Ok(with_pool(&pool))
        })
    }
}

impl<S> ThreadPoolBuilder<S> {
    /// Sets a custom function for spawning threads.
    ///
    /// Note that the threads will not exit until after the pool is dropped. It
    /// is up to the caller to wait for thread termination if that is important
    /// for any invariants. For instance, threads created in [`std::thread::scope`]
    /// will be joined before that scope returns, and this will block indefinitely
    /// if the pool is leaked. Furthermore, the global thread pool doesn't terminate
    /// until the entire process exits!
    ///
    /// # Examples
    ///
    /// A minimal spawn handler just needs to call `run()` from an independent thread.
    ///
    /// `
    /// # use rayon_core as rayon;
    /// fn main() -> Result<(), rayon::ThreadPoolBuildError> {
    ///     let pool = rayon::ThreadPoolBuilder::new()
    ///         .spawn_handler(|thread| {
    ///             std::thread::spawn(|| thread.run());
    ///             Ok(())
    ///         })
    ///         .build()?;
    ///
    ///     pool.install(|| println!("Hello from my custom thread!"));
    ///     Ok(())
    /// }
    /// `
    ///
    /// The default spawn handler sets the name and stack size if given, and propagates
    /// any errors from the thread builder.
    ///
    /// `
    /// # use rayon_core as rayon;
    /// fn main() -> Result<(), rayon::ThreadPoolBuildError> {
    ///     let pool = rayon::ThreadPoolBuilder::new()
    ///         .spawn_handler(|thread| {
    ///             let mut b = std::thread::Builder::new();
    ///             if let Some(name) = thread.name() {
    ///                 b = b.name(name.to_owned());
    ///             }
    ///             if let Some(stack_size) = thread.stack_size() {
    ///                 b = b.stack_size(stack_size);
    ///             }
    ///             b.spawn(|| thread.run())?;
    ///             Ok(())
    ///         })
    ///         .build()?;
    ///
    ///     pool.install(|| println!("Hello from my fully custom thread!"));
    ///     Ok(())
    /// }
    /// `
    ///
    /// This can also be used for a pool of scoped threads like [`crossbeam::scope`],
    /// or [`std::thread::scope`] introduced in Rust 1.63, which is encapsulated in
    /// [`build_scoped`](#method.build_scoped).
    ///
    /// [`crossbeam::scope`]: https://docs.rs/crossbeam/0.8/crossbeam/fn.scope.html
    /// [`std::thread::scope`]: https://doc.rust-lang.org/std/thread/fn.scope.html
    ///
    /// `
    /// # use rayon_core as rayon;
    /// fn main() -> Result<(), rayon::ThreadPoolBuildError> {
    ///     std::thread::scope(|scope| {
    ///         let pool = rayon::ThreadPoolBuilder::new()
    ///             .spawn_handler(|thread| {
    ///                 let mut builder = std::thread::Builder::new();
    ///                 if let Some(name) = thread.name() {
    ///                     builder = builder.name(name.to_string());
    ///                 }
    ///                 if let Some(size) = thread.stack_size() {
    ///                     builder = builder.stack_size(size);
    ///                 }
    ///                 builder.spawn_scoped(scope, || {
    ///                     // Add any scoped initialization here, then run!
    ///                     thread.run()
    ///                 })?;
    ///                 Ok(())
    ///             })
    ///             .build()?;
    ///
    ///         pool.install(|| println!("Hello from my custom scoped thread!"));
    ///         Ok(())
    ///     })
    /// }
    /// `
    pub fn spawn_handler<F>(self, spawn: F) -> ThreadPoolBuilder<CustomSpawn<F>>
    where
        F: FnMut(ThreadBuilder) -> io::Result<()>,
    {
        ThreadPoolBuilder {
            spawn_handler: CustomSpawn::new(spawn),
            // ..self
            num_threads: self.num_threads,
            use_current_thread: self.use_current_thread,
            panic_handler: self.panic_handler,
            get_thread_name: self.get_thread_name,
            stack_size: self.stack_size,
            start_handler: self.start_handler,
            exit_handler: self.exit_handler,
            breadth_first: self.breadth_first,
        }
    }

    /// Returns a reference to the current spawn handler.
    fn get_spawn_handler(&mut self) -> &mut S {
        &mut self.spawn_handler
    }

    /// Get the number of threads that will be used for the thread
    /// pool. See `num_threads()` for more information.
    fn get_num_threads(&self) -> usize {
        if self.num_threads > 0 {
            self.num_threads
        } else {
            let default = || {
                thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(1)
            };

            match env::var("RAYON_NUM_THREADS")
                .ok()
                .and_then(|s| usize::from_str(&s).ok())
            {
                Some(x @ 1..) => return x,
                Some(0) => return default(),
                _ => {}
            }

            // Support for deprecated `RAYON_RS_NUM_CPUS`.
            match env::var("RAYON_RS_NUM_CPUS")
                .ok()
                .and_then(|s| usize::from_str(&s).ok())
            {
                Some(x @ 1..) => x,
                _ => default(),
            }
        }
    }

    /// Get the thread name for the thread with the given index.
    fn get_thread_name(&mut self, index: usize) -> Option<String> {
        let f = self.get_thread_name.as_mut()?;
        Some(f(index))
    }

    /// Sets a closure which takes a thread index and returns
    /// the thread's name.
    pub fn thread_name<F>(mut self, closure: F) -> Self
    where
        F: FnMut(usize) -> String + 'static,
    {
        self.get_thread_name = Some(Box::new(closure));
        self
    }

    /// Sets the number of threads to be used in the rayon threadpool.
    ///
    /// If you specify a non-zero number of threads using this
    /// function, then the resulting thread-pools are guaranteed to
    /// start at most this number of threads.
    ///
    /// If `num_threads` is 0, or you do not call this function, then
    /// the Rayon runtime will select the number of threads
    /// automatically. At present, this is based on the
    /// `RAYON_NUM_THREADS` environment variable (if set),
    /// or the number of logical CPUs (otherwise).
    /// In the future, however, the default behavior may
    /// change to dynamically add or remove threads as needed.
    ///
    /// **Future compatibility warning:** Given the default behavior
    /// may change in the future, if you wish to rely on a fixed
    /// number of threads, you should use this function to specify
    /// that number. To reproduce the current default behavior, you
    /// may wish to use [`std::thread::available_parallelism`]
    /// to query the number of CPUs dynamically.
    ///
    /// **Old environment variable:** `RAYON_NUM_THREADS` is a one-to-one
    /// replacement of the now deprecated `RAYON_RS_NUM_CPUS` environment
    /// variable. If both variables are specified, `RAYON_NUM_THREADS` will
    /// be preferred.
    pub fn num_threads(mut self, num_threads: usize) -> Self {
        self.num_threads = num_threads;
        self
    }

    /// Use the current thread as one of the threads in the pool.
    ///
    /// The current thread is guaranteed to be at index 0, and since the thread is not managed by
    /// rayon, the spawn and exit handlers do not run for that thread.
    ///
    /// Note that the current thread won't run the main work-stealing loop, so jobs spawned into
    /// the thread-pool will generally not be picked up automatically by this thread unless you
    /// yield to rayon in some way, like via [`yield_now()`], [`yield_local()`], or [`scope()`].
    ///
    /// # Local thread-pools
    ///
    /// Using this in a local thread-pool means the registry will be leaked. In future versions
    /// there might be a way of cleaning up the current-thread state.
    pub fn use_current_thread(mut self) -> Self {
        self.use_current_thread = true;
        self
    }

    /// Returns a copy of the current panic handler.
    fn take_panic_handler(&mut self) -> Option<Box<PanicHandler>> {
        self.panic_handler.take()
    }

    /// Normally, whenever Rayon catches a panic, it tries to
    /// propagate it to someplace sensible, to try and reflect the
    /// semantics of sequential execution. But in some cases,
    /// particularly with the `spawn()` APIs, there is no
    /// obvious place where we should propagate the panic to.
    /// In that case, this panic handler is invoked.
    ///
    /// If no panic handler is set, the default is to abort the
    /// process, under the principle that panics should not go
    /// unobserved.
    ///
    /// If the panic handler itself panics, this will abort the
    /// process. To prevent this, wrap the body of your panic handler
    /// in a call to `std::panic::catch_unwind()`.
    pub fn panic_handler<H>(mut self, panic_handler: H) -> Self
    where
        H: Fn(Box<dyn Any + Send>) + Send + Sync + 'static,
    {
        self.panic_handler = Some(Box::new(panic_handler));
        self
    }

    /// Get the stack size of the worker threads
    fn get_stack_size(&self) -> Option<usize> {
        self.stack_size
    }

    /// Sets the stack size of the worker threads
    pub fn stack_size(mut self, stack_size: usize) -> Self {
        self.stack_size = Some(stack_size);
        self
    }

    /// **(DEPRECATED)** Suggest to worker threads that they execute
    /// spawned jobs in a "breadth-first" fashion.
    ///
    /// Typically, when a worker thread is idle or blocked, it will
    /// attempt to execute the job from the *top* of its local deque of
    /// work (i.e., the job most recently spawned). If this flag is set
    /// to true, however, workers will prefer to execute in a
    /// *breadth-first* fashion -- that is, they will search for jobs at
    /// the *bottom* of their local deque. (At present, workers *always*
    /// steal from the bottom of other workers' deques, regardless of
    /// the setting of this flag.)
    ///
    /// If you think of the tasks as a tree, where a parent task
    /// spawns its children in the tree, then this flag loosely
    /// corresponds to doing a breadth-first traversal of the tree,
    /// whereas the default would be to do a depth-first traversal.
    ///
    /// **Note that this is an "execution hint".** Rayon's task
    /// execution is highly dynamic and the precise order in which
    /// independent tasks are executed is not intended to be
    /// guaranteed.
    ///
    /// This `breadth_first()` method is now deprecated per [RFC #1],
    /// and in the future its effect may be removed. Consider using
    /// [`scope_fifo()`] for a similar effect.
    ///
    /// [RFC #1]: https://github.com/rayon-rs/rfcs/blob/main/accepted/rfc0001-scope-scheduling.md
    /// [`scope_fifo()`]: fn.scope_fifo.html
    #[deprecated(note = "use `scope_fifo` and `spawn_fifo` for similar effect")]
    pub fn breadth_first(mut self) -> Self {
        self.breadth_first = true;
        self
    }

    fn get_breadth_first(&self) -> bool {
        self.breadth_first
    }

    /// Takes the current thread start callback, leaving `None`.
    fn take_start_handler(&mut self) -> Option<Box<StartHandler>> {
        self.start_handler.take()
    }

    /// Sets a callback to be invoked on thread start.
    ///
    /// The closure is passed the index of the thread on which it is invoked.
    /// Note that this same closure may be invoked multiple times in parallel.
    /// If this closure panics, the panic will be passed to the panic handler.
    /// If that handler returns, then startup will continue normally.
    pub fn start_handler<H>(mut self, start_handler: H) -> Self
    where
        H: Fn(usize) + Send + Sync + 'static,
    {
        self.start_handler = Some(Box::new(start_handler));
        self
    }

    /// Returns a current thread exit callback, leaving `None`.
    fn take_exit_handler(&mut self) -> Option<Box<ExitHandler>> {
        self.exit_handler.take()
    }

    /// Sets a callback to be invoked on thread exit.
    ///
    /// The closure is passed the index of the thread on which it is invoked.
    /// Note that this same closure may be invoked multiple times in parallel.
    /// If this closure panics, the panic will be passed to the panic handler.
    /// If that handler returns, then the thread will exit normally.
    pub fn exit_handler<H>(mut self, exit_handler: H) -> Self
    where
        H: Fn(usize) + Send + Sync + 'static,
    {
        self.exit_handler = Some(Box::new(exit_handler));
        self
    }
}

use crate::job::{JobFifo, JobRef, StackJob};
use crate::latch::{AsCoreLatch, CoreLatch, Latch, LatchRef, LockLatch, OnceLatch, SpinLatch};
use crate::sleep::Sleep;
use crate::sync::Mutex;
use crate::unwind;
use crate::{
    ErrorKind, ExitHandler, PanicHandler, StartHandler, ThreadPoolBuildError, ThreadPoolBuilder,
    Yield,
};
use crossbeam_deque::{Injector, Steal, Stealer, Worker};
use std::cell::Cell;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::Hasher;
use std::io;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Once};
use std::thread;

/// Thread builder used for customization via
/// [`ThreadPoolBuilder::spawn_handler`](struct.ThreadPoolBuilder.html#method.spawn_handler).
pub struct ThreadBuilder {
    name: Option<String>,
    stack_size: Option<usize>,
    worker: Worker<JobRef>,
    stealer: Stealer<JobRef>,
    registry: Arc<Registry>,
    index: usize,
}

impl ThreadBuilder {
    /// Gets the index of this thread in the pool, within `0..num_threads`.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Gets the string that was specified by `ThreadPoolBuilder::name()`.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Gets the value that was specified by `ThreadPoolBuilder::stack_size()`.
    pub fn stack_size(&self) -> Option<usize> {
        self.stack_size
    }

    /// Executes the main loop for this thread. This will not return until the
    /// thread pool is dropped.
    pub fn run(self) {
        unsafe { main_loop(self) }
    }
}

impl fmt::Debug for ThreadBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThreadBuilder")
            .field("pool", &self.registry.id())
            .field("index", &self.index)
            .field("name", &self.name)
            .field("stack_size", &self.stack_size)
            .finish()
    }
}

/// Generalized trait for spawning a thread in the `Registry`.
///
/// This trait is pub-in-private -- E0445 forces us to make it public,
/// but we don't actually want to expose these details in the API.
pub trait ThreadSpawn {
    private_decl! {}

    /// Spawn a thread with the `ThreadBuilder` parameters, and then
    /// call `ThreadBuilder::run()`.
    fn spawn(&mut self, thread: ThreadBuilder) -> io::Result<()>;
}

/// Spawns a thread in the "normal" way with `std::thread::Builder`.
///
/// This type is pub-in-private -- E0445 forces us to make it public,
/// but we don't actually want to expose these details in the API.
#[derive(Debug, Default)]
pub struct DefaultSpawn;

impl ThreadSpawn for DefaultSpawn {
    private_impl! {}

    fn spawn(&mut self, thread: ThreadBuilder) -> io::Result<()> {
        let mut b = thread::Builder::new();
        if let Some(name) = thread.name() {
            b = b.name(name.to_owned());
        }
        if let Some(stack_size) = thread.stack_size() {
            b = b.stack_size(stack_size);
        }
        b.spawn(|| thread.run())?;
        Ok(())
    }
}

/// Spawns a thread with a user's custom callback.
///
/// This type is pub-in-private -- E0445 forces us to make it public,
/// but we don't actually want to expose these details in the API.
#[derive(Debug)]
pub struct CustomSpawn<F>(F);

impl<F> CustomSpawn<F>
where
    F: FnMut(ThreadBuilder) -> io::Result<()>,
{
    pub(super) fn new(spawn: F) -> Self {
        CustomSpawn(spawn)
    }
}

impl<F> ThreadSpawn for CustomSpawn<F>
where
    F: FnMut(ThreadBuilder) -> io::Result<()>,
{
    private_impl! {}

    #[inline]
    fn spawn(&mut self, thread: ThreadBuilder) -> io::Result<()> {
        (self.0)(thread)
    }
}

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

fn default_global_registry() -> Result<Arc<Registry>, ThreadPoolBuildError> {
    let result = Registry::new(ThreadPoolBuilder::new());

    // If we're running in an environment that doesn't support threads at all, we can fall back to
    // using the current thread alone. This is crude, and probably won't work for non-blocking
    // calls like `spawn` or `broadcast_spawn`, but a lot of stuff does work fine.
    //
    // Notably, this allows current WebAssembly targets to work even though their threading support
    // is stubbed out, and we won't have to change anything if they do add real threading.
    let unsupported = matches!(&result, Err(e) if e.is_unsupported());
    if unsupported && WorkerThread::current().is_null() {
        let builder = ThreadPoolBuilder::new().num_threads(1).use_current_thread();
        let fallback_result = Registry::new(builder);
        if fallback_result.is_ok() {
            return fallback_result;
        }
    }

    result
}

struct Terminator<'a>(&'a Arc<Registry>);

impl<'a> Drop for Terminator<'a> {
    fn drop(&mut self) {
        self.0.terminate()
    }
}

impl Registry {
    pub(super) fn new<S>(
        mut builder: ThreadPoolBuilder<S>,
    ) -> Result<Arc<Self>, ThreadPoolBuildError>
    where
        S: ThreadSpawn,
    {
        // Soft-limit the number of threads that we can actually support.
        let n_threads = Ord::min(builder.get_num_threads(), crate::max_num_threads());

        let breadth_first = builder.get_breadth_first();

        let (workers, stealers): (Vec<_>, Vec<_>) = (0..n_threads)
            .map(|_| {
                let worker = if breadth_first {
                    Worker::new_fifo()
                } else {
                    Worker::new_lifo()
                };

                let stealer = worker.stealer();
                (worker, stealer)
            })
            .unzip();

        let (broadcasts, broadcast_stealers): (Vec<_>, Vec<_>) = (0..n_threads)
            .map(|_| {
                let worker = Worker::new_fifo();
                let stealer = worker.stealer();
                (worker, stealer)
            })
            .unzip();

        let registry = Arc::new(Registry {
            thread_infos: stealers.into_iter().map(ThreadInfo::new).collect(),
            sleep: Sleep::new(n_threads),
            injected_jobs: Injector::new(),
            broadcasts: Mutex::new(broadcasts),
            terminate_count: AtomicUsize::new(1),
            panic_handler: builder.take_panic_handler(),
            start_handler: builder.take_start_handler(),
            exit_handler: builder.take_exit_handler(),
        });

        // If we return early or panic, make sure to terminate existing threads.
        let t1000 = Terminator(&registry);

        for (index, (worker, stealer)) in workers.into_iter().zip(broadcast_stealers).enumerate() {
            let thread = ThreadBuilder {
                name: builder.get_thread_name(index),
                stack_size: builder.get_stack_size(),
                registry: Arc::clone(&registry),
                worker,
                stealer,
                index,
            };

            if index == 0 && builder.use_current_thread {
                if !WorkerThread::current().is_null() {
                    return Err(ThreadPoolBuildError::new(
                        ErrorKind::CurrentThreadAlreadyInPool,
                    ));
                }
                // Rather than starting a new thread, we're just taking over the current thread
                // *without* running the main loop, so we can still return from here.
                // The WorkerThread is leaked, but we never shutdown the global pool anyway.
                let worker_thread = Box::into_raw(Box::new(WorkerThread::from(thread)));

                unsafe {
                    WorkerThread::set_current(worker_thread);
                    Latch::set(&registry.thread_infos[index].primed);
                }
                continue;
            }

            if let Err(e) = builder.get_spawn_handler().spawn(thread) {
                return Err(ThreadPoolBuildError::new(ErrorKind::IOError(e)));
            }
        }

        // Returning normally now, without termination.
        mem::forget(t1000);

        Ok(registry)
    }

    pub(super) fn current() -> Arc<Registry> {
        unsafe {
            let worker_thread = WorkerThread::current();
            let registry = if worker_thread.is_null() {
                global_registry()
            } else {
                &(*worker_thread).registry
            };
            Arc::clone(registry)
        }
    }

    /// Returns the number of threads in the current registry.  This
    /// is better than `Registry::current().num_threads()` because it
    /// avoids incrementing the `Arc`.
    pub(super) fn current_num_threads() -> usize {
        unsafe {
            let worker_thread = WorkerThread::current();
            if worker_thread.is_null() {
                global_registry().num_threads()
            } else {
                (*worker_thread).registry.num_threads()
            }
        }
    }

    /// Returns the current `WorkerThread` if it's part of this `Registry`.
    pub(super) fn current_thread(&self) -> Option<&WorkerThread> {
        unsafe {
            let worker = WorkerThread::current().as_ref()?;
            if worker.registry().id() == self.id() {
                Some(worker)
            } else {
                None
            }
        }
    }

    /// Returns an opaque identifier for this registry.
    pub(super) fn id(&self) -> RegistryId {
        // We can rely on `self` not to change since we only ever create
        // registries that are boxed up in an `Arc` (see `new()` above).
        RegistryId {
            addr: self as *const Self as usize,
        }
    }

    pub(super) fn num_threads(&self) -> usize {
        self.thread_infos.len()
    }

    pub(super) fn catch_unwind(&self, f: impl FnOnce()) {
        if let Err(err) = unwind::halt_unwinding(f) {
            // If there is no handler, or if that handler itself panics, then we abort.
            let abort_guard = unwind::AbortIfPanic;
            if let Some(ref handler) = self.panic_handler {
                handler(err);
                mem::forget(abort_guard);
            }
        }
    }

    /// Waits for the worker threads to get up and running.  This is
    /// meant to be used for benchmarking purposes, primarily, so that
    /// you can get more consistent numbers by having everything
    /// "ready to go".
    pub(super) fn wait_until_primed(&self) {
        for info in &self.thread_infos {
            info.primed.wait();
        }
    }

    /// Waits for the worker threads to stop. This is used for testing
    /// -- so we can check that termination actually works.
    #[cfg(test)]
    pub(super) fn wait_until_stopped(&self) {
        for info in &self.thread_infos {
            info.stopped.wait();
        }
    }

    /// ////////////////////////////////////////////////////////////////////////
    /// MAIN LOOP
    ///
    /// So long as all of the worker threads are hanging out in their
    /// top-level loop, there is no work to be done.

    /// Push a job into the given `registry`. If we are running on a
    /// worker thread for the registry, this will push onto the
    /// deque. Else, it will inject from the outside (which is slower).
    pub(super) fn inject_or_push(&self, job_ref: JobRef) {
        let worker_thread = WorkerThread::current();
        unsafe {
            if !worker_thread.is_null() && (*worker_thread).registry().id() == self.id() {
                (*worker_thread).push(job_ref);
            } else {
                self.inject(job_ref);
            }
        }
    }

    /// Push a job into the "external jobs" queue; it will be taken by
    /// whatever worker has nothing to do. Use this if you know that
    /// you are not on a worker of this registry.
    pub(super) fn inject(&self, injected_job: JobRef) {
        // It should not be possible for `state.terminate` to be true
        // here. It is only set to true when the user creates (and
        // drops) a `ThreadPool`; and, in that case, they cannot be
        // calling `inject()` later, since they dropped their
        // `ThreadPool`.
        debug_assert_ne!(
            self.terminate_count.load(Ordering::Acquire),
            0,
            "inject() sees state.terminate as true"
        );

        let queue_was_empty = self.injected_jobs.is_empty();

        self.injected_jobs.push(injected_job);
        self.sleep.new_injected_jobs(1, queue_was_empty);
    }

    fn has_injected_job(&self) -> bool {
        !self.injected_jobs.is_empty()
    }

    fn pop_injected_job(&self) -> Option<JobRef> {
        loop {
            match self.injected_jobs.steal() {
                Steal::Success(job) => return Some(job),
                Steal::Empty => return None,
                Steal::Retry => {}
            }
        }
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

    #[cold]
    unsafe fn in_worker_cross<OP, R>(&self, current_thread: &WorkerThread, op: OP) -> R
    where
        OP: FnOnce(&WorkerThread, bool) -> R + Send,
        R: Send,
    {
        // This thread is a member of a different pool, so let it process
        // other work while waiting for this `op` to complete.
        debug_assert!(current_thread.registry().id() != self.id());
        let latch = SpinLatch::cross(current_thread);
        let job = StackJob::new(
            |injected| {
                let worker_thread = WorkerThread::current();
                assert!(injected && !worker_thread.is_null());
                op(&*worker_thread, true)
            },
            latch,
        );
        self.inject(job.as_job_ref());
        current_thread.wait_until(&job.latch);
        job.into_result()
    }

    /// Increments the terminate counter. This increment should be
    /// balanced by a call to `terminate`, which will decrement. This
    /// is used when spawning asynchronous work, which needs to
    /// prevent the registry from terminating so long as it is active.
    ///
    /// Note that blocking functions such as `join` and `scope` do not
    /// need to concern themselves with this fn; their context is
    /// responsible for ensuring the current thread-pool will not
    /// terminate until they return.
    ///
    /// The global thread-pool always has an outstanding reference
    /// (the initial one). Custom thread-pools have one outstanding
    /// reference that is dropped when the `ThreadPool` is dropped:
    /// since installing the thread-pool blocks until any joins/scopes
    /// complete, this ensures that joins/scopes are covered.
    ///
    /// The exception is `::spawn()`, which can create a job outside
    /// of any blocking scope. In that case, the job itself holds a
    /// terminate count and is responsible for invoking `terminate()`
    /// when finished.
    pub(super) fn increment_terminate_count(&self) {
        let previous = self.terminate_count.fetch_add(1, Ordering::AcqRel);
        debug_assert!(previous != 0, "registry ref count incremented from zero");
        assert!(previous != usize::MAX, "overflow in registry ref count");
    }

    /// Signals that the thread-pool which owns this registry has been
    /// dropped. The worker threads will gradually terminate, once any
    /// extant work is completed.
    pub(super) fn terminate(&self) {
        if self.terminate_count.fetch_sub(1, Ordering::AcqRel) == 1 {
            for (i, thread_info) in self.thread_infos.iter().enumerate() {
                unsafe { OnceLatch::set_and_tickle_one(&thread_info.terminate, self, i) };
            }
        }
    }

    /// Notify the worker that the latch they are sleeping on has been "set".
    pub(super) fn notify_worker_latch_is_set(&self, target_worker_index: usize) {
        self.sleep.notify_worker_latch_is_set(target_worker_index);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RegistryId {
    addr: usize,
}

struct ThreadInfo {
    /// Latch set once thread has started and we are entering into the
    /// main loop. Used to wait for worker threads to become primed,
    /// primarily of interest for benchmarking.
    primed: LockLatch,

    /// Latch is set once worker thread has completed. Used to wait
    /// until workers have stopped; only used for tests.
    stopped: LockLatch,

    /// The latch used to signal that terminated has been requested.
    /// This latch is *set* by the `terminate` method on the
    /// `Registry`, once the registry's main "terminate" counter
    /// reaches zero.
    terminate: OnceLatch,

    /// the "stealer" half of the worker's deque
    stealer: Stealer<JobRef>,
}

impl ThreadInfo {
    fn new(stealer: Stealer<JobRef>) -> ThreadInfo {
        ThreadInfo {
            primed: LockLatch::new(),
            stopped: LockLatch::new(),
            terminate: OnceLatch::new(),
            stealer,
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////
/// WorkerThread identifiers

pub(super) struct WorkerThread {
    /// the "worker" half of our local deque
    worker: Worker<JobRef>,

    /// the "stealer" half of the worker's broadcast deque
    stealer: Stealer<JobRef>,

    /// local queue used for `spawn_fifo` indirection
    fifo: JobFifo,

    index: usize,

    /// A weak random number generator.
    rng: XorShift64Star,

    registry: Arc<Registry>,
}

// This is a bit sketchy, but basically: the WorkerThread is
// allocated on the stack of the worker on entry and stored into this
// thread local variable. So it will remain valid at least until the
// worker is fully unwound. Using an unsafe pointer avoids the need
// for a RefCell<T> etc.
thread_local! {
    static WORKER_THREAD_STATE: Cell<*const WorkerThread> = const { Cell::new(ptr::null()) };
}

impl From<ThreadBuilder> for WorkerThread {
    fn from(thread: ThreadBuilder) -> Self {
        Self {
            worker: thread.worker,
            stealer: thread.stealer,
            fifo: JobFifo::new(),
            index: thread.index,
            rng: XorShift64Star::new(),
            registry: thread.registry,
        }
    }
}

impl Drop for WorkerThread {
    fn drop(&mut self) {
        // Undo `set_current`
        WORKER_THREAD_STATE.with(|t| {
            assert!(t.get().eq(&(self as *const _)));
            t.set(ptr::null());
        });
    }
}

impl WorkerThread {
    /// Gets the `WorkerThread` index for the current thread; returns
    /// NULL if this is not a worker thread. This pointer is valid
    /// anywhere on the current thread.
    #[inline]
    pub(super) fn current() -> *const WorkerThread {
        WORKER_THREAD_STATE.with(Cell::get)
    }

    /// Sets `self` as the worker thread index for the current thread.
    /// This is done during worker thread startup.
    unsafe fn set_current(thread: *const WorkerThread) {
        WORKER_THREAD_STATE.with(|t| {
            assert!(t.get().is_null());
            t.set(thread);
        });
    }

    /// Returns the registry that owns this worker thread.
    #[inline]
    pub(super) fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }

    /// Our index amongst the worker threads (ranges from `0..self.num_threads()`).
    #[inline]
    pub(super) fn index(&self) -> usize {
        self.index
    }

    #[inline]
    pub(super) unsafe fn push(&self, job: JobRef) {
        let queue_was_empty = self.worker.is_empty();
        self.worker.push(job);
        self.registry.sleep.new_internal_jobs(1, queue_was_empty);
    }

    #[inline]
    pub(super) unsafe fn push_fifo(&self, job: JobRef) {
        self.push(self.fifo.push(job));
    }

    #[inline]
    pub(super) fn local_deque_is_empty(&self) -> bool {
        self.worker.is_empty()
    }

    /// Attempts to obtain a "local" job -- typically this means
    /// popping from the top of the stack, though if we are configured
    /// for breadth-first execution, it would mean dequeuing from the
    /// bottom.
    #[inline]
    pub(super) fn take_local_job(&self) -> Option<JobRef> {
        let popped_job = self.worker.pop();

        if popped_job.is_some() {
            return popped_job;
        }

        loop {
            match self.stealer.steal() {
                Steal::Success(job) => return Some(job),
                Steal::Empty => return None,
                Steal::Retry => {}
            }
        }
    }

    fn has_injected_job(&self) -> bool {
        !self.stealer.is_empty() || self.registry.has_injected_job()
    }

    /// Wait until the latch is set. Try to keep busy by popping and
    /// stealing tasks as necessary.
    #[inline]
    pub(super) unsafe fn wait_until<L: AsCoreLatch + ?Sized>(&self, latch: &L) {
        let latch = latch.as_core_latch();
        if !latch.probe() {
            self.wait_until_cold(latch);
        }
    }

    #[cold]
    unsafe fn wait_until_cold(&self, latch: &CoreLatch) {
        // the code below should swallow all panics and hence never
        // unwind; but if something does wrong, we want to abort,
        // because otherwise other code in rayon may assume that the
        // latch has been signaled, and that can lead to random memory
        // accesses, which would be *very bad*
        let abort_guard = unwind::AbortIfPanic;

        'outer: while !latch.probe() {
            // Check for local work *before* we start marking ourself idle,
            // especially to avoid modifying shared sleep state.
            if let Some(job) = self.take_local_job() {
                self.execute(job);
                continue;
            }

            let mut idle_state = self.registry.sleep.start_looking(self.index);
            while !latch.probe() {
                if let Some(job) = self.find_work() {
                    self.registry.sleep.work_found();
                    self.execute(job);
                    // The job might have injected local work, so go back to the outer loop.
                    continue 'outer;
                } else {
                    self.registry
                        .sleep
                        .no_work_found(&mut idle_state, latch, || self.has_injected_job())
                }
            }

            // If we were sleepy, we are not anymore. We "found work" --
            // whatever the surrounding thread was doing before it had to wait.
            self.registry.sleep.work_found();
            break;
        }

        mem::forget(abort_guard); // successful execution, do not abort
    }

    unsafe fn wait_until_out_of_work(&self) {
        debug_assert_eq!(self as *const _, WorkerThread::current());
        let registry = &*self.registry;
        let index = self.index;

        self.wait_until(&registry.thread_infos[index].terminate);

        // Should not be any work left in our queue.
        debug_assert!(self.take_local_job().is_none());

        // Let registry know we are done
        Latch::set(&registry.thread_infos[index].stopped);
    }

    fn find_work(&self) -> Option<JobRef> {
        // Try to find some work to do. We give preference first
        // to things in our local deque, then in other workers
        // deques, and finally to injected jobs from the
        // outside. The idea is to finish what we started before
        // we take on something new.
        self.take_local_job()
            .or_else(|| self.steal())
            .or_else(|| self.registry.pop_injected_job())
    }

    pub(super) fn yield_now(&self) -> Yield {
        match self.find_work() {
            Some(job) => unsafe {
                self.execute(job);
                Yield::Executed
            },
            None => Yield::Idle,
        }
    }

    pub(super) fn yield_local(&self) -> Yield {
        match self.take_local_job() {
            Some(job) => unsafe {
                self.execute(job);
                Yield::Executed
            },
            None => Yield::Idle,
        }
    }

    #[inline]
    pub(super) unsafe fn execute(&self, job: JobRef) {
        job.execute();
    }

    /// Try to steal a single job and return it.
    ///
    /// This should only be done as a last resort, when there is no
    /// local work to do.
    fn steal(&self) -> Option<JobRef> {
        // we only steal when we don't have any work to do locally
        debug_assert!(self.local_deque_is_empty());

        // otherwise, try to steal
        let thread_infos = &self.registry.thread_infos.as_slice();
        let num_threads = thread_infos.len();
        if num_threads <= 1 {
            return None;
        }

        loop {
            let mut retry = false;
            let start = self.rng.next_usize(num_threads);
            let job = (start..num_threads)
                .chain(0..start)
                .filter(move |&i| i != self.index)
                .find_map(|victim_index| {
                    let victim = &thread_infos[victim_index];
                    match victim.stealer.steal() {
                        Steal::Success(job) => Some(job),
                        Steal::Empty => None,
                        Steal::Retry => {
                            retry = true;
                            None
                        }
                    }
                });
            if job.is_some() || !retry {
                return job;
            }
        }
    }
}

/// ////////////////////////////////////////////////////////////////////////

unsafe fn main_loop(thread: ThreadBuilder) {
    let worker_thread = &WorkerThread::from(thread);
    WorkerThread::set_current(worker_thread);
    let registry = &*worker_thread.registry;
    let index = worker_thread.index;

    // let registry know we are ready to do work
    Latch::set(&registry.thread_infos[index].primed);

    // Worker threads should not panic. If they do, just abort, as the
    // internal state of the threadpool is corrupted. Note that if
    // **user code** panics, we should catch that and redirect.
    let abort_guard = unwind::AbortIfPanic;

    // Inform a user callback that we started a thread.
    if let Some(ref handler) = registry.start_handler {
        registry.catch_unwind(|| handler(index));
    }

    worker_thread.wait_until_out_of_work();

    // Normal termination, do not abort.
    mem::forget(abort_guard);

    // Inform a user callback that we exited a thread.
    if let Some(ref handler) = registry.exit_handler {
        registry.catch_unwind(|| handler(index));
        // We're already exiting the thread, there's nothing else to do.
    }
}

/// If already in a worker-thread, just execute `op`.  Otherwise,
/// execute `op` in the default thread-pool. Either way, block until
/// `op` completes and return its return value. If `op` panics, that
/// panic will be propagated as well.  The second argument indicates
/// `true` if injection was performed, `false` if executed directly.
pub(super) fn in_worker<OP, R>(op: OP) -> R
where
    OP: FnOnce(&WorkerThread, bool) -> R + Send,
    R: Send,
{
    unsafe {
        let owner_thread = WorkerThread::current();
        if !owner_thread.is_null() {
            // Perfectly valid to give them a `&T`: this is the
            // current thread, so we know the data structure won't be
            // invalidated until we return.
            op(&*owner_thread, false)
        } else {
            global_registry().in_worker(op)
        }
    }
}

/// [xorshift*] is a fast pseudorandom number generator which will
/// even tolerate weak seeding, as long as it's not zero.
///
/// [xorshift*]: https://en.wikipedia.org/wiki/Xorshift#xorshift*
struct XorShift64Star {
    state: Cell<u64>,
}

impl XorShift64Star {
    fn new() -> Self {
        // Any non-zero seed will do -- this uses the hash of a global counter.
        let mut seed = 0;
        while seed == 0 {
            let mut hasher = DefaultHasher::new();
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            hasher.write_usize(COUNTER.fetch_add(1, Ordering::Relaxed));
            seed = hasher.finish();
        }

        XorShift64Star {
            state: Cell::new(seed),
        }
    }

    fn next(&self) -> u64 {
        let mut x = self.state.get();
        debug_assert_ne!(x, 0);
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state.set(x);
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Return a value from `0..n`.
    fn next_usize(&self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

```
