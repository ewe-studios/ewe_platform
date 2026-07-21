---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F18-backend-transport"
this_file: "specifications/52-tauri-foundation-platform/features/F18-backend-transport/feature.md"

status: in-progress
priority: critical
created: 2026-07-18

depends_on:
  - "F03-ewe-protocol"
  - "F02-route-handler"
  - "F01-session-backbone"

tasks:
  completed: 5
  uncompleted: 6
  total: 11
  completion_percentage: 45%
---
# F18 — Pluggable BackendTransport (step 5 of execution contract)

## Overview

Step 5 of the execution contract resolves `RouteSource` to actual content
bytes. This feature defines the `BackendTransport` trait and provides
three implementations: stub, closure-based, and real (post-MVP).

[Decision 15](../decisions/15-backend-query-execution.md)

## Requirements

### 1. BackendTransport trait ✅

```rust
pub trait BackendTransport: Send + Sync + 'static {
    fn signal_webview(&self, route: &str) -> Vec<u8>;
    fn dispatch_ipc(&self, target: Option<&str>, route: &str) -> Vec<u8>;
    fn fetch_remote(&self, route: &str) -> Vec<u8>;
}
```

### 2. DefaultTransport (stub) ✅

Returns typed JSON signal envelopes for testing/bootstrapping:
- `signal_webview` → `{"type":"webview_app_signal","route":"..."}`
- `dispatch_ipc` → `{"type":"ipc_shell_dispatch","target":"...","route":"..."}`
- `fetch_remote` → `{"type":"remote_server_fetch","route":"..."}`

### 3. ClosureTransport ✅

Built from closures in `PlatformBuilder::build()`:
- Captures Tauri `AppHandle` for real IPC dispatch
- `signal_webview` → returns HTML page with WASM badge
- `dispatch_ipc` → emits `platform:ipc` event via `AppHandle::emit()`
- `fetch_remote` → returns HTML page with Remote badge

### 4. Session injection ✅

- `session.set_backend(Box<dyn BackendTransport>)` — inject at startup
- `session.backend()` — returns `&dyn BackendTransport` (falls back to `&DEFAULT_TRANSPORT`)
- Called once in `builder.build()` setup hook

### 5. query_backend() dispatch ✅

```rust
pub fn query_backend(transport, decision, route) -> Vec<u8> {
    match decision.source {
        WebviewApp => transport.signal_webview(route),
        IpcShell   => transport.dispatch_ipc(decision.target, route),
        RemoteServer => transport.fetch_remote(route),
    }
}
```

### 6. Real WASM signal (postMessage to WebView) 🔄

Replace `signal_webview` stub: instead of returning HTML, emit a
`postMessage` to the WebView's `foundation-wasm-ui.js` runtime
that triggers the in-WebView WASM app to render at `route`.

The WebView already has the WASM module loaded (from `index.html`).
The platform just needs to tell it: "navigate to /app/home".

### 7. Real IPC dispatch (Tauri command invocation) 🔄

Replace `dispatch_ipc` stub: instead of emitting an event, look up
the registered `#[wasm_app]` instance in the wasmtime registry and
call its exports. Post-MVP per decision 13.

### 8. Real HTTP fetch (remote server) 🔄

Replace `fetch_remote` stub: open an HTTP connection via Tauri's HTTP
plugin or a blocking reqwest call. Attach auth tokens from secure storage.
Return the response body as-is for protocol encoding.

### 9. Offline cache fallback ✅

`execute_decision()` checks cache before backend query. When offline,
`NetworkFirst` routes serve stale cache instead of failing.

### 10. Platform scheme interceptor ✅

[See F19-wasm-annotation-target] — ensures `ewe://` links work on
Android by rewriting to `http://ewe.localhost/`.

## Verification

```bash
cargo test --package foundation_platform
# backend_suite.rs: 6 tests
# walking_skeleton.rs: 10 tests (full execution contract)
# mode_integration.rs: 13 tests (WASM/IPC/Remote via injected transport)
# ewe_handler_integration.rs: 7 tests (ewe:// pipeline)
```
