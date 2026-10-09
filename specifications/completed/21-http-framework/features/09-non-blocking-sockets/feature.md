---
feature: non-blocking-sockets
description: Fix HTTP server to set accepted TCP sockets to non-blocking mode for valtron executor compatibility
status: completed
priority: critical
depends_on: ["06-valtron-keepalive"]
estimated_effort: small
created: 2026-05-13
last_updated: 2026-05-13
author: Claude Code
---

# Feature: Non-Blocking Accepted Sockets

## Overview

Fix the HTTP server to properly set accepted TCP sockets to non-blocking mode. Accepted sockets do NOT inherit non-blocking mode from the listener in Rust's standard library.

## Why This Fix

The valtron executor expects all I/O to be non-blocking. When sockets are in blocking mode:
- `read()` blocks the thread when no data is available
- The valtron worker thread sleeps instead of yielding
- Connections appear to hang or close prematurely
- Concurrent connection handling breaks

## Problem

### Symptom
The `test_valtron_multiplex_concurrent_connections` test was failing with:
- Server closing connections after first few requests
- Client getting `WouldBlock` errors on read
- Sequential requests working but concurrent requests failing

### Root Cause

In `server/mod.rs`, the listener is set to non-blocking:

```rust
listener
    .set_nonblocking(true)
    .expect("Failed to set non-blocking");
```

However, **accepted sockets do NOT inherit non-blocking mode** from the listener. Each accepted `TcpStream` defaults to blocking mode.

The call chain:
1. `serve_loop()` accepts a connection: `listener.accept()` → `Ok((tcp, addr))`
2. `tcp` is wrapped in `RawStream` via `wrap_clone(tcp)`
3. `RawStream::from_tcp()` wraps it in `BufferedReader`
4. When reading, if no data is available, `read()` **blocks** instead of returning `WouldBlock`
5. The valtron worker thread blocks, preventing other connections from being handled

## Solution

Set `set_nonblocking(true)` on each accepted socket before wrapping:

```rust
match listener.accept() {
    Ok((tcp, addr)) => {
        tracing::trace!("Accepted connection from {addr}");

        // Set socket to non-blocking mode for valtron executor compatibility
        if let Err(e) = tcp.set_nonblocking(true) {
            tracing::error!("Failed to set non-blocking mode: {e}");
            continue;
        }

        let client_ip = addr.ip().to_string();
        let wrap_clone = wrap_stream.clone();
        // ... rest of connection handling
    }
    // ... error handling
}
```

## Implementation

### Files Changed

- `backends/foundation_http/src/server/mod.rs`
  - Added `tcp.set_nonblocking(true)` in `serve_loop()` after accepting connection

### Test Results

After the fix:
- `test_valtron_multiplex_concurrent_connections` passes
- Sequential and concurrent requests work correctly
- Server properly multiplexes multiple connections with limited worker threads

## Success Criteria

- [x] `test_valtron_multiplex_concurrent_connections` passes
- [x] Sequential requests work (3+ requests in sequence)
- [x] Concurrent requests work (6+ simultaneous connections with 2 worker threads)
- [x] Server correctly yields with `WouldBlock` instead of blocking
- [x] Valtron executor multiplexes connections properly

## Related

- Feature 06: Valtron Keep-Alive (depends on this)
- Feature 07: Reader EOF Handling (separate issue about Ok(0) interpretation)
- `test_valtron_multiplex_concurrent_connections` in `backends/foundation_http/tests/integration_tests.rs`
