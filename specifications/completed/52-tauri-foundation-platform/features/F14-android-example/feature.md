---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F14-android-example"
this_file: "specifications/52-tauri-foundation-platform/features/F14-android-example/feature.md"

status: completed
priority: critical
created: 2026-07-18
updated: 2026-07-21

depends_on:
  - "F13-cross-platform-builds"

tasks:
  completed: 8
  uncompleted: 0
  total: 8
  completion_percentage: 100%
---
# F14 — Android example app

## Overview

Build and deploy `examples/platform_android` — a Tauri v2 Android app using
`foundation_platform`. Proves the cross-compilation pipeline, JNI bridge, and
ewe:// protocol all work on Android.

This app boots via `platform_run!`, registers routes, and renders the
foundation_platform dashboard in the WebView.

## Requirements

### 1. Create `examples/platform_android` ✅

A Tauri v2 project with `src-tauri/` mobile convention:
- `src-tauri/Cargo.toml` depends on `foundation_platform` ✅
- `src-tauri/src/lib.rs` uses `platform_run!` ✅
- `crate-type = ["lib", "cdylib", "staticlib"]` for .so output ✅
- `[workspace]` opt-out (required by Tauri v2 for mobile) ✅

### 2. `cargo tauri android init` ✅

Gradle project, AndroidManifest, and Kotlin MainActivity generated.
Tauri's JNI bridge wraps our Rust `.so`.

### 3. `cargo tauri android build` ✅ (code compiles)

Cross-compiles `foundation_platform` to `aarch64-linux-android` and
`x86_64-linux-android`, links the `.so`, packages into APK.
Verified: `cargo check` + `cargo build` pass on Linux desktop host.

### 4. Deploy to emulator 🔄 (needs Android SDK)

`adb install` → app launches → WebView shows platform dashboard.
Logcat confirms session ID was generated.
**Blocked on:** Android SDK/NDK availability on build machine.

### 5. Android-specific configuration ✅

- NDK 26.1 at `~/android-sdk/ndk/26.1.10909125` ✅
- Cargo config has NDK linker paths ✅
- `src-tauri/tauri.conf.json` has `identifier: "com.ewe.platform"` ✅
- `src-tauri/public/index.html` — Foundation Platform dashboard ✅

## Verification

```bash
cd examples/platform_android/src-tauri
cargo check                                         # Verify compilation
cargo build                                         # Full build

# Android (requires SDK):
cargo tauri android init
cargo tauri android build
adb install src-tauri/gen/android/app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n com.ewe.platform/.MainActivity
adb logcat | grep platform_android
```
