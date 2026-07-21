---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F17-app-build-pipeline"
this_file: "specifications/52-tauri-foundation-platform/features/F17-app-build-pipeline/feature.md"

status: completed
priority: critical
created: 2026-07-18

depends_on:
  - "F16-app-crate-structure"
  - "F11-entrypoint-codegen"

tasks:
  completed: 0
  uncompleted: 15
  total: 15
  completion_percentage: 0%
---
# F17 — App build pipeline: root build.rs

## Overview

The root `build.rs` is the single orchestrator for compiling the WASM
app, copying artifacts, and generating HTML. It lives outside `src-tauri/`
so it can operate on the `app/` crate before Tauri's own build script runs.

## Requirements

### 1. Scan `app/src/` for wasm annotations

- `#[wasm_bin]` → `WasmKind::Bin`
- `#[wasm_worker]` → `WasmKind::Worker`
- `#[wasm_service]` → `WasmKind::Service`
- Extract function name and annotation target parameter

### 2. Compile `app/` to the correct wasm32 target

Default: `wasm32-unknown-unknown` (browser/WebView).
Per-annotation override: `#[wasm_bin(target = "wasip1")]` → `wasm32-wasip1`.

```rust
fn resolve_wasm_target(eps: &[WasmEntrypoint]) -> String {
    // All entrypoints in one crate share one target.
    // First annotation with `target = "wasip1"` wins.
    // Default: wasm32-unknown-unknown.
}
```

### 3. Copy `.wasm` to `src-tauri/public/`

One crate = one `.wasm`. All JS wrappers reference the crate-name.wasm.

### 4. Copy JS runtimes from foundation crates

- `backends/foundation_wasm/runtime/foundation-wasm.js` → `public/foundation-wasm.js`
- `backends/foundation_wasm_ui/runtimes/foundation-wasm-ui.js` → `public/foundation-wasm-ui.js`
- `backends/foundation_wasm_ui/runtimes/platform-scheme-interceptor.js` → `public/platform-scheme-interceptor.js`

### 5. Generate JS wrappers per entrypoint

| Annotation | Output file | JS wrapper template |
|---|---|---|
| `#[wasm_bin]` | `{name}.js` | `bin_wrapper(name)` — imports runtimes, fetches .wasm, calls entrypoint |
| `#[wasm_worker]` | `{name}.js` | `worker_wrapper(name)` — self-invoking async IIFE, postMessage ready |
| `#[wasm_service]` | `{name}.js` | `service_wrapper(name)` — importScripts, skipWaiting, claim |

### 6. Generate `index.html`

Minimal bootstrap page that:
- Imports each `#[wasm_bin]` wrapper as a module
- Calls `init()` on each
- Shows loading status
- Passes through any WASM-generated DOM content

### 7. `rerun-if-changed` directives

```
cargo:rerun-if-changed=app/src/
cargo:rerun-if-changed=app/Cargo.toml
```

### 8. The build.rs source scanner is self-contained

No dependency on `foundation_platform` (avoids pulling Tauri into the root
package). The annotation scanner is a simple text-based parser, duplicated
from `foundation_platform::codegen` but minimal.

## Verification

```bash
cargo build                          # Root build
ls src-tauri/public/platform_android_wasm.wasm
ls src-tauri/public/platform_dashboard.js
ls src-tauri/public/bg_worker.js
ls src-tauri/public/platform_sw.js
ls src-tauri/public/index.html
```
