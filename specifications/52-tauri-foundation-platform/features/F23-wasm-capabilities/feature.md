---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F23-wasm-capabilities"
this_file: "specifications/52-tauri-foundation-platform/features/F23-wasm-capabilities/feature.md"

status: pending
priority: critical
created: 2026-07-20

depends_on:
  - "F05-capability-registry"
  - "F19-wasm-annotation-target"

tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---
# F23 — WASM-Native Capabilities

## Problem

Capabilities (decision 07, F05) are currently a `foundation_platform`-only
concept. They're tied to `PlatformSession`, the Tauri runtime, and the
`serde_json::Value` wire format. This means:

- **WASM apps running outside the platform** (browser, Deno, testbed) can't
  use capabilities. Every app that wants to read clipboard, access filesystem,
  or use biometrics must be inside a Tauri shell.
- **Route handlers can't programmatically invoke capabilities.** They're only
  reachable via the WebView → session → registry path. A Rust route handler
  that wants to read a file or query secure storage must either duplicate the
  logic or work around the capability system entirely.
- **The wire format is JSON-only.** Arrow/columnar data paths are closed to
  capabilities, even for high-throughput sensor or camera streams.
- **No standard `invoke_capability` API at the WASM level.** Every platform
  invents its own bridge. WASM apps can't write portable capability calls.

## Solution

Elevate the Capability concept to **`foundation_wasm`** as a platform-agnostic
primitive. `foundation_platform` inherits, extends, and provides the Tauri
bridge. WASM apps get a standard `invoke_capability(name, payload)` API that
works everywhere — browser, Deno, Tauri, testbed — with the same contract.

### Core trait — `foundation_wasm::capability::WasmCapability`

```rust
/// A capability that can be invoked from WASM or native code.
///
/// Unlike F05's platform-specific `Capability` trait, this lives in
/// `foundation_wasm` and is available to all targets (wasm32, native).
pub trait WasmCapability: Send + Sync + 'static {
    /// Unique capability name. Used as the lookup key in registries.
    fn name(&self) -> &str;

    /// Invoke the capability with a serialized request.
    ///
    /// Both `Request` and `Response` must implement:
    /// - `serde::Serialize` + `serde::DeserializeOwned` (JSON path)
    /// - `ToArrow` + `FromArrow` (columnar path)
    fn invoke_capability(
        &self,
        request: CapabilityRequest,
    ) -> Result<CapabilityResponse, CapabilityError>;
}

/// A serialized capability invocation.
///
/// Protocol-agnostic: carries either JSON bytes or Arrow record batches.
/// The dispatcher chooses the codec based on the request's content type.
pub struct CapabilityRequest {
    /// Matches a registered `WasmCapability::name()`.
    pub capability: String,
    /// The action to perform (e.g. "read", "write", "capture").
    pub action: String,
    /// Serialized payload. Encoding indicated by `content_type`.
    pub payload: Vec<u8>,
    /// `application/json` or `application/vnd.apache.arrow.batch`.
    pub content_type: CapabilityContentType,
}

pub struct CapabilityResponse {
    /// Echoes the request capability + action for correlation.
    pub capability: String,
    pub action: String,
    /// Serialized result. Encoding indicated by `content_type`.
    pub payload: Vec<u8>,
    pub content_type: CapabilityContentType,
}

pub enum CapabilityContentType {
    Json,
    Arrow,
}

pub enum CapabilityError {
    UnknownCapability(String),
    InvalidPayload(String),
    ExecutionFailed(String),
    PermissionDenied(String),
}
```

### Registry — `foundation_wasm::capability::CapabilityRegistry`

```rust
/// A portable capability registry. Works on wasm32 and native.
///
/// `foundation_platform` wraps this in its own registry that adds
/// profile gating, per-route allowlisting, and stale-page guards.
pub struct CapabilityRegistry {
    capabilities: HashMap<String, Box<dyn WasmCapability>>,
}

impl CapabilityRegistry {
    pub fn new() -> Self { ... }
    pub fn register<C: WasmCapability>(&mut self, capability: C) { ... }
    pub fn invoke(&self, request: CapabilityRequest) -> Result<CapabilityResponse, CapabilityError> { ... }
    pub fn get(&self, name: &str) -> Option<&dyn WasmCapability> { ... }
    pub fn names(&self) -> impl Iterator<Item = &str> { ... }
}
```

### WASM JS bridge — `invoke_capability`

