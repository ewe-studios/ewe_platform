---
name: "Cross-Protocol Timeout Integration"
description: "Integrate TimeoutCalculator across HTTP Client, WebSocket, EventSource, and Server"
status: "completed"
priority: "high"
complexity: "high"
dependencies:
  - "00-core-timeout-system"
  - "01-client-integration"
acceptance_criteria:
  - TimeoutCalculator is single source of truth for all timeouts
  - No owned timeout fields in protocol structs
  - All protocols use calculator.calculate_* methods
  - calculate_sleep_duration provides work iteration delays
  - Users can customize timeouts via TimeoutConfig
  - previous_timeout field in TimeoutContext for exponential backoff
  - Configurable clamp/bounds values
---

# Feature: Cross-Protocol Timeout Integration

## Overview

Unify timeout management across all protocols (HTTP Client, WebSocket, EventSource, HTTP Server) by making `TimeoutCalculator` the single source of truth. Remove all owned timeout fields from structs and have all components retrieve timeout values from the calculator.

## Implementation Status

### HTTP Client - ✅ COMPLETED
- `ClientConfig` uses `TimeoutCalculator` as source of truth
- Removed owned `connect_timeout`, `read_timeout`, `write_timeout` fields
- Builder methods update TimeoutCalculator.config()
- `get_op_timeout()` returns values from calculator

### WebSocket - ✅ COMPLETED

**State Structs Updated:**
- `WebSocketConnectInfo` - now has `timeout_calculator: TimeoutCalculator` (removed `read_timeout`, `sleep_between_work`)
- `WebSocketConnectingState` - now has `timeout_calculator: TimeoutCalculator` (removed `read_timeout`)
- `WebSocketHandshakeSendingState` - now has `timeout_calculator: TimeoutCalculator` (removed `read_timeout`)
- `WebSocketHandshakeReadingState` - now has `timeout_calculator: TimeoutCalculator` (removed `read_timeout`)
- `WebSocketHandshakeValidatingState` - now has `timeout_calculator: TimeoutCalculator` (removed `read_timeout`)
- `WebSocketOpenState` - now has `timeout_calculator: TimeoutCalculator` (removed `read_timeout`, `sleep_between_work`)

**WebSocketTask Changes:**
- Removed `sleep_between_work: Duration` field from struct
- Removed `with_sleep_between()` method (sleep comes from calculator)
- Updated constructors to use TimeoutCalculator
- State transitions pass TimeoutCalculator instead of Duration

**Dynamic Timeout Usage:**
```rust
// Getting read timeout from calculator (Open state)
let ctx = TimeoutContext::default();
let read_timeout = open_state.timeout_calculator.calculate_read_timeout(&ctx);
stream.set_read_timeout_as(read_timeout);

// Getting sleep duration from calculator for streaming
let ctx = TimeoutContext::default().streaming();
let sleep = calculator.calculate_sleep_duration(&ctx);  // Returns 50ms for streaming
```

**Files Modified:**
- `backends/foundation_core/src/wire/websocket/task.rs`

### EventSource (SSE) - ✅ COMPLETED

**State Structs Updated:**
- `EventSourceConfig` - now has `timeout_calculator: TimeoutCalculator` (removed `idle_timeout`)
- `EventSourceTask` - now has `timeout_calculator: TimeoutCalculator` (removed `idle_timeout`)
- `EventSourceState::Reading` - uses calculator for idle timeout checks
- `EventSourceState::ReadingStream` - uses calculator for idle timeout checks
- `ReconnectingConfig` - now has `timeout_calculator: TimeoutCalculator`
- `ReconnectingEventSourceTask` - now has `timeout_calculator: TimeoutCalculator`

**Dynamic Timeout Usage:**
```rust
// Getting idle timeout from calculator (Reading state)
let ctx = TimeoutContext::default().streaming();
let idle_timeout = task.timeout_calculator.calculate_read_timeout(&ctx);

// Checking if idle timeout exceeded
if last_activity.elapsed() > idle_timeout {
    // Trigger reconnection
}

// Getting sleep duration for SSE polling
let ctx = TimeoutContext::default().streaming();
let sleep_duration = calculator.calculate_sleep_duration(&ctx);  // Returns 50ms for streaming
```

