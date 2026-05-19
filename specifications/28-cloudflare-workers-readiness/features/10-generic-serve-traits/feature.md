---
feature: "Generic Serve Traits"
description: "Replace Server enum with generic Router<Serve>, introduce environment-specific Serve traits (Serve, CfServe, WebServe) so each platform handles its own response type natively"
status: "pending"
priority: "critical"
depends_on: ["03-http-wasm-compat", "04-wire-restructure"]
estimated_effort: "large"
created: 2026-05-19
last_updated: 2026-05-19
author: "alex.ewetumo"
tasks:
  completed: 0
  uncompleted: 14
  total: 14
  completion_percentage: 0%
---

# Feature: Generic Serve Traits

## Overview

Replace the `Server` enum in the router with a generic type parameter `S` that flows from `HttpApp<S>` down through `Router<S>`, `RouteMethod<S>`, `RouteSegment<S>`. Instead of forcing every environment through a lowest-common-denominator (`&mut dyn Write` + HTTP/1.1 wire format parsing), each platform gets its own `Serve` trait with an `serve_*` method that accepts the connection/response type native to that environment.

### The Problem Today

The current `ServeWriter` trait forces handlers to write HTTP/1.1 wire format into a `&mut dyn std::io::Write`. On wasm, the handler writes bytes into `WasmStream`, then `parse_raw()` tears those bytes apart to reconstruct status, headers, and body — only so they can be fed into `web_sys::Response` or the CF `Response` constructor. This is unnecessary work: the handler already knows the response components, and the target environment already has native types to hold them.

```
Handler ──write wire bytes──► WasmStream ──parse_raw()──► WasmResponse ──build_response()──► web_sys::Response
         (status, headers, body as bytes)   (re-extract status, headers, body)
```

The round-trip through wire format is fragile. `parse_raw` skips headers without `": "`, loses multi-value header semantics, and defaults status to 200 on malformed input.

### The Solution

Each environment implements the trait it cares about. The handler receives a typed `conn` that captures responses in the environment's native representation. No wire-format parsing needed.

```
Native:    Handler ──serve()─────────► SharedByteBufferStream<RawStream>  (direct TCP write)
Writer:    Handler ──serve_writer()──► &mut dyn Write  (bytes only, caller interprets)
CF:        Handler ──serve_cf()──────► CfConn  (structured fields)
Web:       Handler ──serve_web()─────► WebConn  (structured fields)
```

Each method has a distinct name (`serve`, `serve_writer`, `serve_cf`, `serve_web`) so a single type can implement multiple traits without method collision.

## Architecture

### 1. Serve Traits — One Per Environment / Output Style

Four parallel traits. Users implement whichever matches their target. No compatibility layers — just pick the one you need.

```rust
// Native TCP — writes directly to the TCP stream
#[cfg(not(target_arch = "wasm32"))]
pub trait Serve: Send + Sync + 'static {
    fn serve(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult;
}

// Write-to-writer — any environment that only needs bytes out to a writer.
// On native this backs file/static handlers. On wasm this replaces the
// WasmStream + parse_raw round-trip: the handler writes HTTP/1.1 wire bytes
// to the writer and the caller decides what to do with them.
pub trait ServeWriter: Send + Sync + 'static {
    fn serve_writer(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: &mut dyn std::io::Write,
    ) -> ConnectionResult;
}

// Cloudflare Workers — captures structured response fields, no wire format
#[cfg(target_arch = "wasm32")]
pub trait CfServe: Send + Sync + 'static {
    fn serve_cf(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: &mut CfConn,
    ) -> CfConnectionResult;
}

// Web / browser — captures structured response fields, no wire format
#[cfg(target_arch = "wasm32")]
pub trait WebServe: Send + Sync + 'static {
    fn serve_web(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: &mut WebConn,
    ) -> WebConnectionResult;
}
```

`ServeWriter` is not a backward-compat shim — it's a legitimate option when the caller only wants bytes written to a writer and handles the response format itself. A handler that renders static files or proxies a raw upstream body would implement `ServeWriter`. A handler that wants to set status codes and headers as structured fields on CF Workers implements `CfServe`.

### 2. Environment Connection Types

**`CfConn`** — captures response as structured fields, no wire format:

```rust
pub struct CfConn {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

impl CfConn {
    pub fn new() -> Self {
        Self {
            status: 200,
            headers: Vec::new(),
            body: None,
        }
    }

    pub fn set_status(&mut self, status: u16) { self.status = status; }
    pub fn set_header(&mut self, name: &str, value: &str) { ... }
    pub fn set_body(&mut self, body: Vec<u8>) { self.body = Some(body); }

    /// Convert collected fields into a web_sys::Response
    pub fn into_response(self) -> Result<web_sys::Response, JsError> { ... }
}

pub enum CfConnectionResult {
    Ok,
    Close(Option<ErrorTrace<ServeError>>),
}
```

**`WebConn`** — identical structure but semantically separate so users can implement `Serve<CfConn>` and `Serve<WebConn>` on the same type without conflict (if Rust ever allows it), and so each can carry environment-specific helpers:

