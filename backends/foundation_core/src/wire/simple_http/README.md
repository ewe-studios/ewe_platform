# simple_http - HTTP Client & Server Implementation

Production-quality HTTP client and server implementation for the foundation_core crate.

## Timeout Configuration

### Production Defaults

Based on RFC standards and industry best practices (Google SRE, Netflix, AWS):

| Timeout Type | Default | Range | Notes |
|--------------|---------|-------|-------|
| **Connect Timeout** | 10s | 5s - 30s | TCP handshake completion |
| **Read Timeout (per op)** | 500ms | 100ms - 5s | Individual `read()` calls |
| **Write Timeout** | 10s | 5s - 30s | Request body transmission |
| **Total Request Timeout** | 60s | 30s - 300s | Complete request lifecycle |
| **Time to First Byte (TTFB)** | 5s | 1s - 10s | Server response latency |
| **Max Retries** | 3 | 0 - 10 | For transient failures |

### Timeout Calculation Strategy

The client uses an **adaptive timeout system** that adjusts based on:

1. **Expected Body Size**: Larger bodies get proportionally longer timeouts
2. **Observed Latency**: Historical response times inform future timeouts
3. **Network Conditions**: Detected latency affects base calculations

#### Body Size-Based Timeout Table

| Body Size | Base Read Timeout | Max Read Timeout | Rationale |
|-----------|-------------------|------------------|-----------|
| 0-1 KB | 100ms | 1s | HEAD responses, small JSON |
| 1-10 KB | 250ms | 2s | API responses, small payloads |
| 10-100 KB | 500ms | 5s | Standard API responses |
| 100 KB - 1 MB | 1s | 10s | Large API responses |
| 1-10 MB | 2s | 30s | File uploads/downloads |
| 10-100 MB | 5s | 60s | Large file transfers |
| 100 MB+ | 10s | 300s | Streaming/large transfers |

#### Dynamic Adjustment Formula

```
adjusted_timeout = base_timeout
    × (1 + body_size_factor)
    × (1 + latency_factor)
    × (1 + retry_count × 0.5)
```

Where:
- `body_size_factor = log10(body_size_kb + 1) / 10`
- `latency_factor = observed_p99_latency / base_timeout`

### Configuration Examples

#### High-Performance API Client
```rust
let client = SimpleHttpClient::builder()
    .connect_timeout(Duration::from_secs(5))
    .read_timeout(Duration::from_millis(100))
    .max_retries(2)
    .build();
```

#### File Upload Client
```rust
let client = SimpleHttpClient::builder()
    .connect_timeout(Duration::from_secs(10))
    .read_timeout(Duration::from_secs(5))
    .write_timeout(Duration::from_secs(60))
    .max_retries(5)
    .build();
```

#### Streaming Client
```rust
let client = SimpleHttpClient::builder()
    .connect_timeout(Duration::from_secs(10))
    .read_timeout(Duration::from_secs(30))  // Long for streaming
    .total_timeout(Duration::from_secs(300))  // 5 min max
    .build();
```

### RFC References

- **RFC 9110**: HTTP Semantics - Section 9.5 (Timeouts)
- **RFC 7230**: HTTP/1.1 Message Syntax
- **RFC 6455**: WebSocket Protocol (for upgrade handling)

### Industry References

- Google SRE Book: "Timeout values should be as tight as possible"
- AWS Well-Architected: "Use timeouts to fail fast"
- Envoy Proxy Best Practices: Per-try timeouts < 5s for latency-sensitive services
- Netflix Engineering: Circuit breaker + timeout patterns

## Adaptive Timeout System Design

### Components

1. **TimeoutCalculator**: Calculates timeouts based on body size and history
2. **LatencyTracker**: Tracks observed response times per endpoint
3. **TimeoutAdjuster**: Dynamically adjusts timeouts based on conditions

### Usage

Timeouts are automatically calculated when making requests:

```rust
// Timeout is auto-calculated based on Content-Length header
let response = client.get("https://api.example.com/large-file").send()?;

// Or explicitly set for known sizes
let response = client.post("https://api.example.com/upload")
    .body(file_data)
    .with_expected_body_size(file_data.len())
    .send()?;
```

### Monitoring

The client exposes timeout metrics:
- `http_client_timeouts_total`: Count of timeouts by type
- `http_client_timeout_duration`: Histogram of timeout values used
- `http_client_latency_p99`: 99th percentile latency per endpoint

## Testing

See `../../../tests/simple_http/` for integration tests.

### Test Timeout Configuration

Tests use reduced timeouts for fast feedback:

```rust
// In tests
let client = SimpleHttpClient::for_testing()  // Uses 100ms read timeout
    .with_test_timeout(Duration::from_millis(50));
```

## Architecture

```
simple_http/
├── client/          # HTTP client implementation
│   ├── api.rs      # User-facing API
│   ├── client.rs   # Client configuration
│   ├── pool.rs     # Connection pooling
│   └── tasks/      # Async task implementations
├── sse.rs          # Server-Sent Events
└── ...
```

## Future Improvements

- [ ] HTTP/2 support
- [ ] Connection warmup/pooling
- [ ] Retry with exponential backoff
- [ ] Request coalescing
- [ ] HTTP caching
