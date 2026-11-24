---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F12-mvp-integration"
this_file: "specifications/52-tauri-foundation-platform/features/F12-mvp-integration/feature.md"

status: completed
priority: critical
created: 2026-07-17
updated: 2026-07-21

depends_on:
  - "F09-walking-skeleton"
  - "F10-testing-harness"
  - "F11-entrypoint-codegen"

tasks:
  completed: 5
  uncompleted: 0
  total: 5
  completion_percentage: 100%
---

# F12 — MVP integration and demo app

## Overview

Ship a working `platform_demo` Tauri app exercising all MVP capabilities:
boot, routes (WebviewApp + RemoteServer + CacheFirst), navigation
(push/pop/morph), cache, profiles, capabilities, and codegen pipeline.

### Demo app route set

```rust
#[platform_bin]
fn main(session: PlatformSession) {
    session.route("/app/*", webview_app().with_profile(Profile::App));
    session.route("/remote/*", remote_fetch()
        .with_profile(Profile::TrustedRemote)
        .with_cache_policy(CachePolicy::NetworkFirst));
    session.route("/cached/*", remote_fetch()
        .with_cache_policy(CachePolicy::CacheFirst));
    session.route("/auth/*", remote_fetch().with_profile(Profile::Auth));
    session.register_capability::<CameraCapability>();
}
```

### MVP checklist

| Feature | Must work |
|---|---|
| Platform boots via Tauri | Demo app launches |
| Route handler chain resolves | Navigate to /app, /remote, /cached |
| ewe:// custom protocol | URLs resolve through ewe:// |
| Profiles enforced | /remote/embed UntrustedRemote, no camera |
| Single-WebView stack | Push/pop with screenshot |
| Cache tiers (SQLite) | /cached routes serve offline |
| LWW mutation queue | Offline actions replayed |
| #[platform_bin] codegen | build.rs generates, app compiles |

## Verification
```bash
cargo build --package platform_demo
cargo run --example platform_demo
```
