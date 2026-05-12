---
name: "Client Classification"
description: "Classify clients as Normal/Slow/Suspicious for DoS protection"
status: "completed"
priority: "high"
complexity: "medium"
dependencies:
  - "00-core-timeout-system"
  - "04-load-based-scaling"
acceptance_criteria:
  - ClientClassifier tracks bytes/second per client IP
  - Classification: Normal (>10 KB/s), Slow (1-10 KB/s), Suspicious (<1 KB/s)
  - Progressive penalties for slow/suspicious clients
  - Automatic rehabilitation after successful fast transfers
  - Thread-safe implementation
  - Integration with HTTP server connection handler
  - Security logging for suspicious clients
---

# Feature: Client Classification

## Overview

Implement client classification for DoS protection. Classify clients based on transfer rate (bytes/second) and apply progressive penalties to slow or suspicious clients.

## Client Classification

| Classification | Bytes/Second | Consecutive Slow | Penalty |
|----------------|--------------|------------------|---------|
| **Normal** | >10 KB/s | 0 | None |
| **Slow** | 1-10 KB/s | 1-3 | +50% timeout |
| **Suspicious** | <1 KB/s | 3+ | -50% timeout, log security |

## Architecture

### ClientClassifier

```rust
pub struct ClientClassifier {
    /// Per-client transfer tracking.
    clients: HashMap<String, ClientStats>,
    /// Classification thresholds.
    thresholds: ClassificationThresholds,
    /// Security logger (optional).
    security_logger: Option<Box<dyn SecurityLogger>>,
}

pub struct ClientStats {
    /// Total bytes transferred.
    bytes_transferred: usize,
    /// Transfer start time.
    start_time: Instant,
    /// Current classification.
    classification: ClientClassification,
    /// Consecutive slow transfers.
    consecutive_slow: u32,
    /// Last classification update.
    last_update: Instant,
}

pub enum ClientClassification {
    Normal,
    Slow,
    Suspicious,
}
```

### Classification Logic

```rust
impl ClientClassifier {
    /// Classify a client based on current transfer rate.
    pub fn classify(&mut self, client_ip: &str, bytes: usize, duration: Duration) -> ClientClassification {
        let rate = bytes as f64 / duration.as_secs_f64();
        let kbps = rate / 1024.0;

        let classification = if kbps > self.thresholds.normal_threshold {
            ClientClassification::Normal
        } else if kbps > self.thresholds.slow_threshold {
            ClientClassification::Slow
        } else {
            ClientClassification::Suspicious
        };

        // Update stats and apply progressive penalties
        self.update_client_stats(client_ip, classification);

        classification
    }

    /// Get timeout adjustment for a client.
    pub fn timeout_multiplier(&self, client_ip: &str) -> f64 {
        match self.get_classification(client_ip) {
            ClientClassification::Normal => 1.0,
            ClientClassification::Slow => 1.5,  // +50% timeout
            ClientClassification::Suspicious => 0.5,  // -50% timeout (fail fast)
        }
    }
}
```

### Integration with TimeoutCalculator

```rust
impl TimeoutCalculator {
    /// Calculate timeout with client classification.
    pub fn calculate_read_timeout(&self, ctx: &TimeoutContext, client_ip: Option<&str>) -> Duration {
        let base_timeout = // ... existing calculation

        // Apply client classification
        let adjusted = if let Some(ip) = client_ip {
            if let Some(ref classifier) = self.client_classifier {
                let multiplier = classifier.timeout_multiplier(ip);
                Duration::from_millis((base_timeout.as_millis() as f64 * multiplier) as u64)
            } else {
                base_timeout
            }
        } else {
            base_timeout
        };

        adjusted
    }
}
```

## Implementation Tasks

1. **Create client_classifier.rs module**
   - ClientClassification enum
   - ClassificationThresholds config
   - ClientStats tracking
   - ClientClassifier with HashMap

2. **Implement classification logic**
   - Transfer rate calculation
   - Progressive penalty application
   - Automatic rehabilitation

3. **Add security logging**
   - Log suspicious clients
   - Configurable logger trait

4. **Integrate with TimeoutCalculator**
   - Add client_classifier field
   - Apply timeout multiplier

5. **Integration with HTTP server**
   - Track transfers per connection
   - Classify on request completion

## Memory Bounds

- Per-client stats: ~100 bytes
- With 100k concurrent clients: ~10MB
- Automatic cleanup of idle clients (TTL)

## Performance Requirements

- Classification: <1µs
- Stats update: <100ns
- No locks in hot path (use atomics)

---

_Created: 2026-05-12_
_Last Updated: 2026-05-12_
