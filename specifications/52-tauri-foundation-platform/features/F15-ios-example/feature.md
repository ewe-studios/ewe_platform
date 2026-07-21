---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F15-ios-example"
this_file: "specifications/52-tauri-foundation-platform/features/F15-ios-example/feature.md"

status: pending
priority: critical
created: 2026-07-18

depends_on:
  - "F13-cross-platform-builds"

tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
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
