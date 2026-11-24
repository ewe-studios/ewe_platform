# 10 — API surface: native static library

**Date:** 2026-07-04
**Status:** Resolved (architecture), design exploration

### Decision

This is the "maximum performance" surface. The user compiles their Rust logic
as a native static library (`.a` on iOS, `.so` on Android, native binary on
desktop). The platform shell links against it and calls it directly over FFI
in the same process. Zero-copy Arrow communication between user code and the
platform shell. No WASM runtime overhead. No serialization boundary.

### The Rust shell in this context

The Rust shell is the universal wrapper that ships with EVERY app. In the
native static library context, the shell and the user's code run in the same
process with the same privilege level — this is the deepest integration.

**In the native static library context, the shell:**

- Wraps Tauri's builder at the API level (`PlatformBuilder::new()` internally
  creates `tauri::Builder::default()`) but runs INSIDE Tauri's lifecycle at
  runtime (registered via `builder.setup()`). Tauri owns the event loop,
  windows, WebViews, and platform integration. The shell extends Tauri's
  lifecycle — it doesn't replace or contain it.
- The user's code compiles to a static library (`.a` on iOS, `.so` on Android,
  native binary on desktop). The shell links against it at build time.
- When Tauri fires the setup hook: the shell initializes the session backbone,
  wires transport lanes, opens the cache database.
- Resolves and calls `USER_MAIN` through the platform's stable C ABI. The
  user's code receives a `PlatformSession` handle — same API as every other
  surface, but with direct access to native capabilities since it runs in
  the same process.
- Provides zero-copy Arrow: the user's code allocates a `RecordBatch`, returns
  a pointer. The shell reads the exact same bytes — no serialization, no copy.
- Creates the WebView AFTER calling `USER_MAIN` — the user's code can
  pre-warm caches, open databases, start background workers, and configure
  the session before any rendering begins.
- Mediates WebView communication: the user's native code pushes data to the
  WebView through the shell's channel handles. The WebView's capability
  requests flow through the session to the user's native handlers.

**What the shell always provides, regardless of context:**

- Session backbone — route policy, capability dispatch, cache lookups.
- Transport lanes — all 7 lanes. The user's native code can also access
  Tauri primitives directly through the session handle.
- WebView bootstrap — the shell creates the WebView and loads the runtime.
- Capability registry — native capabilities registered, including the
  user's native handlers.
- Cache database — SQLite, indexed by route, profile-gated.
- Lifecycle management — startup, shutdown, mobile background/foreground.
- Update management — frontend assets can be hot-swapped. The native library
  requires app-store review for changes.

### What the shell provides for this entrypoint

The user's `main()` runs as a native function, called by the shell at startup.
The shell provides:

- **Session handle** — same `PlatformSession` as the WebView surface. The
  native-side session connects to the web-side session through the backbone.
  The user's code runs in the same process; it can directly call platform APIs.
- **Direct FFI into user code** — the shell loads the user's library and calls
  `USER_MAIN` through the platform's C ABI. The user's library can also call
  back into the shell through exported functions. All same process, no IPC.
- **Zero-copy Arrow data lane** — the user's code allocates Arrow `RecordBatch`es
  in its own memory, returns a pointer + length. The shell reads them directly
  (same process, same address space). No serialization, no `memcpy`. Arrow's
  columnar layout IS the in-memory format. A 10MB table crosses the boundary
  in microseconds.
- **Transport lanes for WebView communication:**
  - The shell provides channel handles so the user's native code can push data
    to the WebView through the custom protocol lane (binary `ArrayBuffer`).
  - The shell provides event emission so the user's native code can notify the
    WebView of state changes, sync progress, cache invalidation.
  - The shell routes capability requests from the WebView to the user's native
    handlers (or to Tauri plugins / Swift/Kotlin bridges).
- **Filesystem, database, network** — direct access. The user's code runs
  natively. It can open files, query SQLite, make HTTP requests, use mmap.
  The shell provides safe wrappers around Tauri's permission model so these
  capabilities respect the app's security configuration.
- **Offline mode** — implicit. The native code runs on-device. No network
  dependency for local operations. The shell manages connectivity changes
  as events.
- **Update mechanism** — the native library is compiled into the app binary.
  Updates require app-store review. The WebView shell (HTML, CSS, JS, WASM
  runtime) can still be hot-updated independently.

### How the user wires up

```rust
// The user defines their entrypoint as a C-ABI function.
// The shell links it and calls it at startup.

#[no_mangle]
pub extern "C" fn USER_MAIN(session: &PlatformSession) {
    // Same session API as the WebView surface.
    // The user registers the same route handlers, capabilities, etc.
    session.route("/app/*", RouteDecision::local_wasm());

    // User's native logic runs here — database queries, file operations,
    // Arrow data processing, network requests.

    // To push data to the WebView:
    let arrow_data: Vec<u8> = session.encode_arrow_batch(&my_records);
    session.webview_channel().send_binary(&arrow_data);

    // To handle capability requests from the WebView:
    session.on_capability::<CameraCapability>(|req| {
        // Access native camera API directly — no bridge needed
        let photo = native_camera::capture();
        req.respond(photo);
    });
}
```

### How it works independently

