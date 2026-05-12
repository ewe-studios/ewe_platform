---
name: "Load-Based Scaling"
description: "Adjust timeouts based on system load for resource fairness"
status: "in-progress"
priority: "high"
complexity: "medium"
dependencies:
  - "00-core-timeout-system"
  - "03-latency-tracking"
acceptance_criteria:
  - Load factor calculated from concurrent connections or CPU usage
  - Timeout reduction: 100% -> 90% -> 75% -> 60% based on load level
  - Load tracking is thread-safe and efficient
  - TimeoutCalculator uses load factor for adjustment
  - Integration with HTTP server connection handler
---

# Feature: Load-Based Scaling

## Overview

Implement load-based timeout scaling to reduce timeouts under high load, providing resource fairness and preventing cascading failures.

## Load Levels

| Load Level | Connections/Sec | Timeout Adjustment |
|------------|-----------------|-------------------|
| Low | <100 | 100% (no change) |
| Medium | 100-1000 | 90% (-10%) |
| High | 1000-10000 | 75% (-25%) |
| Critical | >10000 | 60% (-40%) |

## Architecture

### LoadTracker

```rust
pub struct LoadTracker {
    /// Current number of active connections.
    active_connections: AtomicUsize,
    /// Recent connection rate (connections/sec).
    connection_rate: AtomicU64,
    /// Last rate calculation time.
    last_calculation: AtomicInstant,
    /// Calculation interval.
    calculation_interval: Duration,
}

impl LoadTracker {
    /// Get current load level.
    pub fn current_load_level(&self) -> LoadLevel {
        let rate = self.connection_rate();
        match rate {
            r if r < 100 => LoadLevel::Low,
            r if r < 1000 => LoadLevel::Medium,
            r if r < 10000 => LoadLevel::High,
            _ => LoadLevel::Critical,
        }
    }

    /// Get timeout adjustment factor.
    pub fn timeout_factor(&self) -> f64 {
        match self.current_load_level() {
            LoadLevel::Low => 1.0,
            LoadLevel::Medium => 0.9,
            LoadLevel::High => 0.75,
            LoadLevel::Critical => 0.6,
        }
    }
}
```

### Integration with TimeoutCalculator

```rust
impl TimeoutCalculator {
    /// Calculate read timeout with load adjustment.
    pub fn calculate_read_timeout(&self, ctx: &TimeoutContext) -> Duration {
        let base_timeout = // ... size + latency calculation

        // Apply load adjustment
        let adjusted = if let Some(ref load_tracker) = self.load_tracker {
            let factor = load_tracker.timeout_factor();
            Duration::from_millis((base_timeout.as_millis() as f64 * factor) as u64)
        } else {
            base_timeout
        };

        adjusted.clamp(self.config.min_read_timeout, self.config.max_read_timeout)
    }
}
```

## Implementation Tasks

1. **Create load_tracker.rs module**
   - LoadLevel enum
   - LoadTracker with atomic counters
   - Connection rate calculation

2. **Update TimeoutCalculator**
   - Add load_tracker field
   - Apply timeout factor in calculations

3. **Integration points**
   - HTTP server: increment/decrement active connections in ConnectionHandler
   - Update feature documentation

## Performance Requirements

- Load calculation: <1µs (atomic reads)
- Connection tracking: <100ns (atomic increment/decrement)
- No locks in hot path

---

_Created: 2026-05-12_
_Last Updated: 2026-05-12_
