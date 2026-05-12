//! Per-endpoint latency tracking for adaptive timeout calculation.
//!
//! WHY: Different endpoints have different latency characteristics. API endpoints
//! may respond in <100ms while upload endpoints take seconds. Static timeouts
//! can't adapt to these differences.
//!
//! WHAT: LatencyTracker records per-endpoint request latencies and calculates
//! P50 (median) and P99 percentiles. TimeoutCalculator uses these to adjust
//! timeouts based on historical endpoint behavior.
//!
//! HOW: Ring buffer per endpoint stores recent samples. Percentiles are calculated
//! on-demand or cached. Old samples are expired based on TTL.
//!
//! # Example
//!
//! ```
//! use foundation_core::wire::simple_http::timeout::{LatencyTracker, TimeoutCalculator};
//! use std::time::Duration;
//!
//! let tracker = LatencyTracker::with_defaults();
//!
//! // Record a latency sample
//! tracker.record("api.example.com/users", Duration::from_millis(150));
//!
//! // Get percentile stats
//! if let Some(stats) = tracker.get_stats("api.example.com/users") {
//!     println!("P50: {:?}, P99: {:?}", stats.p50, stats.p99);
//! }
//! ```

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// A single latency sample.
#[derive(Debug, Clone, Copy)]
pub struct LatencySample {
    /// When the sample was recorded.
    pub timestamp: Instant,
    /// The latency duration.
    pub duration: Duration,
}

impl LatencySample {
    /// Create a new latency sample.
    #[must_use]
    pub fn new(duration: Duration) -> Self {
        Self {
            timestamp: Instant::now(),
            duration,
        }
    }
}

/// Statistics for an endpoint's latency distribution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatencyStats {
    /// 50th percentile (median) latency.
    pub p50: Duration,
    /// 99th percentile latency.
    pub p99: Duration,
    /// Number of samples in the calculation.
    pub sample_count: usize,
    /// When these stats were calculated.
    pub calculated_at: Instant,
}

/// Per-endpoint latency history with ring buffer.
#[derive(Debug)]
pub struct EndpointLatency {
    /// Ring buffer of samples (oldest at front).
    samples: VecDeque<LatencySample>,
    /// Maximum samples to store.
    max_samples: usize,
    /// Cached statistics.
    cached_stats: Option<LatencyStats>,
    /// When cache was last updated.
    cache_timestamp: Instant,
}

impl EndpointLatency {
    /// Create a new endpoint latency tracker.
    #[must_use]
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples.min(100)),
            max_samples,
            cached_stats: None,
            cache_timestamp: Instant::now(),
        }
    }

    /// Record a new latency sample.
    ///
    /// If at capacity, removes the oldest sample.
    pub fn record(&mut self, sample: LatencySample) {
        // Remove oldest if at capacity
        if self.samples.len() >= self.max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
        // Invalidate cache
        self.cached_stats = None;
    }

    /// Get statistics for this endpoint.
    ///
    /// Calculates percentiles if not cached or if cache is stale.
    #[must_use]
    pub fn get_stats(&mut self, max_cache_age: Duration) -> Option<LatencyStats> {
        // Return cached if fresh
        if let Some(ref stats) = self.cached_stats {
            if self.cache_timestamp.elapsed() < max_cache_age {
                return Some(*stats);
            }
        }

        if self.samples.is_empty() {
            return None;
        }

        // Calculate percentiles
        let stats = self.calculate_percentiles();
        self.cached_stats = Some(stats);
        self.cache_timestamp = Instant::now();
        Some(stats)
    }

    /// Calculate P50 and P99 from samples.
    fn calculate_percentiles(&self) -> LatencyStats {
        let durations: Vec<u64> = self
            .samples
            .iter()
            .map(|s| s.duration.as_millis() as u64)
            .collect();

        let p50 = percentile(&durations, 50);
        let p99 = percentile(&durations, 99);

        LatencyStats {
            p50: Duration::from_millis(p50),
            p99: Duration::from_millis(p99),
            sample_count: durations.len(),
            calculated_at: Instant::now(),
        }
    }

    /// Remove samples older than the given duration.
    pub fn expire_old_samples(&mut self, ttl: Duration) {
        let now = Instant::now();
        while let Some(sample) = self.samples.front() {
            if now.duration_since(sample.timestamp) > ttl {
                self.samples.pop_front();
                // Invalidate cache
                self.cached_stats = None;
            } else {
                break;
            }
        }
    }

    /// Get the number of samples.
    #[must_use]
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }
}

/// Calculate percentile from sorted data.
fn percentile(data: &[u64], p: usize) -> u64 {
    if data.is_empty() {
        return 0;
    }

    let mut sorted = data.to_vec();
    sorted.sort_unstable();

    let idx = (sorted.len() * p) / 100;
    let idx = idx.min(sorted.len() - 1);

    sorted[idx]
}

