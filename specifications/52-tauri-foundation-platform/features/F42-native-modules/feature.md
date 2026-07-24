---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F42-native-modules"
this_file: "specifications/52-tauri-foundation-platform/features/F42-native-modules/feature.md"

status: pending
priority: critical
created: 2026-07-25
updated: 2026-07-25

depends_on:
  - "F41-unified-ipc-ffi"
  - "F06-webview-stack"

tasks:
  completed: 0
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---
# F42 — Native Modules: user Kotlin/Swift code injection

## Problem

IPC handlers like camera, biometrics, modal presentation need platform-native
code — Android `Activity` intents, `DialogFragment`, iOS `UIViewController`
presentation, Face ID prompts. Currently there's no way to add custom
Kotlin/Swift files to the Tauri project and have IPC handlers reference them.

Tauri already manages `gen/android/` and `gen/apple/`. This feature provides
a `foundation_platform_native` crate where each native module (camera, modal,
biometric) lives as a feature-gated sub-directory containing ALL the code for
that module: Kotlin sources, Swift sources, Rust IPC handler, and WASM wrapper.

## Solution

A new crate `backends/foundation_platform_native/`. Users add it as a Cargo
build dependency and enable features for the modules they want:

```toml
[build-dependencies]
foundation_platform_native = { path = "../../backends/foundation_platform_native", features = ["modal", "camera"] }
```

### Directory-per-module layout

Each module is one directory with three layers:

```
foundation_platform_native/
├── Cargo.toml               ← features = ["modal", "camera", "biometric", "chrome"]
├── src/
│   ├── lib.rs                ← re-exports trait + feature-gated module::register()
│   ├── native_module.rs      ← NativeModule trait, *Config types, *Source enums
│   ├── pipeline.rs           ← PlatformCodegen::register() + inject_native_code()
│   └── modal/
│       ├── mod.rs            ← pub fn module() -> impl NativeModule + register fns
│       ├── android/           ← Kotlin sources (injected into gen/android/)
│       │   └── ModalHelper.kt
│       ├── ios/               ← Swift sources (injected into gen/apple/)
│       │   └── ModalHelper.swift
│       ├── handler.rs         ← IPC handler + AndroidIpc impl
│       └── wasm.rs            ← WASM wrapper extension
│
├── camera/
│   ├── mod.rs
│   ├── android/
│   │   └── CameraHelper.kt
│   ├── ios/
│   │   └── CameraHelper.swift
│   ├── handler.rs
│   └── wasm.rs
│
├── biometric/
│   └── ...
└── chrome/
    └── ...
```

## Part A — `NativeModule` trait (in `foundation_platform_native`)

```rust
// foundation_platform_native/src/native_module.rs

pub trait NativeModule: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn android(&self) -> Option<AndroidModuleConfig> { None }
    fn ios(&self) -> Option<IosModuleConfig> { None }
}

pub struct AndroidModuleConfig {
    pub kotlin_sources: Vec<KotlinSource>,
    pub gradle_dependencies: Vec<String>,
    pub permissions: Vec<String>,
}

pub struct IosModuleConfig {
    pub swift_sources: Vec<SwiftSource>,
    pub swift_dependencies: Vec<String>,
    pub info_plist_entries: Vec<(String, String)>,
    pub frameworks: Vec<String>,
}

pub enum KotlinSource {
    File(PathBuf),
    Inline { filename: String, source: &'static str },
}

pub enum SwiftSource {
    File(PathBuf),
    Inline { filename: String, source: &'static str },
}
```

## Part B — Codegen pipeline (in `foundation_platform_native`)

```rust
// foundation_platform_native/src/pipeline.rs

pub struct PlatformCodegen {
    modules: Vec<Box<dyn NativeModule>>,
}

impl PlatformCodegen {
    pub fn new() -> Self;
    pub fn register(&mut self, module: impl NativeModule);
    pub fn inject_native_code(&self, src_tauri: &Path);
}
```

`inject_native_code()` does:

