//! Background Job Registry — fixed thread pool for blocking, fire-and-forget closures.
//!
//! WHY: ThreadedFuture and other code spawns unbounded OS threads for blocking work.
//! A fixed pool prevents thread explosion and reuses threads across jobs.
//!
//! WHAT: `BackgroundJobRegistry` owns N worker threads that pop jobs from a
//! `ConcurrentQueue` and run them to completion with panic protection.
//!
//! HOW: Workers loop: pop job -> catch_unwind(job) -> repeat. When the queue
//! is empty they yield for a configured duration. Shutdown via shared `OnSignal`.

use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use concurrent_queue::ConcurrentQueue;

use crate::synca::{OnSignal, WaitGroup};
use crate::valtron::GenericResult;

/// Type alias for a boxed closure submitted as a background job.
///
/// WHY: Provides a concrete type for the job queue
/// WHAT: A heap-allocated, Send, once-callable closure
pub type BackgroundJob = Box<dyn FnOnce() + Send + 'static>;

/// Default yield duration when the job queue is empty.
///
/// WHY: Prevents busy-spinning when no jobs are available
/// WHAT: Workers sleep for this duration before re-checking the queue
pub const DEFAULT_BG_YIELD_DURATION: Duration = Duration::from_millis(50);

/// Fixed-size thread pool for executing blocking, fire-and-forget closures.
///
/// WHY: Replaces unbounded `std::thread::spawn` patterns with a managed pool
///
/// WHAT: Owns N worker threads that consume jobs from a shared `ConcurrentQueue`.
/// Each job runs to completion on the thread that picks it up. Panics are caught
/// and logged — workers continue processing after a panic.
///
/// HOW: `new()` spawns worker threads with `WaitGroup` tracking. Workers loop:
/// check kill signal -> pop job -> `catch_unwind(job)` -> repeat. Empty queue
/// causes a yield. `shutdown()` turns on the kill signal and waits for workers.
///
/// # Panics
///
/// Never panics. Worker panics from submitted jobs are caught via `catch_unwind`.
pub struct BackgroundJobRegistry {
    job_queue: Arc<ConcurrentQueue<BackgroundJob>>,
    kill_signal: Arc<OnSignal>,
    waitgroup: WaitGroup,
    thread_handles: std::sync::Mutex<Vec<JoinHandle<()>>>,
    num_threads: usize,
}

impl BackgroundJobRegistry {
    /// Create a new `BackgroundJobRegistry` and spawn worker threads.
    ///
    /// WHY: Initializes the background job pool with the given thread count
    /// WHAT: Creates an unbounded job queue, spawns `num_threads` workers
    /// HOW: Each worker gets a `WaitGroupGuard` and loops on the shared queue
    ///
    /// # Arguments
    ///
    /// * `num_threads` - Number of worker threads to spawn (must be >= 1)
    /// * `kill_signal` - Shared kill signal (typically from `ThreadRegistry`)
    /// * `yield_duration` - How long workers sleep when queue is empty
    ///
    /// # Panics
    ///
    /// Panics if `num_threads` is 0.
    pub fn new(
        num_threads: usize,
        kill_signal: Arc<OnSignal>,
        yield_duration: Duration,
    ) -> Arc<Self> {
        assert!(
            num_threads >= 1,
            "BackgroundJobRegistry requires at least 1 thread"
        );

        let job_queue: Arc<ConcurrentQueue<BackgroundJob>> = Arc::new(ConcurrentQueue::unbounded());
        let waitgroup = WaitGroup::new();

        let registry = Arc::new(Self {
            job_queue,
            kill_signal,
            waitgroup,
            thread_handles: std::sync::Mutex::new(Vec::with_capacity(num_threads)),
            num_threads,
        });

        for i in 0..num_threads {
            let queue = registry.job_queue.clone();
            let kill = registry.kill_signal.clone();
            let wg = registry.waitgroup.clone();
            wg.add(1);

            let handle = thread::Builder::new()
                .name(format!("bg-worker-{i}"))
                .spawn(move || {
                    let _guard = wg.guard();
                    tracing::debug!("Background worker {i} started");
                    Self::worker_loop(&queue, &kill, yield_duration, i);
                    tracing::debug!("Background worker {i} exiting");
                })
                .expect("Failed to spawn background worker thread");

            registry.thread_handles.lock().unwrap().push(handle);
        }

        registry
    }