/// Configuration for LatencyTracker.
#[derive(Debug, Clone, Copy)]
pub struct LatencyTrackerConfig {
    /// Maximum samples per endpoint.
    pub max_samples_per_endpoint: usize,
    /// Sample TTL before expiry.
    pub sample_ttl: Duration,
    /// Cache TTL for calculated statistics.
    pub stats_cache_ttl: Duration,
    /// Cleanup interval (0 for on-read cleanup only).
    pub cleanup_interval: Duration,
}

impl Default for LatencyTrackerConfig {
    fn default() -> Self {
        Self {
            max_samples_per_endpoint: 1000,
            sample_ttl: Duration::from_secs(3600), // 1 hour
            stats_cache_ttl: Duration::from_secs(60), // 1 minute
            cleanup_interval: Duration::ZERO, // On-read cleanup
        }
    }
}

/// Thread-safe per-endpoint latency tracker.
///
/// WHY: Multiple threads may record latencies for the same endpoint.
/// WHAT: RwLock-protected HashMap of endpoint -> EndpointLatency.
#[derive(Debug, Clone)]
pub struct LatencyTracker {
    /// Per-endpoint latency history.
    endpoints: Arc<RwLock<HashMap<String, EndpointLatency>>>,
    /// Configuration.
    config: LatencyTrackerConfig,
    /// Last cleanup time.
    last_cleanup: Arc<RwLock<Instant>>,
}

