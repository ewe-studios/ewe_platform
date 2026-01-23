//! Stress test configuration.

use core::time::Duration;

/// Configuration for stress tests.
#[derive(Debug, Clone, Copy)]
pub struct StressConfig {
    /// Number of threads to spawn
    thread_count: usize,
    /// Number of iterations per thread
    iterations: usize,
    /// Optional maximum duration for the test
    duration: Option<Duration>,
}

impl StressConfig {
    /// Creates a new stress test configuration with default values.
    ///
    /// Defaults:
    /// - `thread_count`: 4
    /// - `iterations`: 1000
    /// - `duration`: None (no time limit)
    #[must_use]
    pub const fn new() -> Self {
        Self {
            thread_count: 4,
            iterations: 1000,
            duration: None,
        }
    }

    /// Sets the number of threads to spawn.
    #[must_use]
    pub const fn threads(mut self, count: usize) -> Self {
        self.thread_count = count;
        self
    }

    /// Sets the number of iterations per thread.
    #[must_use]
    pub const fn iterations(mut self, count: usize) -> Self {
        self.iterations = count;
        self
    }

    /// Sets the maximum duration for the test.
    ///
    /// If the duration is reached, threads will stop early.
    #[must_use]
    pub const fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Sets the duration in seconds.
    #[must_use]
    pub const fn duration_secs(mut self, secs: u64) -> Self {
        self.duration = Some(Duration::from_secs(secs));
        self
    }

    /// Returns the thread count.
    #[must_use]
    pub const fn get_thread_count(&self) -> usize {
        self.thread_count
    }

    /// Returns the iteration count.
    #[must_use]
    pub const fn get_iterations(&self) -> usize {
        self.iterations
    }

    /// Returns the optional duration.
    #[must_use]
    pub const fn get_duration(&self) -> Option<Duration> {
        self.duration
    }
}

impl Default for StressConfig {
    fn default() -> Self {
        Self::new()
    }
}