```rust
pub struct WebConn {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

// Same helpers as CfConn
```

### 3. Generic Router and App

The `Server` enum is removed. The router, segments, and method storage all take a generic `S`:

```rust
pub struct Router<S> {
    root: RouteSegment<S>,
    _marker: PhantomData<S>,
}

impl<S: Clone> Router<S> {
    pub fn new() -> Self { ... }

    pub fn add_route(&mut self, method: SimpleMethod, path: &str, handler: Arc<S>) {
        let segment_tree = RouteSegment::parse_route(path).expect("valid route pattern");
        self.root.merge_route(segment_tree, method, handler.clone());
    }

    pub fn dispatch(&self, method: &SimpleMethod, path: &str) -> Option<Arc<S>> {
        let path_without_query = path.split('?').next().unwrap_or(path);
        self.root.match_route(method, path_without_query).ok()
    }
}
```

```rust
pub struct HttpApp<S> {
    ctx: Arc<ContextBag>,
    router: Router<S>,
    middleware: Vec<Box<dyn RequestMiddleware>>,
    _marker: PhantomData<S>,
}

impl<S: Clone + Send + Sync + 'static> HttpApp<S> {
    pub fn new() -> Self { ... }

    pub fn route(&mut self, method: SimpleMethod, path: &str, handler: Arc<S>) -> &mut Self {
        self.router.add_route(method, path, handler);
        self
    }

    pub fn middleware<M: RequestMiddleware>(&mut self, mw: M) -> &mut Self { ... }

    pub fn context(&self) -> &Arc<ContextBag> { &self.ctx }
    pub fn router(&self) -> &Router<S> { &self.router }
    pub fn middleware_chain(&self) -> &[Box<dyn RequestMiddleware>] { &self.middleware }
}
```

### 4. Route Tree With Generic Handler

`RouteSegment<S>`, `RouteMethod<S>` — the matching logic is unchanged, only the stored type becomes generic:

```rust
pub struct RouteSegment<S> {
    segment: SegmentType,
    dynamic_routes: Vec<RouteSegment<S>>,
    static_routes: HashMap<String, RouteSegment<S>>,
    method: RouteMethod<S>,
}

pub struct RouteMethod<S> {
    get: Option<Arc<S>>,
    post: Option<Arc<S>>,
    // ...
}

impl<S> RouteMethod<S> {
    pub fn set_method(&mut self, method: SimpleMethod, handler: Arc<S>) { ... }
    pub fn get_method(&self, method: &SimpleMethod) -> RouteResult<Arc<S>> { ... }
}
```

### 5. How Users Register Handlers

**Native (TCP server):**

```rust
let mut app = HttpApp::<dyn Serve>::new();
// Or use concrete types:
let mut app = HttpApp::<MyServeHandler>::new();

app.route(SimpleMethod::GET, "/api/health", Arc::new(HealthHandler));
app.server("0.0.0.0:8080"); // HttpApp<Serve> has server() method
```

**Cloudflare Workers:**

```rust
let mut app = HttpApp::<dyn CfServe>::new();

app.route_writer_cf(SimpleMethod::GET, "/api/health", Arc::new(HealthCfHandler));

// In bridge/cf.rs dispatch:
//   let handler = app.router().dispatch(&method, &path).unwrap();
//   let mut conn = CfConn::new();
//   handler.serve_cf(&bag, req, &mut conn);
//   conn.into_response()
```

**Browser / generic wasm:**

```rust
let mut app = HttpApp::<dyn WebServe>::new();

app.route_writer_web(SimpleMethod::GET, "/hello", Arc::new(HelloWebHandler));
```

### 6. `ServeWriter` — Just Another Option

`ServeWriter` is not a compat shim or transition artifact — it's a legitimate output strategy for handlers that only need to write bytes to a writer and let the caller handle the response format.

- `ServeWriter` lives alongside `Serve`, `CfServe`, and `WebServe` as a peer trait
- No blanket impls, no automatic conversions — users pick the trait that matches their environment
- A handler rendering static files or proxying a raw upstream body implements `ServeWriter`
- A handler setting structured status/headers/body on CF Workers implements `CfServe`
- `WasmStream` remains the backing store for `ServeWriter` on wasm — the dispatch side decides whether to parse the collected bytes into `WasmResponse`

### 7. Dispatch Flow

**Current (wasm):**
```
handle_request()
  → run_middleware()
  → router.dispatch() → Server::Writer → serve_writer(&bag, req, &mut WasmStream)
  → WasmStream.into_wasm_response()  ← parses wire bytes
  → from_wasm() → web_sys::Response
```

**After (CF):**
```
handle_request_cf()
  → run_middleware()
  → router.dispatch() → Arc<CfServe> → serve_cf(&bag, req, &mut CfConn)
  → CfConn.into_response()  ← no parsing, fields already structured
```

**After (Web):**
```
handle_request_web()
  → run_middleware()
  → router.dispatch() → Arc<WebServe> → serve_web(&bag, req, &mut WebConn)
  → WebConn.into_response()
```

## File Changes

### New files

