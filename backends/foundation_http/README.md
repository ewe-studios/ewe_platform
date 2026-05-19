# foundation_http

Connection-owned, worker-pooled HTTP serving framework built on `foundation_core`.

No async/await — synchronous blocking I/O on worker threads managed by `BackgroundJobRunner`.

## Architecture

- **`shared/`** — wasm-compatible modules (traits, router, middleware, handlers, app builder)
- **`native/`** — TCP server, HTTP reader, protocol upgrades (non-wasm only)
- **`wasm/`** — request dispatch with structured connection types (wasm32 + wasm-test)

## Serve Traits

Four parallel handler traits, each with distinct method names to avoid collision:

| Trait | Method | Target | Connection Type |
|---|---|---|---|
| `Serve` | `fn serve` | native TCP only | `SharedByteBufferStream<RawStream>` |
| `ServeWriter` | `fn serve_writer` | any target | `&mut dyn Write` |
| `CfServe` | `fn serve_cf` | Cloudflare Workers | `&mut CfConn` |
| `WebServe` | `fn serve_web` | browser | `&mut WebConn` |

`CfConn` and `WebConn` capture structured response fields (status, headers, body) —
no HTTP wire format parsing. Their `into_response()` methods convert to `web_sys::Response`
but are gated behind `#[cfg(target_arch = "wasm32")]`.

## Generic Router

The entire routing stack is generic over handler type `S`:

```
HttpApp<S> → Router<S> → RouteSegment<S> → RouteMethod<S>
```

`HttpApp<S>` provides specialized impls for each serve type:
- `HttpApp::new_serve()` / `.route()` / `.route_any()` / `.server()`
- `HttpApp::new_writer()` / `.route_writer()` / `.route_any_writer()`
- `HttpApp::new_cf()` / `.route_cf()` / `.route_any_cf()`
- `HttpApp::new_web()` / `.route_web()` / `.route_any_web()`

## Examples

### Native TCP Server (`Serve`)

For plain TCP or TLS connections running on bare metal or a VPS. Handlers write
HTTP wire bytes directly to a `SharedByteBufferStream<RawStream>`.

```rust
use std::sync::Arc;
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{Serve, ServeFactory, ConnectionResult};
use foundation_http::{SimpleIncomingRequest, SimpleMethod};

struct HomeHandler;

impl ServeFactory for HomeHandler {
    fn create(_bag: &ContextBag) -> Self { HomeHandler }
}

impl Serve for HomeHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        conn: foundation_http::SharedByteBufferStream<foundation_http::RawStream>,
    ) -> ConnectionResult {
        let response = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOk";
        let _ = conn.write_all(response);
        ConnectionResult::Close(None)
    }
}

fn main() {
    let mut app = HttpApp::new_serve();
    app.route::<HomeHandler>(SimpleMethod::GET, "/");
    app.route::<HomeHandler>(SimpleMethod::GET, "/health");

    // With default keep-alive config
    let server = app.server("0.0.0.0:8080");

    let shutdown = Arc::new(foundation_http::OnSignal::new());
    server.serve(&shutdown);
}
```

### ServeWriter (Any Target)

Write to any `&mut dyn Write` — works on both native and wasm targets.
Useful when you want to collect response bytes into a buffer or pipe
to any writable stream.

```rust
use std::sync::Arc;
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{ServeWriter, ServeWriterFactory, ConnectionResult};
use foundation_http::SimpleIncomingRequest;

struct JsonHandler;

impl ServeWriterFactory for JsonHandler {
    fn create(_bag: &ContextBag) -> Self { JsonHandler }
}

impl ServeWriter for JsonHandler {
    fn serve_writer(
        &self,
        _bag: &ContextBag,
        _req: SimpleIncomingRequest,
        conn: &mut dyn std::io::Write,
    ) -> ConnectionResult {
        let body = br#"{"status":"ok"}"#;
        let header = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n";
        let _ = conn.write_all(header);
        let _ = conn.write_all(body);
        ConnectionResult::Close(None)
    }
}

fn main() {
    let mut app = HttpApp::new_writer();
    app.route_writer::<JsonHandler>(SimpleMethod::GET, "/api/status");

    // On native: dispatch returns Arc<dyn ServeWriter>
    // On wasm: call serve_writer with a WasmStream buffer
}
```

### Cloudflare Workers (`CfServe`)

Handlers receive a `&mut CfConn` and set structured fields —
no HTTP wire bytes. The dispatcher converts to `web_sys::Response`.

```rust
use std::sync::Arc;
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::ContextBag;
use foundation_http::wasm::serve_cf::{CfServe, CfServeFactory};
use foundation_http::wasm::cf_conn::{CfConn, CfConnectionResult};
use foundation_http::SimpleIncomingRequest;

struct GreetingHandler;

impl CfServeFactory for GreetingHandler {
    fn create(_bag: &ContextBag) -> Self { GreetingHandler }
}

impl CfServe for GreetingHandler {
    fn serve_cf(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: &mut CfConn,
    ) -> CfConnectionResult {
        // Extract path params matched by the router
        // (params are stored in the request URL at this point)

        conn.set_status(200);
        conn.set_header("Content-Type", "application/json");
        conn.set_body(br#"{"message":"Hello from Cloudflare Workers"}"#.to_vec());

        CfConnectionResult::Ok
    }
}

fn create_app() -> Arc<HttpApp<Arc<dyn CfServe>>> {
    let mut app = HttpApp::new_cf();
    app.route_cf::<GreetingHandler>(SimpleMethod::GET, "/greet");
    app.route_cf::<GreetingHandler>(SimpleMethod::GET, "/api/*");
    Arc::new(app)
}
```

