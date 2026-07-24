---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F42-native-modules"
this_file: "specifications/52-tauri-foundation-platform/features/F42-native-modules/feature.md"

status: in-progress
priority: critical
created: 2026-07-25
updated: 2026-07-25

progress:
  completed: 5
  uncompleted: 2
  total: 7
  completion_percentage: 71%

learnings:
  - "Tauri AssetResolver.iter() returns EMPTY on Android APK — keys() fallback needed"
  - "bundle.active must be true + devUrl removed for Tauri to embed assets in debug APK"
  - "Crate structure: shared/ (WirePayload types), native/ (handlers), wasm/ (typed wrappers)"
  - "WirePayload trait on shared types eliminates manual serialize/deserialize duplication"
  - "ModalIpc takes session directly via PlatformIpc trait — no PluginHandle needed yet"

depends_on:
  - "F41-unified-ipc-ffi"
  - "F06-webview-stack"

tasks:
  completed: 0
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---
# F42 — Native Modules: Tauri plugin crate with Kotlin/Swift + IPC handlers

## Problem

IPC handlers (camera, modal, biometric, chrome) need platform-native code —
Kotlin on Android, Swift on iOS. Tauri already has a plugin system for this:
`tauri::plugin::Plugin` trait, `tauri_plugin::Builder` for build-time code
injection, `Plugin::extend_api` for Tauri commands, and the mobile bridge
(`PluginManager` on Android, `register_ios_plugin` on iOS).

Currently we have `foundation_platform::injector` with `window.eval()` and
a custom codegen pipeline — reinventing what Tauri plugins already solve.

## Solution

A `foundation_platform_native` crate that IS a proper Tauri plugin, using
the established conventions:

```
foundation_platform_native/
├── Cargo.toml
├── build.rs              ← tauri_plugin::Builder::new("ewe-platform-native")
│                           .android_path("android/")
│                           .build()
├── permissions/           ← auto-generated ACL permissions
│   └── default.toml
├── android/               ← Kotlin sources (auto-copied by tauri_plugin build)
│   └── src/main/java/com/ewe/platform/
│       ├── ModalHelper.kt
│       ├── CameraHelper.kt
│       └── BiometricHelper.kt
├── ios/                   ← Swift sources (auto-copied by tauri_plugin build)
│   └── Sources/
│       ├── ModalHelper.swift
│       ├── CameraHelper.swift
│       └── BiometricHelper.swift
├── guest-js/              ← JS API injected into WebView (optional)
│   └── index.js
└── src/
    ├── lib.rs             ← tauri::plugin::Builder init + feature-gated modules
    ├── modal.rs           ← modal IPC handler + WASM wrapper
    ├── camera.rs          ← camera IPC handler + WASM wrapper
    ├── biometric.rs       ← biometric IPC handler + WASM wrapper
    └── chrome.rs          ← chrome IPC handler + WASM wrapper
```

### How Tauri plugins handle native code

**Build time** — `tauri_plugin::Builder` runs during `build.rs`:
1. Copies `android/` directory into the app's Gradle project as a library module
2. Copies `ios/` directory into the Xcode project as a framework
3. Generates ACL permission files
4. The Kotlin/Swift code IS the plugin — it extends `app.tauri.plugin.Plugin`
   and communicates with Rust via `PluginManager.runCommand()`

**Runtime** — `tauri::plugin::Builder` registers:
1. A `setup` hook that runs when the app starts — register IPC handlers here
2. Tauri commands via `invoke_handler` — optional, commingled with app commands
3. JS init scripts via `js_init_script` — injected into WebView

**Mobile bridge** — Kotlin plugin calls `PluginManager.runCommand(id, name, command, data)`:
1. The command name maps to a Tauri command registered on the Rust side
2. JSON payload crosses JNI / FFI boundary
3. Response returns as JSON back to Kotlin

### How we use this for IPC handlers

Instead of our own `NativeModule` trait and `PlatformCodegen`, we use the
Tauri plugin as the delivery vehicle. The plugin's `setup()` hook registers
IPC handlers on the `PlatformSession`.

### User's Cargo.toml

```toml
[dependencies]
foundation_platform_native = { features = ["modal", "camera"] }
```

No separate build-dependency needed — the plugin crate's own `build.rs`
handles the codegen.

### User's src-tauri/src/lib.rs

```rust
use foundation_platform::PlatformBuilder;
use foundation_platform_native;

fn main() {
    platform_run!(PlatformBuilder::new()
        .inject_platform_runtimes()
        .setup(|session| {
            // Each feature-gated module registers its IPC handler
            foundation_platform_native::modal::register(session);
            foundation_platform_native::camera::register(session);

            // App-specific setup...
            setup_routes(session);
        })
    );
}
```

