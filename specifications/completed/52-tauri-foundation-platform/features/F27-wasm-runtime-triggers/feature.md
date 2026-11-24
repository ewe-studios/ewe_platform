---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F27-wasm-runtime-triggers"
this_file: "specifications/52-tauri-foundation-platform/features/F27-wasm-runtime-triggers/feature.md"

status: completed
priority: critical
created: 2026-07-21
updated: 2026-07-21

depends_on:
  - "F23-wasm-capabilities"
  - "F25-ipc-registry"

tasks:
  completed: 5
  uncompleted: 0
  total: 5
  completion_percentage: 100%
---
# F27 — WASM Runtime IPC & Capability Triggers (host → wasm)

## Problem

F23 (capabilities) and F25 (IPC registry) implement the **wasm → host** direction:
WASM code calls `invokeCapability()` / `invokeIpc()` which routes through JS
to the Tauri backend. The reverse direction — **host → wasm** — is not handled.
The host (browser, Tauri, Deno) has no way to deliver a capability request or
IPC invocation INTO the WASM module.

Concrete use cases:
- A Tauri push notification arrives → delivered as a capability invocation to
  the WASM app's registered handler
- A server-sent event arrives via the route handler → dispatched as an IPC
  to the WASM app's registered handler
- The platform needs to call `wasm_app.on_event("background")` or similar

## Solution

