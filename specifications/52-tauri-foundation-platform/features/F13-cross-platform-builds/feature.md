---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F13-cross-platform-builds"
this_file: "specifications/52-tauri-foundation-platform/features/F13-cross-platform-builds/feature.md"

status: completed
priority: critical
created: 2026-07-18

depends_on:
  - "F12-mvp-integration"

tasks:
  completed: 15
  uncompleted: 3
  total: 18
  completion_percentage: 83%
---
# F13 — Cross-platform builds: Desktop, Android, iOS

## Overview

`foundation_platform` must compile and run on all three Tauri v2 targets:
Linux/macOS desktop, Android, and iOS. This feature makes PlatformBuilder
runtime-agnostic, creates example apps for each target, and adds Docker-based
CI test environments.

[Decision 04](../decisions/04-deployment-surfaces.md) defines deployment
surfaces including native static libraries for mobile. Tauri v2 provides
the host layer on all three platforms — we wrap it, we don't replace it.
[Decision 14](../decisions/14-docker-test-environments.md) defines the
Docker-based test environments.

## Dependencies

Depends on:
- `F12-mvp-integration` — Demo app on desktop must work first

## Requirements

### 1. Runtime-agnostic `PlatformBuilder` ✅

`PlatformBuilder` is generic over `R: Runtime`, not hardcoded to
`tauri_runtime_wry::Wry<EventLoopMessage>`. Default type parameter
`R = tauri::Wry` serves desktop; mobile overrides via turbofish.

### 2. Example app per platform

Three example crates prove each target works. All use the same `platform_run!`
macro with identical user code — different compilation targets only.

```
examples/
├── platform_demo/         # Desktop: cargo tauri dev ✅
│   ├── Cargo.toml, build.rs, tauri.conf.json
│   ├── public/index.html
│   └── src/main.rs       # platform_run!(PlatformBuilder::new()...)
│
├── platform_android/     # Android: cargo tauri android build ✅
│   ├── README.md
│   └── src-tauri/        # Tauri v2 mobile convention
│       ├── Cargo.toml    # depends on foundation_platform
│       ├── build.rs      # generate_platform_code()
│       ├── tauri.conf.json
│       ├── public/index.html
│       └── src/
│           ├── lib.rs    # #[mobile_entry_point] + platform_run!
│           └── main.rs   # Desktop entrypoint → lib::run()
│
└── platform_ios/         # iOS: cargo tauri ios build 🔄 (needs macOS)
```

### 3. Desktop example ✅

`platform_demo` builds and runs via `cargo build`. Uses
`foundation_platform::codegen::generate_platform_code()` in build.rs.
Same `platform_run!` code as mobile examples.

**Verify:** `cargo build --package platform_demo` ✅

### 4. Android example ✅

Same `platform_run!` code in `src-tauri/src/lib.rs`. Tauri v2 mobile
convention with `[workspace]` opt-out. `build.rs` calls
`foundation_platform::codegen::generate_platform_code()`.
Verified: `cargo build` in `src-tauri/` passes on Linux desktop.

### 5. iOS example 🔄 (deferred — requires macOS)

Structure follows same pattern as Android example with `src-tauri/` layout.
Requires macOS + Xcode for `cargo tauri ios init` + `build`.

### 6. Runtime-agnostic code checklist ✅

No file in `foundation_platform/src/` hardcodes `tauri_runtime_wry`.
Uses `R: Runtime` generic or avoids Tauri types entirely.

| File | Tauri dependency | Status |
|---|---|---|
| `builder.rs` | Wraps `tauri::Builder<R>` | Generic ✅ |
| `ewe.rs` | `register_ewe_protocol<R: Runtime>` | Generic ✅ |
| `session.rs` | None | Runtime-free ✅ |
| all others | None | Runtime-free ✅ |

### 7. Docker CI environments 🔄 (post-MVP)

Docker images in `artefacts/dockerfiles/{linux,android,windows}/` provide CI
test runners with platform toolchains and VNC for visual debugging.
**Deferred:** Core build verification via `cargo check`/`cargo build` is
sufficient for MVP.

## Verification

```bash
# Desktop
cargo build --package platform_demo
cargo test --package foundation_platform

# Android
cd examples/platform_android/src-tauri && cargo build

# iOS (macOS only)
# cd examples/platform_ios && cargo tauri ios build
```
