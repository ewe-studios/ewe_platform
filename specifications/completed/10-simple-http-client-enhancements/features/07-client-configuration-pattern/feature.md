---
workspace_name: "ewe_platform"
spec_directory: "specifications/10-simple-http-client-enhancements"
feature_directory: "specifications/10-simple-http-client-enhancements/features/07-client-configuration-pattern"
this_file: "specifications/10-simple-http-client-enhancements/features/07-client-configuration-pattern/feature.md"

status: completed
priority: medium
created: "2026-03-25"
completed: "2026-03-26"

depends_on: []

tasks:
  completed: 4
  uncompleted: 0
  total: 4
  completion_percentage: 100
---

# Client Configuration Pattern

## Overview

This feature documents the fluent builder pattern for comprehensive HTTP client configuration.

## WHY: Problem Statement

Users need to configure HTTP clients for different use cases (API fetching, large downloads, streaming). The configuration should be ergonomic and discoverable.

Without a fluent builder pattern:
- Configuration is verbose and error-prone
- Default values are unclear
- Related settings aren't grouped logically
- Discoverability is poor

### Source Pattern Analysis

From `gen_model_descriptors/mod.rs`:

```rust
let mut client = SimpleHttpClient::from_system()
    .max_body_size(None)           // No limit for large payloads
    .batch_size(8192 * 2)          // 16KB read buffer
    .read_timeout(Duration::from_secs(1))
    .max_retries(5)
    .enable_pool(10);              // Max 10 pooled connections
```

### Complete Configuration Example

```rust
let client = SimpleHttpClient::from_system()
    // Body handling
    .max_body_size(Some(100 * 1024 * 1024))  // 100MB max
    .batch_size(16384)                       // 16KB reads
    .full_body_threshold(512 * 1024)         // 512KB threshold

    // Timeouts
    .read_timeout(Duration::from_secs(30))
    .connect_timeout(Duration::from_secs(15))
    .write_timeout(Duration::from_secs(30))

    // Reliability
    .max_retries(5)
    .max_redirects(10)

    // Connection pooling
    .enable_pool(20)
    .pool_idle_timeout(Duration::from_secs(90))

    // Features
    .follow_redirects(true)
    .decompress(true);
```

## WHAT: Solution Overview

Fluent builder pattern with logical grouping:

### Configuration Categories

| Category | Methods | Purpose |
|----------|---------|---------|
| Body Handling | `max_body_size()`, `batch_size()`, `full_body_threshold()` | Control memory usage and streaming |
| Timeouts | `read_timeout()`, `connect_timeout()`, `write_timeout()` | Control request timeouts |
| Reliability | `max_retries()`, `max_redirects()` | Control retry and redirect behavior |
| Pooling | `enable_pool()`, `pool_idle_timeout()` | Control connection reuse |
| Features | `follow_redirects()`, `decompress()` | Enable/disable features |

### Body Handling Configuration

```rust
/// Configure maximum body size.
///
/// # Arguments
/// * `max_size` - Maximum body size in bytes, or None for unlimited
///
/// # Use Cases
/// - `None`: API responses of unknown size
/// - `Some(1024 * 1024)`: Limit to 1MB for safety
/// - `Some(100 * 1024 * 1024)`: Allow 100MB for file downloads
///
/// # Examples
///
/// ```
/// // No limit (API responses)
/// let client = SimpleHttpClient::from_system()
///     .max_body_size(None);
///
/// // 1MB limit (safety)
/// let client = SimpleHttpClient::from_system()
///     .max_body_size(Some(1024 * 1024));
/// ```
pub fn max_body_size(mut self, max_size: Option<u64>) -> Self {
    self.config.max_body_size = max_size;
    self
}

/// Configure read batch size.
///
/// # Arguments
/// * `size` - Number of bytes per read operation
///
/// # Use Cases
/// - `8192`: Default, good for most cases
/// - `16384`: High-latency networks (fewer syscalls)
/// - `4096`: Memory-constrained environments
///
/// # Examples
///
/// ```
/// // High-latency network
/// let client = SimpleHttpClient::from_system()
///     .batch_size(16384);
/// ```
pub fn batch_size(mut self, size: usize) -> Self {
    self.config.batch_size = size;
    self
}

