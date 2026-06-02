---
name: "Latency Tracking"
description: "Per-endpoint latency tracking with P50/P99 calculation and sliding window"
status: "in-progress"
priority: "high"
complexity: "high"
dependencies:
  - "00-core-timeout-system"
  - "02-cross-protocol-integration"
acceptance_criteria:
  - LatencyTracker stores per-endpoint latency history
  - Sliding window with automatic expiry of old samples
  - P50 and P99 calculation from latency samples
  - Memory bounded (max samples per endpoint)
  - Thread-safe implementation (Arc + RwLock)
  - TimeoutCalculator uses latency data for adjustment
  - Integration with HTTP client and server
---

# Feature: Latency Tracking

## Overview

Implement a per-endpoint latency tracking system that records request/response latencies and calculates P50/P99 percentiles. This data is used by TimeoutCalculator to adjust timeouts based on historical endpoint behavior.

## Architecture

### LatencyTracker

```rust
pub struct LatencyTracker {
    /// Per-endpoint latency history
    endpoints: HashMap<String, EndpointLatency>,
    /// Maximum samples per endpoint (memory bound)
    max_samples: usize,
    /// Sample expiry duration
    sample_ttl: Duration,
}

pub struct EndpointLatency {
    /// Ring buffer of samples
    samples: VecDeque<LatencySample>,
    /// Cached P50 (updated on write)
    cached_p50: Duration,
    /// Cached P99 (updated on write)
    cached_p99: Duration,
}

pub struct LatencySample {
    /// When the request started
    timestamp: Instant,
    /// How long the request took
    duration: Duration,
}
```

### Usage in TimeoutCalculator

```rust
impl TimeoutCalculator {
    /// Calculate read timeout with latency adjustment
    pub fn calculate_read_timeout(&self, ctx: &TimeoutContext) -> Duration {
        let base_timeout = self.size_based_read_timeout(ctx);

        // Apply latency adjustment if endpoint is known
        if let Some(ref endpoint) = ctx.endpoint {
            if let Some(stats) = self.latency_tracker.get_stats(endpoint) {
                // Add P99 as safety margin for known endpoints
                let adjusted = base_timeout + stats.p99;
                return adjusted.clamp(self.config.min_read_timeout, self.config.max_read_timeout);
            }
        }

        base_timeout.clamp(self.config.min_read_timeout, self.config.max_read_timeout)
    }
}
```

## Implementation Tasks

1. **Create latency_tracker.rs module**
   - LatencySample struct
   - EndpointLatency with ring buffer
   - LatencyTracker with HashMap

2. **Implement percentile calculation**
   - P50 (median) calculation
   - P99 calculation
   - Cached values updated on write

3. **Add expiry mechanism**
   - Remove samples older than sample_ttl
   - Periodic cleanup or on-read cleanup

4. **Integrate with TimeoutCalculator**
   - Add latency_tracker field
   - Update calculate_read_timeout to use latency data

5. **Integration points**
   - HTTP client: record latency after each request
   - HTTP server: record latency after each response

## Memory Bounds

- Default max_samples: 1000 per endpoint
- With 10k endpoints: ~10k × 1000 × 24 bytes = ~240MB max
- Cleanup removes expired samples

## Performance Requirements

- P50/P99 calculation: <1ms
- Sample recording: <100µs
- Memory allocation: amortized O(1) with ring buffer

---

_Created: 2026-05-12_
_Last Updated: 2026-05-12_
