---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F19-wasm-annotation-target"
this_file: "specifications/52-tauri-foundation-platform/features/F19-wasm-annotation-target/feature.md"

status: completed
priority: high
created: 2026-07-18

depends_on:
  - "F16-app-crate-structure"
  - "F17-app-build-pipeline"
  - "F18-backend-transport"

tasks:
  completed: 2
  uncompleted: 8
  total: 10
  completion_percentage: 20%
---
# F19 — WASM annotation target resolution + platform scheme interceptor

## Overview

`#[wasm_bin]` / `#[wasm_worker]` / `#[wasm_service]` produce wasm32
binaries. Each annotation supports `target = "..."` to override the
compilation target. The build pipeline resolves the target, compiles
the `app/` crate, generates JS wrappers, and copies runtimes.

Additionally, the platform scheme interceptor (JS polyfill) ensures
`ewe://`, `foundation://`, `platform://` links work on Android by
rewriting them to `http://*.localhost/` — the format wry's
`shouldInterceptRequest` recognizes.

## Requirements

### 1. Annotation target parameter

```rust
#[wasm_bin]                         // default: wasm32-unknown-unknown
#[wasm_bin(target = "wasip1")]      // wasm32-wasip1 (wasmtime)
#[wasm_bin(target = "wasip2")]      // wasm32-wasip2 (wasmtime v2)
#[wasm_worker]                      // wasm32-unknown-unknown (web worker)
#[wasm_service(routes = ["/api/*"])] // wasm32-unknown-unknown (service worker)
```

### 2. Target resolution rules

- All entrypoints in one crate share one target (one `cargo build` invocation)
- Default: `wasm32-unknown-unknown`
- If any annotation specifies `wasip1`/`wasip2`, that wins
- If annotations with conflicting targets exist → build error

### 3. Generated JS wrappers ✅ (in root build.rs)

| Annotation | JS file | Template |
|---|---|---|
| `#[wasm_bin]` | `{name}.js` | Imports runtimes, fetches .wasm, instantiates, calls entrypoint |
| `#[wasm_worker]` | `{name}.js` | Self-invoking async IIFE, `postMessage({ready: true})` |
| `#[wasm_service]` | `{name}.js` | `importScripts`, `skipWaiting`, `claim` |

### 4. Platform scheme interceptor ✅

`runtimes/platform-scheme-interceptor.js`:
- Intercepts `ewe://`, `foundation://`, `platform://` link clicks
- Intercepts `location.href =`, `location.assign()`, `location.replace()`
- Intercepts `window.open()`
- Rewrites to `http://{scheme}.localhost/...`
- Active only on Android (`/android/i.test(navigator.userAgent)`)
- Injected into ALL generated bin/worker/service wrappers
- Also injected by `PlatformBuilder::build()` via `window.eval()`

### 5. HTML generation ✅

Root build.rs generates `index.html` with:
- Standard `<style>` block
- `<script type=module>` imports for each `#[wasm_bin]`
- Auto-init calling `init()` on each wrapper module
- Status div showing loading/progress/error

### 6. No hand-authored HTML required

Users never write `public/index.html`. The build pipeline
generates everything. If a user wants custom HTML, they provide
a template; the pipeline injects script tags.

## Verification

```bash
# Verify wasm compilation with target override:
grep 'target.*wasip1' app/src/lib.rs
cargo build --manifest-path app/Cargo.toml --target wasm32-wasip1 --profile uat

# Verify interceptor is included:
grep 'platform-scheme-interceptor' app/src/lib.rs
ls src-tauri/public/platform-scheme-interceptor.js

# Verify generated HTML has script imports:
grep '<script type=module' src-tauri/public/index.html
```
