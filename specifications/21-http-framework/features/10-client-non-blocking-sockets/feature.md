---
feature: client-non-blocking-sockets
description: Investigate and fix non-blocking socket handling for HTTP/WebSocket/EventSource clients
status: investigating
priority: high
depends_on: ["09-non-blocking-sockets"]
estimated_effort: medium
created: 2026-05-13
last_updated: 2026-05-13
author: Claude Code
---

# Feature: Client Non-Blocking Sockets

## Overview

Investigate whether HTTP/WebSocket/EventSource client connections need to be set to non-blocking mode for proper valtron executor integration.

## Background

In Feature 09, we fixed the HTTP server to set accepted TCP sockets to non-blocking mode after discovering that:
1. Accepted sockets do NOT inherit non-blocking mode from the listener
2. Blocking sockets cause the valtron executor to freeze when reading

This raised the question: **Do client connections have the same issue?**

## Current State

### Connection Creation Chain

1. **HTTP Client**: `HttpClientConnection::connect()` → `Connection::with_timeout()` or `Connection::without_timeout()`
2. **WebSocket Client**: `WebSocketTask` → `HttpConnectionPool::create_http_connection()` → same as HTTP
3. **EventSource Client**: `ReconnectingSseTask` → same HTTP client infrastructure

### Code Locations

**Connection Creation** (`backends/foundation_core/src/netcap/connection/mod.rs:311-326`):
```rust
impl Connection {
    pub fn without_timeout(addr: SocketAddr) -> Result<Self, ...> {
        Ok(Self::Tcp(TcpStream::connect(addr)?))  // BLOCKING
    }

    pub fn with_timeout(addr: SocketAddr, timeout: Duration) -> Result<Self, ...> {
        Ok(Self::Tcp(TcpStream::connect_timeout(&addr, timeout)?))  // BLOCKING
    }
}
```

**HTTP Client Connection** (`backends/foundation_core/src/wire/simple_http/client/connection.rs:163-188`):
```rust
for addr in addrs {
    let conn_result = if let Some(timeout_duration) = timeout {
        Connection::with_timeout(addr, timeout_duration)
    } else {
        Connection::without_timeout(addr)
    };
    // ... creates RawStream, wraps in SharedByteBufferStream
}
```

**WebSocket Client** (`backends/foundation_core/src/wire/websocket/task.rs:417`):
```rust
let Ok(connection) = self.pool.create_http_connection(&state.url, None) else {
    error!("Failed to establish HTTP connection");
    return None;
};
```

## Key Questions

### 1. Should Client Sockets Be Non-Blocking?

**Arguments FOR non-blocking:**
- Consistency with server-side behavior
- Prevents valtron executor from blocking on slow/idle connections
- Allows proper `TaskStatus::Delayed` yielding for retries
- Required for true multiplexing of multiple concurrent client connections

**Arguments AGAINST non-blocking (keep blocking):**
- Client connections typically expect blocking behavior with timeouts
- Current retry logic may assume blocking semantics
- `TcpStream::connect_timeout()` provides built-in timeout handling
- Client is usually single-request-per-thread, not multiplexed like server

### 2. What's the Current Behavior?

When client code does `connection.read_line()` and no data is available:
- **Blocking mode**: Thread sleeps, waiting for data
- **Non-blocking mode**: Returns `Err(WouldBlock)` immediately

**For valtron executor:**
- Blocking = bad (freezes the worker thread)
- Non-blocking = good (can yield and retry)

### 3. Where Would Changes Be Needed?

If we decide to make client sockets non-blocking:

**Option A: Connection level** (`netcap/connection/mod.rs`)
- Add `set_nonblocking(true)` after `TcpStream::connect()`
- Affects ALL users of `Connection` (HTTP, WebSocket, SSE, etc.)
- Risk: May break non-valtron users expecting blocking behavior

**Option B: RawStream level** (`netcap/no_wasm.rs`)
- Add to `RawStream::from_tcp()` and `RawStream::from_connection()`
- Affects all RawStream users
- Risk: Same as Option A

**Option C: HTTP client level** (`wire/simple_http/client/connection.rs`)
- Set non-blocking in `HttpClientConnection::connect()`
- Only affects HTTP/WebSocket/SSE clients
- Risk: Isolated to client code only

## Potential Issues with Non-Blocking Clients

### Issue 1: Connection Timeout Handling

Current code relies on `TcpStream::connect_timeout()` which uses blocking semantics:
```rust
TcpStream::connect_timeout(&addr, timeout)?  // This blocks until connected or timeout
```

If we make socket non-blocking:
- `connect()` returns immediately
- Need to use `select()` or `mio` to wait for writable
- Complex error handling for `WouldBlock` during connection

