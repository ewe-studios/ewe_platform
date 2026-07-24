# Goal — Native Presentation Capabilities via foundation_platform_native

**Status:** In Progress
**Created:** 2026-07-25
**Scope:** features/F42-modal-capability, features/F42-native-modules

The goal: `platform_android` proves every presentation mode through
`foundation_platform_native`. Each capability has its own feature doc
with research, implementation, testing, and "what done looks like."

## Capabilities to deliver

| Feature | Native behavior | IPC name | Status |
|---|---|---|---|
| Modal | BottomSheetDialogFragment + WebView | `chrome/present_modal` | **IPC handler done** (7 tests), Kotlin stub |
| Dialog | AlertDialog / DialogFragment + optional WebView | `dialog/show` | **IPC handler done**, Kotlin stub |
| Push | New WebViewWindow, native back | Route handler (existing) | Done |
| Replace | In-place navigate | Route handler (existing) | Done |
| Root | Clear stack, new root | Route handler (existing) | Done |
| External | System browser / Chrome Custom Tabs | Route handler (existing) | Done |

## What done looks like

1. `foundation_platform_native/src/modal.rs` — `present_modal` creates a
   real `BottomSheetDialogFragment` on Android via JNI, returns `modal_id`
2. `foundation_platform_native/src/dialog.rs` — `show` creates a real
   `AlertDialog` on Android with optional embedded WebView
3. Both register as Tauri plugins (Kotlin `Plugin` subclass)
4. Both have typed WASM wrappers (works via `ipc_ffi::ipc_dispatch`)
5. `platform_android` has interactive test pages for each mode
6. Headful testing: run on Android emulator, verify native dialogs appear
7. Unit tests: IPC handler logic, wire format round-trips
8. Regression: all 379 existing tests still pass

## Architecture

```
[WASM app]
  ├─ Modal::present(args) ──→ ipc_ffi::ipc_dispatch("chrome", "present_modal")
  ├─ Dialog::show(args)  ──→ ipc_ffi::ipc_dispatch("dialog", "show")
  │
  ▼
[foundation_platform_native]  ← Tauri plugin crate
  ├─ modal.rs:  ModalIpc (Ipc + PlatformIpc)
  ├─ dialog.rs: DialogIpc (Ipc + PlatformIpc)
  │
  ▼
[foundation_platform]  ← session, WindowManager, WebViewStack
  ├─ session.invoke_ipc(ctx, request)
  ├─ window_manager.ensure(pool, label, url)
  │
  ▼
[Kotlin Plugin]  ← android/src/main/java/com/ewe/platform/
  ├─ ModalHelper.kt:  BottomSheetDialogFragment + WebView
  ├─ DialogHelper.kt: AlertDialog + optional WebView
  │
  ▼
[Native Android]
  BottomSheetDialog / AlertDialog shown over the main activity
```

## Testing strategy

| Layer | Test type | Tool |
|---|---|---|
| Rust IPC handler | Unit tests | `cargo test -p ewe-platform-native` |
| Wire format | Round-trip tests | `cargo test -p foundation_wasm -- ipc_wire` |
| WASM→JS E2E | Integration tests | `node --test` (existing F41 tests) |
| JS→WASM E2E | Integration tests | `node --test` (existing F41 tests) |
| Kotlin plugin | Android emulator | `foundation_testbed` + QEMU / physical device |
| Headful UI | Manual test page | Interactive HTML page in `platform_android` app |

Done means its works, we've proven it, we dont start another native capability without first finishing the first end to end. Test + Actual App showing it works.