**Android:**
1. Resolves package path from `gen/android/app/src/main/java/{pkg}/`
2. Creates `native/{module_name}/` under the package directory
3. Copies each module's `kotlin_sources` there
4. Writes `native_modules.gradle` with all dependency `implementation()` lines
5. Patches `app/build.gradle.kts` to `apply(from = "native_modules.gradle")`
6. Patches `AndroidManifest.xml` with requested permissions

**iOS:**
1. Copies `swift_sources` into `gen/apple/Sources/NativeModules/{module_name}/`
2. Appends `info_plist_entries` to `Info.plist`

## Part C — Module `mod.rs` (the public entry point)

Each module exposes a single function that returns its config:

```rust
// foundation_platform_native/src/modal/mod.rs

use crate::native_module::{NativeModule, AndroidModuleConfig, KotlinSource};
use crate::pipeline::PlatformCodegen;

/// Register this module with the codegen pipeline.
pub fn register(pipeline: &mut PlatformCodegen) {
    pipeline.register(Module);
}

struct Module;

impl NativeModule for Module {
    fn name(&self) -> &str { "modal" }
    fn android(&self) -> Option<AndroidModuleConfig> {
        Some(AndroidModuleConfig {
            kotlin_sources: vec![
                KotlinSource::Inline {
                    filename: "ModalHelper.kt".into(),
                    source: include_str!("android/ModalHelper.kt"),
                },
            ],
            gradle_dependencies: vec![
                "com.google.android.material:material:1.12.0".into(),
            ],
            permissions: vec![],
        })
    }
}
```

Kotlin/Swift sources are embedded at compile time via `include_str!` — they
live alongside the Rust code in `android/` and `ios/` subdirectories.

## Part D — IPC handler + WASM wrapper (in the same module directory)

Each module also provides its `Ipc` + `AndroidIpc` handler and WASM wrapper
extension. These are compiled into the user's binary (NOT build.rs — they're
runtime code in `[dependencies]`, not `[build-dependencies]`).

```rust
// foundation_platform_native/src/modal/handler.rs
// Compiled as part of the user's binary (runtime dependency).

use foundation_wasm::ipc::{Ipc, IpcKind, IpcRequest, IpcResponse, IpcError, IpcContentType};
use foundation_platform::handle::AndroidIpc;
use foundation_platform::PlatformSession;

pub struct ModalIpc;

impl Ipc<Vec<u8>, Vec<u8>> for ModalIpc {
    fn name(&self) -> &str { "chrome" }
    fn kind(&self) -> IpcKind { IpcKind::Capability }
    fn invoke(&self, req: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        // Typically delegates to session — real work in invoke_with_session
        Err(IpcError::ExecutionFailed("use invoke_with_session".into()))
    }
}

impl foundation_platform::ipc::PlatformIpc for ModalIpc {
    fn invoke_with_session(
        &self,
        session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        match req.action.as_str() {
            "present_modal" => present(session, req),
            "dismiss_modal" => dismiss(session, req),
            _ => Err(IpcError::ExecutionFailed(format!("unknown: {}", req.action))),
        }
    }
}
```

And the WASM wrapper:

```rust
// foundation_platform_native/src/modal/wasm.rs
// #[cfg(target_family = "wasm")]

use crate::modal::dispatch_json;

pub struct Modal;

impl Modal {
    pub fn present(args: PresentModalArgs) -> Result<PresentModalResult, IpcError> { ... }
    pub fn dismiss(modal_id: &str) -> Result<(), IpcError> { ... }
}
```

## Part E — How the user wires it all together

### 1. Cargo.toml

```toml
[dependencies]
foundation_platform_native = { path = "../../backends/foundation_platform_native", features = ["modal"] }

[build-dependencies]
foundation_platform_native = { path = "../../backends/foundation_platform_native", features = ["modal"] }
```

### 2. build.rs — inject native code

```rust
use foundation_platform_native::pipeline::PlatformCodegen;
use foundation_platform_native::modal;

fn main() {
    let mut codegen = PlatformCodegen::new();
    modal::register(&mut codegen);    // copies ModalHelper.kt into gen/android/
    codegen.inject_native_code(&src_tauri);

    foundation_platform::codegen::generate_platform_code();
}
```

### 3. src/lib.rs — register IPC handler at runtime