```typescript
// foundation_wasm runtime provides this globally
function invokeCapability(name: string, action: string, payload: any): Promise<any>

// Usage in a WASM app:
const result = await invokeCapability("clipboard", "read", { format: "text" });
// → { text: "Hello from clipboard" }

const image = await invokeCapability("camera", "capture", { resolution: "1080p" });
// → { data: ArrayBuffer, mime: "image/jpeg" }
```

On **Tauri/Android**, `invokeCapability` routes through a dedicated Tauri command `__ewe_capabilities`
(separate from `__ewe_ipc` — capabilities have their own security model, registry, and contract).
On **browser**, it routes through the WASM JS bridge directly.
On **Deno**, it routes through `Deno.core.opAsync`.

### Platform integration — `foundation_platform`

`foundation_platform` wraps `CapabilityRegistry` with its existing security model:

```rust
// foundation_platform/src/capability.rs

pub struct PlatformCapabilityRegistry {
    /// The portable WASM-level registry.
    inner: CapabilityRegistry,
    /// Profile gates, route allowlists, stale-page guards (F05).
    security: CapabilitySecurityLayer,
}

impl PlatformSession {
    /// Register a WASM-native capability with platform security.
    pub fn register_capability<C: WasmCapability>(&self, capability: C) { ... }

    /// Invoke a capability through the full 5-layer defense.
    pub fn invoke_capability(&self, request: CapabilityRequest) -> Result<CapabilityResponse, CapabilityError> { ... }

    /// Get a capability handle for programmatic use from route handlers.
    pub fn get_capability(&self, name: &str) -> Option<&dyn WasmCapability> { ... }
}
```

**Route handlers can now get and invoke capabilities directly:**

```rust
fn my_route_handler(session: &PlatformSession, intent: &NavigationIntent) -> RouteResult {
    // Programmatic invocation — no WebView round-trip needed.
    let clip = session.get_capability("clipboard").unwrap();
    let response = clip.invoke_capability(CapabilityRequest {
        capability: "clipboard".into(),
        action: "read".into(),
        payload: serde_json::to_vec(&json!({"format": "text"})).unwrap(),
        content_type: CapabilityContentType::Json,
    }).unwrap();

    let text: ClipboardContent = serde_json::from_slice(&response.payload).unwrap();
    RouteResult::Html(format!("<p>Clipboard: {}</p>", text.data))
}
```

### Tauri command bridge — `__ewe_capabilities`

The platform registers a dedicated `#[tauri::command] fn __ewe_capabilities` —
separate from `__ewe_ipc` (F25). Capabilities have their own security model,
registry, and contract. Merging them into the IPC command would conflate two
different namespaces and weaken the security boundary:

```
 JS invokeCapability("clipboard", "read", payload)
   → window.__TAURI_INTERNALS__.invoke("__ewe_capabilities", { capability, action, payload })
     → Rust: #[tauri::command] fn __ewe_capabilities(state, capability, action, payload)
       → PlatformSession::invoke_capability(request)
         → 1. Stale-page guard
         → 2. Look up in CapabilityRegistry
         → 3. Profile gate (min_profile check)
         → 4. Per-route allowlist (RouteDecision.capabilities)
         → 5. Execute capability
         → CapabilityResponse
       → Result<Vec<u8>, String> back to JS
```

```rust
#[tauri::command]
fn __ewe_capabilities(
    app_handle: tauri::AppHandle,
    capability: String,
    action: String,
    payload: Vec<u8>,
    content_type: String,
) -> Result<Vec<u8>, String> {
    let session = app_handle.state::<PlatformSession>();
    let request = CapabilityRequest {
        capability,
        action,
        payload,
        content_type: content_type.parse().unwrap_or(CapabilityContentType::Json),
    };
    session.invoke_capability(request)
        .map(|r| r.payload)
        .map_err(|e| e.to_string())
}
```

**Why a separate command and not folded into `__ewe_ipc`:**

| Concern | `__ewe_capabilities` | `__ewe_ipc` (F25) |
|---------|---------------------|---------------------|
| Registry | `CapabilityRegistry` (F23, portable) | `IpcRegistry` (F25, platform-only) |
| Trait | `WasmCapability` (foundation_wasm) | `Ipc` (foundation_platform) |
| Security | 5-layer defense (F05) | Basic namespace lookup |
| Contract | `CapabilityRequest` → `CapabilityResponse` | `IpcRequest` → `IpcResponse` |
| JS API | `invokeCapability(name, action, payload)` | `invokeIpc(name, action, payload)` |
| Works on | wasm32 + native (portable) | native only (platform-specific) |
| Permission errors | `CapabilityError::PermissionDenied` | `IpcError::PermissionDenied` (different model) |

