---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F33-wasmtime-shell"
this_file: "specifications/52-tauri-foundation-platform/features/F33-wasmtime-shell/feature.md"

status: completed
priority: high
created: 2026-07-21

depends_on:
  - "F19-wasm-annotation-target"

tasks:
  completed: 0
  uncompleted: 12
  total: 12
  completion_percentage: 0%
---

# F33 — Wasmtime Shell: `#[wasm_app]` + `foundation_wasmtime`

## Problem

Surfaces 1 (WebView WASM) and 3 (wasmtime WASM) are both WASM, but they
target different runtimes and produce different glue code. Today we only
support Surface 1. Surface 3 (decision 04, 13) lets WASM run in-process
alongside the Tauri shell using wasmtime — zero-copy Arrow IPC, session
access via imports, no WebView overhead.

Decision 13 defined a `foundation_wasmtime` crate and `WasmtimeBuilder`,
but neither exists yet. The codegen knows how to build WebView WASM apps
(`build_wasm_app` → `wasm32-unknown-unknown`) but not wasmtime apps.

## Solution

Three layers, all following the EXACT pattern of today's WebView pipeline:

> **Resolved 2026-07-23 during F40 implementation.** The module is NOW bundled in
> `public/{app_id}/v{version}/` alongside every other app (F40) and loaded at
> **runtime** through `PlatformAssetManager::read_app_file`, not `include_bytes!`.
> An embedded module needs a native rebuild and store release to change — the
> thing F40 exists to remove. See the verification block below for the actual
> generated code.

### Layer 1: `#[wasm_app]` proc macro (foundation_macros)

A compile-time marker in `foundation_macros/src/wasm_modes.rs`, exposed
as `foundation_macros::wasm_app`. Same shape as `#[wasm_bin]` — validates
the attribute syntax (`extern`, `desc`, `js`, etc.) and passes the item
through unchanged. The `extern = "true"` key auto-generates `#[no_mangle]
pub extern "C"`. The marker is what the build scanner reads.

```rust
// In the app crate:
use foundation_macros::wasm_app;

#[wasm_app(extern = "true")]
fn init() {
    // Surface 3 entrypoint
}
```

### Layer 2: `build_wasmtime_app()` in codegen

Called **explicitly** from the project's root `build.rs` — no annotation
gate on explicit calls. Auto-discovery (`generate_platform_code`) still
filters on `#[wasm_app]` so it does not compile every `app-shell*`
directory it finds.

Compiles the crate to `wasm32-wasip1`, resolves the cargo package name
(which may differ from the directory — `app-shell/` holds package
`app_shell`), and copies the artefact to the caller's chosen output
directory. Panics if the artefact is missing rather than silently
skipping.

### Layer 3: `generate_wasmtime_modules()` in codegen

Called by `generate_platform_code()`. Generates a Rust module that loads
the module **at runtime** through the asset manager:

```rust
// Generated in src/generated/shell/app_shell.rs
// Route prefix: /shell/
//
// The module is bundled at public/app-shell/v0.1.0/app-shell.wasm
// and loaded at runtime through the asset manager.

use foundation_platform::PlatformSession;
use foundation_wasmtime::WasmtimeBuilder;

pub const APP_ID: &str = "app-shell";
pub const MODULE_FILE: &str = "app-shell.wasm";

/// Load this shell's module for the app's currently active version.
/// Returns `None` when the module is absent.
pub fn builder(session: &PlatformSession) -> Option<WasmtimeBuilder> {
    let manager = session.asset_manager()?;
    let bytes = manager.read_app_file(APP_ID, MODULE_FILE).ok()?;
    Some(WasmtimeBuilder::new(bytes).with_name(APP_ID))
}
```

The module ships INSIDE `public/app-shell/v{version}/` (F40), so Tauri
bundles it via `bundle.resources` and an OTA can replace it.

### Layer 3: `foundation_wasmtime` crate

New crate at `backends/foundation_wasmtime/`. Thin wrapper around the
`wasmtime` crate (already a workspace dep somewhere in llama.cpp infra).

```rust
// foundation_wasmtime/src/lib.rs

use wasmtime::{Engine, Instance, Module, Store, Linker};

pub struct WasmtimeBuilder {
    wasm_bytes: Vec<u8>,
    name: String,
    session_imports: Vec<SessionImport>,
}

impl WasmtimeBuilder {
    pub fn new(wasm_bytes: impl Into<Vec<u8>>) -> Self { ... }
    pub fn with_name(mut self, name: &str) -> Self { ... }

    /// Register a session method as a WASM import.
    /// e.g. .with_session_import("ewe_session", "resolve_route")
    pub fn with_session_import(mut self, module: &str, name: &str) -> Self { ... }

    /// Build the Engine + Module + Instance. The instance is ready to call.
    pub fn build(self) -> Result<WasmtimeInstance, WasmtimeError> { ... }
}

pub struct WasmtimeInstance {
    engine: Engine,
    store: Store<SessionState>,
    instance: Instance,
}

impl WasmtimeInstance {
    /// Call a WASM export by name.
    pub fn call(&mut self, func: &str, params: &[WasmValue]) -> Result<Vec<WasmValue>, WasmtimeError> { ... }
}
```

