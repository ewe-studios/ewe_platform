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
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# F11 — Entrypoint annotations and build pipeline

## Overview

Implement `#[platform_bin]` as a source parser (in `build.rs`) that discovers
web-side (`#[wasm_bin]`, `#[wasm_worker]`, `#[wasm_service]`) and platform-side
(`#[platform_worker]`, `#[platform_service]`) annotations. Triggers
`WasmBundleGenerator` for web-side codegen. `#[wasm_app]` and
`foundation_wasmtime` are post-MVP.

[Decision 04](../decisions/04-deployment-surfaces.md) defines the entrypoint
taxonomy. The `#[platform_bin]` source parser is the code generation
orchestrator.

## Dependencies

Depends on:
- `F00-crate-skeleton` — Build pipeline lives in foundation_platform
- `F01-session-backbone` — Entrypoint registrations wire into session

## Requirements

### 1. `#[platform_bin]` — mandatory native entrypoint

```rust
// User's src/main.rs
#[platform_bin]
fn main(session: PlatformSession) {
    session.route("/app/*", RouteDecision::webview_app());
    session.route("/remote/*", RouteDecision::remote_fetch()
        .with_cache_policy(CachePolicy::NetworkFirst));
    session.register_capability::<CameraCapability>();
}
```

The `#[platform_bin]` source parser (runs in `build.rs`) discovers all
annotations in the crate and orchestrates code generation.

### 2. `build.rs` pipeline

```rust
// User's build.rs — one-liner:
fn main() {
    foundation_platform::generate_platform_code();
}
```

`generate_platform_code()`:
1. Scans `src/` for platform annotations
2. For web-side annotations (`#[wasm_bin]`, `#[wasm_worker]`, `#[wasm_service]`):
   calls `WasmBundleGenerator` to create `src/bin/*.rs`, compile to
   `wasm32-unknown-unknown`, generate JS wrappers, place in webview assets
3. For platform-side annotations (`#[platform_worker]`, `#[platform_service]`):
   generates native stubs, compiles for target platform
4. `#[wasm_app]` support is post-MVP (see [decision 13](../decisions/13-wasm-app-entrypoint.md))

### 3. `#[wasm_bin]` — frontend entrypoint (web-side, existing)

Runs in the WebView exactly as it does now. The source parser discovers it
and delegates to existing `WasmBundleGenerator`. No change from current
`foundation_wasm_ui` behavior.

### 4. `#[platform_worker]` — background thread (platform-side)

```rust
#[platform_worker]
fn cache_warmer(session: PlatformSession, rx: WorkerReceiver<CacheTask>) {
    for task in rx {
        let content = prefetch(task.url).await;
        session.cache().store(&task.route, &content);
    }
}
```

### 5. `#[platform_service]` — in-process request handler (platform-side)

```rust
#[platform_service(routes = ["/api/data", "/api/sync"])]
fn backend_service(req: PlatformRequest, session: PlatformSession) -> PlatformResponse {
    match req.method() {
        Method::Get => query_data(&req, &session),
        Method::Post => handle_action(&req, &session),
        _ => PlatformResponse::method_not_allowed(),
    }
}
```

## Tasks

### Source parser
- [ ] Implement `generate_platform_code()` in `foundation_platform/src/build.rs`
- [ ] Scan `src/` directory for platform annotations (`syn`-based scanning)
- [ ] Discover `#[wasm_bin]`, `#[wasm_worker]`, `#[wasm_service]`
- [ ] Delegate to existing `WasmBundleGenerator` for web-side codegen
- [ ] Discover `#[platform_worker]`, `#[platform_service]`
- [ ] Generate native stubs for platform-side annotations

### Web-side integration
- [ ] Ensure existing `#[wasm_bin]` pipeline works unchanged
- [ ] Verify JS wrappers are in webview asset directory
- [ ] Test: `#[wasm_bin]` WASM loads and renders in WebView

### Platform-side annotations
- [ ] Implement `#[platform_worker]` annotation → native thread spawning
- [ ] Implement `#[platform_service]` annotation → in-process router
- [ ] Typed channels: `WorkerSender`/`WorkerReceiver`

### build.rs integration
- [ ] User's `build.rs` calls `generate_platform_code()`
- [ ] Test: `cargo build` triggers code generation
- [ ] Generated code is functional

## Verification Commands

```bash
cargo build --package platform_demo
cargo test --package foundation_platform -- entrypoint
```
