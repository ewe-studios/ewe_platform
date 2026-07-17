# 15 — Backend query execution model (Step 5)

**Date:** 2026-07-18
**Status:** Resolved

## Decision

Step 5 of the execution contract — backend query — resolves `RouteSource` to
actual I/O. Each source variant has a concrete execution path. The session
dispatches based on `source` and returns content bytes to the rendering lane.

## Table of Contents

1. [WebviewApp: in-process signal](#webviewapp-in-process-signal)
2. [IpcShell: Tauri command IPC + native shell](#ipcshell-tauri-command-ipc--native-shell)
3. [RemoteServer: HTTP fetch](#remoteserver-http-fetch)
4. [How the session dispatches](#how-the-session-dispatches)
5. [Async model: valtron integration](#async-model-valtron-integration)

---

## WebviewApp: in-process signal

When `source = WebviewApp`, the app code is already running inside the WebView.
The session doesn't fetch anything — it signals the route change via
`postMessage` and the in-WebView code renders directly.

```rust
impl PlatformSession {
    fn query_webview_app(&self, route: &str) -> Vec<u8> {
        // Signal the in-WebView code: "you are now at /route"
        // The WebView's foundation-wasm-ui.js receives the message
        // and the WASM app renders using html! macro, signals, templates.
        //
        // No transport, no network, no IPC. The content comes back
        // through the rendering lane as DomOps or HTML, generated
        // entirely inside the WebView.
        
        // For now, return a status message — the actual rendering
        // happens inside the WebView, not via this return value.
        format!("webview_app: route={route}").into_bytes()
    }
}
```

**Key insight:** `WebviewApp` is NOT a backend query. The session signals the
route change. The in-WebView code renders. The content doesn't pass through
the session at all — it's generated inside the WebView and applied directly
to the DOM by `foundation-wasm-ui.js`.

## IpcShell: Tauri command IPC + native shell

When `source = IpcShell`, content comes from the native shell. The session
sends a request over Tauri's command IPC lane and the shell responds.

```rust
impl PlatformSession {
    fn query_ipc_shell(&self, decision: &RouteDecision, route: &str) -> Vec<u8> {
        // If target is set, route to that wasm_app instance.
        // Otherwise, default to the native shell process.
        let target = decision.target.as_deref().unwrap_or("shell");
        
        // The session sends a command over Tauri IPC:
        //   tauri::command fn platform_route(target, route, params)
        // The shell (native Rust or wasmtime-hosted WASM) handles it,
        // generates content, and returns bytes.
        
        // For now: dispatch to capability-like invocation
        format!("ipc_shell: target={target} route={route}").into_bytes()
    }
}
```

## RemoteServer: HTTP fetch

When `source = RemoteServer`, the session opens a transport to the remote
server and fetches content.

```rust
impl PlatformSession {
    async fn query_remote_server(&self, route: &str) -> Vec<u8> {
        // Open an HTTP connection to the remote backend.
        // Auth tokens attached by shell, never enter JS context.
        // Content streamed back as response body.
        
        // For now: return placeholder
        format!("remote_server: route={route}").into_bytes()
    }
}
```

## How the session dispatches

```rust
impl PlatformSession {
    /// Step 5 of the execution contract: resolve RouteSource to content.
    pub fn query_backend(
        &self,
        decision: &RouteDecision,
        route: &str,
    ) -> Vec<u8> {
        match decision.source {
            RouteSource::WebviewApp => self.query_webview_app(route),
            RouteSource::IpcShell => self.query_ipc_shell(decision, route),
            RouteSource::RemoteServer => self.query_remote_server_sync(route),
        }
    }

    /// Synchronous wrapper for RemoteServer (blocks thread).
    /// Post-MVP: async valtron task instead.
    fn query_remote_server_sync(&self, route: &str) -> Vec<u8> {
        format!("remote_server: route={route}").into_bytes()
    }
}
```

## Async model: valtron integration

Post-MVP, `RemoteServer` queries use valtron async tasks:

```rust
// Post-MVP:
fn query_remote_server(&self, route: &str) -> impl Future<Output = Vec<u8>> {
    let session = self.clone();
    let route = route.to_string();
    async move {
        // Open HTTP connection via foundation_http
        // Stream response bytes
        // Return to session for protocol encoding + rendering
        todo!("valtron async backend query")
    }
}
```

## Implementation priority

| Source | Status |
|---|---|
| WebviewApp | Signal only — content generated in-WebView, no session transport needed |
| IpcShell | Stub — dispatches to capability-like invocation, real IPC post-MVP |
| RemoteServer | Stub — returns placeholder content, real HTTP fetch post-MVP |

All three return content through the session backbone to the rendering lane
(steps 6-8 of the execution contract: protocol selection → encoding → view).