A fully native app with the WebView as the rendering plane. All business
logic runs natively. The WebView is a pure rendering surface — it receives
rendered HTML, DomOps, or Arrow data from the native side and applies them
to the DOM. The user writes native Rust; the shell handles the bridge to
the WebView. Offline is the default. No server dependency.

### How it composes with other surfaces

- **With bundled WASM (surface 9):** the native library and WASM coexist.
  Some logic runs natively for performance; some runs in WASM for flexibility.
  The shell routes between them transparently.
- **With remote server-driven (surface 12):** the native library handles
  local operations; remote updates stream in over SSE/WS for content that
  changes frequently. The route handler decides per-route.
- **With shell + WASM (surface 11):** the native library IS the shell's
  embedded business logic. Or: the user ships a thin native library that
  just calls into WASM through the shell's embedded runtime. The shell
  supports both.
- **With cached/offline replay (surface 13):** remote routes cached locally.
  The native library can pre-warm the cache, validate integrity, manage
  invalidation.

### Platform ABI contract

The shell defines a stable C ABI for the user's entrypoint:

```rust
// The platform calls this at startup. The signature is fixed.
// The session handle is passed as an opaque pointer — the user
// code uses platform-provided functions to interact with it.
extern "C" {
    fn platform_session_route(
        session: *mut PlatformSession,
        pattern: *const c_char,
        decision: RouteDecision,
    );
    fn platform_session_emit(
        session: *mut PlatformSession,
        event: *const c_char,
        payload: *const u8, len: usize,
    );
    fn platform_session_encode_arrow(
        session: *mut PlatformSession,
        record_batch: *const ArrowRecordBatch,
        out_ptr: *mut *const u8,
        out_len: *mut usize,
    );
    // ... etc
}
```

The shell provides a safe Rust wrapper crate (`foundation_platform::native`)
so users don't write raw FFI. The C ABI is the stable contract; the Rust
wrapper is ergonomics.

### What the platform invokes and when

1. App launches → Tauri boots → native shell initializes.
2. Shell loads the user's linked library and resolves `USER_MAIN`.
3. Shell creates the platform session and the WebView.
4. Shell calls `USER_MAIN(&session)` — the user's native code initializes.
5. WebView loads `foundation-wasm-ui.js` → runtime ready.
6. The platform begins the rendering loop.
7. On navigation: session intercepts → runs route handler chain (which may
   call into the user's native code) → executes decision → user's native code
   may push data to the WebView → DOM renders.
8. On shutdown: user's native code gets a shutdown signal through the session.
   State persisted, connections closed, cache flushed.

### Tauri integration: bidirectional hooks

This surface is the deepest Tauri integration — native code runs in the same
process as Tauri itself. The platform keeps the FFI boundary clean while
hooking Tauri primitives in both directions.

**Shell → Tauri (what the platform wraps):**

| Tauri primitive | How this surface hooks in |
|---|---|
| `AppManager` / `AppHandle` | The shell passes a `PlatformSession` (which wraps `AppHandle<R>`) to the user's native `main()`. The user's code calls session methods; the session calls Tauri. The user never touches `AppHandle` directly unless they want to. |
| `StateManager` | User's native code can `manage()` state directly into Tauri's `StateManager`. The session also manages its own types. Same `TypeIdMap`, shared between platform and user code. |
| `tauri::command` IPC | Native code can register Tauri commands directly. The session wraps them: `session.register_command("my_action", my_handler)`. Commands are typed, permissioned, and route-scoped through the session. |
| Custom protocol | Native code can register additional protocol handlers. The shell's protocol handler dispatches to user-registered handlers for custom routes. Zero-copy binary responses when the handler returns Arrow bytes. |
| Event system | Native code can emit Tauri events directly. The session translates them: `session.emit("data_updated", &payload)` fires a Tauri event that the WebView receives. Bidirectional — the WebView can emit events that native listeners receive. |
| Plugin system | Native code can use Tauri plugins directly (filesystem, clipboard, etc.). The session's capability registry wraps plugin permissions so capability requests are auditable and route-scoped. |
| Window/WebView management | Native code can create windows, manage WebViews. The session tracks them — `session.create_window(config)` returns a scoped handle. |

**Tauri → Shell (what Tauri drives and native code intercepts):**

| Tauri callback / event | How native code hooks in |
|---|---|
| `setup()` | Shell initializes session, calls `USER_MAIN(&session)`. User's code runs before the WebView is created — can pre-warm cache, open database, start background workers. |
| `on_navigation()` | Native code can register navigation interceptors. Before the session's route handler chain runs, the native code can pre-process: auth checks, deep link resolution, redirect logic. |
| `RunEvent::Exit` | Native code receives shutdown signal through the session. Closes database connections, flushes buffers, persists state. |
| Mobile lifecycle | Native code gets direct callbacks (no JS bridge needed). Can pause/resume background work, manage power-intensive operations, handle memory pressure. |
| Custom protocol requests | Native code can inspect every resource request. Log, block, redirect, or transform responses before they reach the WebView. |

**Boundary principle:** Unlike other surfaces, the native static library has
direct access to Tauri's API surface. The platform does not hide Tauri — it
wraps it in session-scoped, route-aware, permissioned abstractions. Power
users can drop to raw Tauri APIs when needed. The session ensures that even
raw Tauri calls are coordinated (observable, interceptable, auditable).
