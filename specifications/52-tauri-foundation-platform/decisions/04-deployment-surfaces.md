# 04 — Deployment surfaces: five ways to deploy application logic

**Date:** 2026-07-04
**Status:** Resolved (architecture), design exploration

## Decision

The platform supports five deployment surfaces for application logic. All five
share the same Rust shell and the same `foundation_wasm_ui` rendering runtime.
They differ in where the application logic executes and how it communicates
with the rendering surface.

## Table of Contents

1. [Overview: the five surfaces](#overview-the-five-surfaces)
2. [The Rust shell: what every surface shares](#the-rust-shell-what-every-surface-shares)
3. [Surface 1: Bundled WASM in WebView](#surface-1-bundled-wasm-in-webview)
4. [Surface 2: Native static library](#surface-2-native-static-library)
5. [Surface 3: Shell + WASM runtime](#surface-3-shell--wasm-runtime)
6. [Surface 4: Remote server-driven](#surface-4-remote-server-driven)
7. [Surface 5: Cached offline replay](#surface-5-cached-offline-replay)
8. [How surfaces compose](#how-surfaces-compose)
9. [Entrypoint modes: how to target each surface](#entrypoint-modes-how-to-target-each-surface)
10. [Comparison matrix](#comparison-matrix)

---

## Overview: the five surfaces

| # | Surface | Where logic runs | App-store update? | Offline? | Zero-copy? |
|---|---|---|---|---|---|
| 1 | **Bundled WASM in WebView** | WASM in WebView | WASM hot-swap, no review | Yes (default) | Via shared `ArrayBuffer` |
| 2 | **Native static library** | Same process as shell (.a/.so) | Full review for logic changes | Yes | Yes (same process) |
| 3 | **Shell + WASM runtime** | WASM in embedded runtime (same process) | WASM hot-swap, no review | Yes | Yes (WASM linear memory) |
| 4 | **Remote server-driven** | Remote server (network) | Instant (no review) | With cache (surface 5) | No (network) |
| 5 | **Cached offline replay** | Cache + fallback to remote | Instant (no review) | Yes (cache-first) | No (local disk) |

Users pick per route. A single app can use all five: bundled WASM for the app
shell, native static lib for performance-critical paths, remote server for
dynamic content, cache for offline replay of remote routes.

---

## The Rust shell: what every surface shares

The Rust shell is the universal wrapper that ships with EVERY app. It is a
compiled native binary (`.a`/`.so`/executable) that sits between Tauri and the
user's application code. The shell knows what deployment context it's in and
adapts accordingly.

**What the shell always provides, regardless of surface:**

- **Session backbone** — route policy, capability dispatch, cache lookups
  ([decision 03](03-session-backbone-transport.md)).
- **Transport lanes** — all 7 lanes pre-wired and ready.
- **WebView bootstrap** — `foundation-wasm-ui.js` loaded, runtime initialized.
- **Capability registry** — native capabilities registered and permissioned
  ([decision 18](18-native-capability-contract.md)).
- **Cache database** — SQLite, indexed by route, profile-gated.
- **Lifecycle management** — app startup, shutdown, background/foreground
  events.
- **Update management** — WASM module and frontend asset hot-swap.

**How the shell wraps Tauri:**

- Wraps Tauri's builder at the API level (`PlatformBuilder::new()` internally
  creates `tauri::Builder::default()`).
- Runs INSIDE Tauri's lifecycle at runtime (registered via `builder.setup()`).
- Tauri owns the event loop, windows, WebViews, and platform integration.
- The shell extends Tauri's lifecycle — it doesn't replace or contain it.

**What varies per surface:**
- Where the application logic binary lives (WebView, native process, remote
  server, cache).
- How the session connects to the application logic (in-memory, FFI, IPC,
  network, local disk).
- What transport carries the responses to the rendering lane.
- How updates are deployed.

---

## Surface 1: Bundled WASM in WebView

**Tagline:** Offline-first. The user's application WASM is compiled into the
Tauri app bundle and runs in the WebView alongside the `foundation-wasm-ui`
runtime.

### How it works

1. At build time: the user's Rust code is compiled to WASM
   (`wasm32-unknown-unknown`). The `.wasm` binary is bundled into the Tauri
   app's assets.

2. At app launch: the shell creates the WebView, injects
   `foundation-wasm-ui.js`, and instantiates the user's WASM module inside the
   WebView's JavaScript context.

3. The platform calls `#[platform_entrypoint(webview)] main()` (or
   `#[wasm_bin] main()`) with a `PlatformSession` handle. This handle bridges
   the WASM (running in the WebView sandbox) to the native shell (running in
   the same process with full Tauri access).

4. The WASM owns the rendering surface. It uses `foundation_wasm_ui`'s APIs —
   `html!` macro, signals, templates, components. No network round-trip
   needed.

### Transport wiring

```
WASM (WebView sandbox)
  │ PlatformSession handle
  │ in-memory channel (no serialization)
  ▼
Native shell (same process)
  │ Tauri AppHandle
  ▼
Tauri primitives (IPC, custom protocol, events, plugins)
```

The WASM talks to the native shell through the `PlatformSession` handle:
- **Capability calls:** WASM calls `session.invoke("camera", payload)` → shell
  routes through capability registry → Tauri command or native plugin → result
  returns through session → delivered to WASM scoped to the correct page.
- **Data:** WASM generates DomOps/HTML/Arrow data directly. No transport
  needed — the WASM is in the WebView, the runtime is in the WebView, the
  rendering is in-process.
- **Native events:** Shell pushes events (online/offline, lifecycle) through
  the session → WASM receives them.

### Offline mode

Offline is the default. The WASM runs locally. No network round-trip needed.
Cache lookups are local. The shell handles connectivity changes as events, not
as application-layer concerns.

### Update mechanism

The WASM module can be hot-updated by downloading a new version through the
browser. The entire frontend shell (HTML, CSS, JS) can also be hot-updated.
Only the native binary needs app-store review.

### How the user wires up

```rust
#[platform_entrypoint(webview)]
fn main(session: PlatformSession) {
    session.route("/app/*", RouteDecision::local_wasm());
    session.route("/remote/*", RouteDecision::remote_fetch());
    session.register_capability::<CameraCapability>();
    session.register_capability::<BiometricCapability>();

    // Application setup using foundation_wasm_ui's APIs:
    // signals, templates, components, html! macro, etc.

    // Session starts rendering — the shell bootstraps the WebView,
    // loads the foundation-wasm-ui runtime, and invokes this main()
}
```

### What the platform invokes and when

1. App launches → Tauri boots → native shell initializes.
2. WebView is created → `foundation-wasm-ui.js` is loaded → WASM runtime
   initializes.
3. The platform calls `#[platform_entrypoint(webview)] main()` with the
   session handle.
4. User's code registers routes, capabilities, components.
5. The platform begins the rendering loop — the initial route loads, the
   first screen renders.
6. On navigation: session intercepts link → runs route handler chain →
   executes decision → renders result.
7. On shutdown: session tears down, cache flushed, state persisted.

---

## Surface 2: Native static library

**Tagline:** Maximum performance. The user compiles their Rust logic as a
native static library (`.a` on iOS, `.so` on Android, native binary on
desktop). The platform shell links against it and calls it directly over FFI
in the same process.

### How it works

1. At build time: the user's Rust code is compiled as a native static library
   for the target platform. The shell links against it.

2. At app launch: the shell loads and calls the user's code directly. Same
   process, same privilege level — deepest integration possible.

3. Communication between user code and shell: direct function calls over FFI.
   Arrow `RecordBatch` data crosses the boundary as pointer + length — the
   bytes are in the same address space. No copy, no serialization.

4. The shell still wraps Tauri — `PlatformBuilder::new()` internally creates
   `tauri::Builder::default()`, registered via `builder.setup()`. Tauri owns
   the event loop; the shell extends Tauri's lifecycle.

### Transport wiring

```
User native code (.a/.so)
  │ FFI (direct function calls)
  │ Arrow IPC (pointer + length, same address space)
  ▼
Native shell (same process)
  │ Tauri AppHandle
  ▼
Tauri primitives
  │ Custom protocol (binary ArrayBuffer)
  ▼
WebView (foundation-wasm-ui runtime)
```

### Zero-copy data path

1. User code compiles to a static library. Linked into the same binary as the
   shell.
2. A Rust function allocates an Arrow `RecordBatch`, returns a pointer +
   length.
3. The shell writes those bytes into the Tauri custom protocol response as a
   binary `ArrayBuffer`.
4. The WebView receives the `ArrayBuffer` — Arrow JS or the WASM runtime reads
   it directly.

A 10MB table crosses the Rust→Swift boundary in microseconds — the cost of a
pointer write, not a memory copy. Works today via UniFFI (iOS), JNI (Android),
and direct FFI (desktop).

### Two tiers of native integration, both with zero-copy

| Tier | What the user ships | Zero-copy? | App-store update? |
|---|---|---|---|
| Static library | Native `.a`/`.so` of Rust logic | Yes (same process) | Full review for logic changes |
| Shell + WASM (surface 3) | Native shell (once) + WASM module | Yes (WASM linear memory) | WASM hot-swap, no review |

### Update mechanism

The native static library is baked into the app binary. Any logic change
requires an app-store review (iOS/Android) or a full binary update (desktop).
This is the tradeoff for maximum performance.

---

## Surface 3: Shell + WASM runtime

**Tagline:** Native performance, web-like update agility. A thin, pre-compiled
static library embeds a WASM runtime (wasmtime, wasm3, or wasmi). The user's
application logic is a WASM module loaded and executed by the shell.

### How it works

```
Native app (Swift/Kotlin/C++)
  │ same process, shared memory
  ▼
Native Shell (compiled static library, shipped once)
  ├── Embedded WASM runtime (wasmtime / wasmi)
  └── Arrow buffers in WASM linear memory
  │ loads and executes
  ▼
User WASM module (may also run in the WebView, or only on the backend)
```

1. At build time: the platform shell is compiled ONCE as a native static
   library and distributed with the app. The user's application logic is
   compiled to WASM.

2. At app launch: the shell starts the embedded WASM runtime. Loads the user's
   WASM module. Instantiates it with platform-provided imports (session
   handle, capability stubs).

3. The WASM module runs in the same process as the shell. WASM linear memory
   is directly readable by native code — zero-copy Arrow across WASM↔native.

4. The WebView ALSO runs the user's WASM (for rendering control) or a thinner
   runtime that receives DomOps from the shell-side WASM.

### Transport wiring

```
User WASM module (embedded runtime)
  │ WASM linear memory (direct read by shell)
  │ Arrow RecordBatch in WASM memory
  ▼
Native shell (same process)
  │ Custom protocol (binary ArrayBuffer)
  ▼
WebView (foundation-wasm-ui runtime)
```

### Zero-copy Arrow across WASM ↔ native

The WASM module writes Arrow `RecordBatch`es into its linear memory. The
native shell reads those bytes directly (same process, same address space).
Arrow's columnar layout IS the in-memory format — no serialization, no copy.

### Shell-owned update lifecycle

The shell can fetch the latest WASM modules, frontend shell assets (HTML, CSS,
JS, images), and static resources from an update service; validate integrity;
hot-swap without restart; roll back on failure; manage the cache. The entire
presentation layer updates on the fly without app-store rebuild.

### User WASM runs anywhere

The WebView embeds `foundation-wasm-ui`'s own WASM-based runtime. The user's
application WASM can run in the native shell, in an IPC process, on a local
server, or on a remote server. Wherever it runs, it streams responses to the
runtime in the WebView. Users CAN also deploy their WASM to the frontend for
local execution — it's an architectural choice, not a platform constraint.

---

## Surface 4: Remote server-driven

**Tagline:** App-store bypass. The user's application logic (WASM or native
server) runs on a remote server. The platform shell connects to it over the
network and streams responses — HTML documents, DomOps patches, Arrow data
batches — to the WebView for rendering.

### How it works

1. The user's application logic runs on a remote server. The shell does NOT
   need the user to ship any application code on-device (though hybrid
   local+remote apps are fully supported).

2. At app launch: the shell creates the WebView, loads the JS runtime, and
   connects to the remote server.

3. The app's initial route is fetched from the remote server. Every subsequent
   navigation is routed through the session backbone — the route handler
   decides "this route → remote server."

4. The remote server streams responses in any supported protocol (DomOps,
   HTML, Arrow). The platform transports them; the WebView renders them.

### Transport wiring

```
Remote server (user's application logic)
  │ HTTPS (network)
  │ Auth tokens attached by shell (never enter JS context)
  ▼
Native shell (local process)
  │ Custom protocol or direct WebView load
  ▼
WebView (foundation-wasm-ui runtime)
```

### Communication model

- **HTTP fetch** — request/response. The shell proxies requests, attaches auth
  tokens, returns responses through the custom protocol lane.
- **SSE** — unidirectional server-pushed stream. The shell manages the SSE
  connection lifecycle (connect, reconnect, backoff). Incoming events are
  routed through the session backbone.
- **WebSocket** — bidirectional streaming. The shell manages the WS
  connection. Frames are routed through the session.

### Auth token isolation

When the shell fetches from the remote backend, it attaches auth tokens from
Tauri's secure storage (Keychain on iOS, Keystore on Android, OS credential
store on desktop). Tokens never enter the WebView's JavaScript context. Remote
content (even `trustedRemote`) cannot access the raw credential store.

### Offline behavior

Without explicit caching (surface 5), remote routes are unavailable offline.
The route handler can specify `CachePolicy::NetworkFirst` to serve cached
content when offline, or `CachePolicy::OnlineOnly` to show an offline error.

### Update mechanism

UI and logic changes deploy instantly on the server — no app-store review. The
native shell binary rarely needs updating. This is the fastest deployment
model for content and logic changes.

### Relationship to Hotwire Native

This surface is the closest analogue to Basecamp's Hotwire Native model:
server-rendered HTML, Turbo-powered navigation, native shell wraps the web
content. Our model extends it with:
- Protocol choice (DomOps, Arrow, HTML — not just HTML)
- Route-level granularity (some routes remote, some local WASM, some cached)
- Auth token isolation (tokens never in JS context)
- Transport lane abstraction (HTTP, SSE, WS — not just page loads)

---

## Surface 5: Cached offline replay

**Tagline:** Cache-first. Previously-visited remote content is replayed from
the local cache. When online, content refreshes from the server. When offline,
the cache serves as the backend.

### How it works

1. The user visits a remote route (surface 4). The session caches the response
   in the local SQLite database, indexed by route.

2. On subsequent visits to the same route, the cache policy determines
   behavior:
   - `CacheFirst` — serve from cache immediately. No network check.
   - `NetworkFirst` — try the network; fall back to cache on failure.
   - `StaleWhileRevalidate` — serve from cache immediately, refresh in
     background.

3. When offline, the cache serves as the backend. The WASM runtime receives
   cached responses instead of network responses.

4. When connectivity returns, the session replays pending mutations, refreshes
   stale cached content, and transitions back to online mode.

### Transport wiring

```
Remote server (user's application logic)
  │ (when online: HTTP/SSE/WebSocket)
  ▼
Cache layer (local SQLite)
  │ indexed by route, profile-gated
  ▼
Custom protocol handler (serves cached content as ewe:// responses)
  ▼
WebView (foundation-wasm-ui runtime)
```

### Cache storage

The cache database stores responses as-encoded: if the server returned
`application/primal-columnar`, the cache stores the columnar bytes. On replay,
the cache returns the exact same bytes with the same `Content-Type`. The
runtime renders them identically.

### Update strategies, selected per route

- **Full replace** — the snapshot provides instant responsiveness; when the
  server sends the latest page, the platform replaces the whole view. Best for
  content-driven screens where surgical patching adds no value.

- **Surgical update** — the client sends cached state to the backend ("user
  was on step 3 of this form with these values"), and the backend surgically
  patches only what changed. Best for preserving in-progress work.

### Offline mutation queue

When the user performs an action offline, mutations are enqueued locally in
SQLite. Each mutation has a UUID for idempotency. On connectivity restore, the
queue replays in order. Full details in [decision 05](05-offline-and-sync.md).

### What the platform provides vs what the backend owns

| Layer | Platform provides | Backend owns |
|---|---|---|
| Storage | SQLite database, indexed by route | What to cache |
| Retrieval | `session.cache().get(route)` → cached response | When to invalidate |
| Invalidation | `session.cache().invalidate(routes)` | Which routes are stale |
| Replay | Queue storage, replay triggers on connectivity change | Conflict resolution logic |
| Policy | `CachePolicy` enum (per-route in `RouteDecision`) | Which policy per route |

---

## How surfaces compose

A single app can use all five surfaces simultaneously. The route handler
decides per-route:

```rust
#[platform_entrypoint(webview)]
fn main(session: PlatformSession) {
    // Surface 1: App shell runs locally as WASM
    session.route("/app/*", RouteDecision::local_wasm()
        .with_profile(Profile::App));

    // Surface 2: Performance-critical data path uses native static lib
    session.route("/data/analytics/*", RouteDecision::ipc_shell()
        .with_protocol(ProtocolHint::ArrowIpc));

    // Surface 3: Business logic runs in shell's WASM runtime
    session.route("/api/business/*", RouteDecision::ipc_shell()
        .with_render_mode(RenderMode::DataProjection));

    // Surface 4: Dynamic content from remote server
    session.route("/remote/content/*", RouteDecision::remote_fetch()
        .with_profile(Profile::TrustedRemote)
        .with_cache_policy(CachePolicy::NetworkFirst));

    // Surface 5: Remote content with offline replay
    session.route("/remote/static/*", RouteDecision::remote_fetch()
        .with_cache_policy(CachePolicy::CacheFirst));
}
```

Composition patterns:

- **Surfaces 1 + 4:** Some routes handled locally by WASM, others streamed
  from remote. The route handler decides per-route.
  `session.route("/app/*", local_wasm()); session.route("/cloud/*",
  remote_fetch())`.

- **Surfaces 4 + 5:** Remote routes cached automatically. When offline, the
  route handler falls back to cache.
  `session.route("/cloud/*", network_first_fallback_to_cache())`.

- **Surfaces 1 + 3:** The WASM can run in BOTH places — in the WebView for
  rendering control (surface 1), and in the native shell for zero-copy data
  processing (surface 3). Same WASM binary, two entrypoints.

- **Surfaces 2 + 4:** Performance-critical logic compiled natively (surface
  2); dynamic content served remotely (surface 4). Same app, different routes.

---

## Entrypoint modes: how to target each surface

`foundation_wasm_ui` already has a clean entrypoint annotation system:
`#[wasm_bin]` (main thread), `#[wasm_worker]` (web worker), `#[wasm_service]`
(service worker with route table). These are discovered by `CrateScanner`,
compiled to WASM via `WasmBundleGenerator`, and wrapped with mode-appropriate
JS glue.

`foundation_platform` extends this system. It adds platform-specific modes
that map cleanly to desktop and mobile behavior through deep Tauri integration.

### The unified mode taxonomy

| Mode | Attribute | Where it runs | Maps to surface |
|---|---|---|---|
| **Main thread** | `#[wasm_bin]` | WebView main thread | Surface 1 (Bundled WASM) |
| **Web worker** | `#[wasm_worker]` | WebView worker thread | Surface 1 (auxiliary) |
| **Service worker** | `#[wasm_service]` | Browser/WebView SW scope | Surface 4 (remote), Surface 5 (cache) |
| **Platform shell** | `#[platform_bin]` | Native shell process | Surface 2 (native static lib), Surface 3 (shell + WASM) |
| **Platform worker** | `#[platform_worker]` | Background thread in native shell | Surface 2/3 (background tasks, sync) |
| **Platform service** | `#[platform_service]` | In-process HTTP/router server | Surface 4 (in-process server), Surface 3 (WASM service) |

### How existing web modes map to platform behavior

**`#[wasm_service]` on the platform:**

Doesn't mean "compile to a Service Worker JS file" when targeting
desktop/mobile. It means "this function handles routed requests." The platform
provides the equivalent in-process:

| Web behavior | Platform equivalent |
|---|---|
| Service worker intercepts `fetch` events | Native shell starts an in-process HTTP server (or in-memory router). Incoming requests are routed to the annotated function. |
| Service worker route table (`routes` attribute) | Same `routes` attribute. The platform's router matches incoming requests against registered routes and dispatches to the handler. |
| Service worker responds with `Response` objects | Handler receives a `Request`-like struct, returns a `Response`-like struct. The platform's protocol adapter encodes the response in the selected format. |
| Service worker cache API | `foundation_db` (SQLite, Turso) for persistent caching. Platform cache layer for rendered page caching. |

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
fn backend_service(
    req: PlatformRequest,
    session: PlatformSession,
) -> PlatformResponse {
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

### Default entrypoint selection

If the user doesn't annotate any function, the platform provides sensible
defaults:

- `#[platform_bin]` with an empty `main()` that just starts the WebView with
  no custom routes. The app renders whatever `#[wasm_bin]` produces.
- If no `#[wasm_bin]` exists, the WebView loads `index.html` from the bundle.
- If no platform modes exist, the app is a standard Tauri app with the
  platform shell providing the default session and transport lanes.

### Build pipeline and bootstrap flow

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

**Bootstrap flow:**

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
   assets from a remote update service. Platform-native modes (compiled
   `.a`/`.so`) require app-store review. WASM modes (web + shell+WASM) are
   hot-swappable. The user's `#[platform_bin]` can choose the deployment
   strategy per mode.

### Tauri integration per mode

| Mode | Tauri integration |
|---|---|
| `#[wasm_bin]` | WebView loads the generated JS wrapper. WASM instantiates inside the WebView. Session handle injected by the shell's initialization script. |
| `#[wasm_worker]` | WebView creates the worker. The worker host JS is loaded by the WebView. Communication over `postMessage`. |
| `#[wasm_service]` | **Web:** Service worker registers in the WebView's service worker scope. Fetch events intercepted. **Platform:** Shell starts an in-process HTTP server. The `tauri::custom_protocol` handler proxies `ewe://` requests to it. |
| `#[platform_bin]` | Shell calls the function directly (native) or through WASM runtime (shell+WASM). Full `AppHandle` access through the session. Runs before WebView creation. |
| `#[platform_worker]` | Shell spawns a `std::thread` or Tauri async task. Provides typed channel handles. Manages lifecycle (start on app launch, cancel on shutdown). |
| `#[platform_service]` | Shell registers an in-process router. Tauri's custom protocol handler (`ewe://`) routes matching requests to the handler. Responses delivered through the session backbone's rendering lane. |

### Route attribute reuse

The `route` attribute from `#[wasm_service]` is reused for
`#[platform_service]`:

```rust
// Both modes use the same route attribute syntax
#[wasm_service(routes = ["/api/*"])]       // web: service worker routes
#[platform_service(routes = ["/api/*"])]    // platform: in-process router routes
```

---

## Comparison matrix

| | Surface 1: Bundled WASM | Surface 2: Native static lib | Surface 3: Shell + WASM | Surface 4: Remote server | Surface 5: Cached replay |
|---|---|---|---|---|---|
| **Where logic runs** | WebView (WASM) | Shell process (native) | Shell process (WASM runtime) | Remote server | Cache + remote fallback |
| **Compile target** | wasm32-unknown-unknown | Native (aarch64/x86_64) | wasm32-unknown-unknown | Any (server-side) | N/A (cached content) |
| **App-store review on logic change?** | No (WASM hot-swap) | Yes (full binary review) | No (WASM hot-swap) | No (server deploy) | No (cache update) |
| **Offline?** | Yes (default) | Yes (default) | Yes (default) | With cache (surface 5) | Yes (cache-first) |
| **Zero-copy?** | Via shared ArrayBuffer | Yes (same process) | Yes (WASM linear memory) | No (network) | No (local disk) |
| **Performance** | WASM (near-native) | Native (maximum) | WASM (near-native) | Network-bound | Local disk read |
| **Update speed** | Hot-swap (seconds) | App-store (days) | Hot-swap (seconds) | Instant (server) | Instant (cache refresh) |
| **Primary transport** | In-memory (no transport) | FFI + custom protocol | WASM memory + custom protocol | HTTP/SSE/WebSocket | Custom protocol (from SQLite) |
| **Best for** | App shell, UI logic, local-first apps | Performance-critical paths, data processing, crypto | Business logic with fast update needs | Dynamic content, live updates, content-driven apps | Offline access to remote content |
| **Tauri involvement** | WebView hosts WASM | Shell wraps Tauri; user code is same-process | Shell wraps Tauri + WASM runtime | Shell manages connections; Tauri hosts WebView | Shell manages cache; Tauri custom protocol serves |
