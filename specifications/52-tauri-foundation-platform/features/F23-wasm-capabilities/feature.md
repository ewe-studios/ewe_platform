---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F23-wasm-capabilities"
this_file: "specifications/52-tauri-foundation-platform/features/F23-wasm-capabilities/feature.md"

status: completed
priority: critical
created: 2026-07-20
updated: 2026-07-21

depends_on:
  - "F05-capability-registry"
  - "F19-wasm-annotation-target"

tasks:
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100%

implementation_notes: |
  The trait was split across crates — `WasmCapability` (base, no `'static`)
  in `foundation_wasm`, platform extension in `foundation_platform`.
  'static bound only on registry storage, not the trait.
  CapabilityRequest<T>/CapabilityResponse<T> are generic with default T=Vec<u8>.
  WirePayload trait enables typed⇄wire round-trips without Arrow deps.
---
# F23 — WASM-Native Capabilities

## Problem

Capabilities (decision 07, F05) are `foundation_platform`-only. WASM apps
outside the platform (browser, Deno, testbed) can't use them. The wire format
is JSON-only. No standard `invoke_capability` API at the WASM level.

## Solution

Three layers, same pattern as IPC (F25):

```
foundation_wasm (no_std, wasm32 + native)
├── WirePayload trait        — into_wire_bytes() / from_wire_bytes()
├── CapabilityContentType    — Json | Arrow | Binary
├── CapabilityRequest<T>     — T = Vec<u8> default, into_wire() / into_typed()
├── CapabilityResponse<T>    — same
├── WasmCapability trait     — name(), invoke_capability()
├── CapabilityRegistry       — portable, works on wasm32 + native

foundation_platform (native)
└── PlatformCapability       — extends WasmCapability with session + security
      invoke_with_session(&PlatformSession, request) → 5-layer defense
```

### `WirePayload` trait — the typed⇄wire bridge

No serde or Arrow dependency in `foundation_wasm`. Higher crates add blanket
impls for `Serialize + DeserializeOwned` (→ Json) and `ToArrow + FromArrow`
(→ Arrow). `Vec<u8>` has a passthrough impl.

```rust
pub trait WirePayload: Sized {
    fn into_wire_bytes(self) -> (Vec<u8>, CapabilityContentType);
    fn from_wire_bytes(data: &[u8], content_type: CapabilityContentType) -> Result<Self, WireError>;
}
```

### Generic wire types

```rust
pub struct CapabilityRequest<T = Vec<u8>> {
    pub capability: String,
    pub action: String,
    pub payload: T,
    pub content_type: CapabilityContentType,
}

pub struct CapabilityResponse<T = Vec<u8>> {
    pub capability: String,
    pub action: String,
    pub payload: T,
    pub content_type: CapabilityContentType,
}
```

`into_wire()` on `CapabilityRequest<T>` calls `T::into_wire_bytes()`, returns
`CapabilityRequest<Vec<u8>>`. `into_typed<T>()` reverses it. Same for responses.

### `WasmCapability` trait — no `'static`

