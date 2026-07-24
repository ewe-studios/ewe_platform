---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F42-modal-capability"
this_file: "specifications/52-tauri-foundation-platform/features/F42-modal-capability/feature.md"

status: pending
priority: critical
created: 2026-07-25
updated: 2026-07-25

depends_on:
  - "F41-unified-ipc-ffi"
  - "F06-webview-stack"

tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---
# F42 — Native Modal Capability (Presentation::Modal power)

## Problem

`Presentation::Modal` in the route handler creates a new WebView via Tauri's
`WindowManager`. But the WASM app has no way to *programmatically* dismiss the
modal, configure its presentation style (dialog vs. bottom sheet), or respond to
dismiss events. The route handler owns the declaration, not the control flow
after open.

The route handler says "open this route as modal" — but the WASM app running IN
the modal can't say "close me" or "I'm done, return `{ result }`."

Secondarily, the presentation style matters on Android: a modal should show as a
`BottomSheetDialogFragment` (slides up from bottom) or a full `DialogFragment`
(centered overlay), not just another full-screen WebView window. This is the
`PresentationContext` concept from Hotwire Native (default vs. modal context),
but driven by the WASM app at invoke time.

## Solution

A built-in IPC handler in `foundation_platform/src/ipc/` that wraps
`WindowManager` and `WebViewStack` operations behind the unified `Ipc` trait.
WASM apps call `Chrome::present_modal()` and `Chrome::dismiss_modal()` through
the typed WASM wrapper.

### API

```
IPC name: "chrome"
Actions:
  present_modal  → opens a new WebView as a modal, returns { modal_id }
  dismiss_modal  → closes a modal by ID, returns { ok }
  dismiss_all_modals → closes all modals, returns { ok }
```

Payload for `present_modal`:
```json
{
  "route": "/app/settings/profile",
  "style": "bottom_sheet",    // "bottom_sheet" | "dialog" | "fullscreen"
  "title": "Profile Settings",
  "dismiss_on_back_press": true
}
```

Response:
```json
{
  "modal_id": "modal_3",
  "webview_label": "modal_3"
}
```

### Implementation (via F42-native-modules plugin)

**foundation_platform_native/src/modal.rs** — IPC handler + WASM wrapper:

```rust
// feature-gated: #[cfg(feature = "modal")]
pub fn register(session: &PlatformSession) { session.register_ipc(ModalIpc); }

struct ModalIpc;
impl Ipc for ModalIpc { ... }
impl PlatformIpc for ModalIpc { ... }

#[cfg(target_family = "wasm")]
pub mod wasm { pub struct Modal; impl Modal { ... } }
```

**Kotlin plugin** (auto-injected by `tauri_plugin::Builder`):

```
android/src/main/java/com/ewe/platform/
├── EwePlatformPlugin.kt    ← extends app.tauri.plugin.Plugin
└── ModalHelper.kt          ← BottomSheetDialogFragment / AlertDialog with WebView
```

```
modal/
├── mod.rs       ← pub fn register(pipeline) + NativeModule impl
├── android/
│   └── ModalHelper.kt   ← BottomSheetDialogFragment / DialogFragment with WebView
├── ios/
│   └── ModalHelper.swift
├── handler.rs   ← ModalIpc: impl Ipc + PlatformIpc + AndroidIpc
└── wasm.rs      ← typed WASM wrapper: Modal::present() / dismiss()
```

**handler.rs** — the IPC handler:

```rust
struct ModalIpc;

impl Ipc for ModalIpc {
    fn name(&self) -> &str { "chrome" }
    fn kind(&self) -> IpcKind { IpcKind::Capability }
    fn invoke(&self, req: &IpcRequest) -> Result<IpcResponse, IpcError> {
        match req.action.as_str() {
            "present_modal" => { ... }
            "dismiss_modal" => { ... }
            _ => Err(...)
        }
    }
}
```

The handler calls `session.webview_stack_mut()` to push a modal slot and
`session.window_manager()` to create the WebView — exact same code path as
`record_presentation()` for `Presentation::Modal`, but invoked as a
capability rather than a route decision.

**Modal registry** — tracks active modals by ID so `dismiss_modal` finds them:

```rust
// On PlatformSession
modal_registry: RwLock<HashMap<String, ModalEntry>>,

struct ModalEntry {
    webview_label: String,
    route: String,
    style: ModalStyle,
}
```