### Integration with root build.rs (F40 compliance)

Surface 3 apps share the build-declaration pattern of every other app.
Each crate's version comes from its own `Cargo.toml`, so two apps at
different versions naturally land at different directories — no shared
version file, no collision.

```rust
// examples/platform_android/build.rs (root)
fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let src_tauri = root.join("src-tauri");
    let public_dir = src_tauri.join("public");

    // Surface 2: WebView WASM
    for app_id in ["app", "app-hello"] {
        let app_dir = root.join(app_id);
        if app_dir.join("Cargo.toml").exists() {
            let version = codegen::crate_version(&app_dir);
            codegen::build_wasm_app(
                &app_dir,
                &codegen::app_version_dir(&public_dir, app_id, &version),
            );
        }
    }

    // Surface 3: wasmtime WASM — same shape, different loader
    for app_id in ["app-shell"] {
        let shell_dir = root.join(app_id);
        if shell_dir.join("Cargo.toml").exists() {
            let version = codegen::crate_version(&shell_dir);
            codegen::build_wasmtime_app(
                &shell_dir,
                &codegen::app_version_dir(&public_dir, app_id, &version),
            );
        }
    }

    // Derive bundle.resources from what was actually built.
    codegen::sync_bundle_resources(&src_tauri);
}
```

And in `setup_routes()`:

```rust
fn setup_routes(session: &PlatformSession) {
    // ... existing routes ...

    // The module loads at RUNTIME through the asset manager, so the version
    // directory follows whichever version is currently active — a shell OTA
    // takes effect on the next request without a native rebuild.
    if let Some(builder) = generated::shell::app_shell::builder(session) {
        session.register_wasmtime("shell", builder);
        session.route("/shell/*", wasmtime_app("shell"));
    }
}
```

## Requirements

### R1. `build_wasmtime_app()` — codegen.rs
- Scans for `#[wasm_app]` annotations (new AnnotationKind)
- Compiles crate to `wasm32-wasip1`
- Copies `.wasm` to output directory
- Prints `cargo:rerun-if-changed` directives

### R2. `generate_wasmtime_modules()` — codegen.rs
- Discovers `app-shell/` and `app-shell-*/` dirs from project root
- Generates `src/generated/shell/{name}.rs` with `fn builder() -> WasmtimeBuilder`
- Uses `include_bytes!` to embed the compiled `.wasm`
- Adds `pub mod shell` to `src/generated/mod.rs`

### R3. `foundation_wasmtime` crate — NEW
- `WasmtimeBuilder` with `new()`, `with_name()`, `with_session_import()`, `build()`
- `WasmtimeInstance` wrapping Engine + Store + Instance
- Session bridge: host functions as WASM imports
- No Tauri dependency — pure wasmtime + foundation types
- Depends on `wasmtime` crate (check workspace for version)

### R4. Root build.rs integration
- User calls `build_wasmtime_app()` in root build.rs alongside `build_wasm_app()`
- Separate output directories: `public/` (WebView) vs `shell_wasm/` (wasmtime)
- Non-breaking: existing projects don't need to change anything

### R5. Session registration
- `PlatformSession::register_wasmtime(name, builder)` stores the builder
- `wasmtime_app(name)` creates a `RouteDecision` with source = WasmtimeShell
- `WasmtimeResponder` implements `RouteResponder` — builds instance, calls handler, returns response

### R6. WASM imports (session bridge)
- `ewe_session::resolve_route(url) -> json` — calls session.route_handlers from WASM
- `ewe_session::http_fetch(url) -> bytes` — calls session.http_backend from WASM
- `ewe_session::emit(event, payload)` — calls session.emit from WASM
- `ewe_session::now() -> u64` — returns current timestamp

## Verification

```bash
# Build the crate
cargo check -p foundation_wasmtime

# Build a wasmtime app
cargo build --manifest-path examples/platform_android/Cargo.toml

# Integration: wasmtime app responds to ewe:// routes
cargo test -p foundation_wasmtime -- wasmtime_instance

# End-to-end: #[wasm_app] function served via wasmtime
```

## Files

| File | Action |
|------|--------|
| `backends/foundation_wasmtime/Cargo.toml` | **NEW** — crate manifest |
| `backends/foundation_wasmtime/src/lib.rs` | **NEW** — WasmtimeBuilder, WasmtimeInstance |
| `backends/foundation_wasmtime/src/session_bridge.rs` | **NEW** — host function imports |
| `backends/foundation_platform/src/codegen.rs` | Add `build_wasmtime_app()` + `generate_wasmtime_modules()` |
| `backends/foundation_platform/src/session.rs` | Add `register_wasmtime()` + `WasmtimeResponder` |
| `backends/foundation_platform/src/route.rs` | Add `wasmtime_app()` route constructor |
| `examples/platform_android/build.rs` | Add `build_wasmtime_app()` call (example) |
