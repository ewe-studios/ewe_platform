//! Latency tracker tests (moved from `src/shared/http/latency_tracker.rs`).

use std::time::{Duration, Instant};

use foundation_netio::shared::http::latency_tracker::{
    percentile, EndpointLatency, LatencySample, LatencyTracker,
};

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
