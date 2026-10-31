//! Background Job Registry — fixed thread pool for blocking, fire-and-forget closures.
//!
//! WHY: ThreadedFuture and other code spawns unbounded OS threads for blocking work.
//! A fixed pool prevents thread explosion and reuses threads across jobs.
//!
//! WHAT: `BackgroundJobRegistry` owns N worker threads that take jobs from a
//! shared FIFO queue and run them to completion with panic protection.
//!
//! HOW: The queue is a `Mutex<VecDeque>` paired with a `CondVar` from
//! `foundation_nostd::comp::condvar_comp` (std types under std). Idle workers
//! block on the condvar and `submit()` wakes one of them, so an idle pool costs
//! nothing and a submitted job starts immediately (no sleep-polling).
//! `shutdown()` closes the queue: workers finish every job that was accepted
//! before the close, then exit.

use std::collections::VecDeque;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use foundation_nostd::comp::condvar_comp::{CondVar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

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
    available: CondVar,
}

struct QueueState {
    jobs: VecDeque<BackgroundJob>,
    closed: bool,
    capacity: usize,
}

/// Why a push failed.
enum JobPushError {
    /// The queue is closed; the job will never run.
    Closed,
    /// The queue is at its configured capacity.
    Full,
}

impl JobQueue {
    fn new(capacity: usize) -> Self {
        Self {
            state: Mutex::new(QueueState {
                jobs: VecDeque::new(),
                closed: false,
                capacity,
            }),
            available: CondVar::new(),
        }
    }

