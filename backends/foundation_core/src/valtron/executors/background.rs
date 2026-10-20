//! Background Job Registry — fixed thread pool for blocking, fire-and-forget closures.
//!
//! WHY: ThreadedFuture and other code spawns unbounded OS threads for blocking work.
//! A fixed pool prevents thread explosion and reuses threads across jobs.
//!
//! WHAT: `BackgroundJobRegistry` owns N worker threads that take jobs from a
//! shared FIFO queue and run them to completion with panic protection.
//!
//! HOW: The queue is a `Mutex<VecDeque>` paired with a `Condvar`. Idle workers
//! block on the condvar and `submit()` wakes one of them, so an idle pool costs
//! nothing and a submitted job starts immediately (no sleep-polling).
//! `shutdown()` closes the queue: workers finish every job that was accepted
//! before the close, then exit.

use std::collections::VecDeque;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use crate::synca::WaitGroup;
use crate::valtron::GenericResult;

/// Type alias for a boxed closure submitted as a background job.
///
/// WHY: Provides a concrete type for the job queue
/// WHAT: A heap-allocated, Send, once-callable closure
pub type BackgroundJob = Box<dyn FnOnce() + Send + 'static>;

/// The queue shared by the registry and its workers.
struct JobQueue {
    state: Mutex<QueueState>,
    /// Signalled when a job is pushed (one waiter) or the queue closes (all).
    available: Condvar,
}

struct QueueState {
    jobs: VecDeque<BackgroundJob>,
    closed: bool,
}

impl JobQueue {
    fn new() -> Self {
        Self {
            state: Mutex::new(QueueState {
                jobs: VecDeque::new(),
                closed: false,
            }),
            available: Condvar::new(),
        }
    }