`'static` bound lives on the registry's `Box<dyn WasmCapability + 'static>` and
on `register()`'s generic parameter — never on the trait itself.

```rust
#[cfg(not(target_family = "wasm"))]
pub trait WasmCapability: Send + Sync {
    fn name(&self) -> &str;
    fn invoke_capability(
        &self, request: &CapabilityRequest<Vec<u8>>
    ) -> Result<CapabilityResponse<Vec<u8>>, CapabilityError>;
}
```

### Registry — portable, Mutex-guarded on native

```rust
pub struct CapabilityRegistry {
    #[cfg(not(target_family = "wasm"))]
    inner: Mutex<BTreeMap<String, Box<dyn WasmCapability + Send + Sync + 'static>>>,
    #[cfg(target_family = "wasm")]
    inner: BTreeMap<String, Box<dyn WasmCapability + 'static>>,
}
```

`register(&self, cap: impl WasmCapability + 'static)` — interior mutability on
native (Mutex). `invoke()` takes `&CapabilityRequest<Vec<u8>>`. `invoke_typed<T>()`
converts through wire format.

### Platform integration — `PlatformCapability` trait

Platform extension adds session-aware invocation through the 5-layer defense:

```rust
pub trait PlatformCapability: foundation_wasm::WasmCapability {
    fn invoke_with_session(
        &self, session: &PlatformSession, request: &CapabilityRequest<Vec<u8>>
    ) -> Result<CapabilityResponse<Vec<u8>>, CapabilityError>;
}
```

### JS bridge — `capability-bridge.js`

Pure JS, injected via ScriptInjector (F24). Detects host at runtime:
- Tauri → `__TAURI_INTERNALS__.invoke('__ewe_capabilities', ...)`
- Deno → `Deno.core.opAsync`
- Browser → direct WASM bridge

### Tauri command — `__ewe_capabilities`

Dedicated command, separate from `__ewe_ipc`. Different registry, different
security model.

## Requirements

### 1. `WirePayload` trait — `foundation_wasm`
- `into_wire_bytes()` / `from_wire_bytes()`
- `WireError` enum: EncodeFailed, DecodeFailed, UnsupportedContentType
- Passthrough impl for `Vec<u8>`
- Higher crates add blanket impls for Serialize/Deserialize and Arrow

### 2. `CapabilityContentType` — `foundation_wasm`
- Json | Arrow | Binary (shared with IPC)

### 3. `CapabilityRequest<T>` / `CapabilityResponse<T>` — `foundation_wasm`
- Default `T = Vec<u8>` for wire form
- `into_wire()` / `into_typed()` on both
- `wire()` convenience constructor

### 4. `WasmCapability` trait — `foundation_wasm`
- No `'static` on the trait
- `Send + Sync` on native, no bounds on wasm32
- `name()`, `invoke_capability(&CapabilityRequest<Vec<u8>>)`

### 5. `CapabilityRegistry` — `foundation_wasm`
- `register(&self, impl WasmCapability + 'static)` (interior mutability on native via Mutex)
- `get(&self, name) -> Option<&dyn WasmCapability>`
- `invoke(&self, &CapabilityRequest<Vec<u8>>) -> Result`
- `invoke_typed<T: WirePayload>(&self, CapabilityRequest<T>) -> Result<CapabilityResponse<T>>`
- `names() -> Vec<String>`

### 6. JS bridge
- `capability-bridge.js`: `window.invokeCapability(name, action, payload)`
- Embedded as `CAPABILITY_BRIDGE_JS` in `foundation_wasm_ui::embedded`
- Detects Tauri/Deno/browser at runtime

### 7. Tauri command `__ewe_capabilities`
- Registered by PlatformBuilder via `generate_handler!`
- Extracts `CapabilityRequest` from JSON args, delegates to `PlatformSession::invoke_wasm_capability()`（已读入，当前状态已折叠）

### 8. PlatformCapability trait
- Extends WasmCapability with invoke_with_session(&PlatformSession, ...)
- PlatformSession has register_platform_capability(), get_platform_capability()

### 9. No duplication
- foundation_platform's old Capability types renamed to avoid collision
- foundation_platform re-exports foundation_wasm types, doesn't shadow them

## Verification

```bash
cargo test -p foundation_wasm -- capability      # 8 tests
cargo test -p foundation_platform -- capability   # platform integration
cargo test -p foundation_platform -- __ewe_capabilities
```

## Files

| File | Action |
|------|--------|
| `backends/foundation_wasm/src/capability.rs` | **NEW** — WirePayload, CapabilityRequest<T>, CapabilityResponse<T>, WasmCapability, CapabilityRegistry |
| `backends/foundation_wasm/src/lib.rs` | Add `mod capability; pub use capability::*;` |
| `backends/foundation_wasm/tests/capability_tests.rs` | **NEW** — 8 tests |
| `backends/foundation_wasm_ui/runtimes/capability-bridge.js` | **NEW** — invokeCapability JS bridge |
| `backends/foundation_wasm_ui/src/embedded.rs` | Add CAPABILITY_BRIDGE_JS constant |
| `backends/foundation_platform/src/capability.rs` | Rename old types, add PlatformCapability trait |
| `backends/foundation_platform/src/builder.rs` | Register __ewe_capabilities command |
| `backends/foundation_platform/src/session.rs` | Add wasm_capability_registry, register/get/invoke methods |
