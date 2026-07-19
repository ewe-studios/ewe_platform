---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F25-ipc-registry"
this_file: "specifications/52-tauri-foundation-platform/features/F25-ipc-registry/feature.md"

status: pending
priority: critical
created: 2026-07-20

depends_on:
  - "F02-route-handler"
  - "F23-wasm-capabilities"
  - "F24-script-injector"

tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0%
---
# F25 — IPC Registry: central IPC mechanism with Tauri bridge

## Problem

The platform currently has three separate communication channels between
frontend and backend, each with different contracts, different invocation
patterns, and different error handling:

| Channel | Invocation | Type Safety | Return Values | Push | Streaming |
|---------|-----------|-------------|---------------|------|-----------|
| Tauri `invoke()` | `__TAURI_INTERNALS__.invoke(cmd, args)` | JSON (serde) | Yes | No | No |
| Tauri Events | `emit()` / `listen()` | JSON only | No | Yes | No |
| Route Responder | `ewe://` navigation | HTML/text | Yes (response body) | No | No |
| Capabilities (F05) | `CapabilityRequest` through session | `serde_json::Value` | Yes | No | No |

This fragmentation means:

- **App developers must choose the right channel** for each use case. Sometimes
  a single logical operation (e.g. "sync data") needs invoke for the request,
  events for progress, and a route responder for the final result.
- **No unified invocation API in WASM.** A WASM app targeting the platform
  must know whether to call `invoke()`, `listen()`, `fetch()`, or a
  capability-specific bridge. This breaks portability.
- **Route handlers can't invoke IPCs.** If a handler needs to call a backend
  service (e.g. "query database" → "render HTML"), it must duplicate the logic
  or reach into Tauri internals.
- **No standard error contract.** Each channel has its own error format.
- **The Tauri command namespace is flat.** Every `#[tauri::command]` is global.
  As the platform grows, command name collisions become likely.

## Solution

A **central IPC Registry** in `foundation_platform` that unifies all backend
communication behind a single trait, a single namespace, and a single invocation
API. IPCs are like capabilities (F23) but designed for general backend
communication — they can be invoked from the frontend via a standard JS API,
from route handlers programmatically, and can optionally serve as route handlers
themselves.

### Core concept

```
┌──────────────────────────────────────────────────────────────┐
│                      Platform Commands                        │
│                                                               │
│  JS: invokeIpc(name, action, payload)                         │
│       → __TAURI_INTERNALS__.invoke("__ewe_ipc", ...)          │
│                                                               │
│  JS: invokeCapability(name, action, payload)     (F23)        │
│       → __TAURI_INTERNALS__.invoke("__ewe_capabilities", ...) │
│                                                               │
│  JS: invokeIpcStream(name, action, payload)      (F26)        │
│       → __TAURI_INTERNALS__.invoke("__ewe_ipc_stream", ...)   │
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│                      IPC Registry  (F25)                       │
│                                                               │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                    │
│  │  Query   │  │  Emit    │  │  Page    │                    │
│  │ (invoke) │  │ (events) │  │(route    │                    │
│  │          │  │          │  │ handler) │                    │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘                    │
│       │             │             │                            │
│       └─────────────┴─────────────┘                            │
│                     │                                          │
│               Ipc trait                                        │
│               └─ name(): &str                                  │
│               └─ kind(): IpcKind (Query | Emit | Page)         │
│               └─ invoke(IpcRequest) → Result<IpcResponse, ...> │
│                                                               │
│  Registered on PlatformSession.                                │
│  Invoked via:                                                  │
│    1. JS: invokeIpc(name, action, payload) → Promise<Response> │
│    2. Rust: session.get_ipc(name)?.invoke(request)             │
│    3. HTTP: ewe://localhost/api/{name} (if route-registered)   │
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│                Streaming  (F26 — separate contract)            │
│                                                               │
│  StreamingIpc extends Ipc:                                     │
│  └─ stream() → IpcStream        (server → client chunks)       │
│  └─ accept_stream() → Response  (client → server chunks)       │
│                                                               │
│  PlatformStreamRegistry — separate from IpcRegistry.           │
│  Tauri commands: __ewe_ipc_stream, __ewe_ipc_stream_accept.   │
└──────────────────────────────────────────────────────────────┘
```