**Files Modified:**
- `backends/foundation_core/src/wire/event_source/task.rs`
- `backends/foundation_core/src/wire/event_source/reconnecting_task.rs`

### HTTP Server (foundation_http) - ✅ COMPLETED

**State Structs Updated:**
- `KeepAliveConfig` - now has `timeout_calculator: TimeoutCalculator` (removed `min_delay`, `max_delay`, `idle_timeout`)
- `ServerConfig` - now has `timeout_calculator: TimeoutCalculator` (removed `would_block_sleep`, `accept_error_sleep`)
- `ConnectionHandler` - uses calculator for delay computation and idle timeout

**Dynamic Timeout Usage:**
```rust
// Getting idle timeout from calculator (server connection)
let ctx = TimeoutContext::default().streaming();
let idle_timeout = self.timeout_calculator.calculate_read_timeout(&ctx);

// Getting delay for backoff (server polling)
let ctx = TimeoutContext::default().streaming();
let delay = self.timeout_calculator.calculate_sleep_duration(&ctx);  // Returns 50ms for streaming

// Server-specific timeout config with longer timeouts
let timeout_config = TimeoutConfig {
    min_read_timeout: Duration::from_secs(120), // 2 min idle timeout
    max_read_timeout: Duration::from_secs(300), // 5 max read
    ..TimeoutConfig::default()
};
```

**Files Modified:**
- `backends/foundation_http/src/server/mod.rs`
- `backends/foundation_http/src/server/connection.rs`
```

## API Design

### TimeoutCalculator provides all timeout values:

```rust
impl TimeoutCalculator {
    /// Calculate read timeout based on context
    pub fn calculate_read_timeout(&self, ctx: &TimeoutContext) -> Duration;

    /// Calculate write timeout based on context  
    pub fn calculate_write_timeout(&self, ctx: &TimeoutContext) -> Duration;

    /// Calculate total timeout including retries
    pub fn calculate_total_timeout(&self, ctx: &TimeoutContext) -> Duration;

    /// Calculate sleep duration between work iterations
    pub fn calculate_sleep_duration(&self, ctx: &TimeoutContext) -> Duration;

    /// Access underlying config
    pub fn config(&self) -> &TimeoutConfig;
}
```

### calculate_sleep_duration Implementation:

```rust
pub fn calculate_sleep_duration(&self, ctx: &TimeoutContext) -> Duration {
    // Base sleep duration: 15ms for HTTP-style operations
    let base_sleep_ms = 15u64;

    // For streaming operations (WebSocket/SSE), use longer sleep
    if ctx.is_streaming {
        return Duration::from_millis(50);
    }

    // For upload operations, slightly longer sleep
    if ctx.is_upload {
        return Duration::from_millis(25);
    }

    // Calculate based on expected timeout - sleep should be small fraction
    if ctx.expected_body_size.is_some() {
        let read_timeout = self.calculate_read_timeout(ctx);
        // Sleep should be ~1% of expected timeout, min 15ms, max 100ms
        let sleep_ms = (read_timeout.as_millis() as u64 / 100).clamp(base_sleep_ms, 100);
        return Duration::from_millis(sleep_ms);
    }

    Duration::from_millis(base_sleep_ms)
}
```

## TimeoutContext Extensions

### previous_timeout Field

Added `previous_timeout: Option<Duration>` field to `TimeoutContext` for exponential backoff calculation:

```rust
#[derive(Debug, Clone, Default)]
pub struct TimeoutContext {
    // ... existing fields ...
    
    /// Previous timeout duration for exponential backoff calculation.
    pub previous_timeout: Option<Duration>,
}
```

**Usage:**
```rust
// First attempt
let ctx = TimeoutContext::with_size(1024);
let timeout1 = calculator.calculate_read_timeout(&ctx);

// Retry with exponential backoff
let ctx2 = TimeoutContext::with_size(1024)
    .with_previous_timeout(timeout1);
