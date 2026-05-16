---
feature: "HTTP Wasm Compatibility"
description: "Restructure into core/native/wasm modules, ServeWriter trait, Server enum, bridge module with web/cf bindings"
status: "pending"
priority: "high"
depends_on: ["01-core-wasm-compat"]
estimated_effort: "large"
created: 2026-05-15
last_updated: 2026-05-16
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 12
  total: 12
  completion_percentage: 0%
---

# Feature: HTTP Wasm Compatibility

## Overview

`foundation_http` is a TCP-based HTTP server. The goal is NOT to make `HttpServer` run on wasm, but to make handler logic (route registration, middleware, dispatch) compile cleanly on wasm32 by restructuring into shared core, native-specific, and wasm-specific modules.

## Module Restructure

```
foundation_http/src/
├── lib.rs                    # Always compiled — re-exports core + target module
├── core/                     # Shared between wasm and non-wasm
│   ├── mod.rs                # Serve trait, ServeWriter, ConnectionResult, ServeError, respond::
│   ├── context/              # ContextBag
│   ├── router/               # Router, Server enum, RouteMethod, segments
│   ├── middleware/           # All middleware (pure logic)
│   ├── handlers/             # Built-in handlers (minus static_file.rs)
│   ├── app/                  # HttpApp (route, middleware, context)
│   └── client_ip.rs          # ClientIp marker type
├── native/                   # Only compiled on non-wasm targets
│   ├── mod.rs                # Re-exports server, reader, upgrade
│   ├── server/               # TcpListener, ConnectionHandler, serve_loop, TLS
│   ├── reader/               # HTTP parsing from TCP streams
│   └── upgrade/              # WebSocket accept, SSE streaming
└── wasm/                     # Only compiled on wasm32 targets
    ├── mod.rs                # Re-exports stream, server, bridge
    ├── stream/               # WasmStream (memory-backed io::Read + io::Write)
    ├── server/               # Dispatch-only handle_request
    └── bridge/               # wasm-bindgen glue (web.rs, cf.rs) + tests
```

```rust
// lib.rs
mod core;

#[cfg(not(target_arch = "wasm32"))]
mod native;

#[cfg(target_arch = "wasm32")]
mod wasm;

// Always available
pub use core::*;

// Target-specific
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
pub use wasm::*;
```

## Core Module (shared)

### `serve/` — Traits

Two handler traits — `Serve` (native, takes `RawStream`) and `ServeWriter` (both, takes generic I/O):

```rust
pub trait Serve: Send + Sync + 'static {
    fn serve(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult;
}

pub trait ServeWriter: Send + Sync + 'static {
    fn serve_writer(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<impl std::io::Read + std::io::Write>,
    ) -> ConnectionResult;
}

pub trait ServeFactory: Serve + Sized {
    fn create(bag: &ContextBag) -> Self;
}

pub trait ServeWriterFactory: ServeWriter + Sized {
    fn create(bag: &ContextBag) -> Self;
}
```

### `router/` — Server Enum

```rust
pub enum Server {
    #[cfg(not(target_arch = "wasm32"))]
    Serve(Arc<dyn Serve>),
    #[cfg(target_arch = "wasm32")]
    Serve,  // empty stub — never constructed on wasm
    Writer(Arc<dyn ServeWriter>),
}
```

`RouteMethod` stores `Option<Server>`. `Router::add_route` uses `Server::Serve` (native-only), `Router::add_route_writer` uses `Server::Writer` (both targets).

### `middleware/` — No Changes

All middleware is pure logic on `SimpleIncomingRequest` — already wasm-compatible.

### `app/` — HttpApp

`HttpApp::route()` gated to native (uses `Serve`). `HttpApp::route_any()` and `HttpApp::route_writer()` work on both targets. `server()` and `server_with_config()` gated to native.

### `handlers/` — Built-in Handlers

All handlers are pure logic except `static_file.rs` which reads the filesystem. Gate `static_file.rs` to native only.