    /// Lock the queue state. A job never runs while the lock is held (jobs run
    /// after `pop` returns), so a poisoned lock can only come from a panic in
    /// this module's own bookkeeping; the state is still consistent, so keep
    /// going rather than cascading the panic into every worker.
    fn lock(&self) -> MutexGuard<'_, QueueState> {
        self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn push(&self, job: BackgroundJob) -> Result<(), JobPushError> {
        {
            let mut state = self.lock();
            if state.closed {
                return Err(JobPushError::Closed);
            }
            if state.jobs.len() >= state.capacity {
                return Err(JobPushError::Full);
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
                .unwrap_or_else(|poisoned| poisoned.into_inner());
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

/// How a [`BackgroundJobRegistry`] is sized and shut down.
///
/// WHY: Limits are configuration, not constants (CLAUDE.md rule 8): the
/// shutdown deadline in particular must be tunable, because no single value
/// fits every embedder.
///
/// WHAT: The worker count and the longest `shutdown()` may wait for accepted
/// jobs before giving up on them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackgroundJobConfig {
    num_threads: usize,
    shutdown_timeout: Duration,
    capacity: usize,
}

impl BackgroundJobConfig {
    /// Default for [`Self::shutdown_timeout`]: 30 seconds, Kubernetes' default
    /// `terminationGracePeriodSeconds` — the time an orchestrator gives a
    /// process to finish in-flight work before killing it. (systemd's
    /// `DefaultTimeoutStopSec` is a laxer 90 s.)
    pub const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

    /// Default for [`Self::capacity`]: 1024 pending jobs. Large enough that a
    /// healthy caller never hits it, small enough to bound memory under a
    /// stalled worker (each queued job is a heap closure of tens of bytes, so
    /// 1024 is on the order of tens of KiB).
    pub const DEFAULT_CAPACITY: usize = 1024;

    /// A config for `num_threads` workers with the default shutdown timeout and
    /// queue capacity.
    #[must_use]
    pub fn new(num_threads: usize) -> Self {
        Self {
            num_threads,
            shutdown_timeout: Self::DEFAULT_SHUTDOWN_TIMEOUT,
            capacity: Self::DEFAULT_CAPACITY,
        }
    }

    /// The longest `shutdown()` waits for accepted jobs to finish. Past it,
    /// jobs that have not started are dropped and workers still inside a job
    /// are detached, so shutdown never blocks forever.
    #[must_use]
    pub fn with_shutdown_timeout(mut self, timeout: Duration) -> Self {
        self.shutdown_timeout = timeout;
        self
    }

    /// The bounded queue capacity. A `submit` that would exceed it returns an
    /// error instead of buffering without limit.
    #[must_use]
    pub fn with_capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    /// Number of worker threads.
    #[must_use]
    pub fn num_threads(&self) -> usize {
        self.num_threads
    }

    /// The configured shutdown deadline.
    #[must_use]
    pub fn shutdown_timeout(&self) -> Duration {
        self.shutdown_timeout
    }

    /// The configured queue capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// What [`BackgroundJobRegistry::shutdown`] managed to do before returning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum ShutdownOutcome {
    /// Every accepted job ran and every worker was joined.
    Completed,
    /// The shutdown deadline passed first.
    TimedOut {
        /// Accepted jobs that had not started and were dropped without running.
        abandoned_jobs: usize,
        /// Workers still inside a job; they were detached, not joined, and
        /// exit on their own once that job returns.
        busy_workers: usize,
    },
}

/// Why a [`BackgroundJobRegistry`] could not be created.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundJobError {
    /// The operating system refused to spawn a worker thread.
    SpawnFailed,
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
/// Shutdown stops *intake*, not execution: every job whose `submit()` returned
/// `Ok` — including jobs still queued when `shutdown()` is called — runs exactly
/// once, provided they all finish within the configured shutdown timeout
/// ([`BackgroundJobConfig::with_shutdown_timeout`], default
/// [`BackgroundJobConfig::DEFAULT_SHUTDOWN_TIMEOUT`]). If the deadline passes
/// first, `shutdown()` does not block forever: jobs that have not started are
/// dropped, workers stuck inside a job are detached, and the result is
/// [`ShutdownOutcome::TimedOut`]. Jobs submitted after the close (including
/// from inside a draining job) are rejected with an error.
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
    shutdown_timeout: Duration,
}

impl BackgroundJobRegistry {
    /// Create a new `BackgroundJobRegistry` and spawn worker threads.
    ///
    /// WHY: Initializes the background job pool with the given thread count
    /// WHAT: Creates a bounded job queue, spawns `num_threads` workers
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
    /// # Errors
    ///
    /// Returns [`BackgroundJobError::SpawnFailed`] if the OS refuses to spawn a
    /// worker thread.
    ///
    /// # Panics
    ///
    /// Panics if `num_threads` is 0.
    #[must_use]
    pub fn new(num_threads: usize) -> Result<Arc<Self>, BackgroundJobError> {
        Self::with_config(BackgroundJobConfig::new(num_threads))
    }

    /// Create a registry from a [`BackgroundJobConfig`] and spawn its workers.
    ///
    /// # Errors
    ///
    /// Returns [`BackgroundJobError::SpawnFailed`] if the OS refuses to spawn a
    /// worker thread.
    ///
    /// # Panics
    ///
    /// Panics if `config.num_threads()` is 0.
    #[must_use]
    pub fn with_config(config: BackgroundJobConfig) -> Result<Arc<Self>, BackgroundJobError> {
        let num_threads = config.num_threads;
        assert!(
            num_threads >= 1,
            "BackgroundJobRegistry requires at least 1 thread"
        );

        let registry = Arc::new(Self {
            queue: Arc::new(JobQueue::new(config.capacity)),
            waitgroup: WaitGroup::new(),
            thread_handles: Mutex::new(Vec::with_capacity(num_threads)),
            num_threads,
            shutdown_timeout: config.shutdown_timeout,
        });

        let mut handles: Vec<JoinHandle<()>> = Vec::with_capacity(num_threads);
        for i in 0..num_threads {
            let queue = registry.queue.clone();
            let wg = registry.waitgroup.clone();
            wg.add(1);

            let spawn = thread::Builder::new()
                .name(format!("bg-worker-{i}"))
                .spawn(move || {
                    let _guard = wg.guard();
                    tracing::debug!("Background worker {i} started");
                    Self::worker_loop(&queue, i);
                    tracing::debug!("Background worker {i} exiting");
                });
            let handle = match spawn {
                Ok(handle) => handle,
                Err(e) => {
                    // Close the queue so any already-spawned workers drain and
                    // exit, then join them before returning the error.
                    registry.queue.close();
                    for h in handles.drain(..) {
                        let _ = h.join();
                    }
                    tracing::error!("BackgroundJobRegistry: failed to spawn worker {i}: {e}");
                    return Err(BackgroundJobError::SpawnFailed);
                }
            };
            handles.push(handle);
        }
        *registry
            .thread_handles
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = handles;

        Ok(registry)
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
    /// Returns an error if the queue is closed (the registry is shutting down)
    /// or full (at its configured capacity). The job is not run in that case.
    pub fn submit(&self, job: impl FnOnce() + Send + 'static) -> GenericResult<()> {
        self.queue.push(Box::new(job)).map_err(|e| match e {
            JobPushError::Closed => {
                "BackgroundJobRegistry: queue closed, cannot submit job".into()
            }
            JobPushError::Full => {
                "BackgroundJobRegistry: queue full, cannot submit job".into()
            }
        })
    }

    /// Shut down the registry: stop intake, run the queued jobs, join workers —
    /// all within the configured shutdown timeout.
    ///
    /// WHY: Clean shutdown that neither abandons accepted work nor leaks
    /// threads, and that can never hang on a job that does not return.
    /// WHAT: Closes the queue and waits, up to the shutdown timeout, for the
    /// workers to drain it and exit. See the type-level "Shutdown semantics".
    /// HOW: close + notify_all -> `waitgroup.wait_timeout(deadline)`.
    /// - In time: join every worker. Jobs still queued (only possible if every
    ///   worker died outside a job) run on the calling thread while the
    ///   deadline allows; the rest are dropped.
    /// - Too late: drop the jobs that never started, join the workers that
    ///   already exited, and detach the ones still inside a job (they exit by
    ///   themselves when it returns, since the queue is closed and empty).
    ///
    /// Idempotent; concurrent callers each wait at most the timeout. Must not
    /// be called from inside a background job (it would wait for itself until
    /// the timeout).
    pub fn shutdown(&self) -> ShutdownOutcome {
        let deadline = Instant::now() + self.shutdown_timeout;
        self.queue.close();
        let drained_in_time = self.waitgroup.wait_timeout(self.shutdown_timeout);

        let handles: Vec<JoinHandle<()>> = self
            .thread_handles
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .drain(..)
            .collect();
        let mut busy_workers = 0;
        for handle in handles {
            if !drained_in_time && !handle.is_finished() {
                // Still inside a job: dropping the handle detaches the thread.
                busy_workers += 1;
                continue;
            }
            let name = handle.thread().name().unwrap_or("bg-worker").to_string();
            if let Err(panic_info) = handle.join() {
                tracing::error!("Background worker {name} panicked outside a job: {panic_info:?}");
            }
        }

        let mut abandoned_jobs = 0;
        let leftover = self.queue.drain();
        if !leftover.is_empty() && drained_in_time {
            tracing::warn!(
                "BackgroundJobRegistry::shutdown: {} queued job(s) outlived the workers; running them on the shutdown caller",
                leftover.len()
            );
        }
        for job in leftover {
            if drained_in_time && Instant::now() < deadline {
                run_job(job, "the shutdown caller");
            } else {
                abandoned_jobs += 1;
            }
        }

        if busy_workers == 0 && abandoned_jobs == 0 {
            return ShutdownOutcome::Completed;
        }
        tracing::error!(
            "BackgroundJobRegistry::shutdown: timed out after {:?}; dropped {abandoned_jobs} unstarted job(s), detached {busy_workers} worker(s) still inside a job",
            self.shutdown_timeout
        );
        ShutdownOutcome::TimedOut {
            abandoned_jobs,
            busy_workers,
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
