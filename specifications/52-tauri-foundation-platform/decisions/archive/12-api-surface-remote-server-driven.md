# 12 — API surface: remote server-driven

**Date:** 2026-07-04
**Status:** Resolved (architecture), design exploration

### Decision

This is the "app-store bypass" surface. The user's application logic (WASM or
native server) runs on a remote server. The platform shell connects to it over
the network and streams responses — HTML documents, DomOps patches, Arrow data
batches — to the WebView for rendering. UI and logic changes deploy instantly
without app-store review.

### The Rust shell in this context

The Rust shell is the universal wrapper that ships with EVERY app. In the
remote server-driven context, the shell is the local native process that
manages connections, auth, caching, and rendering — the remote server owns
the application logic, but the shell owns the device-side execution.

**In the remote server-driven context, the shell:**

- Wraps Tauri's builder at the API level (`PlatformBuilder::new()` internally
  creates `tauri::Builder::default()`) but runs INSIDE Tauri's lifecycle at
  runtime (registered via `builder.setup()`). Tauri owns the event loop,
  windows, WebViews, and platform integration.
- The user's application logic runs on a remote server. The shell does NOT
  need the user to ship any application code on-device (though the user
  CAN also ship local WASM or native code for hybrid apps — the shell
  handles both transparently).
- When Tauri fires the setup hook: the shell initializes the session backbone,
  wires transport lanes, opens the cache database, and creates the WebView.
- Connects to the remote backend: establishes SSE/WebSocket connections,
  manages reconnection with exponential backoff, validates TLS, attaches
  auth tokens from secure storage.
- Mediates EVERYTHING between the remote server and the device: auth tokens
  never enter the WebView JS context. The shell attaches them to requests.
  Capability requests from remote content go through the session's
  permission checks. Remote HTML cannot invoke Tauri commands directly.
- Manages the offline fallback: when the remote server is unreachable, the
  shell serves cached content from SQLite. When connectivity returns, the
  shell replays the mutation queue and fetches fresh content. The remote
  server doesn't need to know about offline — the shell handles it.
- Handles the rendering pipeline: the remote server sends HTML, DomOps, or
  Arrow data. The shell delivers it through the rendering lane to the
  WebView. The server picks the protocol; the shell transports it.

**What the shell always provides, regardless of context:**

- Session backbone — route policy (local routes, remote routes, cached
  fallback), capability dispatch (permission-checked, even for remote
  content), cache lookups.
- Transport lanes — all 7 lanes. Remote connections use fetch/SSE/WS;
  local capability calls use IPC; resources use custom protocol.
- WebView bootstrap — runtime loaded. Remote content renders in the WebView.
- Capability registry — remote content can request capabilities. Every
  request goes through the session's profile and permission checks.
- Cache database — SQLite, indexed by route. Remote content is cached
  automatically based on the route's cache policy.
- Lifecycle management — startup, connection management, reconnection,
  shutdown. Mobile background/foreground events pause/resume connections.
- Auth token management — stored in platform secure storage. Attached by
  the shell, never exposed to the WebView or the remote server's JS context.

### What the shell provides for this entrypoint

The user's `main()` runs on a remote server. The shell on-device connects to
it. The shell provides:

- **Connection management** — the shell maintains persistent connections (SSE,
  WebSocket) or makes fetch requests to the remote backend. Reconnection with
  exponential backoff. Connection health monitoring. The user's remote server
  implements the protocol; the shell handles the transport.
- **Session handle** — the native shell holds the session handle. Route
  handlers can decide "this route goes to remote." The shell connects, fetches,
  and delivers the response through the rendering lane.
- **Protocol fan-out** — the remote server can stream any protocol the platform
  supports:
  - HTML fragments (morph into DOM regions).
  - DomOps (typed operations applied by the JS runtime).
  - Arrow IPC (structured data, parsed by Arrow JS in the WebView).
  - Columnar v1 (compact DOM operation batches).
  - JSON payloads (rendered client-side by the UI runtime).
  The server picks the protocol per payload. The shell delivers it.
- **`mount-stream` / `mount-data` integration** — the remote server sends a
  component definition with `mount-stream`; the shell routes the stream to the
  correct island in the WebView. Late-joining clients get backlog replay from
  the `Broadcaster`.
- **Auth token management** — the shell stores auth tokens in secure platform
  storage (iOS Keychain, Android Keystore, desktop credential store). Tokens
  are attached to requests by the shell, not exposed to the WebView's
  JavaScript context. The WebView receives scoped results, not raw tokens.
- **Offline fallback** — when the remote server is unreachable, the shell
  falls back to cached content (see surface 13). The route handler's cache
  policy decides: cache-first, network-first, online-only, local-only.
- **Update mechanism** — the entire UI can change without app-store review.
  The server sends new components, new forms, new flows. The platform shell
  renders them. The native binary is updated rarely.

### How the user wires up (device side)

```rust
// On-device: the shell entrypoint.
// This may be bundled WASM, shell+WASM, or native static lib —
// it just configures remote routes.

#[platform_entrypoint(webview)] // or shell_wasm or native
fn main(session: PlatformSession) {
    // Route remote paths to the remote server.
    session.route("/app/remote/*", RouteDecision::remote_server()
        .with_endpoint("https://myapp.example.com")
        .with_fallback(CachePolicy::StaleWhileRevalidate));

    // Local routes handled on-device.
    session.route("/app/local/*", RouteDecision::local_wasm());

    // The shell handles everything else — connection, auth, caching.
}
```

