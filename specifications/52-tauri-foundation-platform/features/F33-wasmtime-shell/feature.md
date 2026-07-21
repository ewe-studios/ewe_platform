---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F33-wasmtime-shell"
this_file: "specifications/52-tauri-foundation-platform/features/F33-wasmtime-shell/feature.md"

status: pending
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

### Layer 1: `build_wasmtime_app()` in codegen

Same shape as `build_wasm_app()` — compiles a crate to wasm32-wasip1,
copies the `.wasm` to `shell_wasm/{name}.wasm`, copies wasmtime-compatible
JS shims if needed.

```rust
// foundation_platform/src/codegen.rs — NEW function

/// Build a single WASM app for wasmtime (surface 3).
/// Compiles to wasm32-wasip1, outputs to shell_wasm/{name}.wasm.
pub fn build_wasmtime_app(app_dir: &Path, out_dir: &Path) {
    // Scan for #[wasm_app] annotations
    let wasm: Vec<_> = scan_for_annotations(&app_dir.join("src"))
        .into_iter()
        .filter(|a| matches!(a.kind, AnnotationKind::WasmApp))
        .collect();
    if wasm.is_empty() { return; }

    // Compile to wasm32-wasip1 (not wasm32-unknown-unknown!)
    let status = std::process::Command::new("cargo")
        .args(["build", "--target", "wasm32-wasip1", "--release"])
        .current_dir(app_dir)
        .status()
        .expect("cargo build failed for wasmtime app");

    // Copy .wasm to output
    std::fs::create_dir_all(out_dir).ok();
    let wasm_src = app_dir.join("target/wasm32-wasip1/release")
        .join(format!("{}.wasm", app_dir.file_name().unwrap().to_str().unwrap()));
    let wasm_dst = out_dir.join(format!("{}.wasm", app_dir.file_name().unwrap().to_str().unwrap()));
    std::fs::copy(&wasm_src, &wasm_dst).ok();
}
```

### Layer 2: `generate_wasmtime_modules()` in codegen

Called by `generate_platform_code()`. Discovers `app-shell/` and
`app-shell-*/` dirs (same pattern as `app/` and `app-*/`).

Generates a Rust module that wraps the compiled `.wasm` in a
`WasmtimeBuilder`:

```rust
// Generated in src/generated/shell/app_shell.rs

use foundation_wasmtime::WasmtimeBuilder;

/// Returns a WasmtimeBuilder pre-loaded with the compiled WASM bytes.
/// Call session.register_wasmtime("shell", builder) in setup_routes().
pub fn builder() -> WasmtimeBuilder {
    let wasm_bytes: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/shell_wasm/app_shell.wasm"
    ));
    WasmtimeBuilder::new(wasm_bytes)
        .with_name("app_shell")
}
```

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

### Integration with root build.rs

The user adds ONE line:

```rust
// examples/platform_android/build.rs (root)
fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let public_dir = root.join("src-tauri").join("public");

    // Surface 1: WebView WASM (existing)
    foundation_platform::codegen::build_wasm_app(&root.join("app"), &public_dir.join("app"));

    // Surface 3: wasmtime WASM (NEW)
    let shell_dir = root.join("src-tauri").join("shell_wasm");
    foundation_platform::codegen::build_wasmtime_app(&root.join("app-shell"), &shell_dir);
}
```

And in `setup_routes()`:

```rust
fn setup_routes(session: &PlatformSession) {
    // ... existing routes ...

    // Register the wasmtime-backed app
    let builder = generated::shell::app_shell::builder();
    session.register_wasmtime("shell", builder);
    session.route("/shell/*", wasmtime_app("shell"));
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