### User's build.rs

```rust
fn main() {
    // No native code registration needed — the plugin crate's own build.rs
    // handles it automatically. Just run the normal platform codegen.
    foundation_platform::codegen::generate_platform_code();
}
```

### Module structure (feature-gated)

Each module in `src/` is `#[cfg(feature = "modal")]` and provides:

```rust
// foundation_platform_native/src/modal.rs

use foundation_platform::PlatformSession;

/// Register the modal IPC handler on the session.
/// Called during PlatformBuilder::setup().
pub fn register(session: &PlatformSession) {
    session.register_ipc(ModalIpc);
}

// IPC handler
struct ModalIpc;

impl foundation_wasm::ipc::Ipc<Vec<u8>, Vec<u8>> for ModalIpc { ... }
impl foundation_platform::ipc::PlatformIpc for ModalIpc { ... }

// WASM wrapper (only on wasm32)
#[cfg(target_family = "wasm")]
pub mod wasm {
    // typed Modal::present() / Modal::dismiss()
}
```

## Requirements

### R1. Plugin build.rs — `foundation_platform_native/build.rs`
- Use `tauri_plugin::Builder::new("ewe-platform-native")` 
- `.android_path("android/")` — copies Kotlin sources
- `.ios_path("ios/")` — copies Swift sources
- No custom codegen — Tauri handles it

### R2. Kotlin plugin class — `android/.../EwePlatformPlugin.kt`
- Extends `app.tauri.plugin.Plugin`
- Overrides `load(webView, config)` — receives plugin config from Rust
- Provides extension functions called by IPC handlers (e.g. `presentModal()`)

### R3. Feature-gated modules — `src/lib.rs`
- `#[cfg(feature = "modal")] pub mod modal;`
- `#[cfg(feature = "camera")] pub mod camera;`
- `#[cfg(feature = "biometric")] pub mod biometric;`
- `#[cfg(feature = "chrome")] pub mod chrome;`

### R4. IPC registration — via PlatformBuilder::setup()
- Each module's `register(session)` adds its IPC handler to the session
- No separate registry step — one function call

### R5. Typed WASM wrappers — `#[cfg(target_family = "wasm")]` in each module
- Uses `ipc_ffi::ipc_dispatch` — already works for E2E
- Each module exposes a typed struct: `Modal`, `Camera`, `Biometric`, `Chrome`

### R6. Example app integration — `platform_android`
- Add `foundation_platform_native` as a dependency with `features = ["modal"]`
- Call `foundation_platform_native::modal::register(session)` in setup
- No build.rs changes needed

### R7. Test — IPC handler unit tests
- ModalIpc: present, dismiss, dismiss_all with mock session

## Verification

```bash
cargo test -p foundation_platform_native
cargo check -p foundation_platform_native --features modal
cargo check --manifest-path examples/platform_android/src-tauri/Cargo.toml
```

## Files

| File | Action |
|---|---|
| `backends/foundation_platform_native/Cargo.toml` | **UPDATE** — features, tauri-plugin dep, build-dependencies |
| `backends/foundation_platform_native/build.rs` | **NEW** — tauri_plugin::Builder |
| `backends/foundation_platform_native/permissions/default.toml` | **NEW** — ACL permissions |
| `backends/foundation_platform_native/android/src/main/java/com/ewe/platform/EwePlatformPlugin.kt` | **NEW** — Tauri plugin class |
| `backends/foundation_platform_native/android/src/main/java/com/ewe/platform/ModalHelper.kt` | **NEW** |
| `backends/foundation_platform_native/ios/Sources/EwePlatformPlugin.swift` | **NEW** — Tauri plugin class |
| `backends/foundation_platform_native/ios/Sources/ModalHelper.swift` | **NEW** |
| `backends/foundation_platform_native/src/lib.rs` | **UPDATE** — feature-gated modules, no native_module trait |
| `backends/foundation_platform_native/src/modal.rs` | **NEW** — ModalIpc handler + WASM wrapper |
| `backends/foundation_platform_native/src/camera.rs` | **NEW** — CameraIpc handler + WASM wrapper |
| `backends/foundation_platform_native/src/native_module.rs` | **DELETE** — not needed, Tauri Plugin trait replaces it |
| `backends/foundation_platform_native/src/pipeline.rs` | **DELETE** — not needed, tauri_plugin::Builder replaces it |
| `examples/platform_android/Cargo.toml` | Add foundation_platform_native dep |
| `examples/platform_android/src-tauri/src/lib.rs` | Call modal::register(session) in setup |
