//! Performance metrics collection and reporting.

pub mod reporter;

pub use reporter::{PerformanceReport, Reporter};

use core::time::Duration;

/// Performance metrics for stress tests and benchmarks.
#[derive(Debug, Clone)]
pub struct Metrics {
    /// Latency measurements (in nanoseconds)
    pub latencies: Vec<u64>,
    /// Throughput (operations per second)
    pub throughput: f64,
    /// Total operations completed
    pub operations: usize,
    /// Total duration
    pub duration: Duration,
}

impl Metrics {
    /// Creates a new metrics collection.
    #[must_use]
    pub const fn new(operations: usize, duration: Duration) -> Self {
        Self {
            latencies: Vec::new(),
            throughput: 0.0,
            operations,
            duration,
        }
    }

    /// Calculates throughput from operations and duration.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn with_throughput(mut self) -> Self {
        let secs = self.duration.as_secs_f64();
        self.throughput = if secs > 0.0 {
            self.operations as f64 / secs
        } else {
            0.0
        };
        self
    }

    /// Adds latency measurements.
    #[must_use]
    pub fn with_latencies(mut self, latencies: Vec<u64>) -> Self {
        self.latencies = latencies;
        self
    }

    /// Returns the minimum latency in nanoseconds.
    #[must_use]
    pub fn min_latency(&self) -> Option<u64> {
        self.latencies.iter().min().copied()
    }

    /// Returns the maximum latency in nanoseconds.
    #[must_use]
    pub fn max_latency(&self) -> Option<u64> {
        self.latencies.iter().max().copied()
    }

    /// Returns the average latency in nanoseconds.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_latency(&self) -> Option<f64> {
        if self.latencies.is_empty() {
            None
        } else {
            let sum: u64 = self.latencies.iter().sum();
            Some(sum as f64 / self.latencies.len() as f64)
        }
    }

    /// Returns the median latency in nanoseconds.
    #[must_use]
    pub fn median_latency(&self) -> Option<u64> {
        if self.latencies.is_empty() {
            return None;
        }

        let mut sorted = self.latencies.clone();
        sorted.sort_unstable();

        let mid = sorted.len() / 2;
        if sorted.len() % 2 == 0 {
            Some((sorted[mid - 1] + sorted[mid]) / 2)
        } else {
            Some(sorted[mid])
        }
    }

    /// Returns the p95 latency in nanoseconds.
    #[must_use]
    pub fn p95_latency(&self) -> Option<u64> {
        self.percentile_latency(0.95)
    }

    /// Returns the p99 latency in nanoseconds.
    #[must_use]
    pub fn p99_latency(&self) -> Option<u64> {
        self.percentile_latency(0.99)
    }

    /// Returns the latency at the given percentile (0.0 to 1.0).
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn percentile_latency(&self, percentile: f64) -> Option<u64> {
        if self.latencies.is_empty() || !(0.0..=1.0).contains(&percentile) {
            return None;
        }

        let mut sorted = self.latencies.clone();
        sorted.sort_unstable();

        let index = ((sorted.len() as f64) * percentile).ceil() as usize - 1;
        Some(sorted[index.min(sorted.len() - 1)])
    }
}
