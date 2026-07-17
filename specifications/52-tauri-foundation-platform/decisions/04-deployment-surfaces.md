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
  ([decision 18](07-native-capability-contract.md)).
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

### How the user wires up

The user writes annotated functions. The build pipeline generates everything else.

**`#[platform_bin]`** is the native entrypoint — the only one. It's a source
parser (like the existing `CrateScanner` for `#[wasm_bin]`), NOT a proc macro.
It scans the user's crate for all annotations and orchestrates the code
generation pipeline:

```
Build time:
  #[platform_bin] source parser scans the crate
    │
    ├── Finds #[wasm_bin] / #[wasm_worker] / #[wasm_service]
    │     └── Calls the existing WasmBundleGenerator to create src/bin/*.rs
    │         for each function, compile to wasm32-unknown-unknown, generate
    │         JS wrappers, and place bundles in the webview asset directory.
    │         These are for the web/webview ONLY — they use the WebView's
    │         JS runtime, main thread/worker/service worker APIs.
    │
    └── Finds #[wasm_app] (shell-hosted WASM)
          └── Generates src/bin/*.rs for each function, compiles to
              wasm32-wasip1 (or equivalent wasmtime-compatible target),
              places the .wasm in the shell's wasm asset directory
              (separate from the webview assets). Then generates a
              wasmtime wrapper module — a generated/ directory in the
              crate that provides a function returning a wasmtime
              Instance with the WASM loaded and imports wired.
```

**Two separate asset locations:**

| Target | Asset directory | Loaded by |
|---|---|---|
| `#[wasm_bin]` / `#[wasm_worker]` / `#[wasm_service]` | WebView static assets (bundled in Tauri app) | WebView — `foundation-wasm-ui.js` or worker/service worker bootstrap |
| `#[wasm_app]` | Shell WASM assets (not for the WebView) | Native shell — `foundation_wasmtime` loads the bytes, wires imports, returns an instance |

**`foundation_wasmtime`** is a new crate that owns the wasmtime surface. It
takes WASM bytes (via `PackageDirectorate`: bundled in release, local
filesystem in debug) and provides a builder:

```rust
// foundation_wasmtime — takes WASM bytes, returns an instance:
let instance = WasmtimeRuntime::new(wasm_bytes)?
    .with_import("platform", session.exported_functions())
    .with_import("db", db.exported_functions())
    .build()?;

// The session backbone gains access to the instance's exports:
session.register_wasm_app("business_logic", instance);
// Routes can now resolve to IpcShell and the session calls
// into the wasmtime-hosted WASM.
```

**Three ways user code runs locally:**

| Annotation | Where it runs | Who generates it | Session access |
|---|---|---|---|
| `#[wasm_bin]` | WebView main thread | `#[platform_bin]` → `WasmBundleGenerator` → `src/bin` → WASM → webview assets | `PlatformSession` handle bridged from JS |
| `#[wasm_worker]` | WebView worker thread | Same pipeline, generates `{name}-worker.js` + WASM | `postMessage` bridge |
| `#[wasm_service]` | WebView service worker | Same pipeline, generates `{name}-sw.js` + WASM | Fetch event interception within the WebView |
| `#[wasm_app]` | wasmtime inside native shell | `#[platform_bin]` → `src/bin` → WASM → shell assets → generated wasmtime wrapper | Direct — session backbone calls into exports, exports call into session |
| `#[platform_bin]` (native) | Native shell process | Compiled natively, linked into the Tauri binary | Owns the session — creates routes, caps, boots WebView |

The crucial separation: `#[wasm_bin]` / `#[wasm_worker]` / `#[wasm_service]`
are **web-side only**. They compile to WASM, use web APIs (main thread, worker,
service worker), and run inside the WebView's JS context. The platform does not
"map" them to native equivalents — it generates them and loads them in the
WebView, exactly as a browser would.

`#[wasm_app]` is the bridge: WASM running in wasmtime inside the native shell,
with the session backbone as the universal message bus. It's not a web concept
ported to native — it's a native concept that happens to use WASM for isolation
and hot-swap.

### What the platform invokes and when (universal bootstrap)

1. **Build time:** `#[platform_bin]` source parser scans the crate. Web-side
   annotations (`#[wasm_bin]` etc.) trigger WASM bundle generation into webview
   assets. `#[wasm_app]` triggers WASM compilation + wasmtime wrapper generation
   into shell assets. Native `#[platform_bin]` is compiled for the target
   platform. Output: Tauri app bundle with all assets in place.

2. **App launch:** Tauri boots → native shell initializes.
   Shell calls `#[platform_bin] main()` — user registers routes, caps.

3. **Shell starts wasmtime instances** (if `#[wasm_app]` exists) — loads WASM
   bytes via `PackageDirectorate`, wires imports through `foundation_wasmtime`,
   session backbone registers the instance. WASM exports are now callable.

