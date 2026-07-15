# 09 — API surface: bundled WASM in WebView

**Date:** 2026-07-04
**Status:** Resolved (architecture), design exploration

### Decision

This is the "offline-first" surface. The user's application WASM is compiled
into the Tauri app bundle and runs in the WebView alongside the
`foundation-wasm-ui` runtime. The native shell runs alongside it in the
same app. Communication between WASM and the native shell happens over the
transport lanes the session backbone provides.

### The Rust shell in this context

The Rust shell is the universal wrapper that ships with EVERY app. It is a
compiled native binary (`.a`/`.so`/executable) that sits between Tauri and
the user's application code. The shell knows what deployment context it's in
and adapts accordingly.

**In the bundled WASM context, the shell:**

- Wraps Tauri's builder at the API level (`PlatformBuilder::new()` internally
  creates `tauri::Builder::default()`) but runs INSIDE Tauri's lifecycle at
  runtime (registered via `builder.setup()`). Tauri owns the event loop,
  windows, WebViews, and platform integration. The shell extends Tauri's
  lifecycle — it doesn't replace or contain it.
- Registers itself INTO Tauri during builder configuration: setup hooks,
  `ewe://` custom protocol handler, commands, event listeners. The session
  backbone, capability registry, cache database, and transport lanes are
  initialized when Tauri fires the setup hook.
