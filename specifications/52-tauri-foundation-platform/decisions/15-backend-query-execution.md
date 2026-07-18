# 15 — Backend query execution model (Step 5)

**Date:** 2026-07-18
**Status:** Resolved (implemented)

## Decision

Step 5 of the execution contract — backend query — resolves `RouteSource` to
actual I/O through a pluggable `BackendTransport` trait. Each source variant
has a concrete execution path. The session dispatches based on `source` and
routes bytes through protocol selection + encoding to the rendering lane.

## Table of Contents

1. [BackendTransport trait](#backendtransport-trait)
2. [DefaultTransport: bootstrap stub](#defaulttransport-bootstrap-stub)
3. [ClosureTransport: production wiring](#closuretransport-production-wiring)
4. [How the session dispatches](#how-the-session-dispatches)
5. [Offline fallback](#offline-fallback)
6. [Implementation status](#implementation-status)

---

## BackendTransport trait

```rust
/// Pluggable backend dispatch. One method per RouteSource.
/// Tests inject a test transport; production injects an AppHandle-backed
/// transport via session.set_backend().
pub trait BackendTransport: Send + Sync + 'static {
    /// Signal the WASM app running inside the WebView. Returns an
    /// HTML page that signals the in-WebView code to render.
    fn signal_webview(&self, route: &str) -> Vec<u8>;

    /// Dispatch to the native shell via Tauri event.
    /// `target` names the wasm_app instance; `None` means the root shell.
    fn dispatch_ipc(&self, target: Option<&str>, route: &str) -> Vec<u8>;

    /// Fetch content from a remote server over HTTP.
    /// Auth tokens are attached by the shell — they never enter the WebView.
    fn fetch_remote(&self, route: &str) -> Vec<u8>;
}
```

The dispatch in `query_backend()` matches `RouteSource`:

```rust
pub fn query_backend(
    transport: &dyn BackendTransport,
    decision: &RouteDecision,
    route: &str,
) -> Vec<u8> {
    match decision.source {
        RouteSource::WebviewApp => transport.signal_webview(route),
        RouteSource::IpcShell   => transport.dispatch_ipc(decision.target.as_deref(), route),
        RouteSource::RemoteServer => transport.fetch_remote(route),
    }
}
```

## DefaultTransport: bootstrap stub

A unit struct that returns typed signal JSON envelopes. Used when no
real transport has been injected — apps that haven't wired `set_backend()`.

```rust
pub struct DefaultTransport;

impl BackendTransport for DefaultTransport {
    fn signal_webview(&self, route: &str) -> Vec<u8> {
        serde_json::json!({"type":"webview_app_signal","route":route}).to_string().into_bytes()
    }
    fn dispatch_ipc(&self, target: Option<&str>, route: &str) -> Vec<u8> {
        serde_json::json!({"type":"ipc_shell_dispatch","target":target.unwrap_or("shell"),"route":route}).to_string().into_bytes()
    }
    fn fetch_remote(&self, route: &str) -> Vec<u8> {
        serde_json::json!({"type":"remote_server_fetch","route":route}).to_string().into_bytes()
    }
}
```

The session holds `backend_transport: RwLock<Option<Box<dyn BackendTransport>>>`.
When `None`, `session.backend()` returns `&DEFAULT_TRANSPORT` (a static).

## ClosureTransport: production wiring

Built in `PlatformBuilder::build()` from closures capturing the Tauri `AppHandle`:

```rust
let handle = app.handle().clone();
let wasm = move |route: &str| -> Vec<u8> {
    // Return an HTML page signaling the WASM app to render at `route`.
    // Post-MVP: this should emit a postMessage to foundation-wasm-ui.js
    // and return an empty 204 response — the WASM owns rendering.
    build_html_response("WebviewApp", route, "signal_webview")
};
let ipc = move |target: Option<&str>, route: &str| -> Vec<u8> {
    let t = target.unwrap_or("shell");
    // Emit a Tauri event: handle.emit("platform:ipc", {target: t, route})
    // Post-MVP: dispatch to wasmtime instance or native command handler
    build_html_response("IpcShell", route, &format!("dispatch_ipc → {t}"))
};
let remote = move |route: &str| -> Vec<u8> {
    // Post-MVP: real HTTP fetch via foundation_http
    build_html_response("RemoteServer", route, "fetch_remote")
};
session.set_backend(Box::new(ClosureTransport::new(wasm, ipc, remote)));
```

This construction is the SINGLE injection point — `session.set_backend()` is
called once at startup inside `builder.build()`. Tests call it with their own
transport. The raw `BackendTransport` interface is fully testable without Tauri.

## How the session dispatches

`session.execute_decision()` runs the full 9-step contract:

```
1. (done by caller) Navigation intercepted → NavigationIntent
2. session.resolve_route() → RouteDecision
3. Cache check — cache.should_serve_cached() → serve cached if hit
3b. OFFLINE FALLBACK — if offline + can_serve_offline → serve stale
5. backend::query_backend(session.backend(), decision, route)
     → matches RouteSource → calls BackendTransport method
6. Protocol selection: decision hint → ?proto= query → detect → default
7. encode_protocol() → (body, content_type)
9. session.record_navigation() → PageIdentity update
```

## Offline fallback

`execute_decision()` has an offline cache fallback between step 3 and step 5:

- `CacheFirst` / `LocalOnly`: always checked in step 3
- `StaleWhileRevalidate`: served immediately in step 3, caller spawns revalidation
- `NetworkFirst` / `OnlineOnly`: skipped in step 3, BUT when offline + stale entry
  exists, step 3b serves it instead of calling the (unreachable) backend

## Implementation status

| Component | Status |
|---|---|
| `BackendTransport` trait | Implemented |
| `DefaultTransport` | Implemented (stub JSON) |
| `ClosureTransport` | Implemented (closure-based) |
| `session.set_backend()` | Implemented (wired in `builder.build()`) |
| `session.backend()` fallback | Implemented (returns &DEFAULT_TRANSPORT when None) |
| `query_backend()` dispatch | Implemented |
| Offline fallback in `execute_decision` | Implemented |
| Real `signal_webview` (postMessage to WASM) | Post-MVP (currently returns HTML stub) |
| Real `dispatch_ipc` (Tauri event emit) | Partial (emits event, returns HTML stub) |
| Real `fetch_remote` (HTTP fetch) | Post-MVP (currently returns HTML stub) |
| `#[wasm_bin]` app/ crate loaded in WebView | Implemented (app/ compiles, loaded via JS wrapper) |

The transport trait and injection model are complete. The stub implementations
return mode-aware HTML pages with pill badges so every `RouteSource` is
visually distinguishable. The real backend behavior (postMessage signaling,
wasmtime IPC dispatch, HTTP fetch) layers on top without changing the interface.
