---
feature: client-non-blocking-sockets
description: Add opt-in non-blocking socket mode for HTTP/WebSocket/EventSource clients while maintaining backward compatibility
status: design
priority: high
depends_on: ["09-non-blocking-sockets"]
estimated_effort: medium
created: 2026-05-13
last_updated: 2026-05-13
author: Claude Code
---

# Feature: Client Non-Blocking Sockets (Opt-In)

## Overview

Add optional non-blocking socket mode for HTTP/WebSocket/EventSource client connections. This feature provides an **opt-in** mechanism via configuration flags, maintaining **full backward compatibility** with existing blocking behavior as the default.

## Design Philosophy

**Default behavior stays blocking.** Client connections historically expect blocking I/O with timeout-based error handling. Changing this default would break existing code that relies on `SO_RCVTIMEO`/`SO_SNDTIMEO` timeouts and blocking semantics.

**Non-blocking is opt-in via configuration.** Users who need non-blocking behavior (e.g., for valtron executor integration with pooled connections) can enable it explicitly through a chain of configuration flags.

## Motivation

### When Non-Blocking Helps

1. **Pooled Connections Under Load**: Multiple concurrent HTTP requests using a connection pool can multiplex better with non-blocking I/O
2. **WebSocket Long-Lived Connections**: Prevents valtron executor worker threads from blocking on idle WebSocket connections
3. **EventSource Reconnections**: Reconnection attempts don't block the executor during network failures

### When Blocking Is Preferred

1. **Simple Single-Request Clients**: Blocking with `connect_timeout()` provides simpler error handling
2. **Existing Timeout Configuration**: Current `TimeoutConfig` relies on `SO_RCVTIMEO` which doesn't work with non-blocking sockets
3. **WouldBlock Handling Complexity**: Non-blocking mode requires retry loops or `TaskStatus::Delayed` yielding throughout the stack

## API Changes

### Propagation Chain

```
User API
    ↓
SimpleHttpClient::non_blocking(bool)
WebSocketTaskBuilder::non_blocking(bool)
EventSourceBuilder::non_blocking(bool)
    ↓
ClientConfig::non_blocking(bool)
    ↓
HttpClientConnection::connect(..., non_blocking: bool)
    ↓
Connection::with_timeout(..., non_blocking: bool)
Connection::without_timeout(..., non_blocking: bool)
    ↓
RawStream::from_tcp(..., non_blocking: bool)
RawStream::from_connection(..., non_blocking: bool)
    ↓
tcp.set_nonblocking(true/false)
```

### Detailed API Changes

#### 1. `Connection` (`backends/foundation_core/src/netcap/connection/mod.rs`)

**Current:**
```rust
pub fn without_timeout(addr: SocketAddr) -> Result<Self, ...>
pub fn with_timeout(addr: SocketAddr, timeout: Duration) -> Result<Self, ...>
```

**Proposed:**
```rust
pub fn without_timeout(addr: SocketAddr) -> Result<Self, ...>
pub fn without_timeout_non_blocking(addr: SocketAddr) -> Result<Self, ...>

pub fn with_timeout(addr: SocketAddr, timeout: Duration) -> Result<Self, ...>
pub fn with_timeout_non_blocking(addr: SocketAddr, timeout: Duration) -> Result<Self, ...>
```

**Rationale**: Non-blocking `connect()` has different semantics than blocking `connect_timeout()`. Separate methods make this distinction explicit.

#### 2. `RawStream` (`backends/foundation_core/src/netcap/no_wasm.rs`)

**Current:**
```rust
pub fn from_tcp(stream: TcpStream) -> Result<Self, ...>
pub fn from_connection(conn: Connection) -> Result<Self, ...>
```

**Proposed:**
```rust
pub fn from_tcp(stream: TcpStream) -> Result<Self, ...>
pub fn from_tcp_non_blocking(stream: TcpStream) -> Result<Self, ...>

pub fn from_connection(conn: Connection) -> Result<Self, ...>
pub fn from_connection_non_blocking(conn: Connection) -> Result<Self, ...>
```

**Rationale**: RawStream wraps the underlying socket; non-blocking mode must be set on the TcpStream before wrapping.

#### 3. `ClientConfig` (`backends/foundation_core/src/wire/simple_http/client/config.rs`)

**Add field:**
```rust
pub struct ClientConfig {
    // ... existing fields ...
    /// Enable non-blocking socket mode for all connections.
    /// Default: false (blocking mode)
    pub non_blocking: bool,
}
```

**Add builder method:**
```rust
impl ClientConfig {
    #[must_use]
    pub fn with_non_blocking(mut self, enabled: bool) -> Self {
        self.non_blocking = enabled;
        self
    }
}
```

**Rationale**: Central configuration point that propagates to all connection creation.

#### 4. `HttpClientConnection` (`backends/foundation_core/src/wire/simple_http/client/connection.rs`)

**Internal change** to `connect()` method:
- Accept `non_blocking: bool` parameter from `ClientConfig`
- Use appropriate `Connection` constructor based on flag
- Set non-blocking on the underlying `TcpStream` before wrapping in `RawStream`

**Note**: Connection timeout handling differs:
- **Blocking**: Uses `TcpStream::connect_timeout()` with `SO_RCVTIMEO`
- **Non-blocking**: Uses `TcpStream::connect()` + poll for writable (future work)

