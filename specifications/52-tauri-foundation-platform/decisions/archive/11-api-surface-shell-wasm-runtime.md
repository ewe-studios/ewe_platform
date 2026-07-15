# 11 — API surface: shell + WASM (native-side WASM runtime)

**Date:** 2026-07-04
**Status:** Resolved (architecture), design exploration

### Decision

This is the "best of both" surface. The user compiles their logic to WASM
once. The platform shell embeds a WASM runtime (wasmtime, wasm3, or wasmi)
in the native binary and loads the user's WASM module. The WASM runs in the
same process as the shell — zero-copy Arrow access, no static library
compilation per target, and hot-swappable without app-store review. This is
the recommended default for most applications.

### Why this surface exists

The native static library (surface 10) gives maximum performance but requires
per-platform compilation and app-store review for logic changes. The bundled
WASM (surface 9) gives deployment flexibility but the WASM runs inside the
WebView's sandbox with no direct access to native APIs.

This surface is the bridge: the WASM runs in the native shell's process with
direct (sandboxed) access to native capabilities, zero-copy Arrow data
exchange, and the ability to be hot-swapped without app-store review. The user
compiles to WASM once; the shell handles everything else.

### The Rust shell in this context

The Rust shell is the universal wrapper that ships with EVERY app. In the
shell+WASM context, the shell embeds a WASM runtime and loads the user's
WASM module into the native process — combining native performance with
WASM's deployment flexibility.

**In the shell+WASM context, the shell:**

- Wraps Tauri's builder at the API level (`PlatformBuilder::new()` internally
  creates `tauri::Builder::default()`) but runs INSIDE Tauri's lifecycle at
  runtime (registered via `builder.setup()`). Tauri owns the event loop,
  windows, WebViews, and platform integration.
- Embeds a WASM runtime (wasmtime on desktop, wasm3 or wasmi on mobile). The
  user compiles their code to WASM once. The shell loads it at runtime.
- When Tauri fires the setup hook: the shell initializes the session backbone,
  starts the embedded WASM runtime, validates and instantiates the user's WASM
  module.
- Calls the WASM `main()` export with a session handle. The WASM module
  imports host functions (`platform_session_*`) that the shell provides.
  These host functions are the controlled, sandboxed bridge to native
  capabilities — the WASM gets zero-copy Arrow access (same process, shared
  linear memory) without arbitrary native memory access.
- Creates the WebView AFTER calling WASM `main()`. The WASM can pre-warm
  caches and configure the session before rendering starts.
- Owns the WASM lifecycle: fetches updates, validates integrity, hot-swaps
  without restart, rolls back on failure.
- Mediates WebView communication: the WASM pushes data to the WebView through
  shell-provided host functions. Capability requests flow back through the
  session backbone.

**What the shell always provides, regardless of context:**

- Session backbone — route policy, capability dispatch, cache lookups.
- Transport lanes — all 7 lanes pre-wired.
- WebView bootstrap — runtime loaded, WebView created.
- Capability registry — native capabilities registered and permissioned.
  The WASM accesses them through typed host imports, not raw FFI.
- Cache database — SQLite, indexed by route, profile-gated.
- Lifecycle management — startup, shutdown, WASM hot-swap, mobile events.
- Update management — shell fetches, validates, and hot-swaps both the WASM
  module AND frontend assets. No app-store review for either.

### What the shell provides for this entrypoint

The user's `main()` is a WASM function. The shell provides:

- **Embedded WASM runtime** — wasmtime (desktop), wasm3 (mobile/embedded),
  or wasmi (no_std). The shell pre-compiles and caches the WASM module for
  instant startup.
- **Session handle** — same `PlatformSession`, imported into WASM as a host
  function. The WASM module imports `platform_session_*` functions from the
  host. The shell provides them.
- **Zero-copy Arrow data lane** — the WASM module writes Arrow `RecordBatch`es
  into its linear memory. The shell's WASM runtime can read those bytes
  directly from the same process. No serialization, no copy. The shell wraps
  this as `session.encode_arrow_batch()` for the WASM side and reads the
  result from the WASM memory buffer.