Add capability and IPC **trigger** support to the `foundation_wasm` JS runtime
and the `foundation_wasm` Rust crate. This is one-way: host invokes WASM, WASM
responds (or doesn't). Not bidirectional streaming — that's F28.

### JS runtime — `foundation-wasm.js` additions

The `FoundationWasm` runtime gains two trigger dispatch methods:

```js
// foundation-wasm.js additions

FoundationWasm.prototype.triggerCapability = function(request) {
    // request = { capability: string, action: string, payload: Uint8Array }
    // Routes to the registered WasmCapability handler in WASM.
    // Uses the existing protocol dispatch (protocol byte for capability).
    var encoded = encodeCapabilityTrigger(request);
    this.dispatch(/* protocol byte */, /* memory id */, encoded);
};

FoundationWasm.prototype.triggerIpc = function(request) {
    // request = { ipc: string, action: string, payload: Uint8Array }
    // Routes to the registered Ipc handler in WASM.
    var encoded = encodeIpcTrigger(request);
    this.dispatch(/* protocol byte */, /* memory id */, encoded);
};
```

### Rust side — `foundation_wasm` handler traits

```rust
// foundation_wasm/src/capability.rs — additions

/// Callback type for capability triggers arriving from the host.
/// Registered by the WASM app; called by the runtime when the host
/// delivers a capability request.
pub type CapabilityTriggerHandler = Box<dyn Fn(&CapabilityRequest<Vec<u8>>) -> Result<CapabilityResponse<Vec<u8>>, CapabilityError>>;

// foundation_wasm/src/ipc.rs — additions

/// Callback type for IPC triggers arriving from the host.
pub type IpcTriggerHandler = Box<dyn Fn(&IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError>>;
```

These are NOT traits — they're simple callback registrations on the WASM
side. The WASM app registers a handler function; the runtime calls it when
the host delivers a trigger. This avoids the complexity of a full registry
on the WASM side (which already exists on the platform side).

```rust
// foundation_wasm/src/trigger.rs (NEW)

use alloc::boxed::Box;
use crate::capability::{CapabilityRequest, CapabilityResponse, CapabilityError};
use crate::ipc::{IpcRequest, IpcResponse, IpcError};

pub type CapTrigger = Box<dyn Fn(&CapabilityRequest<Vec<u8>>) -> Result<CapabilityResponse<Vec<u8>>, CapabilityError>>;
pub type IpcTrigger = Box<dyn Fn(&IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError>>;

/// Host→WASM trigger registry. The WASM app registers handlers here;
/// the JS runtime calls them when the host delivers capability/IPC requests.
pub struct TriggerRegistry {
    capability_handler: Mutex<Option<CapTrigger>>,
    ipc_handler: Mutex<Option<IpcTrigger>>,
}

impl TriggerRegistry {
    pub fn new() -> Self { ... }

    /// Register the capability trigger handler. Only one handler; replaces previous.
    pub fn set_capability_handler(&self, handler: CapTrigger) { ... }

    /// Register the IPC trigger handler.
    pub fn set_ipc_handler(&self, handler: IpcTrigger) { ... }

    /// Called by the JS runtime when a capability trigger arrives.
    pub fn dispatch_capability(&self, request: &CapabilityRequest<Vec<u8>>) -> Result<CapabilityResponse<Vec<u8>>, CapabilityError> { ... }

    /// Called by the JS runtime when an IPC trigger arrives.
    pub fn dispatch_ipc(&self, request: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> { ... }
}
```

### `foundation_wasm_ui` — additional protocol bytes

`foundation_wasm_ui` registers handlers for the new protocol dispatch types:

```js
// In foundation_wasm_ui (or capability-bridge.js):
// When the host triggers a capability, it calls:
dispatcher.setProtocolHandler(CAPABILITY_TRIGGER_BYTE, function(data) {
    var request = decodeCapabilityTrigger(data);
    var response = wasmApp.handleCapability(request);
    // Response goes back to host via the callback
});
```

### Platform wire-up

On the Tauri side, `__ewe_capabilities` and `__ewe_ipc` already route
host→backend. For host→wasm, the platform's route handlers call
`window.eval()` or the WRY bridge to deliver triggers:

```rust
// When a push notification arrives (platform side):
let trigger_js = format!(
    "FoundationWasm.triggerCapability({{ capability: 'push', action: 'received', payload: new Uint8Array({}) }})",
    serde_json::to_string(&payload).unwrap()
);
window.eval(&trigger_js)?;
```

## Requirements

### 1. `TriggerRegistry` — `foundation_wasm`
- File: `backends/foundation_wasm/src/trigger.rs` (NEW)
- `set_capability_handler()`, `set_ipc_handler()` — single-handler model
- `dispatch_capability()`, `dispatch_ipc()` — called by JS runtime
- `Mutex`-wrapped on native, plain on wasm32
- Type aliases: `CapTrigger`, `IpcTrigger`

### 2. JS runtime additions — `foundation-wasm.js`
- `FoundationWasm.prototype.triggerCapability(request)` — host→wasm capability
- `FoundationWasm.prototype.triggerIpc(request)` — host→wasm IPC
- Both use the existing protocol dispatch (`ProtocolDispatcher`)
- New protocol bytes registered for capability-trigger and ipc-trigger

### 3. Protocol byte registration
- Capability trigger: protocol byte 3
- IPC trigger: protocol byte 4
- Registered in `foundation_wasm_ui` (or `capability-bridge.js`)

### 4. Works on any host
- `foundation_platform` already has Tauri-based host→wasm delivery (route
  handlers, `window.eval()`, WRY bridge) — no changes needed there.
- This feature gives `foundation_wasm` the *wasm-side* infrastructure to
  receive and dispatch capability/IPC triggers regardless of the host.
- Browser host calls `FoundationWasm.triggerCapability(...)` directly.
- Deno host calls through the WASM bridge.
- Tauri host calls through the existing platform delivery → JS runtime.

## Verification

```bash
cargo check -p foundation_wasm
cargo test -p foundation_wasm -- trigger
cargo check -p foundation_wasm_ui
cargo check -p foundation_platform
```

## Files

| File | Action |
|------|--------|
| `backends/foundation_wasm/src/trigger.rs` | **NEW** — TriggerRegistry, CapTrigger, IpcTrigger |
| `backends/foundation_wasm/src/lib.rs` | Add `pub mod trigger;` |
| `backends/foundation_wasm/runtime/foundation-wasm.js` | Add `triggerCapability()`, `triggerIpc()` |
| `backends/foundation_wasm_ui/runtimes/capability-bridge.js` | Register trigger protocol handlers |