### `Ipc` trait — `foundation_platform::ipc`

```rust
/// An IPC handler registered in the platform's IPC registry.
///
/// IPCs are the primary mechanism for frontend ↔ backend communication.
/// They unify Tauri commands, events, route responders, and capabilities
/// behind a single contract.
pub trait Ipc: Send + Sync + 'static {
    /// Unique name. Used as the lookup key and the JS-side invocation target.
    fn name(&self) -> &str;

    /// The IPC kind determines the default transport behavior.
    fn kind(&self) -> IpcKind;

    /// Execute the IPC and return a response.
    ///
    /// This is the single entry point. The platform handles serialization,
    /// transport, and delivery. The handler only deals with domain logic.
    fn invoke(
        &self,
        session: &PlatformSession,
        request: IpcRequest,
    ) -> Result<IpcResponse, IpcError>;

    /// Optional: handle this IPC as an HTTP route.
    ///
    /// When `Some`, the IPC can be registered as a route handler.
    /// GET /api/ipc/{name} → calls `invoke()` and renders the response as HTML.
    fn as_route_handler(&self) -> Option<&dyn RouteResponder> { None }
}

pub enum IpcKind {
    /// Request → Response. Like Tauri invoke(). Single reply.
    Query,
    /// Fire-and-forget. Like Tauri events. Sends data to the frontend.
    /// `invoke()` returns an empty acknowledgment (payload empty, no error).
    Emit,
    /// Request → Response, rendered as a page. Like RouteResponder.
    /// `invoke()` returns the HTML body. Can also be registered as a route handler.
    Page,
}

/// Streaming is NOT part of `IpcKind` — the `Ipc` trait is fundamentally
/// request/response. Streaming (server→client chunks, client→server uploads,
/// backpressure) requires a different contract: `StreamingIpc` (F26) which
/// extends `Ipc` with `fn stream()` and `fn accept_stream()`.
///
/// ```ignore
/// // F26 — different trait, different contract:
/// pub trait StreamingIpc: Ipc {
///     /// Server → client: emit a stream of chunks.
///     fn stream(&self, session: &PlatformSession, request: IpcRequest)
///         -> Result<IpcStream, IpcError>;
///     /// Client → server: accept a stream of chunks from the frontend.
///     fn accept_stream(&self, session: &PlatformSession, request: IpcRequest,
///         input: IpcStreamReceiver) -> Result<IpcResponse, IpcError>;
/// }
/// ```

pub struct IpcRequest {
    /// The IPC name (echoed from invocation, for routing verification).
    pub ipc: String,
    /// The action or sub-command within this IPC.
    pub action: String,
    /// Serialized payload. Encoding per `content_type`.
    pub payload: Vec<u8>,
    pub content_type: IpcContentType,
    /// For Emit IPCs: the target webview label (if scoped to a specific webview).
    /// None = broadcast to all listeners.
    pub target: Option<String>,
}

pub struct IpcResponse {
    pub payload: Vec<u8>,
    pub content_type: IpcContentType,
}

pub enum IpcContentType {
    Json,
    Arrow,
    Binary,
}

pub enum IpcError {
    UnknownIpc(String),
    InvalidPayload(String),
    ExecutionFailed(String),
    PermissionDenied(String),
    /// For Query IPCs that want to signal a domain error (not a platform error).
    DomainError { code: String, message: String },
}
```

### Registry — `foundation_platform::ipc::IpcRegistry`

```rust
pub struct IpcRegistry {
    ipcs: HashMap<String, Box<dyn Ipc>>,
}

