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
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---
# F42 — Native Modules: user Kotlin/Swift code injection

## Problem

IPC handlers like camera, biometrics, modal presentation need platform-native
code — Android `Activity` intents, `DialogFragment`, iOS `UIViewController`
presentation, Face ID prompts. Currently there's no way for a user to add
custom Kotlin/Swift files to the Tauri project and have IPC handlers reference
them. No way to inject Gradle dependencies, Android permissions, or iOS
capabilities (Info.plist entries) from the Rust build pipeline.

Tauri already manages the `gen/android/` and `gen/apple/` directories. This
feature teaches `foundation_platform::codegen` to add user-provided native
files, permissions, and dependencies alongside the Tauri-generated ones.

## Tauri's gen/ directory (ground truth)

Tauri generates the native project scaffolding once. The layout:

```
src-tauri/gen/
├── android/
│   ├── app/
│   │   ├── build.gradle.kts          ← auto-generated + committed
│   │   ├── tauri.build.gradle.kts    ← auto-generated, gitignored
│   │   └── src/main/java/com/ewe/platform/
│   │       ├── MainActivity.kt       ← generated ONCE, user-owned
│   │       └── generated/            ← auto-generated every build, gitignored
│   │           ├── TauriActivity.kt
│   │           ├── WryActivity.kt
│   │           ├── RustWebView.kt
│   │           ├── Ipc.kt
│   │           └── ...
│   └── build.gradle.kts              ← auto-generated + committed
└── apple/                            ← (iOS, not yet generated for this project)
    └── Sources/
        └── ...                       ← Xcode project, user-owned Swift files
```

Key facts:
- `MainActivity.kt` is generated ONCE by `tauri android init` and never
  overwritten. Users own it.
- `generated/*.kt` is auto-generated on every build and gitignored.
- `build.gradle.kts` and `tauri.build.gradle.kts` are regenerated each build.
- Users CAN add Kotlin files alongside `MainActivity.kt` — they won't be
  touched by regeneration.
- For iOS, Tauri generates an Xcode project (`gen/apple/`). Users add Swift
  source files to the Xcode project sources directory.

## Solution

A new crate `foundation_platform_native` (`backends/foundation_platform_native/`)
that users add as a build dependency. It contains:

1. The `NativeModule` trait + codegen pipeline
2. Individual native modules (camera, modal, biometric, chrome, filesystem),
   each feature-gated so users only compile what they need
3. Each module owns its complete stack: Kotlin/Swift sources (via `include_str!`),
   Rust IPC handler (impl `Ipc` + `AndroidIpc`), and WASM wrapper extension

### Crate layout

```
backends/foundation_platform_native/
├── Cargo.toml          ← features = ["camera", "modal", "biometric", "chrome", "filesystem"]
├── src/
│   ├── lib.rs           ← re-exports trait + modules
│   ├── native_module.rs ← NativeModule trait, *Config types, *Source enums
│   ├── pipeline.rs      ← PlatformCodegen: register + inject
│   ├── modal/
│   │   ├── mod.rs       ← feature-gated, re-exports
│   │   ├── native.rs    ← impl NativeModule (manifest + sources)
│   │   ├── handler.rs   ← impl Ipc + AndroidIpc
│   │   └── wasm.rs      ← WASM wrapper extension
│   └── camera/
│       ├── mod.rs
│       ├── native.rs
│       ├── handler.rs
│       └── wasm.rs
```

### User's build.rs

```rust
// Cargo.toml:
//   [build-dependencies]
//   foundation_platform_native = { path = "...", features = ["modal", "camera"] }

use foundation_platform_native::{PlatformCodegen, modal, camera};

fn main() {
    let mut codegen = PlatformCodegen::new();
    codegen.register(modal::module());
    codegen.register(camera::module());
    codegen.inject_native_code(&src_tauri);

    foundation_platform::codegen::generate_platform_code();
}
```

Each module exposes a `fn module() -> impl NativeModule` that carries its
Kotlin/Swift sources via `include_str!` — read at build time, injected into
the Tauri gen/ directory. No separate crate needed per module.

### Part A — `NativeModule` trait (in `foundation_platform_native`)

```rust
// foundation_platform_native/src/native_module.rs

pub trait NativeModule: Send + Sync + 'static {
    /// Unique module name (e.g. "camera", "modal").
    fn name(&self) -> &str;

    /// Android-specific configuration.
    fn android(&self) -> Option<AndroidModuleConfig> { None }

    /// iOS-specific configuration.
    fn ios(&self) -> Option<IosModuleConfig> { None }
}

pub struct AndroidModuleConfig {
    /// Kotlin source files to copy into the project alongside MainActivity.kt.
    /// Keys are relative filenames (e.g. "CameraHelper.kt"), values are
    /// the file contents (either path to source or inline text).
    pub kotlin_sources: Vec<KotlinSource>,

    /// Gradle dependencies to add (e.g. "androidx.camera:camera-core:1.3.0").
    pub gradle_dependencies: Vec<String>,

    /// Android permissions to add to AndroidManifest.xml
    /// (e.g. "android.permission.CAMERA").
    pub permissions: Vec<String>,

    /// Android features to declare (e.g. "android.hardware.camera").
    pub features: Vec<String>,

    /// ProGuard rules to add.
    pub proguard_rules: Vec<String>,
}

pub struct IosModuleConfig {
    /// Swift source files to copy into the Xcode project.
    pub swift_sources: Vec<SwiftSource>,

    /// CocoaPods / SPM dependencies to add.
    pub swift_dependencies: Vec<String>,

    /// Info.plist usage description strings.
    /// Keys map to plist keys (e.g. "NSCameraUsageDescription").
    pub info_plist_entries: Vec<(String, String)>,

    /// Frameworks to link (e.g. "AVFoundation", "LocalAuthentication").
    pub frameworks: Vec<String>,
}
```