Use via the wasm-bindgen bridge in your Worker:

```rust
use foundation_http::wasm::bridge::cf::CfHttpApp;

#[wasm_bindgen]
pub fn create_worker() -> CfHttpApp {
    let mut cf_app = CfHttpApp::new();
    // Routes registered via cf_app.app()
    cf_app
}
```

### Browser / Web Standard (`WebServe`)

Same structured pattern as `CfServe`, but for generic browser environments.

```rust
use std::sync::Arc;
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::ContextBag;
use foundation_http::wasm::serve_web::{WebServe, WebServeFactory};
use foundation_http::wasm::web_conn::{WebConn, WebConnectionResult};
use foundation_http::SimpleIncomingRequest;

struct StaticHandler;

impl WebServeFactory for StaticHandler {
    fn create(_bag: &ContextBag) -> Self { StaticHandler }
}

impl WebServe for StaticHandler {
    fn serve_web(
        &self,
        _bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        conn: &mut WebConn,
    ) -> WebConnectionResult {
        conn.set_status(200);
        conn.set_header("Content-Type", "text/html");
        conn.set_body(b"<h1>Hello from browser wasm</h1>".to_vec());
        WebConnectionResult::Ok
    }
}

fn create_app() -> Arc<HttpApp<Arc<dyn WebServe>>> {
    let mut app = HttpApp::new_web();
    app.route_web::<StaticHandler>(SimpleMethod::GET, "/");
    app.route_web::<StaticHandler>(SimpleMethod::GET, "/static/*");
    Arc::new(app)
}
```

Use via the web-standard bridge:

```rust
use foundation_http::wasm::bridge::web::WasmHttpApp;

#[wasm_bindgen]
pub fn create_web_app() -> WasmHttpApp {
    WasmHttpApp::new()
}
```

### Middleware (All Targets)

Middleware works identically regardless of serve type. Register on any `HttpApp<S>`:

```rust
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::middleware::{RequestMiddleware, MiddlewareResult};
use foundation_http::shared::context::ContextBag;
use foundation_http::SimpleIncomingRequest;
use foundation_http::wasm::serve_cf::CfServe;
use std::sync::Arc;

struct LoggerMiddleware;

impl RequestMiddleware for LoggerMiddleware {
    fn handle(
        &self,
        _ctx: &Arc<ContextBag>,
        req: &mut SimpleIncomingRequest,
    ) -> MiddlewareResult {
        eprintln!("{} {}", req.method, req.request_url.url);
        MiddlewareResult::Continue
    }
}

// Works on any HttpApp<S>
let mut app = HttpApp::<Arc<dyn CfServe>>::new_cf();
app.middleware(LoggerMiddleware);
app.middleware(CorsMiddleware);
app.route_cf::<GreetingHandler>(SimpleMethod::GET, "/greet");
```

A middleware can short-circuit by returning a response directly:

```rust
struct AuthMiddleware;

impl RequestMiddleware for AuthMiddleware {
    fn handle(
        &self,
        _ctx: &Arc<ContextBag>,
        req: &mut SimpleIncomingRequest,
    ) -> MiddlewareResult {
        let auth = req.headers.get(&SimpleHeader::AUTHORIZATION);
        if auth.is_none() {
            return MiddlewareResult::Response(SimpleOutgoingResponse {
                proto: Proto::HTTP11,
                status: Status::Unauthorized,
                headers: SimpleHeaders::new(),
                body: Some(SendSafeBody::Text("Missing authorization".into())),
            });
        }
        MiddlewareResult::Continue
    }
}
```

## Feature Flags

| Flag | Purpose |
|---|---|
| `wasm-test` | Expose `wasm/` module on native target for testing |
| `wasm-bindgen-http` | Enable wasm-bindgen bridge (CF Workers + web) |

### Conditional Compilation Pattern

The `wasm` module uses a two-level cfg gate:

1. **Outer gate** (`lib.rs`): `#[cfg(any(target_arch = "wasm32", feature = "wasm-test"))]`
   - Opens the entire `wasm/` module when compiling for wasm32 **or** when `--features wasm-test` is passed
   - Allows integration tests in `tests/` to import `foundation_http::wasm::*` while running natively

2. **Inner gates** (individual files): `#[cfg(target_arch = "wasm32")]`
   - Guards APIs that depend on `web_sys`, `wasm_bindgen`, or other wasm-only crates
   - Example: `CfConn::into_response()` returns `web_sys::Response` — only valid on wasm32

This pattern lets types like `CfConn`, `WebConn`, and `WasmStream` (pure Rust data
structures) compile and be tested natively, while the `into_response()` conversion
methods and `bridge/` module (which need `web_sys`) are excluded from native builds.

```
Native build (default):    wasm module OFF
Native test (--features    wasm module ON, but #[cfg(target_arch = "wasm32")]
  wasm-test):              blocks excluded (into_response, bridge)
Wasm build (wasm32):       wasm module ON, all code included
```

## Running Tests

```bash
# All tests (native, including wasm types via wasm-test flag)
cargo test --package foundation_http --features wasm-test

# Verify wasm32 compilation
cargo check --package foundation_http --target wasm32-unknown-unknown --features wasm-bindgen-http
```