impl IpcRegistry {
    pub fn new() -> Self { ... }
    pub fn register<I: Ipc>(&mut self, ipc: I) { ... }
    pub fn get(&self, name: &str) -> Option<&dyn Ipc> { ... }
    pub fn invoke(&self, session: &PlatformSession, request: IpcRequest) -> Result<IpcResponse, IpcError> { ... }
    pub fn names(&self) -> impl Iterator<Item = &str> { ... }
}
```

### Tauri command bridge

The platform automatically registers `__ewe_ipc` as the single Tauri command
for all IPC invocations from JS. Capability invocations use a separate command
`__ewe_capabilities` (F23) — capabilities have their own registry, security
model, and contract:

```
JS invokeIpc("system", "get_info", payload)
  → window.__TAURI_INTERNALS__.invoke("__ewe_ipc", { ipc, action, payload })
    → Rust: #[tauri::command] fn __ewe_ipc(state, ipc, action, payload)
      → IpcRegistry::invoke(request)
        → Ipc::invoke(session, request)
      → IpcResponse
    → Result<Vec<u8>, String> back to JS

JS invokeCapability("clipboard", "read", payload)    // F23 — separate path
  → window.__TAURI_INTERNALS__.invoke("__ewe_capabilities", { capability, action, payload })
    → CapabilityRegistry → 5-layer defense → CapabilityResponse
```

```rust
#[tauri::command]
fn __ewe_ipc(
    app_handle: tauri::AppHandle,
    ipc: String,
    action: String,
    payload: Vec<u8>,
    content_type: String,
) -> Result<Vec<u8>, String> {
    let session = app_handle.state::<PlatformSession>();
    let request = IpcRequest {
        ipc, action, payload,
        content_type: content_type.parse().unwrap_or(IpcContentType::Json),
        target: None,
    };
    session.ipc_registry().invoke(&session, request)
        .map(|r| r.payload)
        .map_err(|e| e.to_string())
}
```

This means the Tauri command namespace has exactly TWO platform commands:

| Command | Purpose | Registry | Security |
|---------|---------|----------|----------|
| `__ewe_ipc` | General IPC (query, emit, page) | `IpcRegistry` | Namespace lookup |
| `__ewe_capabilities` (F23) | Security-gated native capabilities | `CapabilityRegistry` | 5-layer defense |

All user-facing communication is namespaced behind these two commands,
eliminating collision risk.

### JS API — `invoke_ipc`

```typescript
// Injected via ScriptInjector (F24) as "foundation_ipc_bridge"
//
// This is the ONLY function WASM apps need to call for any backend communication.

async function invokeIpc(name: string, action: string, payload?: any): Promise<any> {
    const serialized = JSON.stringify(payload ?? {});
    const bytes = new TextEncoder().encode(serialized);

    // Under the hood: calls the single tauri command
    const result = await window.__TAURI_INTERNALS__.invoke('__ewe_ipc', {
        ipc: name,
        action: action,
        payload: Array.from(bytes),
        content_type: 'application/json',
    });

    return JSON.parse(new TextDecoder().decode(new Uint8Array(result)));
}

// Usage:
const config = await invokeIpc('system', 'get_config', { key: 'theme' });
// → { theme: 'dark' }

await invokeIpc('events', 'emit', { event: 'user:login', data: { id: 42 } });
// → void (Emit kind, no response)
```

### Programmatic invocation from route handlers

```rust
fn dashboard_handler(session: &PlatformSession, intent: &NavigationIntent) -> RouteResult {
    // Get an IPC from the registry and invoke it directly.
    let db = session.get_ipc("database").unwrap();
    let result = db.invoke(session, IpcRequest {
        ipc: "database".into(),
        action: "query".into(),
        payload: serde_json::to_vec(&json!({"sql": "SELECT * FROM users"})).unwrap(),
        content_type: IpcContentType::Json,
        target: None,
    }).unwrap();

    let users: Vec<User> = serde_json::from_slice(&result.payload).unwrap();
    RouteResult::Html(render_user_table(&users))
}
```

### IPC as Route Handler

An IPC with `IpcKind::Page` can also serve as a route handler:

```rust
struct SystemInfoIpc;

