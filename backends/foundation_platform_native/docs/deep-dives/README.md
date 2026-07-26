# Deep Dives — foundation_platform_native

## Dual-target architecture

The crate compiles for **two targets** from the same source tree:

| Target | Build path | Purpose |
|---|---|---|
| `not(wasm)` | `src/native/` | IPC handler + Tauri plugin — runs in the binary |
| `wasm32` | `src/wasm/` | Typed wrapper — calls through the IPC bridge |
| Both | `src/shared/` | Wire-format types (`PresentArgs`, `DismissArgs`, etc.) |

```rust
// lib.rs — target-gated module tree
#[cfg(not(target_family = "wasm"))]
pub mod native;
#[cfg(target_family = "wasm")]
pub mod wasm;
pub mod shared;
```

This works because the crate is compiled **twice**:
1. Once for the target architecture (x86_64/aarch64/etc.) as part of the binary — picks up `native/`
2. Once for `wasm32-unknown-unknown` as part of the WASM bundle — picks up `wasm/`

`shared/` is always available on both sides, providing the canonical `WirePayload` types that travel across the IPC boundary.

## IPC flow: WASM → Native

```
WASM app                                         Native binary
────────                                         ─────────────
Modal::present(args)
  ↓
wasm/modal.rs
  → serializes PresentArgs via WirePayload
  → calls ipc_dispatch("chrome", "present_modal", bytes)
    ↓                    IPC FFI bridge              ↓
    ↓───────────────────────────────────────────→   ↓
  ipc_ffi::host_ipc_invoke(ptr, len, callback_id)
    ↓
  IpcRegistry::dispatch("chrome", request)
    ↓
  ModalIpc::invoke_with_session(request, callback)
    ↓
  ModalIpc::present(session, request)   [native/modal.rs]
    ↓
  WebviewWindowBuilder::new(&app_handle, label, url)
    .activity_name("SheetWryActivity")  // Android path
    .build()
    ↓
  callback(Ok(PresentResult { modal_id }))
    ↓                   IPC FFI bridge               ↓
    ↓←───────────────────────────────────────────── ↓
  ipc_ffi::ipc_resolve(callback_id, result_bytes)
```

### Key design decisions

1. **`PlatformIpc::invoke_with_session`** — Native handlers receive the `PlatformSession`, giving them access to `WebViewStack`, `WindowManager`, and `AppHandle`. Plain `Ipc::invoke` doesn't have this.

2. **`IpcCallback`** — The native handler calls `callback(Result<IpcResponse, IpcError>)` when done. This makes the bridge async-ready: the WASM side gets a Promise that resolves when the callback fires.

3. **`WirePayload` trait** — Each shared type implements `into_wire_bytes()` and `from_wire_bytes()`. The WASM side uses `.into_typed::<PresentArgs>()` to decode, keeping serialization logic in one place.

## SheetWryActivity (Android)

When `ModalIpc::present()` builds a WebviewWindow on Android:

1. `tao::Window::new()` detects `activity_name == "SheetWryActivity"` + `decorations == false`
2. Calls `create_activity("SheetWryActivity", height_fraction=0.6)`
3. Kotlin's `WryActivity.startActivityWithHeightFraction()` launches `SheetWryActivity`
4. `SheetWryActivity.onWebViewReady()` applies:
   - `Window.setLayout(MATCH_PARENT, 60%h)` — partial height
   - `Window.setGravity(BOTTOM)` — bottom sheet position
   - `FLAG_DIM_BEHIND` + `setDimAmount(0.5)` — dim background
5. The WebView it gets is a full Tauri WebView — IPC, init scripts, VFS, all wired

**Why extend TauriActivity?** `getPluginManager()` is defined on the generated `TauriActivity`, not on `WryActivity`. Without it, Tauri plugins (including EwePlatformPlugin) crash with `NoSuchMethodError`.

## MultiWryActivity (future)

For same-Activity multi-WebView (Stage 2), a single `MultiWryActivity` manages a `FrameLayout` stack with virtual activity IDs from the Rust side. Each `activity_id` in `ACTIVITY_PROXY` corresponds to a layer in the stack — base WebView at layer 0, overlays above.