- **Controlled native capability access** — the WASM module imports only the
  host functions the shell explicitly provides. Unlike native FFI where the
  user's code has arbitrary memory access, WASM's sandbox means the shell
  controls exactly what capabilities are exposed: filesystem (scoped paths),
  network (allowed origins), database (scoped queries), platform APIs
  (capability registry).
- **Transport lanes pre-wired:**
  - Custom protocol lane to push data to the WebView.
  - Event lane for lifecycle notifications.
  - IPC lane (if the WASM needs to talk to another process).
  - Fetch/SSE/WebSocket for remote server communication.
- **Full update lifecycle** — the shell fetches the latest WASM module from
  an update service, validates integrity (hash/signature check), hot-swaps
  without restart, rolls back on failure, manages version cache. Also updates
  the frontend shell (HTML, CSS, JS, static assets).
- **Offline mode** — the WASM runs locally in the shell's process. No network
  required. Local state via SQLite (through shell-provided host functions).
  Connectivity changes delivered as events.

### How the user wires up

```rust
// foundation_platform provides this attribute.
// The platform compiles this to WASM with the shell's host imports.

#[platform_entrypoint(shell_wasm)]
fn main(session: PlatformSession) {
    // Same session API — register routes, capabilities, etc.
    session.route("/app/*", RouteDecision::local_wasm());
    session.route("/remote/*", RouteDecision::remote_fetch());

    // WASM can call host functions for native capabilities.
    // The shell provides these; WASM's sandbox enforces the boundary.
    let db = session.open_database("my_app.db");
    let records = db.query("SELECT * FROM items WHERE status = ?", &["active"]);

    // Push Arrow data to the WebView for rendering.
    session.webview_channel().send_arrow_batch(&records);

    // Register capability handlers.
    session.on_capability::<FilePickerCapability>(|req| {
        // Shell provides the file picker; WASM gets the result.
        // The WASM never touches the filesystem directly.
        let file = req.open();
        req.respond(file);
    });
}
```

### How it works independently

A fully local app. WASM runs in the native shell. WebView renders the UI.
No remote server needed. Offline by default. All state on-device. The shell
manages the WASM lifecycle, the WebView, and the bridge between them.

### How it composes with other surfaces

- **With bundled WASM (surface 9):** the same WASM binary can have TWO
  entrypoints — `#[platform_entrypoint(shell_wasm)]` for native-side
  execution and `#[platform_entrypoint(webview)]` for WebView-side rendering
  control. The shell routes between them.
- **With native static library (surface 10):** the user can start with
  shell+WASM for rapid iteration and later compile the same code as a native
  library for performance-critical deployments. The shell supports both
  entrypoints from the same codebase.
- **With remote server-driven (surface 12):** some routes handled by local
  WASM, some by remote server. The shell coordinates.
- **With cached/offline replay (surface 13):** remote content cached locally.
  WASM manages the cache, validates integrity, handles invalidation.

### Host function ABI

The shell exposes a stable set of host functions to the WASM module:

```rust
// These are imported by the WASM module at compile time.
// The shell provides them at runtime.

#[link(wasm_import_module = "platform")]
extern "C" {
    fn platform_session_route(pattern: *const u8, pattern_len: usize, decision: u32);
    fn platform_session_emit(event: *const u8, event_len: usize, payload: *const u8, payload_len: usize);
    fn platform_session_open_database(name: *const u8, name_len: usize) -> u32; // db handle
    fn platform_session_query(db_handle: u32, sql: *const u8, sql_len: usize, out_ptr: *mut u32, out_len: *mut u32);
    fn platform_session_encode_arrow_batch(data_ptr: *const u8, data_len: usize, out_ptr: *mut u32, out_len: *mut u32);
    fn platform_webview_send_binary(data: *const u8, len: usize);
    fn platform_webview_send_text(text: *const u8, len: usize);
    // ... etc
}
```

The shell wraps these in safe Rust functions so user code never sees raw
pointers. The ABI is stable; the wrapper is ergonomics.

### What the platform invokes and when