    /// Worker thread main loop.
    ///
    /// WHY: Core execution loop for background workers
    /// WHAT: Pops jobs from queue and executes them with panic protection
    /// HOW: Check kill signal -> pop -> catch_unwind(job) -> yield if empty
    fn worker_loop(
        queue: &ConcurrentQueue<BackgroundJob>,
        kill_signal: &OnSignal,
        yield_duration: Duration,
        worker_id: usize,
    ) {
        loop {
            if kill_signal.probe() {
                break;
            }

            match queue.pop() {
                Ok(job) => {
                    let result = std::panic::catch_unwind(AssertUnwindSafe(job));
                    if let Err(panic_info) = result {
                        tracing::error!(
                            "Background job panicked on worker {worker_id}: {panic_info:?}"
                        );
                    }
                }
                Err(_) => {
                    // Queue empty — yield to avoid busy-spinning
                    thread::sleep(yield_duration);
                }
            }
        }
    }

    /// Submit a job for execution on a background worker thread.
    ///
    /// WHY: Primary API for submitting blocking work
    /// WHAT: Pushes a closure onto the job queue for a worker to pick up
    /// HOW: Wraps the closure in a `Box` and pushes to the `ConcurrentQueue`
    ///
    /// # Errors
    ///
    /// Returns an error if the queue is closed (registry is shutting down).
    pub fn submit(&self, job: impl FnOnce() + Send + 'static) -> GenericResult<()> {
        self.job_queue
            .push(Box::new(job))
            .map_err(|_| "BackgroundJobRegistry: queue closed, cannot submit job".into())
    }

    /// Shut down the registry: signal kill, wait for workers, join handles.
    ///
    /// WHY: Clean shutdown ensuring all workers exit before returning
    /// WHAT: Turns on kill signal, waits for WaitGroup, joins all thread handles
    /// HOW: kill_signal.turn_on() -> close queue -> waitgroup.wait() -> join handles
    ///
    /// # Panics
    ///
    /// Never panics. Thread join failures are silently ignored.
    pub fn shutdown(&self) {
        // Close the queue so no new jobs can be submitted
        self.job_queue.close();
        // Note: kill_signal is shared and typically turned on by PoolGuard,
        // but we ensure it here for standalone usage
        self.kill_signal.turn_on();
        // Wait for all workers to finish
        self.waitgroup.wait();
        // Join all thread handles
        let mut handles = self.thread_handles.lock().unwrap();
        for handle in handles.drain(..) {
            let _ = handle.join();
        }
    }

    /// Returns the number of worker threads in this registry.
    #[must_use]
    pub fn num_threads(&self) -> usize {
        self.num_threads
    }
}

