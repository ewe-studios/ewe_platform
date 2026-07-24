# Goal — Native Presentation + IPC Integration

**Status:** In Progress
**Created:** 2026-07-25

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