- When Tauri fires the setup hook: the shell initializes the session, creates
  the WebView (through Tauri's API), loads `foundation-wasm-ui.js` as an
  initialization script, and instantiates the user's WASM module. The user's
  code runs AFTER the shell's setup completes.
- Loads `foundation-wasm-ui.js` into the WebView as an initialization script.
- Instantiates the user's WASM module inside the WebView's JavaScript context.
- Passes a `PlatformSession` handle to the WASM-side `main()`. This handle
  bridges the WASM (running in the WebView sandbox) to the native shell
  (running in the same process with full Tauri access).
- Mediates every interaction: a WASM-side capability request goes through the
  session handle → shell invokes the Tauri command or native plugin → result
  returns through the session → delivered to the WASM scoped to the correct
  page.

**What the shell always provides, regardless of context:**

- Session backbone — route policy, capability dispatch, cache lookups.
- Transport lanes — all 7 lanes pre-wired and ready.
- WebView bootstrap — `foundation-wasm-ui.js` loaded, runtime initialized.
- Capability registry — native capabilities registered and permissioned.
- Cache database — SQLite, indexed by route, profile-gated.
- Lifecycle management — app startup, shutdown, background/foreground events.
- Update management — WASM module and frontend asset hot-swap.

### What the shell provides for this entrypoint

The user's `main()` runs in WASM inside the WebView. The shell provides:

- **Session handle** — a `PlatformSession` reference injected at startup.
  The WASM-side session coordinates with the native-side session over the
  session backbone. Route policy, capability registry, cache lookups — all
  accessible through this handle.
- **Transport lanes pre-wired:**
  - Tauri command IPC — control lane for native capability calls, app
    metadata, permission checks. The shell wraps `invoke()` so the WASM
    doesn't know about Tauri internals.
  - Tauri events — notification lane for lifecycle, online/offline, sync
    progress. Pushed from the native shell to the WebView.
  - Tauri custom protocol — resource lane for bundled assets (runtime JS,
    CSS, static media). The shell serves these from the app bundle.
  - Fetch/SSE/WebSocket — for reaching remote servers when needed.
- **Native shell IPC** — the WASM talks to the native Rust shell over Tauri
  commands (for control) and custom protocol (for resource/data). For
  high-throughput data, the native shell can push Arrow IPC bytes through
  the custom protocol lane as binary `ArrayBuffer`.
- **Capability registry handle** — capability requests (camera, biometrics,
  file picker, notifications) flow through the session backbone. The WASM
  side makes a capability request; the native side executes it; the result
  returns scoped to the correct route/page.
- **Offline mode** — offline is the default. The WASM runs locally. No
  network round-trip needed. Cache lookups are local. The shell handles
  connectivity changes as events, not as application-layer concerns.
- **Update mechanism** — the WASM module can be hot-updated by downloading a
  new version through the browser. The entire frontend shell (HTML, CSS, JS)
  can also be hot-updated. Only the native binary needs app-store review.

### How the user wires up

```rust
// foundation_platform provides this attribute, like foundation_wasm_ui's
// existing deployment target macros (service worker, webworker, etc.)
#[platform_entrypoint(webview)]
fn main(session: PlatformSession) {
    // User registers their route handlers
    session.route("/app/*", RouteDecision::local_wasm());
    session.route("/remote/*", RouteDecision::remote_fetch());

    // User registers capabilities they need
    session.register_capability::<CameraCapability>();
    session.register_capability::<BiometricCapability>();

    // User sets up their application — signals, templates, components
    // using foundation_wasm_ui's existing APIs (html! macro, etc.)

    // Session starts rendering — the shell bootstraps the WebView,
    // loads the foundation-wasm-ui runtime, and invokes this main()
}
```

### How it works independently

A fully bundled app with no remote server dependency. All WASM runs locally.
All state managed on-device (SQLite, file cache, in-memory). Offline is the
default. Remote content is optional — the route handler can decide some routes
go to remote, but the app boots and functions without any network.

### How it composes with other surfaces

- **With remote server-driven (surface 12):** some routes handled locally by
  WASM, others streamed from remote. The route handler decides per-route.
  `session.route("/app/*", local_wasm()); session.route("/cloud/*",
  remote_fetch())`.
- **With cached/offline replay (surface 13):** remote routes cached
  automatically. When offline, the route handler falls back to cache.
  `session.route("/cloud/*", network_first_fallback_to_cache())`.
- **With shell + WASM (surface 11):** the WASM can run in BOTH places — in
  the WebView for rendering control, and in the native shell for zero-copy
  data processing. Same WASM binary, two entrypoints.

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

### Tauri integration: bidirectional hooks

`foundation_platform` is deeply integrated with Tauri — it is a Tauri crate.
The platform keeps Tauri at the boundary where possible but hooks into its
primitives in both directions.

**Shell → Tauri (what the platform wraps):**

| Tauri primitive | How this surface hooks in |
|---|---|
| `AppManager` / `AppHandle` | The shell holds an `AppHandle<R>`. The session backbone wraps it — route resolution, capability dispatch, and cache lookups flow through the session, not through raw `AppHandle` calls. |
| `StateManager` | The platform's `StateManager` wraps Tauri's — platform-managed types (session state, capability registry, route tables) are inserted via `manage()`. User-managed types are also inserted. Both coexist in the same `TypeIdMap`. |
| `tauri::command` IPC | WASM-side capability calls are routed through the session backbone, which invokes Tauri commands. The session wraps `invoke()` so WASM never sees raw Tauri internals. |
| Custom protocol (`tauri::UriSchemeProtocol`) | The shell registers a platform protocol handler. Bundled assets (JS, WASM, CSS) are served through it. The session routes resource requests through the protocol. |
| Event system (`emit`/`listen`) | The session wraps Tauri events. Lifecycle events (online/offline, app background/foreground) are translated to session events. User code listens on the session, not on raw Tauri events. |
| `WebviewManager` / `WebviewWindow` | The shell creates and manages the WebView through Tauri's API. The session holds a `Webview<R>` handle and injects `foundation-wasm-ui.js` as an initialization script. |
| Plugin system | Native capabilities that Tauri plugins provide (filesystem, clipboard, notifications) are registered in the platform's capability registry and called through the session backbone. |

**Tauri → Shell (what Tauri drives and the shell intercepts):**

| Tauri callback / event | How the shell hooks in |
|---|---|
| `setup()` | Shell initializes: creates platform session, loads WASM module, instantiates runtime, calls user's `main()`. |
| `on_page_load()` | Shell injects session identity into the WebView context. The JS runtime knows which session it belongs to. |
| `on_window_event()` | Shell translates window events into session lifecycle events: focus/blur → session active/inactive, resize → layout invalidation. |
| `on_navigation()` | Shell intercepts WebView navigations through `on_navigation()` or a custom protocol handler. Routes the URL through the session's route handler chain instead of letting the browser handle it directly. |
| `RunEvent::Exit` / `RunEvent::ExitRequested` | Shell tears down: signals user code, persists state, flushes cache, closes connections. |
| Mobile lifecycle (`AppDelegate` / `Activity` callbacks) | Shell translates iOS/Android lifecycle events (didEnterBackground, willEnterForeground, onPause, onResume) into session events. Connectivity changes, memory pressure, and app suspension all flow through the session. |

**Boundary principle:** The platform never bypasses Tauri. It wraps Tauri's
primitives in the session coordination model. User code talks to the session.
The session talks to Tauri. Tauri's primitives remain the authoritative source
for window/webview/plugin/event state — the session just coordinates them.