### How the user wires up (server side)

```rust
// On the server: the user's application.
// This can be foundation_http, any Rust HTTP framework, or any language.
// The server implements the platform's wire protocols.

use foundation_wasm_ui::server::{Broadcaster, BroadcastTx, FrameTransport};

fn handle_connect(req: Request, broadcaster: &Broadcaster) {
    // Each connecting client gets a FrameTransport (SSE, WS, etc.)
    let transport = SseTransport::new(req);
    broadcaster.register(transport);
}

fn handle_action(action: Action, tx: &BroadcastTx) {
    // Server processes the action, pushes updates to all connected clients.
    // The protocol is selected per payload.
    let dom_ops = process_action(action);
    tx.send(&encode_columnar(dom_ops)); // columnar DomOps

    // Or send raw HTML for a full page replace:
    tx.send_text(render_html_page(&state));
}
```

### How it works independently

The app is a thin native shell + WebView. All logic lives on the server. The
device connects on launch. The server streams the initial screen. Every
interaction is a server round-trip (or streamed update). The shell handles
connection lifecycle, auth, caching, and rendering — the server owns
everything else. This is closest to Hotwire Native's architecture.

### How it composes with other surfaces

- **With bundled WASM (surface 9) or shell+WASM (surface 11):** the local
  WASM handles the app shell, navigation, and interactive islands. Remote
  routes fill in content that changes frequently or benefits from
  server-side logic. The route handler decides per-route.
- **With native static library (surface 10):** the native library handles
  performance-critical local operations. Remote handles content and
  collaboration features.
- **With cached/offline replay (surface 13):** remote content cached
  automatically. Offline fallback transparent. The session's cache policy
  selects the strategy: cache-first, network-first, etc.

### Connection lifecycle

1. App launches → shell initializes → WebView renders.
2. Route handler resolves a remote route.
3. Shell establishes connection to the remote endpoint:
   - SSE for unidirectional server→client streaming.
   - WebSocket for bidirectional communication.
   - Fetch for one-shot request/response.
4. Server authenticates the connection (tokens managed by the shell).
5. Server streams the initial response (HTML, DomOps, or data).
6. Shell delivers through the rendering lane → WebView renders.
7. On subsequent actions: shell sends action to server → server processes →
   streams update → shell delivers → WebView renders.
8. On disconnect: shell retries with backoff. Falls back to cache if available.
9. On reconnect: shell replays missed state from server's backlog (like
   `Broadcaster::register` replays for late joiners).

### Tauri integration: bidirectional hooks

This surface has the lightest Tauri integration — the remote server doesn't
know about Tauri at all. But the on-device shell is deeply integrated. The
shell bridges the remote protocol world to Tauri's native world.

**Shell → Tauri (what the platform wraps for remote communication):**

| Tauri primitive | How this surface hooks in |
|---|---|
| `AppManager` / `AppHandle` | The shell holds `AppHandle<R>` for local operations (window management, native UI, capability execution). Remote content is rendered in the WebView that Tauri manages. |
| Custom protocol | Remote responses (HTML, DomOps, Arrow) are delivered to the WebView through the shell's custom protocol handler or through the JS runtime's `fetch()`/SSE/WS. The shell does not proxy every byte — it sets up the connection and lets the WebView receive directly, but can intercept for caching. |
| Event system | Remote server events flow through the shell's connection (SSE/WS) → shell translates to Tauri events → WebView JS runtime receives. The reverse path: WebView action → Tauri event → shell sends to remote server over WS. |
| `tauri::command` IPC | Capability requests from remote content (camera, biometrics) flow: WebView JS → Tauri command → shell → capability registry → execute. Remote HTML cannot invoke Tauri commands directly — all capability access goes through the session's permission check. |
| Secure storage | Auth tokens are stored in Tauri's secure storage (backed by iOS Keychain / Android Keystore). The shell attaches tokens to remote requests. Tokens never enter the WebView's JavaScript context. |
| Plugin system | Native capabilities needed by remote content are registered as Tauri plugins and exposed through the session's capability registry with route-level permission scoping. |

**Tauri → Shell → Remote server (device events flowing upward):**

| Tauri callback | How it reaches the remote server |
|---|---|
| `setup()` | Shell initializes → connects to remote server → establishes SSE/WS → loads initial screen. |
| `on_navigation()` | User navigates to a remote route → shell intercepts → resolves route → connects to server → server streams response. If already connected, shell sends navigation intent over WS. |
| Mobile lifecycle | App backgrounded → shell sends "client_suspended" to server over WS. Server can pause expensive streams. App foregrounded → shell sends "client_resumed" → server resumes. |
| Connectivity changes | Shell detects online/offline through Tauri's network events → notifies remote server → falls back to cache if needed. |
| `RunEvent::Exit` | Shell sends "client_disconnect" to server → closes connections → tears down. |

**Boundary principle:** Tauri is the local execution environment, not the
remote protocol. The remote server speaks platform wire protocols (DomOps,
Arrow IPC, HTML fragments). The shell translates between those protocols and
Tauri's primitives. The remote server never knows it's talking to Tauri —
it talks to a platform shell, which could be Tauri, a browser, or anything
implementing the same protocol contract.