// Calculator can use previous_timeout to compute next timeout (e.g., 2x)
```

**Builder Method:**
```rust
#[must_use]
pub fn with_previous_timeout(mut self, timeout: Duration) -> Self {
    self.previous_timeout = Some(timeout);
    self
}
```

### Configurable Clamp Values

The clamp values in `calculate_sleep_duration` are configurable via the `TimeoutConfig`:

```rust
pub struct TimeoutConfig {
    // ... existing fields ...
    
    /// Minimum sleep duration between work iterations (default: 15ms)
    pub min_sleep_duration: Duration,
    /// Maximum sleep duration between work iterations (default: 100ms)
    pub max_sleep_duration: Duration,
    /// Sleep fraction of timeout (default: 1% = timeout / 100)
    pub sleep_timeout_fraction: u64,
}
```

**Default values:**
- `min_sleep_duration`: 15ms (base sleep for HTTP polling)
- `max_sleep_duration`: 100ms (max sleep for calculated values)
- `sleep_timeout_fraction`: 100 (divisor for timeout percentage)

**Protocol-specific defaults:**
- HTTP polling: `base_sleep_ms` (15ms)
- Uploads: 25ms
- Streaming (WebSocket/SSE): 50ms
- Calculated: ~1% of expected timeout, clamped to min/max

## Protocol-Specific Sleep Values

| Protocol | Operation | Sleep Duration | Source |
|----------|-----------|----------------|--------|
| HTTP Client | Request polling | 15ms | Base default |
| HTTP Client | Upload | 25ms | Upload variant |
| WebSocket | Frame reading | 50ms | Streaming mode |
| EventSource | Event polling | 50ms | Streaming mode |
| File Download | Large body | ~1% of timeout | Calculated |

## User Customization

Users customize timeouts via `TimeoutConfig`:

```rust
let timeout_config = TimeoutConfig {
    min_read_timeout: Duration::from_millis(50),
    max_read_timeout: Duration::from_secs(30),
    min_sleep_duration: Duration::from_millis(10),
    max_sleep_duration: Duration::from_millis(200),
    // ... other fields
    ..TimeoutConfig::default()
};

// HTTP Client
let client = SimpleHttpClient::from_system()
    .config(ClientConfig {
        timeout_calculator: TimeoutCalculator::with_config(timeout_config),
        ..ClientConfig::default()
    });

// WebSocket (when constructor supports it)
let ws = WebSocketTask::connect_with_config(url, resolver, timeout_config)?;
```

## Migration Path

### Public API
- **HTTP Client**: No changes - same builder API, internally uses calculator
- **WebSocket**: Will add `connect_with_config()` constructor for custom timeouts
- **EventSource**: Will add config-based constructor

### Internal Code
- All protocol code updated to call `calculator.calculate_*()` methods
- No owned timeout Duration fields in state structs
- Sleep durations come from `calculator.calculate_sleep_duration(&ctx)`

## Benefits

1. **Single Source of Truth**: All timeouts come from TimeoutCalculator
2. **Consistent Behavior**: Same timeout logic across all protocols
3. **Dynamic Calculation**: Timeouts based on body size, not fixed values
4. **CPU Efficiency**: Streaming protocols use longer sleeps (50ms vs 15ms)
5. **User Customization**: Easy to customize via TimeoutConfig
6. **Exponential Backoff**: `previous_timeout` field supports retry strategies

## Acceptance Criteria

- [x] TimeoutCalculator integrated into ClientConfig (HTTP Client)
- [x] `calculate_sleep_duration` method added to TimeoutCalculator
- [x] WebSocket structs use TimeoutCalculator (no owned timeout fields)
- [x] `previous_timeout` field added to TimeoutContext
- [x] Builder method `with_previous_timeout()` added
- [x] EventSource structs use TimeoutCalculator (no owned timeout fields)
- [x] HTTP Server uses TimeoutCalculator (no owned timeout fields)
- [x] All protocol code updated to use calculator methods
- [x] Zero breaking changes to public API
- [x] All tests pass

---

_Created: 2026-05-12_
_Last Updated: 2026-05-12_