/// Compute how to split total threads between `ThreadRegistry` and `BackgroundJobRegistry`.
///
/// WHY: ThreadRegistry needs the larger share for cooperative tasks; background
/// jobs need fewer threads since they run blocking work to completion
///
/// WHAT: Returns `(task_threads, bg_threads)` where `bg_threads = max(1, total / 3)`
///
/// HOW: Integer division by 3 with a floor of 1 for background threads
///
/// # Panics
///
/// Panics if `total` is less than 2.
#[must_use]
pub fn split_thread_count(total: usize) -> (usize, usize) {
    assert!(total >= 2, "split_thread_count requires at least 2 threads");
    let bg_threads = 1.max(total / 3);
    let task_threads = total - bg_threads;
    (task_threads, bg_threads)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// WHY: split_thread_count must follow the specified allocation table
    /// WHAT: Verify (task_threads, bg_threads) for known input values
    #[test]
    fn test_split_thread_count() {
        assert_eq!(split_thread_count(2), (1, 1));
        assert_eq!(split_thread_count(3), (2, 1));
        assert_eq!(split_thread_count(4), (3, 1));
        assert_eq!(split_thread_count(5), (4, 1));
        assert_eq!(split_thread_count(6), (4, 2));
        assert_eq!(split_thread_count(7), (5, 2));
        assert_eq!(split_thread_count(8), (6, 2));
        assert_eq!(split_thread_count(9), (6, 3));
        assert_eq!(split_thread_count(10), (7, 3));
        assert_eq!(split_thread_count(12), (8, 4));
        assert_eq!(split_thread_count(16), (11, 5));
    }

    #[test]
    #[should_panic(expected = "split_thread_count requires at least 2 threads")]
    fn test_split_thread_count_panics_below_2() {
        let _ = split_thread_count(1);
    }

    /// WHY: BackgroundJobRegistry must execute submitted jobs
    /// WHAT: Submit a job that writes to shared state, verify it ran
    #[test]
    fn test_submit_and_execute_job() {
        let kill_signal = Arc::new(OnSignal::new());
        let registry =
            BackgroundJobRegistry::new(2, kill_signal.clone(), Duration::from_millis(10));

        let result = Arc::new(Mutex::new(false));
        let result_clone = result.clone();

        registry
            .submit(move || {
                *result_clone.lock().unwrap() = true;
            })
            .expect("submit should succeed");

        // Give worker time to pick up the job
        thread::sleep(Duration::from_millis(100));

        assert!(*result.lock().unwrap(), "job should have executed");

        registry.shutdown();
    }

    /// WHY: Workers must survive panicking jobs
    /// WHAT: Submit a panicking job, then a normal job — both should be handled
    #[test]
    #[cfg_attr(
        cranelift_backend,
        ignore = "cranelift does not support panic unwinding"
    )]
    fn test_panic_recovery() {
        let kill_signal = Arc::new(OnSignal::new());
        let registry =
            BackgroundJobRegistry::new(1, kill_signal.clone(), Duration::from_millis(10));

        // Submit a panicking job
        registry
            .submit(|| {
                panic!("intentional test panic");
            })
            .expect("submit should succeed");

        // Give worker time to process the panic
        thread::sleep(Duration::from_millis(100));

        // Submit a normal job — worker should still be alive
        let result = Arc::new(Mutex::new(false));
        let result_clone = result.clone();

        registry
            .submit(move || {
                *result_clone.lock().unwrap() = true;
            })
            .expect("submit should succeed after panic");

        thread::sleep(Duration::from_millis(100));

        assert!(
            *result.lock().unwrap(),
            "worker should recover from panic and execute next job"
        );

        registry.shutdown();
    }

    /// WHY: shutdown must cause workers to exit and join cleanly
    /// WHAT: Create registry, submit work, shutdown, verify no hanging threads
    #[test]
    fn test_shutdown() {
        let kill_signal = Arc::new(OnSignal::new());
        let registry =
            BackgroundJobRegistry::new(2, kill_signal.clone(), Duration::from_millis(10));

        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        for _ in 0..10 {
            let c = counter.clone();
            registry
                .submit(move || {
                    c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                })
                .expect("submit should succeed");
        }

        // Give time for jobs to complete
        thread::sleep(Duration::from_millis(200));

        registry.shutdown();

        assert_eq!(
            counter.load(std::sync::atomic::Ordering::SeqCst),
            10,
            "all 10 jobs should have executed"
        );
    }

    /// WHY: submit must fail after shutdown
    /// WHAT: Shut down registry, try to submit — should return Err
    #[test]
    fn test_submit_after_shutdown_fails() {
        let kill_signal = Arc::new(OnSignal::new());
        let registry =
            BackgroundJobRegistry::new(1, kill_signal.clone(), Duration::from_millis(10));

        registry.shutdown();

        let result = registry.submit(|| {});
        assert!(result.is_err(), "submit after shutdown should fail");
    }
}
