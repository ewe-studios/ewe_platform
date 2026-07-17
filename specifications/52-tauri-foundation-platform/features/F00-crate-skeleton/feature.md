---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F00-crate-skeleton"
this_file: "specifications/52-tauri-foundation-platform/features/F00-crate-skeleton/feature.md"

status: pending
priority: critical
created: 2026-07-17

depends_on: []

tasks:
  completed: 0
  uncompleted: 9
  total: 9
  completion_percentage: 0%
---

# F00 — Crate scaffold and shared types

## Overview

Create `foundation_platform` crate with Tauri dependency, define all shared
types in `foundation_ui_traits`, and set up the `PlatformBuilder` that wraps
`tauri::Builder`. This is the skeleton every other feature builds on.

[Decision 01](../decisions/01-platform-and-crates.md) defines the crate
boundaries. All types defined here are pure data — no I/O, no Tauri
dependency in `foundation_ui_traits`.

## Dependencies

None — this is the foundation.

Required by: every other feature.

## Requirements

### 1. Create `foundation_platform` crate

```
backends/foundation_platform/
  ├── Cargo.toml        ← depends on tauri 2, foundation_ui_traits, foundation_wasm_ui
  └── src/
      └── lib.rs        ← re-exports, PlatformBuilder
```

`Cargo.toml` dependencies (from [decision 01](../decisions/01-platform-and-crates.md)):
```toml
[dependencies]
tauri = { version = "2", features = [...] }
foundation_ui_traits = { path = "../foundation_ui_traits" }
foundation_wasm_ui = { path = "../foundation_wasm_ui" }
foundation_db = { path = "../foundation_db" }
foundation_auth = { path = "../foundation_auth" }
foundation_nativeapis = { path = "../foundation_nativeapis" }
foundation_arrow = { path = "../foundation_arrow" }
foundation_signals = { path = "../foundation_signals" }
foundation_macros = { path = "../foundation_macros" }
foundation_wasmtime = { path = "../foundation_wasmtime" }
foundation_packager = { path = "../foundation_packager" }
```

### 2. Define shared types in `foundation_ui_traits`

All types from [decision 02](../decisions/02-route-policy-model.md) that are
pure data with no platform dependency:

```rust
// foundation_ui_traits/src/platform_types.rs

/// Navigation intent intercepted by the session backbone.
struct NavigationIntent {
    url: Url,
    method: Method,
    source: IntentSource,
    referrer: Option<Url>,
}

enum IntentSource {
    LinkClick,
    FormSubmit,
    ServerPush,
    NativeGesture,
    Programmatic,
}

/// Route handler output. Pure data — the platform executes it.
struct RouteDecision {
    source: RouteSource,
    presentation: Presentation,
    view_kind: ViewKind,
    protocol: ProtocolHint,
    cache_policy: CachePolicy,
    profile: Profile,
    native_view_id: Option<String>,
    target: Option<String>,
    capabilities: Vec<CapabilityId>,
}

enum RouteSource { WebviewApp, IpcShell, RemoteServer }
enum Presentation { Morph, Push, Modal, Replace, External, Root }
enum ViewKind { WebView, Native }
enum ProtocolHint { Default, Columnar, Arrow, ArrowIpc, Json, Html }
enum CachePolicy { CacheFirst, NetworkFirst, OnlineOnly, LocalOnly, StaleWhileRevalidate }
enum Profile { App, TrustedRemote, UntrustedRemote, Auth, Devtools }
```

Also define session scoping types:
```rust
struct SessionId(/* opaque */);
struct PageIdentity { session_id: SessionId, route: String, visit_id: u64 }
```

And capability contract types:
```rust
struct CapabilityRequest { id: Uuid, page_identity: PageIdentity, capability: String, action: String, payload: serde_json::Value }
struct CapabilityResponse { id: Uuid, page_identity: PageIdentity, status: Result<serde_json::Value, String> }
```

### 3. `PlatformBuilder` wraps `tauri::Builder`

```rust
// foundation_platform/src/lib.rs
pub struct PlatformBuilder {
    inner: tauri::Builder,
}

impl PlatformBuilder {
    pub fn new() -> Self { ... }
    pub fn build(self) -> tauri::App { ... }
}
```

### 4. Protocol enum in `foundation_ui_traits`

```rust
enum Protocol { Columnar, Arrow, ArrowIpc, Json, Html, CustomBinary }
```

## Architecture

```
foundation_ui_traits (dependency-free)
  ├── NavigationIntent, RouteDecision, RouteSource
  ├── Presentation, ViewKind, ProtocolHint
  ├── CachePolicy, Profile, CapabilityId
  ├── SessionId, PageIdentity
  ├── CapabilityRequest, CapabilityResponse
  └── Protocol enum
    ↑
    |
foundation_platform (Tauri-dependent)
  ├── PlatformBuilder (wraps tauri::Builder)
  ├── PlatformSession (wraps AppHandle<R>)
  └── RouteHandler trait
```

## Tasks

### Crate creation
- [ ] Create `backends/foundation_platform/` with Cargo.toml and src/lib.rs
- [ ] Wire Cargo.toml dependencies (tauri 2 + all foundation crates)
- [ ] Add `foundation_platform` to workspace Cargo.toml members

### foundation_ui_traits types
- [ ] Add `NavigationIntent` struct with `IntentSource` enum
- [ ] Add `RouteDecision` struct with all 10 fields
- [ ] Add `RouteSource` enum (WebviewApp, IpcShell, RemoteServer)
- [ ] Add `Presentation` enum (Morph, Push, Modal, Replace, External, Root)
- [ ] Add `ViewKind` enum (WebView, Native) with post-MVP annotation
- [ ] Add `ProtocolHint` enum (Default, Columnar, Arrow, ArrowIpc, Json, Html)
- [ ] Add `CachePolicy` enum (CacheFirst, NetworkFirst, OnlineOnly, LocalOnly, StaleWhileRevalidate)
- [ ] Add `Profile` enum (App, TrustedRemote, UntrustedRemote, Auth, Devtools)
- [ ] Add `SessionId`, `PageIdentity` types
- [ ] Add `CapabilityRequest` and `CapabilityResponse` types
- [ ] Add `Protocol` enum (Columnar, Arrow, ArrowIpc, Json, Html, CustomBinary)
- [ ] Add `CapabilityId` newtype or identifier
- [ ] Test: types compile for both native and wasm32 targets

### PlatformBuilder
- [ ] Implement `PlatformBuilder::new()` wrapping `tauri::Builder::default()`
- [ ] Implement `PlatformBuilder::build()` that sets up Tauri app
- [ ] Test: `PlatformBuilder::new()` compiles

### RouteDecision convenience constructors
- [ ] `RouteDecision::webview_app()` — source=WebviewApp, view_kind=WebView
- [ ] `RouteDecision::ipc_shell()` — source=IpcShell, target=None
- [ ] `RouteDecision::ipc_shell_with(name)` — source=IpcShell, target=Some(name)
- [ ] `RouteDecision::remote_fetch()` — source=RemoteServer

## Verification Commands

```bash
cargo check --package foundation_ui_traits
cargo check --package foundation_ui_traits --target wasm32-unknown-unknown
cargo check --package foundation_platform
cargo test --package foundation_ui_traits
```