### Part B — Codegen pipeline (in `foundation_platform_native`)

```rust
// foundation_platform_native/src/pipeline.rs

pub struct PlatformCodegen {
    modules: Vec<Box<dyn NativeModule>>,
}

impl PlatformCodegen {
    pub fn new() -> Self { Self { modules: vec![] } }
    pub fn register(&mut self, module: impl NativeModule) { self.modules.push(Box::new(module)); }
    pub fn inject_native_code(&self, src_tauri: &Path);
}

`inject_native_code()` does:

**Android:**
1. Create `gen/android/app/src/main/java/{package}/native/` directory
2. Copy each registered module's `kotlin_sources` there
3. Append `gradle_dependencies` to a `native.dependencies.gradle` file (applied
   by `tauri.build.gradle.kts`)
4. Append `permissions` and `features` to `AndroidManifest.xml`
5. Append `proguard_rules` to a `native.proguard.pro` file

**iOS:**
1. Copy `swift_sources` into `gen/apple/Sources/NativeModules/`
2. Append `info_plist_entries` to `Info.plist`
3. Append `frameworks` to the Xcode project

### Part C — IPC handler → native module binding

An IPC handler declares its native module dependency:

```rust
struct NativeCamera;

impl Ipc for NativeCamera { ... }

#[cfg(target_os = "android")]
impl AndroidIpc for NativeCamera {
    fn invoke_android(&self, ctx, handle, req) -> Result<..., IpcError> {
        // Calls into com.ewe.platform.native.CameraHelper
        // which was injected by the native module system
        ...
    }
}
```

The Kotlin file `CameraHelper.kt` (injected into `native/` directory):

```kotlin
package com.ewe.platform.native

import android.app.Activity
import android.content.Intent
import android.provider.MediaStore

class CameraHelper(private val activity: Activity) {
    fun openCamera(requestCode: Int) {
        val intent = Intent(MediaStore.ACTION_IMAGE_CAPTURE)
        activity.startActivityForResult(intent, requestCode)
    }
}
```

### Part D — `KotlinSource` / `SwiftSource`

```rust
pub enum KotlinSource {
    /// Path to a .kt file relative to the crate root.
    File(PathBuf),
    /// Inline source for small helpers.
    Inline { filename: String, source: &'static str },
}

pub enum SwiftSource {
    File(PathBuf),
    Inline { filename: String, source: &'static str },
}
```

Most modules use `File` pointing to a `native/` directory in their crate.
Inline is for trivial helpers (one function wrappers).

### Part E — Example: Modal IPC native module

The modal capability (F43) uses this system. Its `NativeModule` impl:

```rust
struct NativeModalModule;

impl NativeModule for NativeModalModule {
    fn name(&self) -> &str { "modal" }

    fn android(&self) -> Option<AndroidModuleConfig> {
        Some(AndroidModuleConfig {
            kotlin_sources: vec![
                KotlinSource::File(PathBuf::from("native/android/ModalHelper.kt")),
            ],
            gradle_dependencies: vec![
                "com.google.android.material:material:1.12.0".into(),
            ],
            permissions: vec![],
            features: vec![],
            proguard_rules: vec![],
        })
    }