    /// Lock the queue state. A job never runs while the lock is held (jobs run
    /// after `pop` returns), so a poisoned lock can only come from a panic in
    /// this module's own bookkeeping; the state is still consistent, so keep
    /// going rather than cascading the panic into every worker.
    fn lock(&self) -> MutexGuard<'_, QueueState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn push(&self, job: BackgroundJob) -> Result<(), BackgroundJob> {
        {
            let mut state = self.lock();
            if state.closed {
                return Err(job);
            }
            state.jobs.push_back(job);
        }
        self.available.notify_one();
        Ok(())
    }

    /// Block until a job is available (`Some`) or the queue is closed and
    /// empty (`None`).
    fn pop_blocking(&self) -> Option<BackgroundJob> {
        let mut state = self.lock();
        loop {
            if let Some(job) = state.jobs.pop_front() {
                return Some(job);
            }
            if state.closed {
                return None;
            }
            state = self
                .available
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn close(&self) {
        self.lock().closed = true;
        self.available.notify_all();
    }

    fn drain(&self) -> Vec<BackgroundJob> {
        self.lock().jobs.drain(..).collect()
    }
}

/// Run one job, catching (and logging) a panic so the calling thread survives.
fn run_job(job: BackgroundJob, runner: &str) {
    if let Err(panic_info) = std::panic::catch_unwind(AssertUnwindSafe(job)) {
        tracing::error!("Background job panicked on {runner}: {panic_info:?}");
    }
}

/// Fixed-size thread pool for executing blocking, fire-and-forget closures.
///
/// WHY: Replaces unbounded `std::thread::spawn` patterns with a managed pool
///
/// WHAT: Owns N worker threads that consume jobs from a shared FIFO queue. Each
/// job runs to completion on the thread that picks it up. Panics are caught and
/// logged — workers continue processing after a panic.
///
/// HOW: `new()` spawns the workers. Each blocks on a condvar while the queue is
/// empty and is woken by `submit()`. `shutdown()` closes the queue, lets the
/// workers drain it, and joins them.
///
/// # Shutdown semantics
///
/// Every job whose `submit()` returned `Ok` runs exactly once, including jobs
/// still queued when `shutdown()` is called: shutdown stops *intake*, not
/// execution. `shutdown()` therefore returns only after all accepted jobs have
/// finished, so a job that never returns blocks it. Jobs submitted after the
/// close (including from inside a draining job) are rejected with an error.
///
/// # Panics
///
/// Never panics after construction. Worker panics from submitted jobs are
/// caught via `catch_unwind`.
pub struct BackgroundJobRegistry {
    queue: Arc<JobQueue>,
    waitgroup: WaitGroup,
    thread_handles: Mutex<Vec<JoinHandle<()>>>,
    num_threads: usize,
}

impl BackgroundJobRegistry {
    /// Create a new `BackgroundJobRegistry` and spawn worker threads.
    ///
    /// WHY: Initializes the background job pool with the given thread count
    /// WHAT: Creates an unbounded job queue, spawns `num_threads` workers
    /// HOW: Each worker gets a `WaitGroupGuard` and blocks on the shared queue
    ///
    /// The registry's lifetime is its own: workers run until `shutdown()` (or
    /// until the registry is dropped), independent of any pool kill signal, so
    /// accepted jobs are never abandoned mid-queue.
    ///
    /// # Arguments
    ///
    /// * `num_threads` - Number of worker threads to spawn (must be >= 1)
    ///
    /// # Panics
    ///
    /// Panics if `num_threads` is 0, or if the OS refuses to spawn a worker
    /// thread.
    #[must_use]
    pub fn new(num_threads: usize) -> Arc<Self> {
        assert!(
            num_threads >= 1,
            "BackgroundJobRegistry requires at least 1 thread"
        );

        let registry = Arc::new(Self {
            queue: Arc::new(JobQueue::new()),
            waitgroup: WaitGroup::new(),
            thread_handles: Mutex::new(Vec::with_capacity(num_threads)),
            num_threads,
        });

        let mut handles = Vec::with_capacity(num_threads);
        for i in 0..num_threads {
            let queue = registry.queue.clone();
            let wg = registry.waitgroup.clone();
            wg.add(1);

            let handle = thread::Builder::new()
                .name(format!("bg-worker-{i}"))
                .spawn(move || {
                    let _guard = wg.guard();
                    tracing::debug!("Background worker {i} started");
                    Self::worker_loop(&queue, i);
                    tracing::debug!("Background worker {i} exiting");
                })
                .expect("Failed to spawn background worker thread");
            handles.push(handle);
        }
        *registry
            .thread_handles
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = handles;

        registry
    }

    /// Worker thread main loop.
    ///
    /// WHY: Core execution loop for background workers
    /// WHAT: Takes jobs from the queue and executes them with panic protection
    /// HOW: Block until a job arrives -> catch_unwind(job) -> repeat; exit once
    /// the queue is closed and empty
    fn worker_loop(queue: &JobQueue, worker_id: usize) {
        let runner = format!("worker {worker_id}");
        while let Some(job) = queue.pop_blocking() {
            run_job(job, &runner);
        }
    }

    /// Submit a job for execution on a background worker thread.
    ///
    /// WHY: Primary API for submitting blocking work
    /// WHAT: Queues a closure and wakes an idle worker to run it
    ///
    /// # Errors
    ///
    /// Returns an error if the queue is closed (registry is shutting down).
    /// The job is not run in that case.
    pub fn submit(&self, job: impl FnOnce() + Send + 'static) -> GenericResult<()> {
        self.queue
            .push(Box::new(job))
            .map_err(|_| "BackgroundJobRegistry: queue closed, cannot submit job".into())
    }

    /// Shut down the registry: stop intake, run every queued job, join workers.
    ///
    /// WHY: Clean shutdown that neither abandons accepted work nor leaks threads
    /// WHAT: Closes the queue, waits for the workers to drain it and exit, then
    /// joins them. See the type-level "Shutdown semantics".
    /// HOW: close + notify_all -> waitgroup.wait() -> join handles. If jobs are
    /// somehow still queued afterwards (every worker died outside a job), they
    /// are run on the calling thread so the guarantee still holds.
    ///
    /// Idempotent; concurrent callers all return once the workers are gone.
    /// Must not be called from inside a background job (it would wait for
    /// itself).
    pub fn shutdown(&self) {
        self.queue.close();
        self.waitgroup.wait();

        let handles: Vec<JoinHandle<()>> = self
            .thread_handles
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain(..)
            .collect();
        for handle in handles {
            let name = handle.thread().name().unwrap_or("bg-worker").to_string();
            if let Err(panic_info) = handle.join() {
                tracing::error!("Background worker {name} panicked outside a job: {panic_info:?}");
            }
        }

        let leftover = self.queue.drain();
        if !leftover.is_empty() {
            tracing::warn!(
                "BackgroundJobRegistry::shutdown: {} queued job(s) outlived the workers; running them on the shutdown caller",
                leftover.len()
            );
            for job in leftover {
                run_job(job, "the shutdown caller");
            }
        }
    }

    /// Returns the number of worker threads in this registry.
    #[must_use]
    pub fn num_threads(&self) -> usize {
        self.num_threads
    }
}

impl Drop for BackgroundJobRegistry {
    /// Close the queue so the workers drain what was accepted and exit instead
    /// of blocking forever. Drop does not join them; call `shutdown()` to wait.
    fn drop(&mut self) {
        self.queue.close();
    }
}

/// Compute how to split total threads between `ThreadRegistry` and `BackgroundJobRegistry`.
///
/// WHY: Background jobs run I/O-heavy blocking work (cargo builds, network calls,
/// file I/O) where threads spend most of their time waiting, not burning CPU.
/// A larger background pool prevents queue backup when multiple blocking operations
/// coincide. Task threads park efficiently on readiness signals, so they need fewer.
///
/// WHAT: Returns `(task_threads, bg_threads)` where `bg_threads = total` and
/// `task_threads = total / 2`. The bg pool gets burst capacity; the task pool
/// still gets enough threads to multiplex since idle tasks park.
///
/// HOW: task_threads = max(2, total / 2), bg_threads = total.
///
/// # Panics
///
/// Panics if `total` is less than 3 (need ≥ 2 task threads + ≥ 1 bg thread).
#[must_use]
pub fn split_thread_count(total: usize) -> (usize, usize) {
    assert!(total >= 3, "split_thread_count requires at least 3 threads");
    let task_threads = 2.max(total / 2);
    let bg_threads = total;
    (task_threads, bg_threads)
}
