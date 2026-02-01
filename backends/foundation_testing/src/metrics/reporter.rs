//! Performance report generation.

use super::Metrics;
use std::fmt;

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

        report.push_str(&format!("=== {} ===\n", self.title));
        report.push_str(&format!("Operations: {}\n", self.metrics.operations));
        report.push_str(&format!("Duration: {:?}\n", self.metrics.duration));
        report.push_str(&format!(
            "Throughput: {:.2} ops/sec\n",
            self.metrics.throughput
        ));

        if !self.metrics.latencies.is_empty() {
            report.push_str("\nLatency (ns):\n");
            if let Some(min) = self.metrics.min_latency() {
                report.push_str(&format!("  Min: {}\n", min));
            }
            if let Some(avg) = self.metrics.avg_latency() {
                report.push_str(&format!("  Avg: {:.0}\n", avg));
            }
            if let Some(median) = self.metrics.median_latency() {
                report.push_str(&format!("  Median: {}\n", median));
            }
            if let Some(p95) = self.metrics.p95_latency() {
                report.push_str(&format!("  P95: {}\n", p95));
            }
            if let Some(p99) = self.metrics.p99_latency() {
                report.push_str(&format!("  P99: {}\n", p99));
            }
            if let Some(max) = self.metrics.max_latency() {
                report.push_str(&format!("  Max: {}\n", max));
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