The separation means a capability can be invoked from either JS path — the
dedicated `invokeCapability` (which goes through security) or a route handler
via `session.get_capability(name)?.invoke_capability(request)` (which also
goes through the same security layer). Neither path goes near the IPC registry.

## Requirements

### 1. `WasmCapability` trait — `foundation_wasm`
- File: `backends/foundation_wasm/src/capability.rs` (NEW)
- Trait: `WasmCapability` with `name()`, `invoke_capability()`
- Types: `CapabilityRequest`, `CapabilityResponse`, `CapabilityContentType`, `CapabilityError`
- All request/response types implement `Serialize` + `DeserializeOwned`
- Arrow support via `ToArrow`/`FromArrow` traits (when arrow feature enabled)
- Re-exported from `foundation_wasm::capability`

### 2. `CapabilityRegistry` — `foundation_wasm`
- Portable registry working on wasm32 + native
- `register()`, `invoke()`, `get()`, `names()` methods
- Thread-safe: `Send + Sync` on native, `RefCell` on wasm32
- No platform dependencies (no Tauri, no tokio)

### 3. WASM JS bridge
- `invokeCapability(name, action, payload)` function in foundation_wasm runtime
- Returns `Promise<any>`
- Protocol: JSON by default, Arrow when `contentType: 'arrow'` specified
- On Tauri: routes through `window.__TAURI_INTERNALS__.invoke('__ewe_capabilities', ...)`
- On browser: routes through the WASM JS bridge directly
- On Deno: routes through `Deno.core.opAsync`

### 3.5. Tauri command `__ewe_capabilities`
- Dedicated `#[tauri::command] fn __ewe_capabilities(...)` registered by PlatformBuilder
- Separate from `__ewe_ipc` (F25) — different registry, different security model
- Routes through `PlatformSession::invoke_capability()` → full 5-layer defense
- Returns `Result<Vec<u8>, String>` (serialized `CapabilityResponse` on success)
- Command name is NOT configurable — `__ewe_capabilities` is the contract

### 4. Platform integration
- `PlatformCapabilityRegistry` wraps `CapabilityRegistry` + F05 security layers
- `PlatformSession::register_capability()` accepts `WasmCapability` impls
- `PlatformSession::get_capability(name)` returns `Option<&dyn WasmCapability>`
- Existing F05 `#[platform_capability]` macro updated to generate `WasmCapability` impls
- Backward compatible: existing F05 capabilities continue to work

### 5. Migration path
- F05 `Capability` trait gains blanket impl bridge to `WasmCapability`
- Existing capabilities work through both old and new APIs during transition
- `serde_json::Value` → `Vec<u8>` conversion in the bridge layer
- Deprecation warning on old API after F25 IPC Registry lands

## Verification

```bash
# Native-side: standard #[test] in tests/ (trait + types + registry logic)
cargo test -p foundation_wasm -- capability

# WASM runtime tests via foundation_testbed (Deno + browser CDP/BiDi)
cargo test -p foundation_testbed --features wasm -- wasm_capability

# Platform integration tests
cargo test -p foundation_platform -- capability

# Tauri command bridge — dedicated __ewe_capabilities command
cargo test -p foundation_platform -- __ewe_capabilities

# Existing F05 tests still pass
cargo test -p foundation_platform -- platform_capability

# Browser console (manual smoke): invokeCapability from a wasm app page
```

## Files

| File | Action |
|------|--------|
| `backends/foundation_wasm/src/capability.rs` | **NEW** — `WasmCapability` trait + types + `CapabilityRegistry` |
| `backends/foundation_wasm/src/lib.rs` | Add `pub mod capability;` |
| `backends/foundation_wasm/runtimes/capability-bridge.js` | **NEW** — `invokeCapability` JS bridge |
| `backends/foundation_platform/src/capability.rs` | Update — wrap `CapabilityRegistry`, bridge F05 |
| `backends/foundation_platform/src/session.rs` | Add `get_capability()`, update `register_capability()` |
| `backends/foundation_macros/src/platform_capability.rs` | Update — generate `WasmCapability` impl |
