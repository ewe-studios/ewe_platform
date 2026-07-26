# Getting Started — foundation_platform_native

Add native platform capabilities (modals, dialogs, camera, biometrics) to a Tauri-based EWE Platform app. Each capability has:

- **A Kotlin/Swift helper** — native UI (bottom sheets, dialogs, etc.)
- **A Rust IPC handler** — `Ipc + PlatformIpc` impl, registered on the session
- **A WASM wrapper** — typed function that apps import and call

## Quick Start

### 1. Add the dependency

```toml
# Cargo.toml of your Tauri binary crate (e.g. platform_android/src-tauri/Cargo.toml)
[dependencies]
foundation_platform_native = { features = ["modal", "dialog"] }
```

### 2. Register the plugin

```rust
// In your app's lib.rs — add to PlatformBuilder setup
.plugin(foundation_platform_native::native::plugin::plugin())
```

### 3. Register IPC handlers on the session

```rust
fn setup_routes(session: Arc<PlatformSession>) {
    foundation_platform_native::native::modal::register(Arc::clone(&session));
    foundation_platform_native::native::dialog::register(Arc::clone(&session));
}
```

### 4. Call from WASM

```rust
use foundation_platform_native::shared::modal_types::PresentArgs;
use foundation_platform_native::wasm::modal::Modal;

let result = Modal::present(PresentArgs {
    route: "/app/settings".into(),
    style: Some("bottom_sheet".into()),
    title: Some("Settings".into()),
}).await?;
```

## Architecture at a glance

```
┌─────────────────────────────────────────────────┐
│  WASM app (dashboard.wasm)                      │
│  ┌─────────────────────────────────────────┐   │
│  │ wasm/modal.rs — Modal::present()        │   │
│  │ → calls ipc_dispatch("chrome","present")│   │
│  └──────────────┬──────────────────────────┘   │
│                 │ IPC bridge (FFI)              │
├─────────────────┼──────────────────────────────┤
│  Native binary (.so on Android)                │
│  ┌──────────────▼──────────────────────────┐   │
│  │ native/modal.rs — ModalIpc::present()   │   │
│  │ → WebviewWindowBuilder + SheetWryActivity│   │
│  │ → WindowManager + WebViewStack          │   │
│  └──────────────┬──────────────────────────┘   │
│                 │ Tauri PluginHandle            │
│  ┌──────────────▼──────────────────────────┐   │
│  │ Kotlin: SheetWryActivity, MultiWryActivity│  │
│  └─────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
```

## API surface

| Capability | WASM API | IPC channel | Native handler |
|---|---|---|---|
| **Modal** | `Modal::present()` / `dismiss()` | `chrome` | `ModalIpc` — `WebviewWindowBuilder` + `SheetWryActivity` |
| **Dialog** | `Dialog::show()` / `dismiss()` | `chrome` | `DialogIpc` — Kotlin `AlertDialog` |

## Creating a new capability

See [Adding Native Capabilities](../adding_native_capabilities/) for the step-by-step guide.