1. App launches → Tauri boots → native shell initializes.
2. Shell loads the embedded WASM runtime.
3. Shell fetches the latest WASM module (from bundle or update service).
4. Shell instantiates the WASM module with host imports.
5. Shell calls the WASM `main()` export with the session handle.
6. User's WASM code initializes — registers routes, capabilities, opens
   database, sets up state.
7. WebView loads `foundation-wasm-ui.js` → runtime ready.
8. Platform begins the rendering loop.
9. On navigation: session intercepts → runs route handler chain → may call
   into WASM → executes decision → WASM may push data to WebView → DOM renders.
10. On WASM update: shell downloads new module → validates → hot-swaps
    (instantiates new module, migrates session state, tears down old module)
    → app continues without restart.
11. On shutdown: shell sends shutdown signal to WASM. WASM persists state,
    closes connections. Shell tears down WASM runtime, flushes cache.

### Tauri integration: bidirectional hooks

The shell runs as native code with full Tauri access; the user's WASM runs
in a sandbox with controlled host imports. The platform bridges them without
sacrificing efficiency — WASM gets the capabilities it needs through
explicit imports, not through a lowest-common-denominator abstraction.

**Shell → Tauri (what the platform wraps for WASM):**

| Tauri primitive | How this surface hooks in |
|---|---|
| `AppManager` / `AppHandle` | The shell holds `AppHandle<R>`. The WASM module does NOT see it. Instead, the shell exports `platform_session_*` host functions that wrap Tauri calls. WASM calls `platform_session_route()` → shell calls Tauri's navigation API. |
| `StateManager` | WASM does not access Tauri's `StateManager` directly (type erasure doesn't cross the WASM boundary). The shell provides typed host functions for state the WASM needs: `platform_session_get_config()`, `platform_session_set_cache_policy()`. |
| `tauri::command` IPC | WASM-side capability requests are serialized through the host import ABI. The shell deserializes, invokes the Tauri command, serializes the result back. The WASM sees `session.call_capability("camera", params)` → shell calls `invoke()`. |
| Custom protocol | The shell registers the protocol handler in Tauri. WASM-requested resources (cached pages, Arrow data, assets) are served by the shell through the protocol. The WASM tells the shell "serve this data at this route"; the shell handles the Tauri protocol registration. |
| Event system | WASM imports `platform_session_emit()` and `platform_session_on_event()`. The shell translates: WASM emit → Tauri event → WebView receives. WebView emits → Tauri event → shell calls WASM's registered callback. |
| Plugin system | WASM cannot call Tauri plugins directly (FFI doesn't cross the WASM boundary). The shell wraps plugin capabilities as host functions. WASM calls `platform_camera_capture()` → shell calls the Tauri camera plugin → returns result to WASM. |

**Tauri → Shell → WASM (events flowing downward):**

| Tauri callback | How it reaches WASM |
|---|---|
| `setup()` | Shell initializes → creates WASM runtime → instantiates module → calls WASM `main()`. WASM's `main()` runs after Tauri setup completes. |
| `on_page_load()` | Shell receives the event → calls WASM's registered callback (if any) through a host import. WASM can push initial state to the new page. |
| `on_navigation()` | Tauri fires → shell intercepts → translates to session event → calls WASM's route handler through `platform_session_resolve_route()` host import → WASM returns decision → shell executes. |
| Mobile lifecycle | Tauri mobile plugin fires → shell translates: `app_did_enter_background` → `platform_session_on_lifecycle()` import → WASM's registered handler. WASM can pause work, persist state. |
| `RunEvent::Exit` | Shell sends shutdown signal to WASM → WASM tears down → shell destroys WASM runtime → Tauri exits. |

**Boundary principle:** The WASM boundary is a capability firewall. WASM
imports exactly the host functions the shell provides — no more, no less.
The shell exposes Tauri primitives through a typed, versioned host ABI.
The WASM module never sees raw Tauri handles, pointers, or FFI. This is
safer than raw native FFI (surface 10) while preserving zero-copy Arrow
performance (same process, shared WASM linear memory).