## Native Module (non-wasm only)

### `server/` — TCP Server

Current implementation: `HttpServer`, `ConnectionHandler`, `serve_loop`, TLS support. Unchanged — just moved.

### `reader/` — HTTP Parsing from Streams

Reads from TCP streams, handles `WouldBlock`, keep-alive, 100-continue. Moved as-is.

### `upgrade/` — WebSocket + SSE

`accept_websocket` and `SseStream` use `SharedByteBufferStream<RawStream>`. Moved as-is.

## Wasm Module (wasm32 only)

### `stream/` — WasmStream

Memory-backed `io::Read + io::Write` for wasm32:

```rust
pub struct WasmStream {
    response: Vec<u8>,
}

impl std::io::Read for WasmStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> { Ok(0) }
}

impl std::io::Write for WasmStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.response.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}
```

### `server/` — Dispatch Only

Takes a `SimpleIncomingRequest`, runs middleware + routing, returns response bytes:

```rust
pub fn handle_request(
    app: &HttpApp,
    req: SimpleIncomingRequest,
) -> Result<Vec<u8>, ErrorTrace<ServeError>> { ... }

pub fn handle_request_with_bag(
    bag: Arc<ContextBag>,
    app: &HttpApp,
    req: SimpleIncomingRequest,
) -> Result<Vec<u8>, ErrorTrace<ServeError>> { ... }
```

### `bridge/` — wasm-bindgen Glue

**`web.rs`** — Standard web: `web_sys::Request` → `SimpleIncomingRequest` → dispatch → `web_sys::Response`

```rust
#[wasm_bindgen(js_name = handleRequest)]
pub async fn handle_request(req: Request) -> Result<Response, JsError>
```

**`cf.rs`** — Cloudflare Workers: same dispatch but extracts `env` bindings for D1 (`env.DB`), R2 (`env.BUCKET`), secrets:

```rust
#[wasm_bindgen(js_name = handleRequest)]
pub async fn handle_request(req: Request, env: JsValue) -> Result<Response, JsError>
```

## Tests

Tests move alongside their modules:
- `core/handlers/tests/` — handler unit tests (both targets)
- `native/server/tests/` — server integration tests (native only)
- `wasm/bridge/tests/` — wasm-bindgen tests (wasm32 only, run with `wasm-pack test`)

## Tasks

1. [ ] Create `core/` directory and move shared modules (serve, context, router, middleware, handlers, app, client_ip)
2. [ ] Add `ServeWriter` and `ServeWriterFactory` traits to `core/serve/mod.rs`
3. [ ] Add `Server` enum to `core/router/mod.rs` with cfg-gated `Serve` variant
4. [ ] Update `RouteMethod` to store `Server` instead of `ArcServe`
5. [ ] Create `native/` directory and move TCP modules (server, reader, upgrade)
6. [ ] Gate `static_file.rs` handler to native only
7. [ ] Gate `HttpApp::server()` and `server_with_config()` to native
8. [ ] Create `wasm/` directory with `stream/`, `server/`, `bridge/`
9. [ ] Add `WasmStream` to `wasm/stream/mod.rs`
10. [ ] Add `handle_request` dispatch to `wasm/server/mod.rs`
11. [ ] Add `bridge/web.rs` and `bridge/cf.rs` with wasm-bindgen entry points
12. [ ] Update `lib.rs` with conditional module selection and re-exports

## Verification

```bash
# Wasm compilation
cargo build -p foundation_http --target wasm32-unknown-unknown \
  --no-default-features --features foundation_core/ssl-rustls-awsrc,foundation_core/std \
  2>&1 | tee /tmp/wasm-http.log

# Native (no regression)
cargo build -p foundation_http 2>&1 | tee /tmp/native-http.log

# Wasm tests (bridge module)
wasm-pack test --headless --chrome
```

---

_Created: 2026-05-15_
