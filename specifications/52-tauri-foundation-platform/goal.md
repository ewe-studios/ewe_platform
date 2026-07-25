# Goal — Native Presentation + IPC Integration

**Status:** In Progress
**Created:** 2026-07-25
**Last updated:** 2026-07-25

## Current status (2026-07-25)

### Modal — working (JS bridge path)
- F43 ReplyEncoder: IPC responses use `[100:Begin][18:Uint8ArrayBuffer][slot_id:u64][101:End]` format ✓
- EventDispatcher.deliver: wired to `signalDeliver` (was empty noop) ✓
- Kotlin: `EwePlatformPlugin.presentModal()` → `ModalHelper.present()` → `BottomSheetDialog` ✓
- JS bridge: `invokeIpc('chrome','present_modal',...)` → PluginHandle → BottomSheet ✓
- WASM button callback path: ctx.callback → signalDeliver → invoke_signal_callback — ready but need viewport fix for button taps

### Dialog — working (JS bridge path)
- Kotlin: `EwePlatformPlugin.showDialog()` → `ModalHelper.presentTextDialog()` → `AlertDialog` ✓
- JS bridge: `invokeIpc('dialog','show',...)` → PluginHandle → AlertDialog ✓
- WebView-content dialog variant: needs Kotlin implementation

### Dismiss — working (JS bridge path)
- `invokeIpc('chrome','dismiss_modal',...)` / `dismissAllModals` → Kotlin dismiss ✓

### Other presentation modes — not started
- Push, Replace, Root, External: zero implementation

### WebView integration
- Plain `android.webkit.WebView` in modals — no `__TAURI_INTERNALS__`
- `WebviewBuilder::add_child` is `#[cfg(desktop)]` — not available on Android
- `findWebViewInstance()` utility available in ModalHelper.kt for future use
- Track Wry #495 (expose native handles) — when available, detach/reattach Tauri WebView into dialogs

### Documentation
- Not started — need `docs/{getting_started/, deep-dives/, adding_native_capabilities/, connecting_to_wasm/}` for all 4 crates

## Target

The `platform_android` example app demonstrates every presentation mode working
natively through `foundation_platform_native`:

| Presentation | Native behavior |
|---|---|
| **Modal** | `BottomSheetDialogFragment` slides up from bottom with embedded `WebView` |
| **Dialog** | `AlertDialog` or `DialogFragment` — centered overlay, configurable buttons, optional `WebView` or custom content |
| **Push** | New Activity/WebView, native back gesture pops to previous |
| **Replace** | In-place navigation — no stack change |
| **Root** | Clear stack, new root WebView |
| **External** | System browser (Chrome Custom Tabs on Android) |

## Architecture

```
foundation_platform_native/
├── src/modal.rs       ← IPC handler + WASM wrapper (present/dismiss)
├── src/dialog.rs      ← IPC handler + WASM wrapper (AlertDialog/DialogFragment)
├── src/navigation.rs  ← IPC handler for stack ops (Push, Replace, Root)
├── android/           ← Kotlin sources (auto-injected by tauri_plugin::Builder)
│   └── ...ModalHelper.kt, DialogHelper.kt, NavigationHelper.kt
└── ios/               ← Swift sources
```

Each module provides three layers:
1. **Kotlin/Swift** — native OS code (DialogFragment, Activity launch, etc.)
2. **Rust IPC handler** — `Ipc + PlatformIpc` registered on the session
3. **WASM wrapper** — typed API for WASM apps (`Modal::present()`, `Dialog::show()`)

## Interaction paths

```
WASM app              →  IPC FFI (ipc_ffi::ipc_dispatch)
  → Rust handler       →  Ipc::invoke / PlatformIpc::invoke_with_session
    → Kotlin/Swift     →  PluginManager.runCommand() / JNI
      → Native UI      →  BottomSheet / Dialog / Activity
        ← Response     ←  JSON back through the same chain
```

And the reverse (host → WASM):
```
Native UI event (button tap) → PluginManager.runCommand()
  → Rust handler              → session.emit_to_page() / ipc_handle_event
    → WASM callback           → TriggerRegistry delivers to WASM app
```

## Validation

Every capability is tested in `platform_android`:
1. Compile and run on Android emulator
2. Interactive test page with buttons for each presentation mode
3. Verify native behavior (dialog appears, bottom sheet slides, stack nav works)
4. Verify WASM round-trip (WASM app calls capability, gets response, receives events)
5. Done means its works, we've proven it, we dont start another native capability without first finishing the first end to end. Test + Actual App showing it works.

## Documentation

Each crate gets `docs/` directories:
- `foundation_wasm/docs/` — IPC FFI, WirePayload, TriggerRegistry
- `foundation_wasm_ui/docs/` — DOM runtime, batch protocol, columnar parser
- `foundation_platform/docs/` — session, route handler, window manager, WebView stack
- `foundation_platform_native/docs/` — adding native capabilities, Kotlin/Swift helpers, connecting WASM

Each has a `getting_started/`, `deep-dives/`, `adding_native_capabilities/`, and `connecting_to_wasm/` directory, linked from the crate's `README.md`.

Done means its works, we've proven it, we dont start another native capability without first finishing the first end to end. Test + Actual App showing it works.
