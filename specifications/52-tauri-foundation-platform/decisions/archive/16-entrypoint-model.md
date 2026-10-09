# 16 — Entrypoint model: extending `#[wasm_bin]`/`#[wasm_worker]`/`#[wasm_service]` into platform targets

**Date:** 2026-07-04
**Status:** Resolved

### Decision

`foundation_wasm_ui` already has a clean entrypoint annotation system:
`#[wasm_bin]` (main thread), `#[wasm_worker]` (web worker), `#[wasm_service]`
(service worker with route table). These are discovered by `CrateScanner`,
compiled to WASM via `WasmBundleGenerator`, and wrapped with mode-appropriate
JS glue (worker host, service worker fetch interception, main thread
instantiation).

`foundation_platform` extends this system rather than inventing a parallel
one. It adds platform-specific modes that map cleanly to desktop and mobile
behavior through deep Tauri integration. Where a web concept has a natural
mobile/desktop analogue, the platform provides it. Where a concept is
web-only or platform-only, new modes fill the gap.

### The unified mode taxonomy

| Mode | Attribute | Where it runs | What the platform builds |
|------|-----------|---------------|--------------------------|
| **Main thread** | `#[wasm_bin]` | WebView main thread | JS wrapper that instantiates WASM, loads runtime, calls `main()`. Unchanged from current system. |
| **Web worker** | `#[wasm_worker]` | WebView worker thread | Worker JS + host JS pair. Unchanged. |
| **Service worker** | `#[wasm_service]` | Browser/WebView service worker scope | Service worker JS with route table + fetch interception. Unchanged for browser. **Extended for platform:** maps to an in-process HTTP server in the native shell. |
| **Platform shell** | `#[platform_bin]` | Native shell process (desktop/mobile) | Compiles as native binary (not WASM). Shell calls `USER_MAIN` through FFI or loads as WASM in embedded runtime. The primary mode for desktop/mobile apps. |
| **Platform worker** | `#[platform_worker]` | Background thread in native shell | Rust `std::thread` or Tauri async task. Service-worker-like behavior natively: handles background sync, cache warming, periodic tasks. |
| **Platform service** | `#[platform_service]` | In-process HTTP/router server in native shell | Receives queries and actions, responds with results. The native analogue of `#[wasm_service]`. Shell starts an HTTP server or in-process router; this function handles incoming requests. |

### How existing web modes map to platform behavior

**`#[wasm_service]` on the platform:**

This doesn't mean "compile to a Service Worker JS file" when targeting
desktop/mobile. It means "this function handles routed requests." The
platform provides the equivalent in-process:

| Web behavior | Platform equivalent |
|---|---|
| Service worker intercepts `fetch` events | Native shell starts an in-process HTTP server (or in-memory router). Incoming requests are routed to the annotated function. |
| Service worker route table (`routes` attribute) | Same `routes` attribute. The platform's router matches incoming requests against the registered routes and dispatches to the handler. |
| Service worker responds with `Response` objects | Handler receives a `Request`-like struct, returns a `Response`-like struct. The platform's protocol adapter encodes the response in the selected format. |
| Service worker cache API | `foundation_db` (SQLite, Turso) for persistent caching. `foundation_platform` cache layer for rendered page caching. |

```rust
// This works identically on web AND platform:
#[wasm_service(routes = ["/api/items", "/api/users"])]
async fn api_handler(req: ServiceRequest) -> ServiceResponse {
    match req.route() {
        "/api/items" => fetch_items().into(),
        "/api/users" => fetch_users().into(),
        _ => ServiceResponse::not_found(),
    }
}

// On web: compiles to WASM, runs as Service Worker, intercepts fetch events.
// On platform: the shell starts an in-process HTTP server,
//   routes matching requests to this function,
//   encodes responses in the selected protocol,
//   and delivers them to the WebView through the rendering lane.
```

**`#[wasm_worker]` on the platform:**

| Web behavior | Platform equivalent |
|---|---|
| Web Worker runs in a separate thread | `std::thread::spawn` or Tauri async task. The function runs on a background thread managed by the shell. |
| `postMessage` for host↔worker communication | The shell provides typed channels: `WorkerSender`/`WorkerReceiver`. Same message-passing semantics, native performance. |
| Worker processes data off the main thread | Same. CPU-intensive work, data processing, Arrow encoding — all off the rendering thread. |

**`#[wasm_bin]` on the platform:**

This is the frontend entrypoint. It compiles to WASM and runs in the WebView
exactly as it does on the web. No change needed — the platform provides the
WebView; the existing `wasm_bin` wrapper instantiates inside it.

### New platform-specific modes

These don't have web equivalents because they leverage native capabilities
that don't exist in the browser:

**`#[platform_bin]` — the native app entrypoint:**

```rust
#[platform_bin]
fn main(session: PlatformSession) {
    // Runs as native code in the shell's process.
    // Full access to Tauri primitives through the session.
    // Can register routes, capabilities, database connections.
    // The shell calls this at startup, before the WebView is created.
    session.route("/app/*", RouteDecision::local_wasm());
    session.register_capability::<CameraCapability>();
}
```

The build pipeline can compile this as:
- Native binary (desktop) — linked directly into the Tauri app.
- Static library (mobile) — compiled as `.a`/`.so`, linked by the shell.
- WASM (shell+WASM mode) — loaded by the embedded WASM runtime.

