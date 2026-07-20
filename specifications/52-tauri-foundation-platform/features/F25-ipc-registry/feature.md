---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F25-ipc-registry"
this_file: "specifications/52-tauri-foundation-platform/features/F25-ipc-registry/feature.md"

status: in-progress
priority: critical
created: 2026-07-20

depends_on:
  - "F02-route-handler"
  - "F23-wasm-capabilities"
  - "F24-script-injector"

tasks:
  completed: 6
  uncompleted: 4
  total: 10
  completion_percentage: 60%

implementation_notes: |
  Trait split: `Ipc` in foundation_wasm (base, no 'static, wasm32+native),
  `PlatformIpc` in foundation_platform (extends with invoke_with_session +
  as_route_handler). 'static only on registry storage. IpcRequest<T>/
  IpcResponse<T> are generic with default T=Vec<u8>. Same WirePayload
  approach as F23. Streaming is NOT an IpcKind — separate StreamingIpc
  (F26) trait with stream() and accept_stream().
---
# F25 — IPC Registry: central IPC mechanism with Tauri bridge

## Problem

Multiple communication channels (Tauri invoke, events, route responders,
capabilities) with different contracts. No unified invocation API.
Route handlers can't invoke IPCs. No standard error contract.

## Solution

Trait split across crates, same pattern as F23:

```
foundation_wasm (no_std, wasm32 + native)
├── IpcKind                 — Query | Emit | Page
├── Ipc trait               — name(), kind(), invoke(&IpcRequest<Vec<u8>>)
├── IpcRequest<T>           — T = Vec<u8> default, into_wire() / into_typed()
├── IpcResponse<T>          — same
├── IpcError                — UnknownIpc, InvalidPayload, ExecutionFailed, ...

foundation_platform (native)
├── PlatformIpc: Ipc        — adds invoke_with_session(), as_route_handler()
├── IpcRegistry             — stores Box<dyn PlatformIpc + 'static>
├── __ewe_ipc               — Tauri command bridge
└── ipc_route()             — convenience RouteDecision constructor
```

### `Ipc` trait — `foundation_wasm`

No `'static` on the trait. `Send + Sync` on native, no bounds on wasm32.

```rust
#[cfg(not(target_family = "wasm"))]
pub trait Ipc: Send + Sync {
    fn name(&self) -> &str;
    fn kind(&self) -> IpcKind;
    fn invoke(&self, request: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError>;
}
```

### `IpcKind` — `foundation_wasm`

```rust
pub enum IpcKind { Query, Emit, Page }
```

Streaming is NOT a variant. The `Ipc` trait is request/response. Streaming
needs bidirectional chunks + backpressure — `StreamingIpc` (F26).

### Generic `IpcRequest<T>` / `IpcResponse<T>` — `foundation_wasm`

Same `WirePayload` approach as F23. Default `T = Vec<u8>` for wire form.
`into_wire()` / `into_typed()` on both. `IpcContentType` is a type alias
for `CapabilityContentType` (Json | Arrow | Binary).

### `PlatformIpc` — `foundation_platform`

Extends the base `Ipc` with platform-specific capabilities:

```rust
pub trait PlatformIpc: foundation_wasm::ipc::Ipc {
    fn invoke_with_session(
        &self, session: &PlatformSession, request: &IpcRequest<Vec<u8>>
    ) -> Result<IpcResponse<Vec<u8>>, IpcError>;

    fn as_route_handler(&self) -> Option<&dyn RouteResponder> { None }
}
```

### `IpcRegistry` — `foundation_platform`

Stores `Box<dyn PlatformIpc + 'static>`. `'static` on storage, not the trait.

```rust
pub struct IpcRegistry {
    handlers: RwLock<HashMap<String, Box<dyn PlatformIpc + 'static>>>,
}
```

Methods: `register()`, `get() -> Option<&dyn PlatformIpc>`, `invoke()`, `names()`.

### Tauri command — `__ewe_ipc`

Separate from `__ewe_capabilities` (F23). Registered by PlatformBuilder.
Extracts `IpcRequest` from JSON args, delegates to `IpcRegistry::invoke()`.

### Platform commands overview

| Command | Purpose | Registry | Trait base |
|---------|---------|----------|------------|
| `__ewe_ipc` | General IPC (query, emit, page) | `IpcRegistry` | `foundation_wasm::ipc::Ipc` |
| `__ewe_capabilities` (F23) | Security-gated native capabilities | `CapabilityRegistry` | `foundation_wasm::WasmCapability` |
| `__ewe_ipc_stream` (F26) | Server→client streaming | `PlatformStreamRegistry` | `StreamingIpc` |

## Requirements

### 1. `Ipc` trait — `foundation_wasm`
- No `'static` on the trait
- `Send + Sync` on native, no bounds on wasm32
- `name()`, `kind() -> IpcKind`, `invoke(&IpcRequest<Vec<u8>>)`

### 2. `IpcKind` — `foundation_wasm`
- Query | Emit | Page (NOT Stream — F26)

### 3. `IpcRequest<T>` / `IpcResponse<T>` — `foundation_wasm`
- Default `T = Vec<u8>`, `WirePayload` for typed conversion
- `IpcContentType` = type alias for `CapabilityContentType`
- `IpcError` with `DomainError { code, message }` variant

### 4. `PlatformIpc` — `foundation_platform`
- Extends `foundation_wasm::ipc::Ipc`
- `invoke_with_session(&PlatformSession, &IpcRequest<Vec<u8>>)` — session-aware
- `as_route_handler() -> Option<&dyn RouteResponder>`

### 5. `IpcRegistry` — `foundation_platform`
- `Box<dyn PlatformIpc + 'static>` storage
- `register(I: PlatformIpc + 'static)`
- `get(name) -> Option<&dyn PlatformIpc>`
- `invoke(&PlatformSession, &IpcRequest<Vec<u8>>)`

### 6. Tauri command `__ewe_ipc`
- Registered by PlatformBuilder via `generate_handler!`
- Content-type aware (arrow/binary/json)

### 7. Session API
- `PlatformSession::ipc_registry()`, `register_ipc()`, `get_ipc()`
- Handlers invocable from route handlers via `session.get_ipc(name)?.invoke_with_session(...)`

### 8. IPC as Route Handler
- `PlatformIpc::as_route_handler()` → implements `RouteResponder`
- Registered with `ipc_shell()` route decision (not `webview_app()`)

### 9. No duplication
- `IpcKind` lives only in `foundation_wasm::ipc`, not duplicated in platform

## Verification

```bash
cargo test -p foundation_wasm -- ipc
cargo test -p foundation_platform -- ipc       # 3 tests (register, unknown, names)
cargo test -p foundation_platform -- streaming  # 2 tests (registry, chunk order)
```

## Files

| File | Action |
|------|--------|
| `backends/foundation_wasm/src/ipc.rs` | **NEW** — IpcKind, Ipc trait, IpcRequest<T>, IpcResponse<T>, IpcError |
| `backends/foundation_wasm/src/lib.rs` | Add `pub mod ipc;` |
| `backends/foundation_platform/src/ipc/mod.rs` | **NEW** — PlatformIpc, IpcRegistry, ipc_route() |
| `backends/foundation_platform/src/ipc/streaming.rs` | **NEW** — StreamingIpc, IpcStreamReceiver, PlatformStreamRegistry |
| `backends/foundation_platform/src/session.rs` | Add ipc_registry, stream_registry, register/get methods |
| `backends/foundation_platform/src/builder.rs` | Register __ewe_ipc + __ewe_ipc_stream commands |
