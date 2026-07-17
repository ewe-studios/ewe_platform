---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F11-entrypoint-codegen"
this_file: "specifications/52-tauri-foundation-platform/features/F11-entrypoint-codegen/feature.md"

status: pending
priority: medium
created: 2026-07-17

depends_on:
  - "F00-crate-skeleton"
  - "F01-session-backbone"

tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# F11 — Entrypoint annotations and build pipeline

## Overview

`#[platform_bin]` source parser (runs in `build.rs` via
`foundation_platform::generate_platform_code()`) that discovers web-side
annotations and triggers code generation. `#[wasm_app]` + `foundation_wasmtime`
are post-MVP per [decision 13](../decisions/13-wasm-app-entrypoint.md).

[Decision 04](../decisions/04-deployment-surfaces.md) entrypoint taxonomy.

---

### A.1 — `build.rs` pipeline

```rust
// User's build.rs:
fn main() { foundation_platform::generate_platform_code(); }
```

Scanner discovers: `#[wasm_bin]`, `#[wasm_worker]`, `#[wasm_service]` →
delegates to existing `WasmBundleGenerator`. `#[platform_worker]`,
`#[platform_service]` → generates native stubs.

### A.2 — `#[platform_bin]` — mandatory

```rust
#[platform_bin]
fn main(session: PlatformSession) {
    session.route("/app/*", webview_app());
    session.route("/remote/*", remote_fetch());
}
```

---

## Verification
```bash
cargo build --package platform_demo
```