The user writes one function. The build pipeline targets it for each platform.

**`#[platform_worker]` — background processing:**

```rust
#[platform_worker]
fn cache_warmer(session: PlatformSession, rx: WorkerReceiver<CacheTask>) {
    // Runs on a background thread managed by the shell.
    // Receives tasks, processes them, sends results back.
    for task in rx {
        let content = prefetch(task.url).await;
        session.cache().store(&task.route, &content);
    }
}
```

The shell manages the thread lifecycle, the channel, and shutdown signaling.

**`#[platform_service]` — in-process request handler:**

```rust
#[platform_service(routes = ["/api/data", "/api/sync"])]
fn backend_service(req: PlatformRequest, session: PlatformSession) -> PlatformResponse {
    // Handles queries (GET) and actions (POST/PUT/DELETE).
    // Responds with structured data — Arrow IPC, DomOps, HTML.
    match req.method() {
        Method::Get => query_data(&req, &session),
        Method::Post => handle_action(&req, &session),
        _ => PlatformResponse::method_not_allowed(),
    }
}
```

The shell starts an in-process router. Incoming requests (from WebView actions,
IPC messages, or HTTP fetches) are matched against registered routes and
dispatched to the handler. The handler returns a response; the shell encodes
and delivers it through the appropriate transport lane.

### How the build pipeline discovers and compiles

The existing `WasmBundleGenerator` pattern is extended:

```rust
// Current system (foundation_wasm_ui)
WasmBundleGenerator::new(crate_dir, output_dir)?
    // Scans for: #[wasm_bin], #[wasm_worker], #[wasm_service]
    // Compiles: wasm32-unknown-unknown
    // Outputs: {name}.js, {name}.wasm, {name}-worker.js, {name}-sw.js

// Extended system (foundation_platform)
PlatformBundleGenerator::new(crate_dir, output_dir, target: PlatformTarget)?
    // Scans for: ALL six modes
    // Compiles: wasm32 for web modes, native target for platform modes
    // Outputs: wasm bundles + native binaries + shell wrappers

enum PlatformTarget {
    Web,           // wasm32 only, existing behavior
    Desktop,       // native binary + wasm bundles
    Ios,           // .a static lib + wasm bundles
    Android,       // .so shared lib + wasm bundles
    All,           // everything
}
```

The `route` attribute from `#[wasm_service]` is reused for `#[platform_service]`:

```rust
// Both modes use the same route attribute syntax
#[wasm_service(routes = ["/api/*"])]       // web: service worker routes
#[platform_service(routes = ["/api/*"])]    // platform: in-process router routes
```

### How Tauri integrates with each mode

| Mode | Tauri integration |
|---|---|
| `#[wasm_bin]` | WebView loads the generated JS wrapper. WASM instantiates inside the WebView. Session handle injected by the shell's initialization script. |
| `#[wasm_worker]` | WebView creates the worker. The worker host JS is loaded by the WebView. Communication over `postMessage`. |
| `#[wasm_service]` | **Web:** Service worker registers in the WebView's service worker scope. Fetch events intercepted. **Platform:** Shell starts an in-process HTTP server. The `tauri::custom_protocol` handler proxies `ewe://` requests to it. The handler function receives requests, returns responses. |
| `#[platform_bin]` | Shell calls the function directly (native) or through WASM runtime (shell+WASM). Full `AppHandle` access through the session. Runs before WebView creation. |
| `#[platform_worker]` | Shell spawns a `std::thread` or Tauri async task. Provides typed channel handles. Manages lifecycle (start on app launch, cancel on shutdown). |
| `#[platform_service]` | Shell registers an in-process router. Tauri's custom protocol handler (`ewe://`) routes matching requests to the handler. Responses delivered through the session backbone's rendering lane. |

### Packaging and bootstrap flow

1. **Build time:** `PlatformBundleGenerator` scans the user's crate for all six
   mode attributes. Compiles WASM for web modes. Compiles native for platform
   modes. Generates JS wrappers (web modes) and native shell stubs (platform
   modes). Packages assets for Tauri's bundler.

2. **App launch (desktop/mobile):** Tauri boots → native shell initializes →
   shell calls `#[platform_bin]` main (native or WASM) → user code registers
   routes, capabilities, services → shell creates WebView → loads
   `foundation-wasm-ui.js` → instantiates `#[wasm_bin]` WASM → rendering
   loop begins.

3. **Dev mode:** Shell watches for changes. Web modes hot-reload through the
   dev server. Platform modes recompile and restart. Tauri's dev server proxies
   `ewe://` requests to the in-process router.

4. **Update flow:** The shell can fetch updated WASM modules and frontend
   assets from a remote update service. Platform-native modes (compiled `.a`/
   `.so`) require app-store review. WASM modes (web + shell+WASM) are
   hot-swappable. The user's `#[platform_bin]` can choose the deployment
   strategy per mode.

### Default entrypoint selection

If the user doesn't annotate any function, the platform provides sensible
defaults:

- `#[platform_bin]` with an empty `main()` that just starts the WebView with
  no custom routes. The app renders whatever `#[wasm_bin]` produces.
- If no `#[wasm_bin]` exists, the WebView loads `index.html` from the bundle.
- If no platform modes exist, the app is a standard Tauri app with the
  platform shell providing the default session and transport lanes.
