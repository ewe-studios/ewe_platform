//! Performance report generation.

use super::Metrics;
use std::fmt;
use std::fmt::Write;

/// Performance report with formatted metrics.
pub struct PerformanceReport {
    title: String,
    metrics: Metrics,
}

impl PerformanceReport {
    /// Creates a new performance report.
    #[must_use]
    pub fn new(title: impl Into<String>, metrics: Metrics) -> Self {
        Self {
            title: title.into(),
            metrics,
        }
    }

    /// Returns the metrics.
    #[must_use]
    pub const fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    /// Generates a human-readable report.
    #[must_use]
    pub fn to_string_pretty(&self) -> String {
        let mut report = String::new();

        let _ = writeln!(report, "=== {} ===", self.title);
        let _ = writeln!(report, "Operations: {}", self.metrics.operations);
        let _ = writeln!(report, "Duration: {:?}", self.metrics.duration);
        let _ = writeln!(
            report,
            "Throughput: {:.2} ops/sec",
            self.metrics.throughput
        );

        if !self.metrics.latencies.is_empty() {
            report.push_str("\nLatency (ns):\n");
            if let Some(min) = self.metrics.min_latency() {
                let _ = writeln!(report, "  Min: {min}");
            }
            if let Some(avg) = self.metrics.avg_latency() {
                let _ = writeln!(report, "  Avg: {avg:.0}");
            }
            if let Some(median) = self.metrics.median_latency() {
                let _ = writeln!(report, "  Median: {median}");
            }
            if let Some(p95) = self.metrics.p95_latency() {
                let _ = writeln!(report, "  P95: {p95}");
            }
            if let Some(p99) = self.metrics.p99_latency() {
                let _ = writeln!(report, "  P99: {p99}");
            }
            if let Some(max) = self.metrics.max_latency() {
                let _ = writeln!(report, "  Max: {max}");
            }
        }

        report
    }
}

impl fmt::Display for PerformanceReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_pretty())
    }
}

/// Reporter for generating performance reports.
pub struct Reporter;

impl Reporter {
    /// Generates a report from metrics.
    #[must_use]
    pub fn generate(title: impl Into<String>, metrics: Metrics) -> PerformanceReport {
        PerformanceReport::new(title, metrics)
    }
}
