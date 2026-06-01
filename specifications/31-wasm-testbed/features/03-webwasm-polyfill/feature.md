---
feature: "foundation_webwasm - WASM API Polyfill Crate"
description: "Central crate for polyfilling std APIs (Instant, SystemTime, etc.) on wasm32-unknown-unknown with std::time re-exports on native targets"
status: "in_progress"
priority: "high"
depends_on: []
estimated_effort: "medium"
created: 2026-06-01
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0%
---

# foundation_webwasm Feature

## Overview

Create `foundation_webwasm` — a workspace-level foundation crate that provides polyfilled `std` API equivalents for wasm32 targets. It mirrors the pattern used in `foundation_nostd` for conditional compilation: gate the implementation by target arch and feature flags, re-export `std` on native targets, and use JS-backed implementations on wasm.

## Problem

`std::time::Instant::now()` and `std::time::SystemTime::now()` panic on `wasm32-unknown-unknown` because the platform has no native time API. Tests and production code that need timing (e.g., the wasm_js_yield_integration tests) fail at runtime. Rather than adding `web-time` as an external dependency per-crate, we centralize this polyfill in one foundation crate so all other crates can depend on it uniformly.

## Solution

A new crate `backends/foundation_webwasm/` with:

### Conditional module structure

```
backends/foundation_webwasm/src/
  lib.rs          — gate: wasm32 polyfill vs std re-export
  std/            — native target: re-export std::time types (empty/pass-through)
    mod.rs
  wasm/           — wasm32-unknown-unknown: JS-backed polyfill via wasm-bindgen
    mod.rs
    instant.rs    — Instant using Performance.now()
    system_time.rs — SystemTime using Date.now()
```

### lib.rs pattern

```rust
#[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
mod wasm;
#[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
pub use wasm::*;

#[cfg(not(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none"))))]
mod std;
#[cfg(not(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none"))))]
pub use std::*;
```

### What to polyfill (replicate web-time's implementation)

- `Instant` — backed by `Performance.now()` via wasm-bindgen FFI
- `SystemTime` — backed by `js_sys::Date::now()`
- Re-export `std::time::Duration`, `UNIX_EPOCH`, etc. (already available on wasm32)
- `SystemTimeError` type

The JS binding code mirrors web-time:

```rust
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
extern "C" {
    type Global;
    fn performance(this: &Global) -> JsValue;
    type Performance;
    fn now(this: &Performance) -> f64;
}

thread_local! {
    static PERFORMANCE: Performance = { /* init from js_sys::global() */ };
}
```

### Features

- `std` — enables `std` feature on dependencies (mirrors web-time's pattern)
- Default: no features (works in `no_std` wasm)

### Dependencies (wasm32 only)

- `wasm-bindgen` — for JS FFI bindings
- `js-sys` — for `Date::now()` in `SystemTime`

On non-wasm32 targets: zero dependencies (just re-exports `std::time`).

## Implementation Steps

### 1. Create crate skeleton

- `backends/foundation_webwasm/Cargo.toml`
- `backends/foundation_webwasm/src/lib.rs`
- `backends/foundation_webwasm/src/std/mod.rs`
- `backends/foundation_webwasm/src/wasm/mod.rs`
- `backends/foundation_webwasm/src/wasm/instant.rs`
- `backends/foundation_webwasm/src/wasm/system_time.rs`

### 2. Register in workspace

Add to `Cargo.toml` workspace members and `[workspace.dependencies]`:
```
foundation_webwasm = { path = "./backends/foundation_webwasm", version = "0.1.0" }
```

### 3. Wire into foundation_core

Replace `web-time = "1.1"` dev-dependency in `foundation_core/Cargo.toml` with:
```
foundation_webwasm = { workspace = true }
```
Update test imports: `use web_time::Instant` → `use foundation_webwasm::Instant`.

### 4. Update wasm_js_yield_integration.rs

Change `use web_time::Instant` → `use foundation_webwasm::Instant` and rerun tests.

## Verification

```
cargo run -p foundation_wasm_testbed -- test bindgen-deno ./backends/foundation_core --features js-wasmbindgen
```
Expected: all 4 wasm_js_yield_integration tests pass.

Also verify native tests still compile:
```
cargo test -p foundation_core -- --test-threads=1
```