#### 5. `SimpleHttpClient` (`backends/foundation_core/src/wire/simple_http/client/mod.rs`)

**Add builder method:**
```rust
impl SimpleHttpClient {
    #[must_use]
    pub fn non_blocking(self, enabled: bool) -> Self {
        self.config = self.config.with_non_blocking(enabled);
        self
    }
}
```

#### 6. `WebSocketTask` (`backends/foundation_core/src/wire/websocket/task.rs`)

**Add to `WebSocketTaskBuilder`:**
```rust
impl WebSocketTaskBuilder {
    #[must_use]
    pub fn non_blocking(mut self, enabled: bool) -> Self {
        self.non_blocking = enabled;
        self
    }
}
```

**Internal**: Pass flag to `HttpConnectionPool::create_http_connection()`.

#### 7. `EventSource` (`backends/foundation_core/src/wire/event_source/`)

**Add to `EventSourceBuilder`:**
```rust
impl EventSourceBuilder {
    #[must_use]
    pub fn non_blocking(mut self, enabled: bool) -> Self {
        self.non_blocking = enabled;
        self
    }
}
```

## What Stays the Same

### Default Behavior
- All clients default to **blocking mode** (`non_blocking: false`)
- Existing code continues to work without changes
- `TcpStream::connect_timeout()` remains the default for blocking mode
- `SO_RCVTIMEO`/`SO_SNDTIMEO` timeouts continue to work

### Connection Pool
- Pool behavior unchanged for blocking connections
- Pooled connections are returned in the mode they were created
- No mode switching on checkin/checkout

### Timeout Configuration
- `TimeoutConfig` remains effective for blocking mode
- No changes to `TimeoutCalculator` or `TimeoutContext`

## What Changes (When Opted In)

### When `non_blocking(true)` is set:

1. **Connection Establishment**
   - Uses `TcpStream::connect()` instead of `connect_timeout()`
   - Caller must handle `WouldBlock` during connection (future work)

2. **Read/Write Operations**
   - All I/O operations may return `WouldBlock`
   - Existing code that expects blocking semantics will fail
   - Requires `TaskStatus::Delayed` handling for valtron integration

3. **Timeout Behavior**
   - `SO_RCVTIMEO`/`SO_SNDTIMEO` have no effect on non-blocking sockets
   - Timeouts must be handled via polling or delayed retries

## Backward Compatibility

### Guaranteed Compatibility

| Aspect | Blocking (Default) | Non-Blocking (Opt-In) |
|--------|-------------------|----------------------|
| Existing code | Works unchanged | Must opt-in explicitly |
| Timeout handling | `SO_RCVTIMEO` | Application-level |
| Connect API | `connect_timeout()` | `connect()` + poll |
| Error handling | Standard I/O errors | + `WouldBlock` handling |

### Migration Path

Users who want non-blocking mode:

```rust
// HTTP Client
let client = SimpleHttpClient::new()
    .non_blocking(true)  // Opt-in
    .build();

// WebSocket
let ws = WebSocketTask::builder()
    .non_blocking(true)  // Opt-in
    .build();

// EventSource
let es = EventSource::builder()
    .non_blocking(true)  // Opt-in
    .build();
```

## Files Requiring Changes

### Core Networking
- `backends/foundation_core/src/netcap/connection/mod.rs` - Add non-blocking constructors
- `backends/foundation_core/src/netcap/no_wasm.rs` - Add non-blocking RawStream constructors

### HTTP Client
- `backends/foundation_core/src/wire/simple_http/client/config.rs` - Add `non_blocking` field
- `backends/foundation_core/src/wire/simple_http/client/connection.rs` - Propagate flag
- `backends/foundation_core/src/wire/simple_http/client/mod.rs` - Add builder method

### WebSocket
- `backends/foundation_core/src/wire/websocket/task.rs` - Add builder method and propagate flag

### EventSource
- `backends/foundation_core/src/wire/event_source/builder.rs` - Add builder method
- `backends/foundation_core/src/wire/event_source/reconnecting_task.rs` - Propagate flag

## Future Work (Out of Scope)

This feature only adds the **API infrastructure** for non-blocking mode. Actually using non-blocking mode requires:

1. **WouldBlock Handling** - All read/write paths need retry logic or `TaskStatus::Delayed` yielding
2. **Non-Blocking Connect** - Implementation of `connect()` + poll for writable
3. **Timeout Reimplementation** - Application-level timeout handling for non-blocking sockets
4. **Testing** - Comprehensive tests for all client paths with non-blocking mode

These are deferred to separate features to keep this change focused on API design.

## Success Criteria

- [ ] All existing tests pass without modification (backward compatibility)
- [ ] New `non_blocking()` builder methods available on all client types
- [ ] Configuration propagates through the entire chain
- [ ] Default behavior remains blocking
- [ ] Documentation updated with opt-in usage examples

## Related

- Feature 09: Server Non-Blocking Sockets (server-side fix already implemented)
- `backends/foundation_core/src/netcap/connection/mod.rs`
- `backends/foundation_core/src/netcap/no_wasm.rs`
- `backends/foundation_core/src/wire/simple_http/client/connection.rs`
- `backends/foundation_core/src/wire/websocket/task.rs`
- `backends/foundation_core/src/wire/event_source/reconnecting_task.rs`
