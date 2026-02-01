//! Simple thread pool implementation using `CondVar`.

use foundation_nostd::primitives::{CondVar, CondVarMutex};
use std::sync::Arc;
use std::thread;

type Job = Box<dyn FnOnce() + Send + 'static>;

/// A simple thread pool for executing jobs concurrently.
///
/// # Examples
///
/// ```
/// use foundation_testing::scenarios::ThreadPool;
/// use std::sync::atomic::{AtomicUsize, Ordering};
/// use std::sync::Arc;
///
/// let pool = ThreadPool::new(4);
/// let counter = Arc::new(AtomicUsize::new(0));
///
/// for _ in 0..10 {
///     let counter_clone = Arc::clone(&counter);
///     pool.execute(move || {
///         counter_clone.fetch_add(1, Ordering::Relaxed);
///     });
/// }
///
/// // Wait for all jobs to complete
/// drop(pool);
/// assert_eq!(counter.load(Ordering::Relaxed), 10);
/// ```
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Arc<JobQueue>,
}

impl ThreadPool {
    /// Creates a new thread pool with the given number of workers.
    ///
    /// # Panics
    ///
    /// Panics if `size` is 0.
    #[must_use]
    pub fn new(size: usize) -> Self {
        assert!(size > 0, "ThreadPool size must be > 0");

        let sender = Arc::new(JobQueue::new());
        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&sender)));
        }

        Self { workers, sender }
    }

    /// Executes a job on the thread pool.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.push(job);
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // Signal shutdown
        self.sender.shutdown();

        // Wait for workers to finish
        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                thread.join().expect("Worker thread panicked");
            }
        }
    }
}

struct Worker {
    #[allow(dead_code)]
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, queue: Arc<JobQueue>) -> Self {
        let thread = thread::spawn(move || {
            while let Some(job) = queue.pop() {
                job();
            }
        });

        Self {
            id,
            thread: Some(thread),
        }
    }
}

struct JobQueue {
    queue: CondVarMutex<Vec<Job>>,
    condvar: CondVar,
    shutdown: CondVarMutex<bool>,
}

impl JobQueue {
    fn new() -> Self {
        Self {
            queue: CondVarMutex::new(Vec::new()),
            condvar: CondVar::new(),
            shutdown: CondVarMutex::new(false),
        }
    }

    fn push(&self, job: Job) {
        let mut guard = match self.queue.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        guard.push(job);
        drop(guard);
        self.condvar.notify_one();
    }

    fn pop(&self) -> Option<Job> {
        let mut guard = match self.queue.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };

        loop {
            if let Some(job) = guard.pop() {
                return Some(job);
            }

            // Check for shutdown
            if *match self.shutdown.lock() {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            } {
                return None;
            }

            guard = match self.condvar.wait(guard) {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            };
        }
    }

    fn shutdown(&self) {
        *match self.shutdown.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        } = true;
        self.condvar.notify_all();
    }
}
