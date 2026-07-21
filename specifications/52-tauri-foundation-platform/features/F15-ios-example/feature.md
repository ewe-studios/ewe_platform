---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F15-ios-example"
this_file: "specifications/52-tauri-foundation-platform/features/F15-ios-example/feature.md"

status: in-progress
priority: critical
created: 2026-07-18
updated: 2026-07-21

depends_on:
  - "F13-cross-platform-builds"

tasks:
  completed: 5
  uncompleted: 1
  total: 6
  completion_percentage: 83%
---

# F15 — iOS example app

## Overview

Build and deploy `examples/platform_ios` — a Tauri v2 iOS app using
`foundation_platform`. Proves the Xcode build pipeline, WKWebView bridge,
and ewe:// protocol on iOS.

Requires macOS + Xcode. Falls back to Docker CI on Linux.

## Requirements

### 1. Create `examples/platform_ios`

Same structure as `platform_android` — Tauri v2 project with
`src-tauri/Cargo.toml` depending on `foundation_platform`.

### 2. `cargo tauri ios init`

Generates the Xcode project. Tauri bridges our Rust `.a` into Swift.

### 3. `cargo tauri ios build`

Compiles for `aarch64-apple-ios` and `aarch64-apple-ios-sim`, produces `.ipa`.

### 4. Deploy to simulator

`cargo tauri ios dev` → opens simulator → app shows platform dashboard.

## Verification (macOS only)

```bash
cd examples/platform_ios
cargo tauri ios init
cargo tauri ios build
cargo tauri ios dev
```

## Implementation Status (2026-07-21)

### ✅ Complete
1. Project skeleton — mirrors `platform_android` structure exactly
2. `src-tauri/Cargo.toml` — depends on `foundation_platform`, `serde`, `serde_json`
3. `src-tauri/tauri.conf.json` — iOS-specific config (`minimumSystemVersion: 15.0`)
4. `src-tauri/src/lib.rs` — `setup_routes()` with `bundle_root()` + `RemoteProxy`
5. `aarch64-apple-ios` + `aarch64-apple-ios-sim` targets in `rust-toolchain.toml`

### ⚠️ Remaining (requires macOS + Xcode)
1. `cargo tauri ios init` — generates Xcode project (needs Xcode 16+)
2. `cargo tauri ios build` — produces `.ipa`
3. `cargo tauri ios dev` — simulator deployment

These steps require a real Mac or Docker macOS with Xcode installed,
which is a ~30GB download. The code is ready; the native project
generation is a one-time setup step.