impl Ipc for SystemInfoIpc {
    fn name(&self) -> &str { "system" }
    fn kind(&self) -> IpcKind { IpcKind::Page }

    fn invoke(&self, _session: &PlatformSession, request: IpcRequest) -> Result<IpcResponse, IpcError> {
        let info = get_system_info();
        let html = render_system_page(&info);
        Ok(IpcResponse {
            payload: html.into_bytes(),
            content_type: IpcContentType::Binary, // HTML
        })
    }

    fn as_route_handler(&self) -> Option<&dyn RouteResponder> { Some(self) }
}

// Register as BOTH an IPC and a route:
session.register_ipc(SystemInfoIpc);
session.register_route_with("/api/system/*", ipc_shell(), &SystemInfoIpc);
```

Now `invokeIpc('system', 'get_info')` and navigating to `ewe://localhost/api/system/`
both work, using the same handler.

### Core in `foundation_wasm`, platform in `foundation_platform`

Following the F23 pattern, the core IPC contract lives in `foundation_wasm`:

```
foundation_wasm::ipc
  └─ IpcRequest, IpcResponse, IpcError, IpcContentType  (shared types)

foundation_platform::ipc
  └─ Ipc trait, IpcRegistry, IpcKind, __ewe_ipc command  (platform specifics)
```

This means WASM apps can import `IpcRequest`/`IpcResponse` types and write
IPC-agnostic code that works on any platform. Only `foundation_platform`
knows about Tauri.

The `invokeIpc` JS function is provided by `foundation_wasm` runtime. On
Tauri, it delegates to `__TAURI_INTERNALS__.invoke('__ewe_ipc', ...)`. On
browser (non-Tauri), it uses a direct WASM bridge. The WASM app code never
knows the difference.

### withGlobalTauri

Set `withGlobalTauri: true` in `tauri.conf.json` so the `window.__TAURI__`
global is always available. This gives WASM apps direct access to Tauri APIs
when they need them, while `invokeIpc` remains the recommended path.

## Requirements

### 1. `Ipc` trait — `foundation_platform`
- File: `backends/foundation_platform/src/ipc.rs` (NEW)
- Trait: `Ipc` with `name()`, `kind()`, `invoke()`, `as_route_handler()`
- Types: `IpcRequest`, `IpcResponse`, `IpcError`, `IpcKind`, `IpcContentType`
- `IpcKind`: `Query`, `Emit`, `Page`
- `IpcError::DomainError` variant for domain-level errors
- Streaming is explicitly NOT an `IpcKind` — the `Ipc` trait is request/response. Streaming (bidirectional chunks, backpressure) requires `StreamingIpc` (F26), which extends `Ipc` with its own `stream()` and `accept_stream()` contract.

### 2. Shared types — `foundation_wasm`
- File: `backends/foundation_wasm/src/ipc.rs` (NEW)
- `IpcRequest`, `IpcResponse`, `IpcError`, `IpcContentType`
- `Serialize` + `DeserializeOwned` + `ToArrow`/`FromArrow`
- Re-exported from `foundation_wasm::ipc`

### 3. `IpcRegistry`
- `HashMap<String, Box<dyn Ipc>>` storage
- `register()`, `get()`, `invoke()`, `names()` methods
- Thread-safe: `RwLock<HashMap<...>>`
- Integrated into `PlatformSession`

### 4. Tauri command bridge
- `#[tauri::command] fn __ewe_ipc(...)` registered automatically by PlatformBuilder
- Routes IPC invocations from JS to the `IpcRegistry`
- Capability invocations use `__ewe_capabilities` (F23) — separate command, separate registry
- No user-facing command registration needed
- Two-command namespace: `__ewe_ipc` (general IPC) + `__ewe_capabilities` (security-gated capabilities)

