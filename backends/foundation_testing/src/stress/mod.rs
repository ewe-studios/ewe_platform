//! Stress test framework for synchronization primitives.
//!
//! Provides configurable high-contention testing with:
//! - Thread count control
//! - Iteration limits
//! - Time-based duration
//! - Success rate tracking
//! - Performance metrics collection

use core::time::Duration;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

pub mod config;
pub mod sync;

pub use config::StressConfig;

/// Result of a stress test run.
#[derive(Debug, Clone)]
pub struct StressResult {
    /// Total operations completed successfully
    pub successes: usize,
    /// Total operations that failed
    pub failures: usize,
    /// Total time taken for the test
    pub duration: Duration,
    /// Number of threads used
    pub thread_count: usize,
}

impl StressResult {
    /// Creates a new stress test result.
    #[must_use]
    pub const fn new(
        successes: usize,
        failures: usize,
        duration: Duration,
        thread_count: usize,
    ) -> Self {
        Self {
            successes,
            failures,
            duration,
            thread_count,
        }
    }

    /// Returns the total number of operations.
    #[must_use]
    pub const fn total_operations(&self) -> usize {
        self.successes + self.failures
    }

    /// Returns the success rate as a value between 0.0 and 1.0.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn success_rate(&self) -> f64 {
        if self.total_operations() == 0 {
            0.0
        } else {
            self.successes as f64 / self.total_operations() as f64
        }
    }

    /// Returns operations per second.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn operations_per_second(&self) -> f64 {
        let secs = self.duration.as_secs_f64();
        if secs == 0.0 {
            0.0
        } else {
            self.total_operations() as f64 / secs
        }
    }
}

/// Base stress test harness.
///
/// Spawns multiple threads that execute a closure repeatedly
/// until the test completes (based on iteration count or duration).
pub struct StressHarness {
    config: StressConfig,
}

impl StressHarness {
    /// Creates a new stress test harness with the given configuration.
    #[must_use]
    pub const fn new(config: StressConfig) -> Self {
        Self { config }
    }

    /// Runs a stress test with the given operation closure.
    ///
    /// The closure receives:
    /// - `thread_id`: Index of the thread (`0..thread_count`)
    /// - `iteration`: Iteration number for this thread
    ///
    /// Returns `true` on success, `false` on failure.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_testing::stress::{StressConfig, StressHarness};
    /// use std::sync::atomic::{AtomicUsize, Ordering};
    /// use std::sync::Arc;
    ///
    /// let counter = Arc::new(AtomicUsize::new(0));
    /// let config = StressConfig::new().threads(4).iterations(100);
    /// let harness = StressHarness::new(config);
    ///
    /// let counter_clone = Arc::clone(&counter);
    /// let result = harness.run(move |_thread_id, _iteration| {
    ///     counter_clone.fetch_add(1, Ordering::Relaxed);
    ///     true
    /// });
    ///
    /// assert_eq!(counter.load(Ordering::Relaxed), 400); // 4 threads * 100 iterations
    /// assert_eq!(result.successes, 400);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if any worker thread panics during the stress test execution.
    pub fn run<F>(self, operation: F) -> StressResult
    where
        F: Fn(usize, usize) -> bool + Send + Sync + 'static,
    {
        let start = std::time::Instant::now();
        let operation = Arc::new(operation);

        let successes = Arc::new(AtomicUsize::new(0));
        let failures = Arc::new(AtomicUsize::new(0));
        let stop_flag = Arc::new(AtomicBool::new(false));

        // Spawn timeout thread if duration is set
        if let Some(duration) = self.config.get_duration() {
            let stop_flag_clone = Arc::clone(&stop_flag);
            thread::spawn(move || {
                thread::sleep(duration);
                stop_flag_clone.store(true, Ordering::Release);
            });
        }

        // Spawn worker threads
        let mut handles = Vec::with_capacity(self.config.get_thread_count());

        for thread_id in 0..self.config.get_thread_count() {
            let operation = Arc::clone(&operation);
            let successes = Arc::clone(&successes);
            let failures = Arc::clone(&failures);
            let stop_flag = Arc::clone(&stop_flag);
            let iterations = self.config.get_iterations();

            let handle = thread::spawn(move || {
                for iteration in 0..iterations {
                    // Check stop condition
                    if stop_flag.load(Ordering::Acquire) {
                        break;
                    }

                    // Execute operation
                    if operation(thread_id, iteration) {
                        successes.fetch_add(1, Ordering::Relaxed);
                    } else {
                        failures.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });

            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().expect("Thread panicked during stress test");
        }

        let duration = start.elapsed();

        StressResult::new(
            successes.load(Ordering::Relaxed),
            failures.load(Ordering::Relaxed),
            duration,
            self.config.get_thread_count(),
        )
    }
}
