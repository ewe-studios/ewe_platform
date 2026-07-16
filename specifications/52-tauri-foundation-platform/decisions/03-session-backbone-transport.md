# 03 — Session backbone, transport lanes, and the `ewe://` custom protocol

**Date:** 2026-07-04
**Status:** Resolved

## Decision

A central platform session (inspired by Hotwire Native's `Session` pattern)
spans both `foundation_wasm_ui` (web side) and `foundation_platform` (native
side). Everything plugs into it as a peer. No subsystem talks directly to
another without the session knowing.

The platform provides transport lanes that carry `foundation_wasm_ui`'s
wire protocols. Protocols stay in `wasm_ui`; transports are added by
`foundation_platform`. The primary transport is the `ewe://` custom URI
scheme, registered with Tauri's `UriSchemeProtocol`.

## Table of Contents

1. [Session backbone](#session-backbone)
2. [State ownership and communication model](#state-ownership-and-communication-model)
3. [Protocol vs transport](#protocol-vs-transport)
4. [All transport lanes](#all-transport-lanes)
5. [Protocol selection per response](#protocol-selection-per-response)
6. [The `ewe://` custom protocol](#the-ewe-custom-protocol)
7. [Arrow vs Columnar v1](#arrow-vs-columnar-v1-protocol-positioning)
8. [How it spans both crates](#how-it-spans-both-crates)
9. [Native UI integration tiers](#native-ui-integration-tiers)
10. [What Tauri already provides vs what we build](#what-tauri-already-provides-vs-what-we-build)
11. [Design is our own](#design-is-our-own)

---

## Session backbone

### What it coordinates

- **Navigation intents** — link clicks, form submits, server pushes, native
  gestures. The session intercepts them, runs the route handler chain
  ([decision 02](02-route-policy-model.md)), and executes the resulting
  `RouteDecision`.
- **Bridge messages** — capability requests, native API calls, component
  registration. The session routes them to the appropriate handler and
  delivers responses scoped to the correct route/page.
- **Cache lookups** — a cache hit tells the session "I have content for this
  route"; the session routes it through the rendering lane. The cache doesn't
  inject into the DOM directly.
- **Native capability results** — return to the session, not to JS directly.
  The session delivers them scoped to the correct route/page/session.

### Subsystem peer model

Rendering, networking, caching, native capabilities, and custom protocols
don't talk directly to each other. They register with the session backbone
and communicate through it:

```
Cache subsystem ──→ "I have cached content for /route"
                    Session ──→ routes through rendering lane → DOM

Native capability ──→ "biometric auth result"
                    Session ──→ scoped delivery to correct route/page → WebView

WebView action ──→ "user clicked link"
                  Session → route handler chain → decide → execute
```

### Handler chain registration

Every subsystem registers with the session as a handler. The session iterates
handlers in registration order; first claim wins:

```rust
// Route handlers — claim navigation intents
session.register_handler(MyAppRouter::new(db, auth));
session.on_navigate(|intent, session| { ... });
session.route("/app/*", RouteDecision::webview_app());

// Capability handlers — claim capability requests
session.register_capability::<CameraCapability>();
session.register_capability::<BiometricCapability>();

// Cache — the session checks cache before querying backends
session.cache().register();

// Transport lanes — the session routes through them
session.register_transport(Transport::CustomProtocol);
session.register_transport(Transport::Ipc);
session.register_transport(Transport::RemoteFetch);
```

### Handler chain execution on navigation

When a navigation intent arrives, the session:

1. Constructs a `NavigationIntent` from the intercepted event.
2. Iterates registered `RouteHandler`s in order. First `Some(decision)` wins.
3. If no handler claims the navigation, applies the platform default (external
   browser, or user-configured fallback).
4. Executes the `RouteDecision` — cache check, presentation, backend query,
   protocol selection, transport, rendering. Full trace in [decision 02
   execution contract](02-route-policy-model.md#execution-contract-what-happens-after-a-decision).

### Session lifecycle

```
App launch → Tauri boots
  → setup() hook fires
    → PlatformSession::initialize()
      → Transport lanes pre-wired
      → Capability registry populated
      → Cache database opened
      → Route handlers registered
      → WebView created, foundation-wasm-ui.js injected
      → User's main() called with session handle
        → Rendering loop begins

App suspend (mobile: didEnterBackground)
  → Session emits "background" event
  → Cache screenshots
  → Release idle WebView pool
  → Pause non-critical transports (SSE keepalive only)

App resume (mobile: willEnterForeground)
  → Session emits "foreground" event
  → Validate stale content
  → Re-warm WebView pool if needed
  → Reconnect transports

App shutdown
  → Session emits "shutdown" event
  → Signal user code (graceful teardown)
  → Persist state
  → Flush cache
  → Close transport connections
  → Tauri RunEvent::Exit
```

### State ownership and communication model

"Server" means wherever the application logic runs: WASM in the WebView, Rust
in the native shell, an IPC process on the device, a local embedded server, or
a remote HTTP server. Whatever it is, it owns state. The platform does not
decide where state lives — the application does.

- `foundation_wasm_ui` components do NOT own state by default. They receive
  state from the Rust side and render it. Only `mount-data` and `mount-stream`
  bindings explicitly declare "I need to send state to get state-backed
  updates." This means the majority of the UI is stateless and cache-friendly.
- Users define what state matters when building the app. Components can cache
  state for performance, but it's deliberate, not forced.
- The platform's job is transports and caching. It does not own state
  machines, conflict resolution, or sync protocols. Those are the backend's
  domain.

Users can bring whatever they want — state machines, sync protocols, CRDTs,
event sourcing — as additional tools layered on top of the platform. A native
integrated system for state and sync may come later; for now the focus is
getting the core platform correct.

**Communication model:**

`foundation_wasm_ui` only dictates how changes are **streamed to the frontend
for rendering** (DomOps, HTML fragments, Arrow/JSON payloads, morph patches).
Users are free to decide how and what they wish to communicate for everything
else:

- **RPC** — if users want request-response semantics, the platform provides
  tooling for that over the appropriate transport.
- **WebSocket** — bidirectional streaming; the platform makes it
  straightforward.
- **HTTP** — standard fetch/request; the platform's custom protocol and
  resource lanes handle this.
- **IPC** — for same-device communication to a local Rust process; the
  platform builds a resilient and efficient IPC process.
- **SSE** — unidirectional server-pushed streaming; already supported by
  `foundation_wasm_ui`'s server module.

The platform does not decide the communication model. It provides the tooling,
constructs, and capabilities to make each option pain-free.

**Why this separation:**

`foundation_wasm_ui`'s existing model is clean: the Rust side executes, the UI
renders. Adding state ownership or communication model decisions to the
platform layer would constrain users unnecessarily. The platform is a host,
not a framework — it provides the lanes, the user drives on them however they
want.

---

## Protocol vs transport

**Protocols** (defined by `foundation_wasm_ui`, NOT re-litigated by the platform):

- Custom binary batch instructions (DomOps)
- Columnar v1 (wasm-loop, no-std, TypedArray-friendly DOM operation batches)
- JSON DOM operation representation
- Arrow — raw RecordBatch binary. Single batch: Arrow's in-memory columnar
  layout cast to `&[u8]`. For request/response data payloads. The default
  Arrow protocol for most use cases.
- ArrowIpc — the Arrow IPC streaming format (Schema + DictionaryBatch +
  RecordBatch messages with continuation markers, EOS indicator). For
  multi-batch data streams where the schema may change or multiple batches
  are sent over a single connection.
- HTML fragments (for server-rendered markup)
- Event payloads and function-call ABI frames

**Transports** (added by `foundation_platform`):

- Tauri command IPC — control lane for small typed request/response
- Tauri events — notification lane for lifecycle, progress, invalidation
- Tauri custom protocol — resource lane for bundled/cached/generated/remote
  assets served through the session backbone
- Native shell IPC — zero-copy data lane for same-process Rust↔native
  communication (Arrow RecordBatch bytes delivered as shared-memory `ArrayBuffer`)
- Local embedded server — WebSocket/HTTP lane when the Rust backend runs as a
  standalone process on the device
- SSE/WebSocket to remote servers — standard web streaming for server-driven
  updates
- Browser fetch — standard web resource loading

The platform's transport adapter wraps `foundation_wasm_ui`'s protocol
encoders. The platform imports the encoders from `foundation_wasm_ui`. The
transport is Tauri. The encoders are pure `Vec<u8>` producers — they don't
know or care how the bytes are delivered.

---

## All transport lanes

All lanes are v1. No lane is deferred. The shell provides all of them. The
user chooses which to use per route/component.

### Tauri command IPC — control lane

Small typed request/response. Used for: capability calls (auth, file picker,
camera), sync triggers, app metadata queries, permission checks. Also supports
Arrow binary payloads via `InvokeBody::Raw` on desktop and iOS (not Android).

**Tauri primitive used:** `tauri::command` invoke. Tauri's `InvokeBody` enum
(`ipc/mod.rs:59`) supports both `Json(JsonValue)` AND `Raw(Vec<u8>)`. On
desktop and iOS, Arrow RecordBatch binary can be sent directly as a `Raw`
payload — no base64 wrapping needed. On Android, `Raw` is unsupported; the
platform falls back to the custom protocol lane for Arrow data on Android.

**When to use Arrow in the command lane vs other lanes:**

| Platform | Command IPC (Raw) | Custom protocol lane | Native shell IPC |
|---|---|---|---|
| Desktop | ✅ Arrow binary as `Raw(Vec<u8>)` | ✅ Streaming binary response | ✅ Same-process zero-copy |
| iOS | ✅ Arrow binary as `Raw(Vec<u8>)` | ✅ Streaming binary response | ✅ Same-process zero-copy |
| Android | ❌ `Raw` unsupported (fall back to custom protocol) | ✅ Streaming binary response | ✅ Same-process zero-copy |

The command lane is still best for small control messages (where JSON overhead
is negligible), but it is NOT limited to JSON — binary Arrow payloads work
everywhere except Android. For streaming or Android, use the custom protocol
lane. For same-process, the native shell IPC lane is zero-copy and always
available.

**How it works:**
1. WASM-side code calls `session.invoke("capability_name", payload)`.
2. The session wraps the call in a `CapabilityRequest` with `PageIdentity`.
3. The request is serialized and sent via Tauri's IPC bridge.
4. The native side receives it, routes through the capability registry, and
   executes the handler.
5. The response is serialized back and delivered to the WASM scoped to the
   requesting page.

**Characteristics:**
- Serialization inherent — JSON or MessagePack. Not zero-copy.
- Best for small payloads: auth tokens, capability params, sync commands.
- NOT for data payloads — use custom protocol or native shell IPC for those.
- Bound to Tauri's command lifecycle — request/response only, no streaming.

### Tauri events — notification lane

Lifecycle, progress, and invalidation notifications. Used for: online/offline
status, sync progress, cache invalidation, app lifecycle events.

**Tauri primitive used:** `AppHandle::emit()` / `AppHandle::listen()`.

**How it works:**
1. Native side emits an event: `session.emit("sync_complete", payload)`.
2. Tauri's event system delivers it to all listeners (or scoped listeners).
3. WASM-side code listens: `session.on("sync_complete", |payload| { ... })`.
4. The session wraps Tauri's raw event bus — scoping, deduplication, and
   stale-page guards are added by the session layer.

**Events the platform emits:**
- `online` / `offline` — connectivity changes
- `background` / `foreground` — app lifecycle
- `sync_progress` — background sync status updates
- `cache_invalidated` — routes that need refresh
- `mutation_queue_drained` — offline mutations synced
- `update_available` — new WASM module or frontend assets available
- `memory_pressure` — platform requests content to reduce memory

### Tauri custom protocol — resource lane

The primary data transport. Registered as `ewe://` with Tauri's
`UriSchemeProtocol`. Bundled assets, cached content, generated responses, and
remote-proxied content all flow through this lane.

**Tauri primitive used:** `UriSchemeProtocol` handler.

**Full specification:** [The `ewe://` custom protocol](#the-ewe-custom-protocol) section below.

### Native shell IPC — zero-copy data lane

For same-process communication between the shell and user code — whether that
code is native Rust compiled as a static library OR user WASM running in an
embedded wasmtime/wasmi runtime. From the platform's perspective, both paths
are identical: the shell makes a local call, gets back a pointer + length to
Arrow RecordBatch bytes, and delivers them.

**Two paths, same transport:**

| Path | What runs | How bytes are generated | Zero-copy? |
|---|---|---|---|
| **Native static library** | User Rust code compiled as `.a`/`.so` | Rust allocates `RecordBatch` in process heap, returns `(ptr, len)` | Yes — pointer write, no memory copy |
| **WASM in wasmtime/wasmi** | User WASM module loaded by the shell's embedded WASM runtime | WASM allocates `RecordBatch` in WASM linear memory; the shell reads the bytes directly from the WASM memory buffer | Yes — WASM linear memory is a contiguous `Vec<u8>` in the host process |

Both paths deliver Arrow RecordBatch binary. The platform does not need to
know which path generated the bytes — `RouteSource::IpcShell` covers both
(see [decision 02](02-route-policy-model.md)).

**How it works (both paths):**
1. User code (native or WASM) allocates an Arrow `RecordBatch`.
2. Shell calls into the user code (function call for native, WASM exported
   function for wasmtime) and gets back a pointer + length.
3. The shell writes the bytes into the WebView's buffer via the custom
   protocol lane as an `ArrayBuffer`.
4. Arrow JS (or the WASM runtime) reads the `ArrayBuffer` directly.

**Characteristics:**
- True zero-copy in both native and WASM paths (same process, no sandbox).
- On mobile (WebView in separate OS process): copy-minimized binary transfer.
- Arrow's columnar memory layout means the wire format IS the in-memory
  format — cast the `RecordBatch` to `&[u8]`, done. No serialization.
- A 10MB table crosses the boundary in microseconds — the cost of a pointer
  write, not a memory copy.

**Copy behavior, ordered by cost:**
1. **WASM linear memory as shared buffer** — native code or shell reads
   directly from WASM linear memory (same process). No copy at all.
2. **Custom protocol + binary response** — Rust serves Arrow RecordBatch
   bytes over Tauri custom protocol. Single copy (Rust buffer → fetch buffer).
   Recommended default for data payloads.
3. **postMessage + transferable ArrayBuffer** — ownership transfer where
   supported. Platform-dependent.
4. **SharedArrayBuffer** — true shared memory. Requires COOP/COEP headers.
   Desktop-feasible; mobile support mixed.
5. **Tauri command IPC** — serialization inherent. Control lane only.
6. **WebSocket to local process** — TCP/IP overhead even on localhost.
7. **Disk-based handoff (mmap)** — page sharing on desktop; forced copy on
   mobile. Only for data too large to buffer.

### Local embedded server — HTTP/WebSocket lane

When the Rust backend runs as a standalone process on the device. The shell
connects to `localhost:{port}` over HTTP or WebSocket.

**How it works:**
1. The platform shell (or the OS) starts the backend process.
2. The backend binds to a local port.
3. The shell connects: HTTP for request/response, WebSocket for streaming.
4. Responses flow through the session backbone and into the rendering lane.

**Use cases:** Native shell + WASM configurations where the WASM runtime is
a standalone process. IPC across processes on the same device. Backend
services that need process isolation.

### SSE/WebSocket to remote servers — streaming lane

Standard web streaming for server-driven updates. The platform provides the
connection management, auth token attachment, and reconnection logic.

**How it works:**
1. The session opens an SSE or WebSocket connection to the remote server.
2. Auth tokens are attached by the shell (from Tauri's secure storage) — never
   enter the WebView's JavaScript context.
3. Incoming messages (SSE events, WS frames) are routed through the session
   backbone.
4. The session delivers them to the correct WebView/route.
5. Reconnection: the session manages backoff, retry, and connectivity
   detection.

**Use cases:** Live DomOps streams from a remote server. Real-time
collaboration. Server-pushed navigation commands. Continuous data updates
(dashboards, chat, notifications).

### Browser fetch — standard web lane

Standard web resource loading. The WebView's native `fetch()` API. Used for
resources that don't need platform mediation: CDN assets, third-party APIs,
standard HTTP requests.

**How it works:**
1. The WebView's JavaScript context calls `fetch(url)`.
2. The request goes through the browser's networking stack.
3. No session backbone involvement — this is standard web behavior.
4. CSP restrictions from the WebView profile apply.

**When the session IS involved:** If the URL uses the `ewe://` scheme, the
custom protocol handler intercepts it. Regular `https://` URLs bypass the
session unless the route handler explicitly routes them.

---

## Protocol selection per response

The response protocol is determined by, in priority order:

1. **Route handler's `RouteDecision.protocol`** — the user explicitly chooses.
2. **`proto` query parameter** — the WebView requests a specific format
   (e.g. `?proto=arrow`).
3. **Backend's native format** — whatever the backend (WASM, native, server)
   produces. If the backend emits Arrow RecordBatch binary, that's what gets delivered.
4. **Platform default** — columnar v1 for DomOps, Arrow binary for data.

```rust
fn select_protocol(
    route_decision: &RouteDecision,
    query_hint: Option<&str>,
    backend_output: &[u8],
) -> Protocol {
    route_decision.protocol
        .or_else(|| query_hint.and_then(Protocol::from_str))
        .unwrap_or_else(|| detect_protocol(backend_output))
}
```

Response `Content-Type` headers:

| Protocol | Content-Type |
|---|---|
| Columnar v1 (DomOps) | `application/primal-columnar` |
| Arrow | `application/primal-arrow` (raw RecordBatch binary — single batch, no streaming headers) |
| ArrowIpc | `application/vnd.apache.arrow.stream` (Arrow IPC streaming format: Schema + DictionaryBatch + RecordBatch messages, EOS marker) |
| JSON | `application/primal-json` |
| HTML fragment | `text/html; charset=utf-8` |
| Custom binary | `application/primal-binary` |

---

## The `ewe://` custom protocol

The platform registers `ewe://` as a custom URI scheme with Tauri's
`UriSchemeProtocol`. The handler parses the URI, routes through the session
backbone, encodes the response in the selected protocol, and returns an HTTP
response with appropriate headers.

Sub-schemes differentiate transport purpose: `ewe://`, `ewe+ipc://`,
`ewe+ws://`, `ewe+http://`.

### Why `ewe://`

`platform://` is too generic — it doesn't identify the project. `ewe://` is
specific to the ewe platform. Sub-schemes follow the `scheme+transport`
convention for explicit transport selection.

### Tauri integration

Tauri's `UriSchemeProtocol` signature (verified against source):

```rust
// tauri/crates/tauri/src/manager/webview.rs
pub struct UriSchemeProtocol<R: Runtime> {
    pub handler: Box<
        dyn Fn(UriSchemeContext<'_, R>, http::Request<Vec<u8>>, UriSchemeResponder)
            + Send + Sync
    >,
}
```

The handler receives the full HTTP request (method, headers, body, URI) and
responds asynchronously via `UriSchemeResponder::respond(response)`.

The platform registers the `ewe://` handler in Tauri's `setup()` hook:

```rust
app.webview_on_all(|webview_builder| {
    webview_builder.register_uri_scheme_protocol(
        "ewe",
        platform_protocol_handler,
    );
});
```

### Protocol routing pipeline

```
Browser/WebView requests:  ewe://localhost/app/items?proto=arrow
                                    │
                                    ▼
Tauri intercepts ──→ UriSchemeProtocol handler fires
                                    │
                                    ▼
Platform parses URI:
  ├── scheme:   ewe
  ├── transport: default (custom protocol)
  ├── route:    /app/items
  ├── protocol hint: arrow (from query param)
  └── method:   GET
                                    │
                                    ▼
Session backbone resolves route:
  ├── Route handler chain returns RouteDecision
  ├── Cache policy checked (serve from cache? fetch fresh?)
  └── Backend queried (WebviewApp, IPC to native shell, remote server)
                                    │
                                    ▼
Platform encodes response:
  ├── Protocol selected (from RouteDecision or query hint)
  ├── Content encoded (Arrow binary, columnar, JSON, HTML)
  └── HTTP response built with correct Content-Type
                                    │
                                    ▼
UriSchemeResponder::respond(response)
                                    │
                                    ▼
WebView receives response ──→ foundation-wasm-ui runtime renders
```

### URI structure

```
ewe://localhost/{route}?{params}
ewe+ipc://localhost/{route}?{params}
ewe+ws://localhost/{route}?{params}
ewe+http://localhost/{route}?{params}
```

| Component | Example | Meaning |
|---|---|---|
| Scheme | `ewe` | Platform custom protocol. All ewe traffic. |
| Sub-scheme | `+ipc`, `+ws`, `+http` | Transport hint (defaults to Tauri command IPC for control, custom protocol binary response for data). |
| Host | `localhost` | Always `localhost` — the protocol is local within Tauri's WebView. |
| Route | `/app/items` | App route. Mapped through the session's route handler chain. |
| `proto` param | `?proto=arrow` | Protocol preference hint (columnar, arrow, json, html). The backend can override. |
| `cache` param | `?cache=stale-while-revalidate` | Cache policy hint. The session's cache policy takes precedence. |
| `action` param | `?action=write` | Indicates a mutation (POST/PUT/DELETE semantics over the protocol). |

### Transport modes

| Scheme | Transport | Use case |
|---|---|---|
| `ewe://` | Tauri command IPC + custom protocol | Default. Control messages over IPC, data over binary custom protocol response. |
| `ewe+ipc://` | Tauri command IPC explicitly | Small structured request/response. Capability calls, metadata queries. |
| `ewe+ws://` | WebSocket (through custom protocol upgrade or direct WS) | Bidirectional streaming. Live DomOps, collaborative editing, real-time sync. |
| `ewe+http://` | HTTP fetch (through custom protocol or dev server proxy) | Server-rendered HTML, RESTful API calls, standard web semantics. |

The platform automatically selects the right Tauri primitive for each transport:

| Transport | Tauri primitive used |
|---|---|
| `ewe://` (default) | `UriSchemeProtocol` handler → binary response with typed Content-Type |
| `ewe+ipc://` | `tauri::command` invoke — JSON/MessagePack serialized |
| `ewe+ws://` | WebView `WebSocket` to local or remote endpoint, mediated by shell |
| `ewe+http://` | `UriSchemeProtocol` → proxies to dev server or fetches from remote, returns response |

### Binary streaming

For large payloads or live streams, the custom protocol handler supports:

1. **Chunked transfer encoding** — the `UriSchemeResponder` accepts a
   streaming body. The platform writes chunks as they arrive from the backend.
   The WebView receives them progressively.

2. **WebSocket upgrade** — for `ewe+ws://`, the custom protocol handler
   detects the WebSocket upgrade request and hands the connection to the
   platform's WS handler. The `Broadcaster` / `FrameTransport` model from
   `foundation_wasm_ui` plugs in directly — the WS connection is a
   `FrameTransport`.

3. **ArrayBuffer delivery** — binary responses (Arrow binary, columnar) are
   delivered as `ArrayBuffer` in the WebView. Arrow JS reads them directly
   without parsing. Single copy (Rust buffer → WebView buffer).

### Security

- **`ewe://` is a trusted scheme.** Only the platform registers it. Remote
  content cannot register custom `ewe://` handlers. The `untrustedRemote`
  WebView profile blocks all `ewe://` access.
- **Route-scoped access.** A capability request on `ewe+ipc://app/remote/items`
  is scoped to the `/app/remote/items` route. It cannot access resources
  belonging to `/app/local/settings`.
- **No filesystem exposure.** The handler does not map URIs to filesystem
  paths. It maps URIs to session routes. The session resolves the route; the
  backend provides the content. No `../../../etc/passwd` attack surface.
- **CSP integration.** The `ewe://` scheme is added to the appropriate CSP
  directives (`connect-src`, `script-src`) per WebView profile. `App` profile
  allows `ewe://*`; `trustedRemote` allows `ewe://` for allowed origins only;
  `untrustedRemote` blocks all `ewe://`.
- **Auth token isolation.** When the handler fetches from a remote backend, it
  attaches auth tokens from Tauri's secure storage. Tokens never enter the
  WebView's JavaScript context.

### How `foundation_wasm_ui` protocols plug in

The platform's transport adapter wraps `foundation_wasm_ui`'s protocol
encoders:

```rust
// foundation_platform's protocol handler

fn handle_route_request(
    session: &PlatformSession,
    route: &str,
    protocol_hint: Option<Protocol>,
) -> HttpResponse<Vec<u8>> {
    // 1. Resolve route through session
    let decision = session.resolve_route(route);

    // 2. Check cache
    if let Some(cached) = session.cache().get(route)
        && decision.cache_policy == CachePolicy::CacheFirst {
        return cached.into_response();
    }

    // 3. Query backend — gets DomOps or Arrow bytes
    let (ops, preferred_protocol) = session.query_backend(route, &decision);

    // 4. Select protocol and encode
    let protocol = select_protocol(&decision, protocol_hint, &ops);
    let (body, content_type) = match protocol {
        Protocol::Columnar => (
            foundation_wasm_ui::encode_columnar(&ops),
            "application/primal-columnar",
        ),
        Protocol::Arrow => (
            // Raw RecordBatch binary — single batch, no streaming headers.
            // Cast the RecordBatch to &[u8] and send.
            foundation_wasm_ui::encode_arrow(&ops),
            "application/primal-arrow",
        ),
        Protocol::ArrowIpc => (
            // Arrow IPC streaming format: Schema + DictionaryBatch +
            // RecordBatch messages, continuation markers, EOS indicator.
            // For multi-batch data streams.
            foundation_wasm_ui::encode_arrow_ipc(&ops),
            "application/vnd.apache.arrow.stream",
        ),
        Protocol::Json => (
            foundation_wasm_ui::encode_json(&ops),
            "application/primal-json",
        ),
        Protocol::Html => (
            foundation_wasm_ui::render_html(&ops),
            "text/html; charset=utf-8",
        ),
    };

    // 5. Build HTTP response
    HttpResponse::builder()
        .status(200)
        .header("Content-Type", content_type)
        .header("Access-Control-Allow-Origin", "ewe://localhost")
        .body(body)
        .unwrap()
}
```

### What the platform does NOT provide

- **A new protocol format.** The wire formats are `foundation_wasm_ui`'s
  domain.
- **Server-side protocol handling.** The remote server implements the same
  protocols. The platform transports them; the server produces them.
- **Protocol negotiation beyond the hint.** The platform selects the protocol
  from the decision chain. Complex content negotiation (Accept headers, etc.)
  can be added later if needed.

### Arrow vs Columnar v1: protocol positioning

**TODO**: The ui can use arrow or columnvar v1 - dont be pedantic, and honestly the platform should not care about this, it takes it and delivers.

Arrow protocols are for **structured data payloads**, NOT for UI DOM operations.
The existing `foundation_wasm_ui` protocols handle UI operations. Arrow is used
for data sets and analytics-style payloads where its columnar layout and
zero-copy guarantees add real value.

**Two Arrow protocol variants:**

| Variant | Wire format | Content-Type | When to use |
|---|---|---|---|
| `Arrow` | Raw RecordBatch binary — columnar bytes cast to `&[u8]`. No metadata, no framing. Single batch. | `application/primal-arrow` | The default. Request/response with a single RecordBatch. Zero-copy: the in-memory layout IS the wire format. |
| `ArrowIpc` | Arrow IPC streaming format — Schema message → DictionaryBatch messages → RecordBatch messages → EOS marker. Each message is prefixed with a 4-byte continuation indicator. | `application/vnd.apache.arrow.stream` | Multi-batch streams over a single connection (SSE, WebSocket, streaming HTTP). When the schema may change between batches. Interoperable with Arrow Flight, pyarrow, etc. |

**When to use which:**

| Use case | Protocol |
|---|---|
| Single query result (e.g. "get users where age > 30") | `Arrow` — one RecordBatch, raw bytes |
| Stream of analytics results (e.g. live dashboard updates) | `ArrowIpc` — streaming format, may include schema changes |
| Native↔native communication in same process | `Arrow` — zero-copy, just the bytes |
| Export to external tools (pyarrow, pandas) | `ArrowIpc` — standard streaming format they understand |
| Capability response with data payload | `Arrow` — single batch, fits in `InvokeBody::Raw` |

**What Arrow is for (both variants):**
- Large structured data payloads (analytics, tables, sync logs).
- High-throughput Rust↔native communication (same process, shared memory).
- Data delivered to the WebView as binary `ArrayBuffer` for direct reading.

**What Arrow is NOT for:**
- UI DOM operations — those use columnar v1 (wasm-loop, no-std,
  TypedArray-friendly DOM operation batches from `foundation_wasm_ui`).
- Small control messages — those are JSON over Tauri command IPC.
- HTML fragments — those are text/html responses.

**Terminology distinction:**
- **Columnar v1** — wasm-loop, no-std, typed-array-friendly layout used for
  efficient DOM operation batches. Owned by `foundation_wasm_ui`.
- **Arrow** — raw RecordBatch binary, owned by `foundation_arrow`. Single
  batch, no streaming metadata. The default for data payloads.
- **ArrowIpc** — Arrow IPC streaming format, owned by `foundation_arrow`.
  Multi-batch, schema-capable, interoperable with the Apache Arrow ecosystem.

**Zero-copy directions:**

| Direction | Mechanism | Zero-copy? |
|---|---|---|
| Rust ↔ Native platform (same process) | `Arrow`: cast RecordBatch to bytes, no serialization. | Yes |
| Rust → WebView (cross-process on mobile) | `Arrow` or `ArrowIpc` over custom protocol binary response. Single copy (Rust buffer → fetch buffer). | Copy-minimized |
| Native shell + WASM (same process) | WASM linear memory directly readable by native code. | Yes |
| Multi-batch stream (remote server) | `ArrowIpc` over SSE/WebSocket. Standard framing. | Network cost plus one decode. |

This distinction is why `ProtocolHint` in `RouteDecision` separates
`Columnar`, `Arrow`, and `ArrowIpc` — they serve different purposes and are
routed through different encoders.
through different encoders.

---

## How it spans both crates

**Web side (`foundation_wasm_ui`).** The existing JS runtime already has a
dispatch loop, protocol framing, event routing, and DOM operation application.
The session extends this — it becomes the central bus for navigation intents,
bridge messages, and rendering updates, making existing capabilities (DomOps,
morphing, signals, templates, event runtime) available through a single
coordination surface.

**Native side (`foundation_platform`).** The platform crate provides route
policy execution, capability registry, custom protocol serving, cache
integration, native stack navigation, and the bridge to Swift/Kotlin. It
plugs into Tauri's primitives (`AppManager`, `StateManager`, `Listeners`,
events) but wraps them in the session coordination model that Tauri does not
provide.

**Both sides are peers.** The web-side session and native-side session are the
same coordination graph. A user action on the web side flows into the session;
the session decides (local WASM, cached replay, IPC, remote fetch, native
stack push); the result flows back through the rendering lane.

### What the shell owns

Regardless of deployment model (shell + compiled app as static lib, shell +
WASM, or shell that loads a remote app), the shell owns:

- Communication between WebView and native APIs via Tauri's primitives
- Communication between the app and remote servers via the resource lane
- Communication between IPC peers via the native shell IPC lane
- Protocol selection per payload (the Rust side decides; the shell carries)

### Tauri integration: bidirectional hooks

**Shell → Tauri (what the platform wraps):**

| Tauri primitive | How the platform hooks in |
|---|---|
| `AppManager` / `AppHandle` | The shell holds an `AppHandle<R>`. The session backbone wraps it — route resolution, capability dispatch, and cache lookups flow through the session, not through raw `AppHandle` calls. |
| `StateManager` | The platform's `StateManager` wraps Tauri's — platform-managed types (session state, capability registry, route tables) are inserted via `manage()`. User-managed types are also inserted. Both coexist in the same `TypeIdMap`. |
| `tauri::command` IPC | WASM-side capability calls are routed through the session backbone, which invokes Tauri commands. The session wraps `invoke()` so WASM never sees raw Tauri internals. |
| Custom protocol (`tauri::UriSchemeProtocol`) | The shell registers the `ewe://` protocol handler. Bundled assets (JS, WASM, CSS) are served through it. The session routes resource requests through the protocol. |
| Event system (`emit`/`listen`) | The session wraps Tauri events. Lifecycle events (online/offline, app background/foreground) are translated to session events. User code listens on the session, not on raw Tauri events. |
| `WebviewManager` / `WebviewWindow` | The shell creates and manages the WebView through Tauri's API. The session holds a `Webview<R>` handle and injects `foundation-wasm-ui.js` as an initialization script. |
| Plugin system | Native capabilities that Tauri plugins provide (filesystem, clipboard, notifications) are registered in the platform's capability registry and called through the session backbone. |

**Tauri → Shell (what Tauri drives and the shell intercepts):**

| Tauri callback / event | How the shell hooks in |
|---|---|
| `setup()` | Shell initializes: creates platform session, loads WASM module, instantiates runtime, calls user's `main()`. |
| `on_page_load()` | Shell injects session identity into the WebView context. The JS runtime knows which session it belongs to. |
| `on_window_event()` | Shell translates window events into session lifecycle events: focus/blur → session active/inactive, resize → layout invalidation. |
| `on_navigation()` | Shell intercepts WebView navigations. Routes the URL through the session's route handler chain instead of letting the browser handle it directly. |
| `RunEvent::Exit` / `RunEvent::ExitRequested` | Shell tears down: signals user code, persists state, flushes cache, closes connections. |
| Mobile lifecycle (`AppDelegate` / `Activity` callbacks) | Shell translates iOS/Android lifecycle events (didEnterBackground, willEnterForeground, onPause, onResume) into session events. Connectivity changes, memory pressure, and app suspension all flow through the session. |

**Boundary principle:** The platform never bypasses Tauri. It wraps Tauri's
primitives in the session coordination model. User code talks to the session.
The session talks to Tauri. Tauri's primitives remain the authoritative source
for window/webview/plugin/event state — the session just coordinates them.

### Native UI integration tiers

The JS DOM applicator remains the primary rendering surface for web content.
Native UI (Swift/Kotlin) is additive, not a replacement. The platform also
supports a native shell that embeds a WASM runtime for zero-copy Arrow
communication between Rust and native code in the same process.

Three tiers of native integration:

**Tier 1 — Use Tauri's built-in capabilities** where they meet our needs:
window management, native menus, platform plugins, filesystem APIs. These are
wrapped in the platform's capability registry and called through the session
backbone (see [decision 18](07-native-capability-contract.md)).

**Tier 2 — Build native Swift/Kotlin bridges** where Tauri falls short:
native navigation controllers, tab bars, sheets, biometric flows,
platform-specific media APIs, background services, hardware integration. A
`NativeBridge` capability wraps platform-specific code behind the same
`Capability` trait:

```rust
#[platform_capability]
struct BiometricAuth {
    #[native(ios = "BiometricAuthIOS", android = "BiometricAuthAndroid")]
    native_impl: NativeBinding,
}
// On iOS: the shell calls into a Swift class via UniFFI or raw FFI.
// On Android: the shell calls into a Kotlin class via JNI.
// On desktop: falls back to a Tauri plugin or pure-Rust implementation.
// The capability contract is the same on all platforms.
```

**Tier 3 — Sync between web and native:** the session backbone lets Rust/JS
content drive native UI state (e.g., update a native tab badge from a server
stream) and lets native UI events feed back into the web runtime (e.g., a
native back gesture routing through the session's navigation policy). This is
Hotwire Native's ability to sync into native navigation stacks, tab bars, and
platform chrome — a net benefit that `foundation_platform` embraces.

Nothing is wrong with writing native code when it adds value. The platform
provides the seam (the session backbone); native code hooks into it on one
side, web code hooks into it on the other.

**Native shell + WASM (same process):**

A thin, pre-compiled static library that embeds a WASM runtime (wasmtime,
wasm3, or wasmi) serves as an intermediary between native platform code and
user-provided WASM modules:

```
Native app (Swift/Kotlin/C++)
  │ same process, shared memory
  ▼
Native Shell (compiled static library, shipped once)
  ├── Embedded WASM runtime
  └── Arrow buffers in WASM linear memory
  │ loads and executes
  ▼
User WASM module (may also run in the WebView, or only on the backend)
```

**Zero-copy Arrow across WASM ↔ native:** The WASM module writes Arrow
`RecordBatch`es into its linear memory. The native shell reads those bytes
directly (same process, same address space). Arrow's columnar layout IS the
in-memory format — no serialization, no copy.

**Two tiers of native deployment, both with zero-copy:**

| Tier | What the user ships | Zero-copy? | App-store update? |
|---|---|---|---|
| Static library | Native `.a`/`.so` of Rust logic | Yes (same process) | Full review for logic changes |
| Shell + WASM | Native shell (once) + WASM module | Yes (WASM linear memory) | WASM hot-swap, no review |

**User WASM runs anywhere:** The WebView embeds `foundation-wasm-ui`'s own
WASM-based runtime. The user's application WASM can run in the native shell,
in an IPC process, on a local server, or on a remote server. Wherever it runs,
it streams responses to the runtime in the WebView. Users CAN also deploy
their WASM to the frontend for local execution — it's an architectural
choice, not a platform constraint.

Full deployment surface details (bundled WASM, native static lib, shell +
WASM, remote server, cached replay) are in [decision
04](04-deployment-surfaces.md).

---

## What Tauri already provides vs what we build

Verified against source (`manager/mod.rs`, `manager/webview.rs`, `webview/mod.rs`,
`webview/webview_window.rs`, `window/mod.rs`, `state.rs`, `app.rs`, `ios.rs`,
`tauri-runtime/src/lib.rs`, `wry/src/lib.rs`):

**Tauri provides:**
- `AppManager` (`manager/mod.rs:187`) — container: owns `WebviewManager`,
  `PluginStore`, `StateManager`, `Listeners`, `ResourceTable`, `Config`.
- `WebviewManager<R>` (`manager/webview.rs:70`) — `pub webviews: Mutex<HashMap<String, Webview<R>>>`. Tracks all WebViews by label.
- `WebviewWindow<R>` (`webview/webview_window.rs:1441`) — window + single WebView. The primary creation API. `build()`, `on_navigation()`, `navigate()`, `eval()`.
- `Window::add_child()` (`window/mod.rs:1129`) — child WebView within a desktop window. **Gated:** `#[cfg(all(desktop, feature = "unstable"))]`. Not on mobile.
- `StateManager` — type-indexed dependency injection (`TypeId → Pin<Box<dyn Any>>`). No lifecycle, no route scoping, no message routing.
- `Listeners` — typed pub-sub with target scoping. Raw event bus.
- `on_navigation(Url) -> bool` — per-WebView navigation interception. Returns false to block. The session backbone transforms this into the handler chain.
- `UriSchemeProtocol` — custom protocol handler. Foundation for `ewe://`.
- `PluginBuilder` / `mobile::PluginBuilder` — plugin system. Foundation for the capability registry.
- `tauri-runtime` escape hatches — `run_on_main_thread()`, `run_on_android_context()`, `SceneRequested` event. Foundation for native view instantiation.

**What Tauri does NOT provide (we build these):**
- Route/navigation policy — [decision 02](02-route-policy-model.md)
- Session lifecycle management — this document
- Capability registry — [decision 18](07-native-capability-contract.md)
- Native view registration and instantiation — [decision 02](02-route-policy-model.md) (Tauri only does WebViews; we build native views via thread hooks)
- Bridge component routing
- Cache/offline policy — [decision 05](05-offline-and-sync.md)
- Page/screen identity tracking
- `ewe://` custom protocol adapter — this document
- WebView stack manager — [decision 21](10-multi-webview-stack.md)
- WebView profiles — [decision 14](06-webview-profiles.md)
- Background sync — [decision 05](05-offline-and-sync.md)

---

## Design is our own

Hotwire Native's Session/Navigator split is a reference point, not a
blueprint. Our design is a Rust-and-WASM-native backbone, not a Swift/Kotlin
shell around a Turbo web app. The web side runs in `foundation_wasm_ui`. The
native side runs in `foundation_platform`. The session is the seam between
them.