### 5. JS API
- `invokeIpc(name, action, payload?)` function
- Injected via ScriptInjector (F24) as `foundation_ipc_bridge`
- Returns `Promise<any>`
- Handles serialization/deserialization transparently
- Works on Tauri (via `__TAURI_INTERNALS__.invoke`) and browser (direct bridge)

### 6. Session API
- `PlatformSession::register_ipc(ipc)` — register an IPC handler
- `PlatformSession::get_ipc(name) -> Option<&dyn Ipc>` — get for programmatic use
- `PlatformSession::ipc_registry() -> &IpcRegistry` — access the registry
- Route handlers can call `session.get_ipc(name)?.invoke(...)`

### 7. Dual registration (IPC + Route)
- IPC with `IpcKind::Page` can implement `RouteResponder`
- Same handler serves both programmatic invocation and HTTP navigation
- Route registration uses `ipc_shell()` (not `webview_app()` — IPC routes are
  backend endpoints, not WASM app webviews)
- `session.register_route_with("/api/system/*", ipc_shell(), &handler)`

### 8. Android example — 3 IPC types
- **Query IPC** (`system`): invoke → get system info back as JSON
- **Emit IPC** (`events`): push-based updates. Rust emits ticker, JS listens
- **Page IPC** (`dashboard`): registered as both IPC and route handler

### 9. No forced routes
- Users register IPCs on whatever routes they want
- No `/api/ipc/*` convention enforced — it's just a pattern, not a requirement
- IPCs work via JS `invokeIpc()` even without a route registration

### 10. Backward compatibility
- Existing `#[tauri::command]` functions continue to work
- Existing F05 capabilities continue to work
- Migration path: wrap commands/capabilities in `Ipc` trait impls

## Verification

```bash
# Core types compile on wasm32
cargo build -p foundation_wasm --target wasm32-unknown-unknown

# Platform IPC registry
cargo test -p foundation_platform -- ipc

# Tauri command bridge
cargo test -p foundation_platform -- __ewe_ipc

# Android: invokeIpc from JS
# Chrome DevTools → Console:
# > await invokeIpc('system', 'get_info')
# // { os: 'android', arch: 'x86_64', uptime: 12345 }

# Android: IPC as route handler
# Navigate to ewe://localhost/api/system/
# → Rendered system info page served by SystemInfoIpc

# Android: route handler invokes IPC programmatically
# Navigate to ewe://localhost/app/
# → Dashboard that calls session.get_ipc("database")?.invoke(...)
```

## Files

| File | Action |
|------|--------|
| `backends/foundation_wasm/src/ipc.rs` | **NEW** — Shared `IpcRequest`, `IpcResponse`, `IpcError`, `IpcContentType` |
| `backends/foundation_wasm/src/lib.rs` | Add `pub mod ipc;` |
| `backends/foundation_platform/src/ipc.rs` | **NEW** — `Ipc` trait, `IpcRegistry`, `IpcKind`, `__ewe_ipc` command |
| `backends/foundation_platform/src/lib.rs` | Add `pub mod ipc;` |
| `backends/foundation_platform/src/session.rs` | Add `ipc_registry`, `register_ipc()`, `get_ipc()` |
| `backends/foundation_platform/src/builder.rs` | Auto-register `__ewe_ipc` command, `IpcRegistry` |
| `backends/foundation_wasm_ui/runtimes/ipc-bridge.js` | **NEW** — `invokeIpc` JS function |
| `backends/foundation_platform/src/injector.rs` | Register `ipc-bridge.js` via ScriptInjector (F24) |
| `examples/platform_android/src-tauri/src/lib.rs` | Register 3 IPC types, wire routes |
| `examples/platform_android/src-tauri/tauri.conf.json` | Set `withGlobalTauri: true` |