| File | Description |
|---|---|
| `src/wasm/cf_conn.rs` | `CfConn` struct, `CfConnectionResult`, `into_response()` |
| `src/wasm/web_conn.rs` | `WebConn` struct, `WebConnectionResult`, `into_response()` |
| `src/wasm/serve_cf.rs` | `CfServe` trait definition + `CfServeFactory` |
| `src/wasm/serve_web.rs` | `WebServe` trait definition + `WebServeFactory` |

### Modified files

| File | Change |
|---|---|
| `src/lib.rs` | Export new wasm types behind `wasm-test` / `wasm32` |
| `src/shared/router/mod.rs` | `Router` → `Router<S>`, remove `Server` enum |
| `src/shared/router/method.rs` | `RouteMethod` → `RouteMethod<S>` |
| `src/shared/router/segments.rs` | `RouteSegment` → `RouteSegment<S>` |
| `src/shared/app/mod.rs` | `HttpApp` → `HttpApp<S>`, add `route_writer_cf` / `route_writer_web` methods |
| `src/shared/serve/mod.rs` | Keep `Serve`, `ServeWriter` unchanged; no new traits here |
| `src/wasm/server.rs` | `handle_request` → `handle_request_cf` + `handle_request_web` |
| `src/wasm/bridge/cf.rs` | Use `CfConn` instead of `WasmStream` + `parse_raw` |
| `src/wasm/bridge/web.rs` | Use `WebConn` instead of `WasmStream` + `parse_raw` |
| `src/wasm/stream.rs` | Deprecate `WasmStream` (kept for `ServeWriter` backward compat) |
| `Cargo.toml` | No new dependencies |

### Unchanged files

- All middleware modules (pure logic, no reference to `Server`)
- `ServeError`, `ConnectionResult` (kept as-is, though env-specific variants added)
- `SimpleIncomingRequest`, `SimpleHeaders`, etc. (wire types still used for request parsing)
- Native server, reader, upgrade modules

## Tasks

1. [ ] Create `CfConn`, `CfConnectionResult` in `src/wasm/cf_conn.rs`
2. [ ] Create `WebConn`, `WebConnectionResult` in `src/wasm/web_conn.rs`
3. [ ] Define `CfServe` trait in `src/wasm/serve_cf.rs`
4. [ ] Define `WebServe` trait in `src/wasm/serve_web.rs`
5. [ ] Make `RouteMethod<S>` generic, update all method signatures
6. [ ] Make `RouteSegment<S>` generic, update merge/dispatch logic
7. [ ] Make `Router<S>` generic, remove `Server` enum
8. [ ] Make `HttpApp<S>` generic, add `route_writer_cf` / `route_writer_web`
9. [ ] Update `wasm/server.rs` dispatch to use `CfConn` / `WebConn`
10. [ ] Update `wasm/bridge/cf.rs` to use `CfConn` instead of `WasmStream`
11. [ ] Update `wasm/bridge/web.rs` to use `WebConn` instead of `WasmStream`
12. [ ] Add comprehensive tests for `CfConn` / `WebConn` response collection
13. [ ] Verify native build unchanged (`cargo check -p foundation_http`)
14. [ ] Verify wasm build compiles with new traits (`cargo check --target wasm32-unknown-unknown --features wasm-bindgen-http`)

## Key Design Decisions

### Why separate `CfConn` and `WebConn` if they have the same fields?

- Future-proofing: CF Workers may need `env`-aware helpers (e.g., `conn.bind_d1_response(db_result)`) that make no sense in the browser
- If Rust ever allows specialization or if/when the platforms diverge in how they construct `web_sys::Response`, the split is already there
- Users implementing `CfServe` signal intent — their handler is explicitly for Cloudflare

### Why keep `ServeWriter` at all?

- Existing code depends on it. Migration is additive, not breaking.
- Simple handlers that just write bytes don't care about the target environment.
- `WasmStream` remains the backing store for `ServeWriter` on wasm.

### Why not a single `Serve<C>` trait with generic `C`?

- `CfConn` and `WebConn` are not interchangeable semantically, even if structurally similar.
- Having separate trait names (`CfServe`, `WebServe`) means a type can implement both without conflict (once Rust allows it via associated types or specialization).
- Method names `serve_cf` and `serve_web` are self-documenting — you immediately know which environment a handler targets.

### What about `ConnectionResult`?

Native uses `ConnectionResult::{Keep, Take, Close}` because TCP connections have lifecycle. Wasm environments don't have persistent connections — each request is independent. So `CfConnectionResult` and `WebConnectionResult` are simplified: `{Ok, Close(error)}`. No `Keep` or `Take`.

## Verification

```bash
# Native build — must compile unchanged
cargo check -p foundation_http 2>&1 | tee /tmp/native-http.log

# Wasm build with new traits
cargo check -p foundation_http --target wasm32-unknown-unknown --features wasm-bindgen-http 2>&1 | tee /tmp/wasm-http.log

# Tests with wasm-test feature
cargo test -p foundation_http --features wasm-test 2>&1 | tee /tmp/wasm-tests.log
```

---

_Created: 2026-05-19_
