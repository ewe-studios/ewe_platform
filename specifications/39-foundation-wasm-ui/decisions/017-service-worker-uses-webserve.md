# 017 — ServiceWorker uses `foundation_http` WebServe for routing

**Date:** 2026-06-08
**Status:** Resolved

### Decision

`#[wasm_service]` leverages `foundation_http`'s `WebServe` for routing — no custom routing layer needed.

### How it works

```rust
#[wasm_service]
async fn handler(req: Request) -> Response {
    // Uses foundation_http::WebServe routing
    let routes = Router::new()
        .get("/api/users", get_users)
        .post("/api/auth", login)
        .sse("/api/stream", stream_handler);
    
    routes.handle(req).await
}
```

### Service Worker ↔ WASM ↔ WebServe flow

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

### What this means

- **No custom routing** — `foundation_http`'s `WebServe` handles routes, middleware, SSE, etc.
- **ServiceWorker is just a bridge** — intercepts fetch, passes to WASM, returns response
- **Routes are defined in WASM** — service worker doesn't know the routes, it just forwards everything
- **Proc macro generates the bridge** — `#[wasm_service]` generates both the WASM handler wrapper and the service worker JS that forwards requests

### Route discovery — `service_route()`

WASM returns just the `HttpApp` — the macro extracts routes from it:

```rust
#[wasm_service]
pub fn create_app() -> HttpApp<Arc<dyn WebServe>> {
    let mut app = HttpApp::new_web();
    app.route_web::<UserHandler>(SimpleMethod::Get, "/api/users");
    app
}
```

`Router` caches routes incrementally during registration — `routes()` is O(1):

```rust
pub struct Router<S> {
    root: RouteSegment<S>,
    route_cache: Vec<String>,  // cached flat list, built during add_route()
}

impl Router<S> {
    pub fn add_route(&mut self, method: SimpleMethod, path: &str, handler: S) {
        self.root.merge_route(...);
        self.route_cache.push(format!("{method} {path}"));  // amortized
    }
    
    pub fn routes(&self) -> &[String] {
        &self.route_cache  // O(1), no tree walk
    }
}
```

Then `HttpApp::routes()` just delegates to `self.router.routes()`. Zero overhead, zero user effort.

The proc macro calls the user's function, calls `app.routes()`, and embeds them into the generated service worker JS. Zero user effort.

```javascript
// Generated service-worker.js — routes baked in at build time
const WASM_ROUTES = ["/api/users", "/api/auth"];

self.addEventListener('fetch', (event) => {
    const url = new URL(event.request.url);
    const matches = WASM_ROUTES.some(route => url.pathname.startsWith(route));
    
    if (matches) {
        event.respondWith(handleViaWasm(event.request));
    }
    // else: let browser handle normally → goes to server
});
```

### Why this design

- **No unnecessary WASM round trips** — only matched requests cross the bridge
- **Static routes only** — no dynamic params in the discovery list, just prefixes
- **Works for any execution mode** — the same route list can be used by main-thread WASM to know which requests to handle locally vs proxy to server
- **Build-time baked in** — proc macro extracts routes from `WebServe` registration and writes them into the generated JS

### Build-time route delivery

The proc macro returns both the app and the route list. The route list is written into the generated JS at build time. No TokenStream walking needed — routes are explicitly declared by the developer.

This pattern also works for main-thread WASM — the same `(HttpApp, Vec<&str>)` return lets any WASM mode know which requests to handle locally vs proxy to server.
