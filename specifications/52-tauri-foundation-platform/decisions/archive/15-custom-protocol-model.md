# 15 — Custom protocol model: `ewe://` transport adapter

**Date:** 2026-07-04
**Status:** Resolved

### Decision

`foundation_wasm_ui` defines the wire protocols. `foundation_platform` provides
the Tauri transport adapter that carries them. The platform registers `ewe://`
as a custom URI scheme with Tauri's `UriSchemeProtocol`. The handler parses
the URI, routes through the session backbone, encodes the response in the
selected protocol, and returns an HTTP response with appropriate headers.

Sub-schemes differentiate transport purpose: `ewe://`, `ewe+ipc://`,
`ewe+ws://`, `ewe+http://`.

### Why `ewe://`

`platform://` is too generic — it doesn't identify the project. `ewe://` is
specific to the ewe platform. Sub-schemes follow the `scheme+transport`
convention for explicit transport selection.

### How it integrates with Tauri

Tauri's `UriSchemeProtocol` signature (verified against source):

```rust
// tauri/crates/tauri/src/manager/webview.rs, line 63-68
pub struct UriSchemeProtocol<R: Runtime> {
    pub handler: Box<
        dyn Fn(UriSchemeContext<'_, R>, http::Request<Vec<u8>>, UriSchemeResponder)
            + Send + Sync
    >,
}
```

The handler receives the full HTTP request (method, headers, body, URI) and
responds asynchronously via `UriSchemeResponder::respond(response)`. This maps
cleanly to the platform's protocol pipeline.

The platform registers the `ewe://` handler in Tauri's `setup()` hook:

```rust
// In foundation_platform's Tauri setup
app.webview_on_all(
    |webview_builder| {
        webview_builder.register_uri_scheme_protocol(
            "ewe",
            platform_protocol_handler,
        );
    }
);
```

### Protocol routing: URI → session → encoder → response

```
Browser/WebView requests:  ewe://localhost/app/items?proto=arrow-ipc
                                    │
                                    ▼
Tauri intercepts ──→ UriSchemeProtocol handler fires
                                    │
                                    ▼
Platform parses URI:
  ├── scheme:   ewe
  ├── transport: default (IPC via Tauri command lane)
  ├── route:    /app/items
  ├── protocol hint: arrow-ipc (from query param)
  └── method:   GET
                                    │
                                    ▼
Session backbone resolves route:
  ├── Route handler returns RouteDecision
  ├── Cache policy checked (serve from cache? fetch fresh?)
  └── Backend queried (local WASM, IPC to native shell, remote server)
                                    │
                                    ▼
Platform encodes response:
  ├── Protocol selected (from RouteDecision or query hint)
  ├── Content encoded (Arrow IPC, columnar, JSON, HTML)
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
| `proto` param | `?proto=arrow-ipc` | Protocol preference hint (columnar, arrow-ipc, json, html). The backend can override. |
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

### Protocol selection per response

The response protocol is determined by, in priority order:

1. **Route handler's `RouteDecision.protocol`** — the user explicitly chooses.
2. **`proto` query parameter** — the WebView requests a specific format.
3. **Backend's native format** — whatever the backend (WASM, native, server)
   produces. If the backend emits Arrow IPC, that's what gets delivered.
4. **Platform default** — columnar v1 for DomOps, Arrow IPC for data.

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
| Arrow IPC | `application/primal-arrow` or `application/vnd.apache.arrow.stream` |
| JSON | `application/primal-json` |
| HTML fragment | `text/html; charset=utf-8` |
| Custom binary | `application/primal-binary` |

### Binary streaming

For large payloads or live streams, the custom protocol handler supports:

1. **Chunked transfer encoding** — the `UriSchemeResponder` accepts a streaming
   body. The platform writes chunks as they arrive from the backend. The WebView
   receives them progressively.

2. **WebSocket upgrade** — for `ewe+ws://`, the custom protocol handler detects
   the WebSocket upgrade request and hands the connection to the platform's WS
   handler. The `Broadcaster` / `FrameTransport` model from `foundation_wasm_ui`
   plugs in directly here — the WS connection is a `FrameTransport`.

3. **ArrayBuffer delivery** — binary responses (Arrow IPC, columnar) are
   delivered as `ArrayBuffer` in the WebView. Arrow JS reads them directly
   without parsing. Single copy (Rust buffer → WebView buffer).

### Security

- **`ewe://` is a trusted scheme.** Only the platform registers it. Remote
  content cannot register custom `ewe://` handlers. The `untrustedRemote`
  WebView profile blocks all `ewe://` access.
- **Route-scoped access.** A capability request on `ewe+ipc://app/remote/items`
  is scoped to the `/app/remote/items` route. It cannot access resources
  belonging to `/app/local/settings`.
- **No filesystem exposure.** The handler does not map URIs to filesystem paths.
  It maps URIs to session routes. The session resolves the route; the backend
  provides the content. No `../../../etc/passwd` attack surface.
- **CSP integration.** The `ewe://` scheme is added to the appropriate CSP
  directives (`connect-src`, `script-src`) per WebView profile. `App` profile
  allows `ewe://*`; `trustedRemote` allows `ewe://` for allowed origins only;
  `untrustedRemote` blocks all `ewe://`.
- **Auth token isolation.** When the handler fetches from a remote backend, it
  attaches auth tokens from Tauri's secure storage. Tokens never enter the
  WebView's JavaScript context.

### How `foundation_wasm_ui` protocols plug in

The platform's transport adapter wraps `foundation_wasm_ui`'s protocol encoders:

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
        Protocol::ArrowIpc => (
            foundation_wasm_ui::encode_arrow_ipc(&ops),
            "application/primal-arrow",
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

The platform imports the encoders from `foundation_wasm_ui`. The transport
is Tauri. The encoders are pure `Vec<u8>` producers — they don't know or care
how the bytes are delivered.

### What the platform does NOT provide

- **A new protocol format.** The wire formats are `foundation_wasm_ui`'s domain.
- **Server-side protocol handling.** The remote server implements the same
  protocols. The platform transports them; the server produces them.
- **Protocol negotiation beyond the hint.** The platform selects the protocol
  from the decision chain. Complex content negotiation (Accept headers, etc.)
  can be added later if needed.