```rust
use foundation_platform_native::modal::handler::ModalIpc;

fn setup_routes(session: &PlatformSession) {
    session.register_ipc(ModalIpc);
    // ... rest of setup
}
```

### 4. WASM app — typed wrapper

```rust
// In the WASM app (compiled with foundation_platform_native):
use foundation_platform_native::modal::wasm::Modal;

let result = Modal::present(PresentModalArgs {
    route: "/app/settings".into(),
    style: "bottom_sheet".into(),
})?;
```

## Part F — modal::register() is the single entry point

Users call one function. The module does the rest:

```
modal::register(&mut pipeline)  →  injects ModalHelper.kt into gen/android/
                                →  injects ModalHelper.swift into gen/apple/

session.register_ipc(ModalIpc)  →  registers "chrome" IPC handler at runtime

Modal::present(args)             →  typed WASM wrapper calling chrome/present_modal
```

## Requirements

### R1. `NativeModule` trait — `foundation_platform_native/src/native_module.rs`
- `name()`, `android()`, `ios()` methods
- `AndroidModuleConfig`, `IosModuleConfig`, `KotlinSource`, `SwiftSource`

### R2. Codegen pipeline — `foundation_platform_native/src/pipeline.rs`
- `PlatformCodegen::register(impl NativeModule)`
- `PlatformCodegen::inject_native_code(src_tauri: &Path)`
- Copies Kotlin/Swift files, patches permissions and Gradle deps

### R3. Module structure — per-module directory
- `{module}/mod.rs` — `pub fn register(pipeline)` + `NativeModule` impl
- `{module}/android/*.kt` — Kotlin sources (included via `include_str!`)
- `{module}/ios/*.swift` — Swift sources
- `{module}/handler.rs` — IPC handler + platform traits
- `{module}/wasm.rs` — WASM wrapper extension

### R4. Feature gating — `Cargo.toml`
- `modal`, `camera`, `biometric`, `chrome`, `filesystem` features
- Each feature enables the corresponding `pub mod` and `register()` fn

### R5. Example app integration — `platform_android`
- `[build-dependencies] foundation_platform_native = { features = ["modal"] }`
- `build.rs` calls `modal::register(&mut pipeline)`
- `src-tauri/src/lib.rs` registers `ModalIpc` on the session

### R6. Test — `foundation_platform_native/tests/injection_tests.rs`
- Register a module via `modal::register()`
- Call `inject_native_code()` into a temp directory
- Verify Kotlin files, Gradle deps, and manifest patches are written correctly

### R7. Test — `foundation_platform_native/tests/modal_handler.rs`
- Instantiate `ModalIpc`, invoke with `present_modal` / `dismiss_modal`
- Verify stack operations (WebViewStack + WindowManager)

## Verification

```bash
cargo test -p foundation_platform_native
cargo check --manifest-path examples/platform_android/src-tauri/Cargo.toml
```

## Files

| File | Action |
|---|---|
| `backends/foundation_platform_native/Cargo.toml` | **NEW** — features per module |
| `backends/foundation_platform_native/src/lib.rs` | **NEW** — feature-gated re-exports |
| `backends/foundation_platform_native/src/native_module.rs` | **NEW** — trait + configs |
| `backends/foundation_platform_native/src/pipeline.rs` | **NEW** — PlatformCodegen |
| `backends/foundation_platform_native/src/modal/mod.rs` | **NEW** — modal `register()` + NativeModule |
| `backends/foundation_platform_native/src/modal/android/ModalHelper.kt` | **NEW** — Kotlin helper |
| `backends/foundation_platform_native/src/modal/ios/ModalHelper.swift` | **NEW** — Swift helper |
| `backends/foundation_platform_native/src/modal/handler.rs` | **NEW** — ModalIpc |
| `backends/foundation_platform_native/src/modal/wasm.rs` | **NEW** — WASM wrapper |
| `backends/foundation_platform_native/tests/injection_tests.rs` | **NEW** |
| `backends/foundation_platform/src/codegen_native.rs` | **DELETE** — moved to foundation_platform_native |
| `examples/platform_android/Cargo.toml` | Add build + runtime deps |
| `examples/platform_android/build.rs` | Add modal::register() |
