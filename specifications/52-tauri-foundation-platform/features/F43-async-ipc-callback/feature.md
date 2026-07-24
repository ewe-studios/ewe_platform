---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F43-async-ipc-callback"
this_file: "specifications/52-tauri-foundation-platform/features/F43-async-ipc-callback/feature.md"

status: pending
priority: critical
dependencies:
  - F41-unified-ipc-ffi
  - F42-native-modules
created: 2026-07-25
updated: 2026-07-25

tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# F43 — Async IPC callback: bridge WASM sync imports → Tauri async invoke

## Problem

`ipc_dispatch()` in `ipc_ffi.rs` calls `host_ipc_invoke(ptr, len)`, a **synchronous
WASM import**. The WASM call frame freezes until the host returns a `u64`
allocation id holding the response bytes.

Tauri's `__TAURI_INTERNALS__.invoke('__ewe_ipc', args)` is a **JS `Promise`** —
it resolves on the event loop, not inline. The existing `registerIpcTriggers`
handler returns the Promise object itself, which `_dispatchIpcInvoke` reads as
`undefined`, then returns `0n` (error). Every WASM→IPC call on Tauri silently
fails.

**No WASM `ipc_dispatch` call has ever worked on Tauri.** The IPC demo pages
work because JS calls `invokeIpc()` directly — a different code path that
bypasses `host_ipc_invoke` entirely.

## Solution

### R1. Callback-based `PlatformIpc` trait

Change from sync `invoke_with_session` returning `Result<IpcResponse, IpcError>`
to an **async-compatible callback** pattern.

```rust
// OLD (sync — can't bridge to Tauri)
fn invoke_with_session(
    &self, session: &PlatformSession, request: &IpcRequest<Vec<u8>>,
) -> Result<IpcResponse<Vec<u8>>, IpcError>;

// NEW (callback — async-compatible)
fn invoke_with_session(
    &self,
    session: &PlatformSession,
    request: &IpcRequest<Vec<u8>>,
    callback: Box<dyn FnOnce(Result<IpcResponse<Vec<u8>>, IpcError>) + Send>,
) -> Result<(), IpcError>;
```

`Ok(())` means "request dispatched, callback WILL fire." The handler calls
`callback(Ok(response))` (or `callback(Err(...))`). Synchronous handlers
call the callback immediately — no behavior change. Async handlers
(network, Kotlin plugin bridge) can defer it.

### R2. `PlatformSession` passed as `Arc`

The callback may outlive the call frame (async dispatch). Handlers hold their
own `Arc<PlatformSession>` — no borrowed references across async boundaries.

```rust
// Each handler stores this at registration time
struct ModalIpc {
    session: Arc<PlatformSession>,
}
```

### R3. WASM side: fire-and-forget with token

```rust
// ipc_ffi.rs — new export pair
pub fn ipc_dispatch<T: WirePayload, U: WirePayload>(
    name: &str,
    request: IpcRequest<T>,
    callback: impl FnOnce(Result<IpcResponse<U>, IpcError>) + 'static,
) -> Result<IpcToken, IpcError>;
```

1. Serialize request
2. Register callback in global registry keyed by `token: u64`
3. `host_ipc_invoke_async(ptr, len, token)` — returns immediately
4. Return `Ok(token)`

When the host resolves, it calls `ipc_resolve(token, resp_ptr, resp_len)` —
a WASM export that reads the response, deserializes it, and fires the callback.

### R4. JS host side

```js
// foundation-wasm.js — new
_dispatchIpcInvokeAsync(ptr, len, token) {
  var req = FoundationWasm._ipcDecodeRequest(bytes);
  invokeIpc(req.ipc, req.action, req.payload)  // async — returns Promise
    .then(function(result) {
      // Serialize response into WASM memory
      var respBytes = FoundationWasm._ipcEncodeResponse(...);
      var allocId = this.memory.create(respBytes.length);
      this.memory.write(allocId, respBytes);
      // Call WASM export to deliver the response
      instance.exports.ipc_resolve(token, allocId);
    });
  return 0n; // fire-and-forget
}
```

### R5. Typed WASM wrappers

```rust
// wasm/modal.rs
impl Modal {
    pub fn present(args: PresentArgs) -> Result<PresentResult, IpcError> {
        // F43: sync shim — block on a parked callback
        // Later: return a Future / StreamIterator for true async
        let (tx, rx) = oneshot();
        ipc_dispatch("chrome", IpcRequest {
            ipc: "chrome".into(), action: "present_modal".into(),
            payload: args, content_type: IpcContentType::Json, target: None,
        }, move |result| { let _ = tx.send(result); })?;
        rx.recv().unwrap() // blocks WASM thread until host resolves
    }
}
```

## Implementation plan

| # | Step | Files |
|---|------|-------|
| 1 | Add callback to `PlatformIpc::invoke_with_session` | `foundation_platform/src/ipc/mod.rs` |
| 2 | `Session` → `Arc<PlatformSession>` in handler storage | `foundation_platform/src/ipc/mod.rs`, `session.rs` |
| 3 | Add `host_ipc_invoke_async` + `ipc_resolve` to WASM ABI | `foundation_wasm/src/host_runtime.rs` |
| 4 | JS `_dispatchIpcInvokeAsync` | `foundation-wasm.js` |
| 5 | `ipc_dispatch` with callback registry | `foundation_wasm/src/ipc_ffi.rs` |
| 6 | Update all `PlatformIpc` impls (ModalIpc, DialogIpc, Echo, etc.) | `foundation_platform_native`, `platform_android` |
| 7 | Update tests | all test files |
| 8 | E2E proof on Android emulator | `platform_android` |

## Impact on existing code

| Crate | Surface | Change |
|-------|---------|--------|
| `foundation_wasm` | `Ipc` trait | `invoke` stays sync for wasm32; `PlatformIpc` diverges |
| `foundation_platform` | `PlatformIpc`, `IpcRegistry` | callback signature + `Arc<Session>` |
| `foundation_platform_native` | `ModalIpc`, `DialogIpc` | store `Arc<PlatformSession>` at registration |
| `foundation-wasm.js` | `_dispatchIpcInvoke` | new async variant |
| `ipc-ffi.rs` | `ipc_dispatch` | callback registry + token |

## Verification

```bash
cargo test -p foundation_platform -- ipc
cargo test -p ewe-platform-native --features modal
cargo check --manifest-path examples/platform_android/src-tauri/Cargo.toml
```

Android E2E: deploy, tap "Present Modal", screenshot showing native modal.
Console log: `[E2E] IPC OK modal_id=modal_1`.
