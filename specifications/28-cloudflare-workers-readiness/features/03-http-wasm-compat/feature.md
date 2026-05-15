---
feature: "HTTP Wasm Compatibility"
description: "Gate TCP-dependent code in foundation_http so wasm32 compiles cleanly, make Serve trait usable without RawStream on wasm"
status: "pending"
priority: "high"
depends_on: ["01-core-wasm-compat"]
estimated_effort: "medium"
created: 2026-05-15
last_updated: 2026-05-15
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# Feature: HTTP Wasm Compatibility

## Overview

`foundation_http` is fundamentally a TCP-based HTTP server. Cloudflare Workers provides HTTP requests via JavaScript bindings, not raw TCP sockets. The goal is NOT to make `HttpServer` run on wasm, but to make the **handler logic** (Serve trait implementations, Router, respond helpers, middleware) reusable in a wasm context.

## Architecture Decision

**What stays wasm-incompatible (gated behind `#[cfg(not(target_arch = "wasm32"))]`):**
- `HttpServer` — TCP accept loop
- `server/mod.rs` — BackgroundJobRunner, worker pool
- `reader/mod.rs` — raw HTTP parsing from byte streams

**What must be wasm-compatible:**
- `Serve` trait — core handler abstraction
- `ConnectionResult` — three outcomes
- `respond` helpers — json(), text(), html(), redirect(), etc.
- `Router` — route matching, ArcServe
- `HttpApp` — application builder (minus server() methods)
- All middleware (Auth, Cors, Logger, Compression, BodyLimit)

## Approach

### Serve Trait Refinement

The `Serve` trait's `serve()` method takes `SharedByteBufferStream<RawStream>` which is TCP-dependent. For wasm, handlers need access to the request and a way to write the response, but without the raw TCP stream.

**Option A — Two trait methods:** Add a `serve_wasm` method that takes request bytes and returns response bytes.

**Option B — Abstract the connection:** Create a `ServeConnection` trait with two implementations: `TcpConnection` (native) and `WasmConnection` (wasm). The `Serve` trait uses the abstract trait.

**Decision: Option B** — cleaner, doesn't require duplicating handler logic.

```rust
/// Abstract write target for HTTP responses.
pub trait ResponseSink: Send + Sync {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()>;
}

/// Native TCP connection implementation.
#[cfg(not(target_arch = "wasm32"))]
impl ResponseSink for SharedByteBufferStream<RawStream> { ... }

/// Wasm connection — collects response bytes.
#[cfg(target_arch = "wasm32")]
pub struct WasmResponseSink {
    bytes: Vec<u8>,
    // ... headers, status
}

#[cfg(target_arch = "wasm32)]
impl ResponseSink for WasmResponseSink { ... }
```

The Serve trait becomes:
```rust
pub trait Serve: Send + Sync + 'static {
    fn serve(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: &mut impl ResponseSink,
    ) -> ConnectionResult;
}
```

### HttpApp Server Methods

Gate `server()` and `server_with_config()` behind `#[cfg(not(target_arch = "wasm32"))]`:

```rust
#[cfg(not(target_arch = "wasm32"))]
pub fn server(self, addr: &str) -> HttpServer { ... }
```

### Module Structure

Add `#[cfg(target_arch = "wasm32")]` / `#[cfg(not(target_arch = "wasm32"))]` gates:

```rust
// lib.rs
#[cfg(not(target_arch = "wasm32"))]
pub mod server;

#[cfg(target_arch = "wasm32")]
mod wasm_conn; // WasmResponseSink implementation
```

## Tasks

1. [ ] Create `ResponseSink` trait in serve/mod.rs
2. [ ] Refactor `Serve` trait to use `&mut impl ResponseSink` instead of concrete type
3. [ ] Gate `server/` module behind `#[cfg(not(target_arch = "wasm32"))]`
4. [ ] Gate `HttpApp::server()` and `server_with_config()` behind `#[cfg(not(target_arch = "wasm32"))]`
5. [ ] Create `WasmResponseSink` for wasm32 target

## Verification

```bash
# Wasm compilation
cargo build -p foundation_http --target wasm32-unknown-unknown \
  --no-default-features --features foundation_core/ssl-rustls-awsrc,foundation_core/std \
  2>&1 | tee /tmp/wasm-http.log

# Native (no regression)
cargo build -p foundation_http 2>&1 | tee /tmp/native-http.log
```

---

_Created: 2026-05-15_