    fn ios(&self) -> Option<IosModuleConfig> {
        Some(IosModuleConfig {
            swift_sources: vec![
                SwiftSource::File(PathBuf::from("native/ios/ModalHelper.swift")),
            ],
            swift_dependencies: vec![],
            info_plist_entries: vec![],
            frameworks: vec!["UIKit".into()],
        })
    }
}
```

The Kotlin file `ModalHelper.kt`:

```kotlin
package com.ewe.platform.native

import android.app.Activity
import android.webkit.WebView
import com.ewe.platform.generated.RustWebView

class ModalHelper(private val activity: Activity) {
    fun openModal(url: String, title: String): ModalHandle {
        // Creates a BottomSheetDialogFragment with a WebView loading `url`
        // Returns a handle with a dismiss() method
        ...
    }
}
```

### Part F — How the build.rs pipeline works end-to-end

```
1. User adds files to their crate at native/android/*.kt and native/ios/*.swift
2. User's build.rs calls codegen.register_native_module(NativeCameraModule)
3. codegen.inject_native_code(&src_tauri) runs:
   a. Reads the AndroidManifest.xml, appends any new permissions
   b. Copies .kt files to gen/android/.../native/
   c. Appends Gradle deps to native.dependencies.gradle
   d. For iOS: copies .swift files, patches Info.plist
4. Standard Tauri build continues (tauri_build::build())
```

`native.dependencies.gradle` is already auto-included because
`tauri.build.gradle.kts` (which IS generated) reads it. We add a one-line
hook to the generated `tauri.build.gradle.kts` via a regex patch — or better,
we append to `app/build.gradle.kts` dependencies block via a structured
JSON config file that the Gradle script reads at build time.

Simpler approach: we write a `native-modules.json` file into
`gen/android/app/src/main/assets/` that `tauri.build.gradle.kts` reads.
But that's fragile.

**Safest approach:** Write a `native.gradle` file in `gen/android/app/`
and have `build.rs` add `apply(from = "native.gradle")` to the end of
`app/build.gradle.kts` if not already present. The `native.gradle` file
is regenerated each build with all registered module dependencies.

For permissions, we use Tauri's existing plugin permission system —
`src-tauri/capabilities/default.json`. Each native module can declare
Android permissions there.

### Part G — Gradle multi-module support

If a native module needs MORE than just a Kotlin file and a dependency
(e.g., a full Android library module), it can declare:

```rust
AndroidModuleConfig {
    /// Instead of individual Kotlin sources, point to a Gradle module
    /// directory. The codegen copies this under gen/android/modules/{name}/
    /// and includes it in settings.gradle.
    gradle_module_dir: Option<PathBuf>,
    ...
}
```

Most modules won't need this. It's there for big capabilities (camera with
CameraX, biometrics with BiometricPrompt, maps with Mapbox).

## Requirements

### R1. `NativeModule` trait — `foundation_platform/src/codegen/native.rs`
- `name()`, `android()`, `ios()` methods
- `AndroidModuleConfig` and `IosModuleConfig` structs
- `KotlinSource` and `SwiftSource` enums (File + Inline)

### R2. Codegen pipeline — `foundation_platform/src/codegen/native.rs`
- `PlatformCodegen::register_native_module(impl NativeModule)`
- `PlatformCodegen::inject_native_code(src_tauri_dir: &Path)`
- Copies Kotlin/Swift files, patches permissions and deps

### R3. Example app integration — `platform_android/build.rs`
- Calls `codegen.inject_native_code()` before `generate_platform_code()`

### R4. Plumbing — `foundation_platform/src/lib.rs`
- Re-export `NativeModule`, `AndroidModuleConfig`, `IosModuleConfig`,
  `KotlinSource`, `SwiftSource`
- `pub mod codegen` already exists — add `pub mod native` inside it

### R5. Test — `foundation_platform/tests/native_modules.rs`
- Register a mock module, call `inject_native_code()`, verify files exist
  and permissions are written correctly

### R6. Documentation
- Example `CUSTOM_NATIVE_MODULES.md` in `examples/platform_android/`
  showing a complete camera module from Kotlin → Rust → WASM

## Verification

```bash
cargo test -p foundation_platform -- native_modules
cargo check --manifest-path examples/platform_android/src-tauri/Cargo.toml
# Verify gen/android/app/src/main/java/com/ewe/platform/native/ exists
# with the expected Kotlin files
```

## Files

| File | Action |
|---|---|
| `backends/foundation_platform_native/Cargo.toml` | **NEW** — crate with features per module |
| `backends/foundation_platform_native/src/native_module.rs` | **NEW** — `NativeModule` trait, `*Config` structs, `*Source` enums |
| `backends/foundation_platform_native/src/pipeline.rs` | **NEW** — `PlatformCodegen::register()` + `inject_native_code()` |
| `backends/foundation_platform_native/src/modal/native.rs` | **NEW** — modal `NativeModule` impl (Kotlin sources via `include_str!`) |
| `backends/foundation_platform_native/src/modal/handler.rs` | **NEW** — `ModalIpc` — impl `Ipc` + `AndroidIpc` |
| `backends/foundation_platform_native/src/modal/wasm.rs` | **NEW** — `Chrome::present_modal()` / `dismiss_modal()` |
| `backends/foundation_platform_native/src/camera/native.rs` | **NEW** — camera `NativeModule` impl |
| `backends/foundation_platform_native/src/camera/handler.rs` | **NEW** — `CameraIpc` |
| `backends/foundation_platform_native/src/camera/wasm.rs` | **NEW** — WASM wrapper extension |
| `backends/foundation_platform_native/tests/injection_tests.rs` | **NEW** — codegen injection tests |
| `backends/foundation_platform/src/codegen_native.rs` | **DELETE** — moved to `foundation_platform_native` |
| `examples/platform_android/Cargo.toml` | Add `[build-dependencies] foundation_platform_native` |
| `examples/platform_android/build.rs` | Import from `foundation_platform_native`, register modules |