/// Configure full body threshold.
///
/// # Arguments
/// * `threshold` - Size threshold for buffering full body
///
/// # Use Cases
/// - Responses smaller than threshold are buffered
/// - Responses larger are streamed
///
/// # Examples
///
/// ```
/// // Buffer responses up to 512KB, stream larger
/// let client = SimpleHttpClient::from_system()
///     .full_body_threshold(512 * 1024);
/// ```
pub fn full_body_threshold(mut self, threshold: u64) -> Self {
    self.config.full_body_threshold = threshold;
    self
}
```

### Timeout Configuration

```rust
/// Configure read timeout.
///
/// # Arguments
/// * `timeout` - Maximum time for read operations
///
/// # Use Cases
/// - `Duration::from_secs(1)`: Fast-fail for responsive APIs
/// - `Duration::from_secs(30)`: Slow APIs, large responses
/// - `Duration::from_secs(300)`: File downloads
///
/// # Examples
///
/// ```
/// // Fast-fail for responsive API
/// let client = SimpleHttpClient::from_system()
///     .read_timeout(Duration::from_secs(1));
/// ```
pub fn read_timeout(mut self, timeout: Duration) -> Self {
    self.config.read_timeout = timeout;
    self
}

/// Configure connect timeout.
///
/// # Arguments
/// * `timeout` - Maximum time for TCP connection
///
/// # Use Cases
/// - `Duration::from_secs(5)`: Fast networks
/// - `Duration::from_secs(15)`: Default, handles slow DNS
/// - `Duration::from_secs(60)`: Very slow networks
///
/// # Examples
///
/// ```
/// // Handle slow DNS resolution
/// let client = SimpleHttpClient::from_system()
///     .connect_timeout(Duration::from_secs(30));
/// ```
pub fn connect_timeout(mut self, timeout: Duration) -> Self {
    self.config.connect_timeout = timeout;
    self
}

/// Configure write timeout.
///
/// # Arguments
/// * `timeout` - Maximum time for write operations
///
/// # Use Cases
/// - `Duration::from_secs(10)`: Default for most requests
/// - `Duration::from_secs(60)`: Large uploads
///
/// # Examples
///
/// ```
/// // Large file uploads
/// let client = SimpleHttpClient::from_system()
///     .write_timeout(Duration::from_secs(300));
/// ```
pub fn write_timeout(mut self, timeout: Duration) -> Self {
    self.config.write_timeout = timeout;
    self
}
```

### Reliability Configuration

```rust
/// Configure maximum retries.
///
/// # Arguments
/// * `max_retries` - Maximum retry attempts
///
/// # Use Cases
/// - `0`: No retries (fail fast)
/// - `3`: Idempotent operations
/// - `5`: Default, handles transient failures
/// - `100`: Very resilient for critical operations
///
/// # Examples
///
/// ```
/// // No retries for idempotent operations
/// let client = SimpleHttpClient::from_system()
///     .max_retries(0);
///
/// // High resilience for critical fetches
/// let client = SimpleHttpClient::from_system()
///     .max_retries(10);
/// ```
pub fn max_retries(mut self, max_retries: usize) -> Self {
    self.config.max_retries = max_retries;
    self
}

/// Configure maximum redirects.
///
/// # Arguments
/// * `max_redirects` - Maximum redirect chain length
///
/// # Use Cases
/// - `0`: No redirects (security)
/// - `5`: Default, handles most redirect chains
/// - `10`: Complex redirect scenarios
///
/// # Examples
///
/// ```
/// // No redirects for security
/// let client = SimpleHttpClient::from_system()
///     .max_redirects(0);
/// ```
pub fn max_redirects(mut self, max_redirects: u8) -> Self {
    self.config.max_redirects = max_redirects;
    self
}
```

### Connection Pooling Configuration

```rust
/// Enable connection pooling.
///
/// # Arguments
/// * `max_connections` - Maximum connections in pool
///
/// # Use Cases
/// - `0`: Disable pooling (new connection per request)
/// - `10`: Default, good for moderate concurrency
/// - `100`: High concurrency applications
///
/// # Examples
///
/// ```
/// // Pool for parallel fetches
/// let client = SimpleHttpClient::from_system()
///     .enable_pool(10);
///
/// // No pooling (always fresh connection)
/// let client = SimpleHttpClient::from_system()
///     .enable_pool(0);
/// ```
pub fn enable_pool(mut self, max_connections: usize) -> Self {
    self.config.pool_size = max_connections;
    self
}