4. **WebView is created** → `foundation-wasm-ui.js` injected → WASM runtime
   inits → `#[wasm_bin]` WASM module instantiated inside the WebView (loaded
   from webview assets).

5. **Rendering loop begins** — initial route loads, first screen renders.
   Navigations flow through the route handler chain, which dispatches to
   `WebviewApp`, `IpcShell` (including wasmtime-hosted `#[wasm_app]`), or
   `RemoteServer`.

6. **Shutdown:** session tears down, wasmtime instances dropped, cache
   flushed, state persisted.

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

3. The shell calls `#[platform_bin] main()` (native side) to register routes
   and capabilities. The WebView loads the generated `#[wasm_bin]` JS wrapper
   (from webview assets), which instantiates the WASM module inside the WebView's
   JS context. The WASM owns the rendering surface. The `PlatformSession` handle
   bridges between them.

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

Route wiring and universal bootstrap flow are in [The Rust shell](#the-rust-shell-what-every-surface-shares) above — they apply to all surfaces, not just Surface 1.

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
  │ Arrow binary (pointer + length, same address space)
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
- Route-level granularity (some routes WebviewApp, some ipc_shell, some cached)
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
queue replays in order. Full details in [decision 12](12-mutation-queue-and-conflict.md).

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
#[platform_bin]
fn main(session: PlatformSession) {
    // Surface 1: App shell runs locally as WASM
    session.route("/app/*", RouteDecision::webview_app()
        .with_profile(Profile::App));

    // Surface 2: Performance-critical data path uses native static lib
    session.route("/data/analytics/*", RouteDecision::ipc_shell()
        .with_protocol(ProtocolHint::Arrow));

    // Surface 3: Business logic runs in shell's WASM runtime,
    // results delivered to WebView (content format is foundation_wasm_ui's concern)
    session.route("/api/business/*", RouteDecision::ipc_shell()
        .with_view_kind(ViewKind::WebView));

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
  `session.route("/app/*", webview_app()); session.route("/cloud/*",
  remote_fetch())`.

- **Surfaces 4 + 5:** Remote routes cached automatically. When offline, the
  route handler falls back to cache.
  `session.route("/cloud/*", network_first_fallback_to_cache())`.

- **Surfaces 1 + 3:** The WASM can run in BOTH places — in the WebView for
  rendering control (surface 1), and in the native shell for zero-copy data
  processing (surface 3). Same source, different compilation targets
  (`wasm32-unknown-unknown` for WebView, wasmtime-compatible WASM for shell).

- **Surfaces 2 + 4:** Performance-critical logic compiled natively (surface
  2); dynamic content served remotely (surface 4). Same app, different routes.

---
## Entrypoint annotations and code generation

### Two sides, clearly separated

| Side | Annotations | Where they run | Who generates them |
|---|---|---|---|
| **Web side** | `#[wasm_bin]`, `#[wasm_worker]`, `#[wasm_service]` | Inside the WebView — main thread, worker, service worker. Use web APIs. | `#[platform_bin]` source parser → `WasmBundleGenerator` → `src/bin/*.rs` → WASM → webview assets |
| **Platform side** | `#[platform_bin]`, `#[wasm_app]`, `#[platform_worker]`, `#[platform_service]` | Inside the native shell process (or its wasmtime runtime). Use Tauri primitives through the session. | `#[platform_bin]` source parser → compilation target varies by annotation |

The `#[platform_bin]` source parser (NOT a proc macro — a source scanner like
the existing `CrateScanner`) is the build-time orchestrator. It discovers all
annotations in the user's crate and triggers the appropriate code generation
for each.

### Web-side annotations (generated for the WebView)

These exist TODAY in `foundation_wasm_ui`. The `#[platform_bin]` source parser
discovers them and delegates to the existing `WasmBundleGenerator` — no change
from how they work now. They compile to `wasm32-unknown-unknown`, generate JS
wrappers, and are placed in the webview static assets directory. The WebView
loads them exactly as a browser would.

| Annotation | WebView role | Generated output |
|---|---|---|
| `#[wasm_bin]` | Main thread WASM app — owns the rendering surface | `{name}.js` + `{name}.wasm` |
| `#[wasm_worker]` | Web Worker — off-main-thread processing | `{name}-worker.js` + `{name}.wasm` |
| `#[wasm_service]` | Service Worker — intercepts fetch events, caches, routes | `{name}-sw.js` + `{name}.wasm` |

**These are NOT "mapped to platform equivalents."** They are web concepts that
run in the WebView using web APIs (`Worker`, `ServiceWorker`, `postMessage`,
`fetch` event interception). The platform provides the WebView; the web side
owns everything inside it.

### Platform-side annotations (generated for the native shell)

These are NEW in `foundation_platform`. The `#[platform_bin]` source parser
discovers them and generates the appropriate compilation targets.

**`#[platform_bin]` — the native entrypoint (mandatory):**

```rust
#[platform_bin]
fn main(session: PlatformSession) {
    session.route("/app/*", RouteDecision::webview_app());
    session.route("/native/*", RouteDecision::ipc_shell());
    session.route("/remote/*", RouteDecision::remote_fetch()
        .with_cache_policy(CachePolicy::NetworkFirst));
    session.register_capability::<CameraCapability>();
}
```

Compiled natively for the target platform. It is the ONLY mandatory annotation.
It sets up routes, capabilities, and boots the WebView. The build pipeline
compiles it as native binary (desktop) or static library (mobile).

**`#[wasm_app]` — WASM hosted in wasmtime inside the shell:**

```rust
#[wasm_app]
fn business_logic(session: PlatformSession) {
    // Compiled to wasm32-wasip1 (or equivalent wasmtime-compatible target).
    // The session backbone calls into this when a route resolves to IpcShell.
    // The WASM exports functions; the session calls them.
    // The WASM can call back into the session for capabilities, DB, etc.
}
```

The source parser:
1. Generates `src/bin/{name}.rs`, compiles to wasmtime-compatible WASM.
2. Places the `.wasm` in the shell's wasm asset directory (NOT webview assets).
3. Generates a wasmtime wrapper module — a `generated/` directory in the
   crate with a function that uses `foundation_wasmtime` to load the WASM
   bytes (via `PackageDirectorate`), wire imports, and return an instance.

`foundation_wasmtime` is a new crate that wraps wasmtime:

```rust
// Generated wrapper (conceptual — generated by #[platform_bin] scanner):
pub fn load_business_logic(session: &PlatformSession) -> WasmtimeInstance {
    let wasm_bytes = PackageDirectorate::get("shell_wasm/business_logic.wasm");
    foundation_wasmtime::Runtime::new(wasm_bytes)?
        .with_import("platform", session.exported_functions())
        .with_import("db", session.db().exported_functions())
        .build()
}
```

`PackageDirectorate` bundles the WASM file in release builds and reads from
the local filesystem in debug mode — the same pattern used elsewhere in the
codebase.

The session backbone registers the instance:

```rust
let instance = generated::business_logic::load(&session);
session.register_wasm_app("business_logic", instance);
// Routes can now resolve to IpcShell and the session calls
// into the wasmtime-hosted WASM.
```

**`#[platform_worker]` — background thread in native shell:**

```rust
#[platform_worker]
fn cache_warmer(session: PlatformSession, rx: WorkerReceiver<CacheTask>) {
    for task in rx {
        let content = prefetch(task.url).await;
        session.cache().store(&task.route, &content);
    }
}
```

The shell spawns a `std::thread` or Tauri async task, provides typed channels,
and manages lifecycle. Full details in [decision 11](11-background-workers.md).

**`#[platform_service]` — in-process HTTP/router server:**

```rust
#[platform_service(routes = ["/api/data", "/api/sync"])]
fn backend_service(req: PlatformRequest, session: PlatformSession) -> PlatformResponse {
    match req.method() {
        Method::Get => query_data(&req, &session),
        Method::Post => handle_action(&req, &session),
        _ => PlatformResponse::method_not_allowed(),
    }
}
```

The shell starts an in-process router. Full details in
[Platform-side annotations](#platform-side-annotations-generated-for-the-native-shell) above.
For long-running foreground services, see [decision 11](11-background-workers.md).

### Code generation pipeline

```
Build time (single pass, driven by #[platform_bin] source parser):

  source parser scans user crate
    │
    ├── Web-side annotations found
    │     ├── #[wasm_bin]    ──→ WasmBundleGenerator → src/bin → wasm32 → webview assets
    │     ├── #[wasm_worker] ──→ WasmBundleGenerator → src/bin → wasm32 → webview assets
    │     └── #[wasm_service]──→ WasmBundleGenerator → src/bin → wasm32 → webview assets
    │
    ├── Platform-side annotations found
    │     ├── #[platform_bin]    ──→ compiled natively (no generation)
    │     ├── #[wasm_app]        ──→ src/bin → wasmtime target → shell wasm assets
    │     │                              + generated/ dir with wasmtime wrapper
    │     ├── #[platform_worker] ──→ compiled natively as background thread
    │     └── #[platform_service]──→ compiled natively with in-process router
    │
    └── Output: Tauri app bundle
          ├── webview assets/   ← JS + WASM bundles (for the WebView)
          ├── shell_wasm/       ← WASM binaries (for wasmtime in the shell)
          └── native binary     ← compiled #[platform_bin] + platform modes
```

### Default entrypoint selection

If the user doesn't annotate any function, the platform provides defaults:

- `#[platform_bin]` with an empty `main()` that starts the WebView.
  The app renders whatever `#[wasm_bin]` produces.
- If no `#[wasm_bin]` exists, the WebView loads `index.html` from the bundle.
- If no platform modes exist, the app is a standard Tauri app with the
  platform shell providing default session and transport lanes.

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
