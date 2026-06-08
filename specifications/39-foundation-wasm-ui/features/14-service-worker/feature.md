# Feature 14: Service Worker Routing

## Description

`#[wasm_service]` leverages `foundation_http`'s `WebServe` for routing. Service worker intercepts fetch, passes to WASM, returns response. Proc macro wraps user's function and generates exports for both request handling and CLI route extraction.

**Decision:** 017

## Module

- Rust: `crates/foundation_wasm_ui/src/wasm_service/` (proc macro)
- JS: generated `service-worker.js` by build pipeline

## How it works

```rust
#[wasm_service]
async fn handler(req: Request) -> Response {
    let routes = Router::new()
        .get("/api/users", get_users)
        .post("/api/auth", login)
        .sse("/api/stream", stream_handler);
    routes.handle(req).await
}
```

## Proc macro generated code

```rust
// Wraps user's function
#[wasm_export]
pub fn __invoke_service(req_ptr: u32, req_len: u32) -> Response {
    let app = user_function();  // instantiate HttpApp
    app.handle_request(/* ... */)
}

// CLI route extraction
#[wasm_export]
pub fn __ewe_extract_routes() -> u32 {
    let app = user_function();
    let routes = app.routes();
    // serialize and return pointer to route list in WASM memory
}
```

## CLI route extraction

1. Compiles WASM via `cargo build --target wasm32-unknown-unknown`
2. Runs WASM binary in lightweight runtime (`wasmer`/`wasmtime`)
3. Calls `__ewe_extract_routes()` to get route list
4. Embeds routes into generated service worker JS

## Generated service worker

```javascript
// Routes baked in at build time
const WASM_ROUTES = ["/api/users", "/api/auth"];

self.addEventListener('fetch', (event) => {
    const url = new URL(event.request.url);
    const matches = WASM_ROUTES.some(route => url.pathname.startsWith(route));
    if (matches) {
        event.respondWith(handleViaWasm(event.request));
    }
});
```

## Flow

```
Browser fetch("/api/users")
  ↓
ServiceWorker intercepts fetch
  ↓ (via postMessage or shared memory)
WASM receives request
  ↓ (foundation_http::WebServe routing)
Route matches → handler executes
  ↓ (response via Arrow or JSON protocol)
ServiceWorker responds to fetch with Response
  ↓
Browser receives response
```

## Dependencies

- Feature 12 (build pipeline — for route extraction)
- Feature 07 (protocol handling — for response encoding)

## Testing

- Proc macro: wraps function, generates both exports
- CLI: extracts routes from compiled WASM
- Service worker: intercepts matching fetch, passes to WASM
- Service worker: non-matching fetch → passes through to server
- Offline: cached responses served