impl LatencyTracker {
    /// Create a new latency tracker with default config.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(LatencyTrackerConfig::default())
    }

    /// Create a new latency tracker with custom config.
    #[must_use]
    pub fn with_config(config: LatencyTrackerConfig) -> Self {
        Self {
            endpoints: Arc::new(RwLock::new(HashMap::new())),
            config,
            last_cleanup: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Record a latency sample for an endpoint.
    ///
    /// Creates the endpoint entry if it doesn't exist.
    pub fn record(&self, endpoint: &str, duration: Duration) {
        let sample = LatencySample::new(duration);

        let mut endpoints = self.endpoints.write().expect("poisoned lock");
        let endpoint_latency = endpoints
            .entry(endpoint.to_string())
            .or_insert_with(|| EndpointLatency::new(self.config.max_samples_per_endpoint));

        endpoint_latency.record(sample);
    }

    /// Get latency statistics for an endpoint.
    ///
    /// Returns None if no samples exist for the endpoint.
    #[must_use]
    pub fn get_stats(&self, endpoint: &str) -> Option<LatencyStats> {
        // Try cleanup if needed
        self.maybe_cleanup();

        let mut endpoints = self.endpoints.write().expect("poisoned lock");
        endpoints
            .get_mut(endpoint)
            .and_then(|ep| ep.get_stats(self.config.stats_cache_ttl))
    }

    /// Check if we have enough samples for an endpoint to provide reliable stats.
    #[must_use]
    pub fn has_reliable_stats(&self, endpoint: &str, min_samples: usize) -> bool {
        let endpoints = self.endpoints.read().expect("poisoned lock");
        endpoints
            .get(endpoint)
            .map(|ep| ep.sample_count() >= min_samples)
            .unwrap_or(false)
    }

    /// Remove an endpoint from tracking.
    pub fn remove_endpoint(&self, endpoint: &str) {
        let mut endpoints = self.endpoints.write().expect("poisoned lock");
        endpoints.remove(endpoint);
    }

    /// Clear all endpoints.
    pub fn clear(&self) {
        let mut endpoints = self.endpoints.write().expect("poisoned lock");
        endpoints.clear();
    }

    /// Get the number of tracked endpoints.
    #[must_use]
    pub fn endpoint_count(&self) -> usize {
        let endpoints = self.endpoints.read().expect("poisoned lock");
        endpoints.len()
    }

    /// Run cleanup to remove expired samples.
    fn maybe_cleanup(&self) {
        // Skip if cleanup interval is 0 (on-read cleanup only)
        if self.config.cleanup_interval.is_zero() {
            return;
        }

        let should_cleanup = {
            let last = self.last_cleanup.read().expect("poisoned lock");
            last.elapsed() > self.config.cleanup_interval
        };

        if should_cleanup {
            let mut endpoints = self.endpoints.write().expect("poisoned lock");
            for endpoint_latency in endpoints.values_mut() {
                endpoint_latency.expire_old_samples(self.config.sample_ttl);
            }

            // Remove empty endpoints
            endpoints.retain(|_, ep| ep.sample_count() > 0);

            let mut last = self.last_cleanup.write().expect("poisoned lock");
            *last = Instant::now();
        }
    }

    /// Force immediate cleanup of expired samples.
    pub fn cleanup(&self) {
        let mut endpoints = self.endpoints.write().expect("poisoned lock");
        for endpoint_latency in endpoints.values_mut() {
            endpoint_latency.expire_old_samples(self.config.sample_ttl);
        }

        // Remove empty endpoints
        endpoints.retain(|_, ep| ep.sample_count() > 0);

        let mut last = self.last_cleanup.write().expect("poisoned lock");
        *last = Instant::now();
    }
}

impl Default for LatencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_sample_new() {
        let before = Instant::now();
        let sample = LatencySample::new(Duration::from_millis(100));
        let after = Instant::now();

        assert_eq!(sample.duration, Duration::from_millis(100));
        assert!(sample.timestamp >= before);
        assert!(sample.timestamp <= after);
    }

    #[test]
    fn test_endpoint_latency_record() {
        let mut ep = EndpointLatency::new(3);

        ep.record(LatencySample::new(Duration::from_millis(100)));
        ep.record(LatencySample::new(Duration::from_millis(200)));
        ep.record(LatencySample::new(Duration::from_millis(300)));

        assert_eq!(ep.sample_count(), 3);

        // Should evict oldest
        ep.record(LatencySample::new(Duration::from_millis(400)));
        assert_eq!(ep.sample_count(), 3);
    }

    #[test]
    fn test_percentile_calculation() {
        let data = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];

        let p50 = percentile(&data, 50);
        let p99 = percentile(&data, 99);

        // P50 should be around the median
        assert!(p50 >= 50 && p50 <= 60);
        // P99 should be near the max
        assert_eq!(p99, 100);
    }

    #[test]
    fn test_endpoint_latency_stats() {
        let mut ep = EndpointLatency::new(100);

        // Add samples: 10, 20, 30, ..., 100
        for i in 1..=10 {
            ep.record(LatencySample::new(Duration::from_millis(i * 10)));
        }

        let stats = ep.get_stats(Duration::from_secs(60)).unwrap();

        // P50 should be around 50-60ms
        assert!(stats.p50.as_millis() >= 50 && stats.p50.as_millis() <= 60);
        // P99 should be around 100ms
        assert!(stats.p99.as_millis() >= 90 && stats.p99.as_millis() <= 100);
        assert_eq!(stats.sample_count, 10);
    }

    #[test]
    fn test_latency_tracker_record_and_get() {
        let tracker = LatencyTracker::new();

        tracker.record("api.example.com", Duration::from_millis(100));
        tracker.record("api.example.com", Duration::from_millis(200));
        tracker.record("api.example.com", Duration::from_millis(300));

        let stats = tracker.get_stats("api.example.com").unwrap();
        assert_eq!(stats.sample_count, 3);
        assert!(stats.p50.as_millis() > 0);
    }

    #[test]
    fn test_latency_tracker_multiple_endpoints() {
        let tracker = LatencyTracker::new();

        tracker.record("api1.example.com", Duration::from_millis(100));
        tracker.record("api2.example.com", Duration::from_millis(200));

        assert_eq!(tracker.endpoint_count(), 2);

        let stats1 = tracker.get_stats("api1.example.com").unwrap();
        let stats2 = tracker.get_stats("api2.example.com").unwrap();

        assert_eq!(stats1.sample_count, 1);
        assert_eq!(stats2.sample_count, 1);
    }

    #[test]
    fn test_latency_tracker_unknown_endpoint() {
        let tracker = LatencyTracker::new();

        let stats = tracker.get_stats("unknown.example.com");
        assert!(stats.is_none());
    }

    #[test]
    fn test_latency_tracker_clear() {
        let tracker = LatencyTracker::new();

        tracker.record("api.example.com", Duration::from_millis(100));
        assert_eq!(tracker.endpoint_count(), 1);

        tracker.clear();
        assert_eq!(tracker.endpoint_count(), 0);
    }

    #[test]
    fn test_cache_invalidation() {
        let mut ep = EndpointLatency::new(100);

        ep.record(LatencySample::new(Duration::from_millis(100)));
        let stats1 = ep.get_stats(Duration::from_secs(60)).unwrap();

        // Add more samples - cache should be invalidated
        ep.record(LatencySample::new(Duration::from_millis(500)));
        ep.record(LatencySample::new(Duration::from_millis(500)));
        let stats2 = ep.get_stats(Duration::from_secs(60)).unwrap();

        // P50 should have increased
        assert!(stats2.p50 > stats1.p50);
    }

    #[test]
    fn test_empty_percentile() {
        let data: Vec<u64> = vec![];
        let p50 = percentile(&data, 50);
        assert_eq!(p50, 0);
    }

    #[test]
    fn test_single_sample_percentile() {
        let data = vec![42];
        let p50 = percentile(&data, 50);
        let p99 = percentile(&data, 99);
        assert_eq!(p50, 42);
        assert_eq!(p99, 42);
    }
}