/// Configure pool idle timeout.
///
/// # Arguments
/// * `timeout` - Time before idle connections are closed
///
/// # Use Cases
/// - `Duration::from_secs(15)`: Aggressive cleanup
/// - `Duration::from_secs(90)`: Default, balances reuse and resources
/// - `Duration::from_secs(300)`: Keep connections warm
///
/// # Examples
///
/// ```
/// // Keep connections warm for bursty traffic
/// let client = SimpleHttpClient::from_system()
///     .enable_pool(10)
///     .pool_idle_timeout(Duration::from_secs(300));
/// ```
pub fn pool_idle_timeout(mut self, timeout: Duration) -> Self {
    self.config.pool_idle_timeout = timeout;
    self
}
```

## HOW: Implementation

### Pre-configured Client Presets

Provide common configuration presets:

```rust
impl SimpleHttpClient {
    /// Create a client optimized for API fetching.
    ///
    /// Configuration:
    /// - No body size limit (API responses)
    /// - 1s read timeout (fast-fail)
    /// - 5 retries (resilient)
    /// - 10 connection pool (parallel fetches)
    pub fn for_api_fetching() -> Self {
        Self::from_system()
            .max_body_size(None)
            .batch_size(8192 * 2)
            .read_timeout(Duration::from_secs(1))
            .max_retries(5)
            .enable_pool(10)
    }

    /// Create a client optimized for file downloads.
    ///
    /// Configuration:
    /// - 100MB body limit
    /// - 30s read timeout (large files)
    /// - 3 retries (transient failures)
    /// - 4 connection pool (limited parallelism)
    pub fn for_downloads() -> Self {
        Self::from_system()
            .max_body_size(Some(100 * 1024 * 1024))
            .batch_size(65536)
            .read_timeout(Duration::from_secs(30))
            .max_retries(3)
            .enable_pool(4)
    }

    /// Create a client optimized for streaming.
    ///
    /// Configuration:
    /// - No body size limit
    /// - Large batch size (16KB)
    /// - 10s read timeout
    /// - No pooling (streaming doesn't benefit)
    pub fn for_streaming() -> Self {
        Self::from_system()
            .max_body_size(None)
            .batch_size(16384)
            .read_timeout(Duration::from_secs(10))
            .enable_pool(0)
    }

    /// Create a client for parallel fetch operations.
    ///
    /// Configuration:
    /// - No body size limit
    /// - 5s read timeout (balanced)
    /// - 3 retries (fast recovery)
    /// - 20 connection pool (high parallelism)
    pub fn for_parallel_fetch() -> Self {
        Self::from_system()
            .max_body_size(None)
            .batch_size(8192 * 2)
            .read_timeout(Duration::from_secs(5))
            .max_retries(3)
            .enable_pool(20)
    }
}
```

### Usage Examples

```rust
// API fetching preset
let client = SimpleHttpClient::for_api_fetching();

// Custom configuration for specific needs
let client = SimpleHttpClient::from_system()
    // Allow large responses
    .max_body_size(None)
    // Fast timeout for responsive API
    .read_timeout(Duration::from_secs(1))
    // Moderate retries for resilience
    .max_retries(5)
    // Pool for parallel requests
    .enable_pool(10);

// High-throughput configuration
let client = SimpleHttpClient::from_system()
    .max_body_size(None)
    .batch_size(65536)
    .read_timeout(Duration::from_secs(10))
    .max_retries(10)
    .enable_pool(50)
    .pool_idle_timeout(Duration::from_secs(120));
```

## Implementation Location

Configuration methods exist in:
```
backends/foundation_core/src/wire/simple_http/client/client.rs
```

Documentation should be added to:
```
documentation/simple_http/doc.md (MODIFY - Add configuration guide)
```

## Success Criteria

- [ ] Fluent builder pattern documented
- [ ] All configuration options listed with defaults
- [ ] Use case recommendations provided
- [ ] Example configurations for common scenarios
- [ ] Pre-configured presets implemented
- [ ] Configuration interaction explained

## Verification Commands

```bash
# Build the module
cargo build --package foundation_core

# Run tests
cargo test --package foundation_core -- wire::simple_http::client

# Check formatting
cargo fmt -- --check

# Run clippy
cargo clippy --package foundation_core -- -D warnings
```

## Notes for Agents

### Important Considerations

1. **Reasonable Defaults**: Provide sensible defaults so users don't need to configure everything.

2. **Method Ordering**: Document recommended method ordering (body handling → timeouts → reliability → pooling).

3. **Interaction Effects**: Document how settings interact (e.g., pooling + timeout interactions).

4. **Use Case Examples**: Provide complete examples for common use cases.

### Common Pitfalls

1. Setting body size limit too low for expected responses
2. Timeout too aggressive for slow APIs
3. Pool size too small for parallel operations
4. Not enabling pooling for multiple requests
5. Setting retries too high (causes long delays on persistent failures)

---

_Created: 2026-03-25_
_Source: gen_model_descriptors client configuration analysis_