### Issue 2: Read/Write Retry Logic

Current HTTP client code may not handle `WouldBlock` properly:
```rust
// In ByteBufferPointer::fill_up()
let read = match self.reader.read(&mut copied) {
    Ok(read_size) => read_size,  // Currently assumes blocking
    Err(err) => return Err(err), // WouldBlock propagates as error
};
```

Would need to add retry loops or `TaskStatus::Delayed` handling.

### Issue 3: Pool Reuse

Pooled connections (`ConnectionPool`) may have different non-blocking requirements:
- When checking out: ensure non-blocking
- When checking in: should we reset to blocking? (no, pool is shared)

## Investigation Plan

### Phase 1: Identify Current Behavior
- [ ] Create test that demonstrates client blocking behavior
- [ ] Trace client connection lifecycle with logging
- [ ] Document which operations block and for how long

### Phase 2: Evaluate Impact
- [ ] Test WebSocket client with slow connection
- [ ] Test EventSource with intermittent data
- [ ] Test HTTP client with pooled connections under load
- [ ] Check if valtron workers get stuck on client I/O

### Phase 3: Prototype Fix (if needed)
- [ ] Implement Option C (HTTP client level only)
- [ ] Add proper `WouldBlock` handling in client read paths
- [ ] Ensure connection timeouts still work
- [ ] Test with slow/unavailable servers

### Phase 4: Decision
- [ ] If fix improves performance/reliability: merge
- [ ] If fix causes issues: document why blocking is required
- [ ] Update architecture docs with findings

## Test Scenarios

### Scenario 1: Slow WebSocket Connection
```rust
// Server accepts connection but sends no handshake
// Client should yield with Delayed, not block thread
```

### Scenario 2: HTTP Client with Pooled Connections
```rust
// Multiple concurrent HTTP requests using pool
// Should multiplex, not block
```

### Scenario 3: EventSource Reconnection
```rust
// Server drops connection, client reconnects
// Should not block executor during reconnect
```

## Success Criteria

- [ ] Clear decision on whether client sockets should be non-blocking
- [ ] If non-blocking: Implementation that handles `WouldBlock` properly
- [ ] All existing tests pass
- [ ] Performance metrics showing improvement (if implemented)
- [ ] Documentation updated

## Related

- Feature 09: Server Non-Blocking Sockets (the fix we already applied)
- `backends/foundation_core/src/netcap/connection/mod.rs`
- `backends/foundation_core/src/wire/simple_http/client/connection.rs`
- `backends/foundation_core/src/wire/websocket/task.rs`
- `backends/foundation_core/src/wire/event_source/reconnecting_task.rs`

## Locations Requiring WouldBlock Handling

If client sockets are made non-blocking, the following locations currently do NOT handle `WouldBlock` errors and would need to be updated:

### Connection Establishment

**File**: `backends/foundation_core/src/netcap/connection/mod.rs:311-326`
```rust
pub fn without_timeout(addr: SocketAddr) -> Result<Self, ...> {
    Ok(Self::Tcp(TcpStream::connect(addr)?))  // BLOCKING - returns Err(WouldBlock) if non-blocking
}

pub fn with_timeout(addr: SocketAddr, timeout: Duration) -> Result<Self, ...> {
    Ok(Self::Tcp(TcpStream::connect_timeout(&addr, timeout)?))  // BLOCKING
}
```
**Issue**: `connect_timeout()` assumes blocking. For non-blocking, would need to use `TcpStream::connect()` then poll for writable.

**File**: `backends/foundation_core/src/netcap/connection/mod.rs:330-344`
```rust
fn read_timeout_into(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, io::Error> {
    self.set_read_timeout(Some(timeout))?;
    let result = self.read(buf);  // Would need WouldBlock retry loop
    self.set_read_timeout(previous_read_timeout)?;
    result
}
```
**Issue**: Sets timeout and reads, but if non-blocking, `read()` returns `Err(WouldBlock)` immediately instead of waiting.

### HTTP Client Request Sending

**File**: `backends/foundation_core/src/wire/simple_http/client/tasks/request_redirect.rs:200-215`
```rust
if let Err(err) = connection.stream_mut().write_all(request_string.as_bytes()) { ... }
if let Err(err) = connection.stream_mut().flush() { ... }
```
**Issue**: `write_all()` expects blocking. For non-blocking, would need to handle partial writes and `WouldBlock`.

**File**: `backends/foundation_core/src/wire/simple_http/client/tasks/request_redirect.rs:222-231`
```rust
if let Err(err) = connection.stream_mut().set_read_timeout_as(read_timeout) { ... }
```
**Issue**: Sets timeout on socket, but non-blocking sockets don't honor `SO_RCVTIMEO` the same way - they return `WouldBlock` immediately.