**wasm/chrome.rs** — typed wrapper additions:

```rust
impl Chrome {
    pub fn present_modal(args: PresentModalArgs) -> Result<PresentModalResult, IpcError> { ... }
    pub fn dismiss_modal(args: DismissModalArgs) -> Result<(), IpcError> { ... }
}
```

### Android-specific: native Dialog/BottomSheet

Tauri v2 supports multiple WebView windows on Android. `WindowManager` already
calls `WebviewWindowBuilder::new()` which on Android creates a new Activity
(or a DialogFragment if we configure it). The `ModalStyle` enum maps to Android
presentation:

| ModalStyle | Android | iOS |
|---|---|---|
| `fullscreen` | New Activity (default WebviewWindow) | `presentViewController(animated:)` |
| `dialog` | `DialogFragment` with WebView | `UIModalPresentationStyle.pageSheet` |
| `bottom_sheet` | `BottomSheetDialogFragment` with WebView | `UIModalPresentationStyle.pageSheet` |

For now, all three styles use the same Tauri `WebviewWindowBuilder` path (new
window). The `style` field is stored in the modal registry and used when
platform-native dialog support is added.

**F42-native-modules implementation plan:**
`ModalHelper.kt` creates a `BottomSheetDialogFragment` or `AlertDialog`
with an embedded `WebView` loading the route URL. The dialog does NOT
navigate the main webview — it overlays a native Android dialog on top.
The Kotlin code registers click listeners on the dialog buttons and
calls back through JNI (or the Tauri IPC bridge) to deliver the result.

This is the exact same pattern as Hotwire Native's `Modal` presentation context:
a native view controller/dialog containing a WebView, shown over the main stack.

### Dismiss and return value

When the WASM app in the modal calls `Chrome::dismiss_modal()`, the handler:
1. Finds the modal entry by `modal_id`
2. Calls `window_manager.destroy(pool, &label)` to close the WebView
3. Pops the modal slot from the stack
4. Fires `ipc_handle_event` to the parent page with the result payload

The parent page receives:
```json
{
  "event": "modal_dismissed",
  "modal_id": "modal_3",
  "result": { "saved": true }
}
```

## Requirements

### R1. Built-in IPC handler — `foundation_platform/src/ipc/modal.rs`
- `impl Ipc for ModalIpc` — name = "chrome"
- Handles `present_modal`, `dismiss_modal`, `dismiss_all_modals`
- Uses existing `WindowManager` + `WebViewStack` infrastructure

### R2. Modal registry on PlatformSession
- `modal_registry: RwLock<HashMap<String, ModalEntry>>`
- Tracks `webview_label`, `route`, `style` per modal
- `register_modal()` / `get_modal()` / `remove_modal()`

### R3. Typed WASM wrapper — `foundation_platform/src/wasm/chrome.rs`
- `Chrome::present_modal(args) -> Result<PresentModalResult, IpcError>`
- `Chrome::dismiss_modal(args) -> Result<(), IpcError>`

### R4. Reverse event on dismiss
- When modal dismisses, push `modal_dismissed` event to parent page
  via `session.emit_to_page()` or `ipc_handle_event`

### R5. Integration test in platform_android
- Register the chrome capability
- Add a "Open Modal" button that navigates to a modal route
- Add a "Close Modal" button inside the modal that dismisses itself

## Verification

```bash
cargo test -p foundation_platform -- modal
cargo check --manifest-path examples/platform_android/src-tauri/Cargo.toml
```

## Files

| File | Action |
|---|---|
| `backends/foundation_platform_native/src/modal/mod.rs` | **NEW** — `modal::register()` + `NativeModule` impl |
| `backends/foundation_platform_native/src/modal/android/ModalHelper.kt` | **NEW** — DialogFragment with WebView |
| `backends/foundation_platform_native/src/modal/ios/ModalHelper.swift` | **NEW** — iOS modal presentation |
| `backends/foundation_platform_native/src/modal/handler.rs` | **NEW** — `ModalIpc` handler |
| `backends/foundation_platform_native/src/modal/wasm.rs` | **NEW** — `Modal::present()` / `dismiss()` |
| `backends/foundation_platform/src/session.rs` | Add `modal_registry` |
| `examples/platform_android/src-tauri/src/lib.rs` | Integration test |