**File**: `backends/foundation_core/src/wire/simple_http/client/tasks/send_request.rs:289`
```rust
if let Err(err) = conn.stream_mut().flush() { ... }
```
**Issue**: `flush()` with non-blocking socket may return `WouldBlock` if send buffer is full.

### HTTP Client Connection Timeout Setup

**File**: `backends/foundation_core/src/wire/simple_http/client/connection.rs:751-755`
```rust
.write_all(connect_request.as_bytes())
.flush()
```
**Issue**: Direct `write_all` and `flush` without WouldBlock handling. Used in proxy CONNECT and TLS handshake.

**File**: `backends/foundation_core/src/wire/simple_http/client/connection.rs:891-895`
```rust
.write_all(connect_request.as_bytes())
.flush()
```
**Issue**: Same issue in proxy authentication handling.

### WebSocket Client

**File**: `backends/foundation_core/src/wire/websocket/task.rs:483-491`
```rust
let _ = state.connection.write_all(chunk);
let _ = state.connection.flush();
```
**Issue**: WebSocket handshake writing. Would need WouldBlock handling for non-blocking sockets.

**File**: `backends/foundation_core/src/wire/websocket/task.rs:712-736`
```rust
if let Err(err) = open_state.stream.write_all(&encoded) { ... }
if let Err(err) = open_state.stream.flush() { ... }
if let Err(err) = open_state.stream.flush() { ... }  // line 736
```
**Issue**: WebSocket frame writing in Open state. Multiple write/flush calls without WouldBlock handling.

**File**: `backends/foundation_core/src/wire/websocket/task.rs:799-800`
```rust
let _ = open_state.stream.write_all(&pong_bytes);
let _ = open_state.stream.flush();
```
**Issue**: Pong frame response writing without WouldBlock handling.

### EventSource (SSE) Client

**File**: `backends/foundation_core/src/wire/event_source/task.rs:379`
```rust
let _ = stream_writer.flush();
```
**Issue**: EventSource stream flushing without WouldBlock handling.

### Timeout Configuration

**File**: `backends/foundation_core/src/wire/simple_http/timeout.rs:32-68`
```rust
pub struct TimeoutConfig {
    pub connect_timeout: Duration,
    pub read_timeout_per_kb: Duration,
    pub write_timeout_per_kb: Duration,
    pub min_read_timeout: Duration,
    pub max_read_timeout: Duration,
    pub max_total_timeout: Duration,
    pub ttfb_timeout: Duration,
    pub max_retries: usize,  // <-- Retry logic
    ...
}
```
**Issue**: All timeout calculations assume blocking I/O with `SO_RCVTIMEO`/`SO_SNDTIMEO`. Non-blocking sockets ignore these timeouts and return `WouldBlock` immediately.

### Retry Middleware

**File**: `backends/foundation_core/src/wire/simple_http/client/middleware.rs:633-698`
```rust
pub struct RetryMiddleware {
    max_retries: u32,
    retry_status_codes: Vec<u16>,
    backoff: BackoffStrategy,
}
```
**Issue**: Retry logic triggers on response status codes (429, 502, 503, 504), not on `WouldBlock` errors. Would need to add `WouldBlock` as a retryable error.

### Body Reader

**File**: `backends/foundation_core/src/wire/simple_http/client/body_reader.rs:646-693`
```rust
fn write_from_stream<I, W>(...) { ... }
fn write_from_chunked_stream<I, W>(...) { ... }
```
**Issue**: Body writing operations without WouldBlock handling.

## Impact Summary

**Total locations requiring updates: ~20**

**Categories**:
1. Connection establishment (3 locations)
2. HTTP request writing (4 locations)
3. HTTP timeout setting (2 locations)
4. WebSocket I/O (5 locations)
5. EventSource I/O (1 location)
6. Proxy/TLS handshake (2 locations)
7. Timeout configuration (1 location)
8. Retry middleware (1 location)
9. Body reading/writing (2 locations)

**Risk Assessment**: HIGH
- Many I/O operations assume blocking semantics
- Timeout/retry logic needs significant rework
- Would require testing all client paths

## Notes

The HTTP server fix was critical because:
1. Server uses valtron executor for connection multiplexing
2. Blocking on one connection blocks the worker thread
3. This prevents handling other concurrent connections

For clients, the impact may be different:
1. Clients typically make one request at a time per connection
2. Pooled connections may benefit from non-blocking
3. WebSocket/EventSource long-lived connections may benefit

Need to measure actual impact before deciding to implement.
