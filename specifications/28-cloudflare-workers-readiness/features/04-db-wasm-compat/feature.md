---
feature: "Database Wasm Compatibility"
description: "Gate turso/libsql embedded backends for wasm, enable D1/R2 wasm bindings via wasm-bindgen to Cloudflare JS APIs"
status: "pending"
priority: "high"
depends_on: ["01-core-wasm-compat"]
estimated_effort: "medium"
created: 2026-05-15
last_updated: 2026-05-15
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# Feature: Database Wasm Compatibility

## Overview

`foundation_db` has multiple backends with different wasm compatibility profiles. On wasm32-unknown-unknown (Cloudflare Workers):
- **Turso/libsql embedded** — requires filesystem/WASI, not available on Workers
- **Turso remote (HTTP)** — works on wasm (HTTP-based)
- **D1** — two modes: REST API (works), JS bindings (preferred via wasm-bindgen)
- **R2** — two modes: REST API (works), JS bindings (preferred via wasm-bindgen)
- **Memory/JSON file** — memory works, JSON file needs filesystem

## Architecture

### Gating Strategy

```rust
// Embedded backends — not available on wasm32-unknown-unknown
#[cfg(not(target_arch = "wasm32"))]
mod turso_backend;

#[cfg(not(target_arch = "wasm32"))]
mod libsql_backend;

// HTTP-based backends — available on all targets including wasm
mod d1_kvstore;    // Cloudflare D1 via REST API
mod r2_blobstore;   // Cloudflare R2 via REST API

// wasm-bindgen bindings — only on wasm32
#[cfg(target_arch = "wasm32")]
mod d1_binding;    // D1 via env.DB JavaScript binding
#[cfg(target_arch = "wasm32)]
mod r2_binding;    // R2 via env.BUCKET JavaScript binding
```

### D1 JS Bindings via wasm-bindgen

```rust
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use js_sys::{Object, Promise};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = Object)]
    pub type D1Database;

    #[wasm_bindgen(method, js_name = prepare)]
    pub fn prepare(this: &D1Database, query: &str) -> D1PreparedStatement;

    #[wasm_bindgen(extends = Object)]
    pub type D1PreparedStatement;

    #[wasm_bindgen(method, js_name = bind)]
    pub fn bind(this: &D1PreparedStatement, values: &JsValue) -> D1PreparedStatement;

    #[wasm_bindgen(method, js_name = first)]
    pub fn first(this: &D1PreparedStatement) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = run)]
    pub fn run(this: &D1PreparedStatement) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = all)]
    pub fn all(this: &D1PreparedStatement) -> js_sys::Promise;
}
```

### R2 JS Bindings

```rust
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = Object)]
    pub type R2Bucket;

    #[wasm_bindgen(method, js_name = get)]
    pub fn get(this: &R2Bucket, key: &str) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = put)]
    pub fn put(this: &R2Bucket, key: &str, value: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = delete)]
    pub fn delete(this: &R2Bucket, key: &str) -> js_sys::Promise;
}
```

### Turso Remote Mode

Turso's HTTP endpoint mode works on wasm since it uses HTTP, not local SQLite. The `turso` crate may need feature flags or a separate `SimpleHttpClient`-based backend.

**Decision:** For wasm32, gate `turso` and `libsql` crate dependencies behind `#[cfg(not(target_arch = "wasm32"))]`. The D1 and R2 backends remain available via both REST API (native) and JS bindings (wasm).

## Tasks

1. [ ] Gate `turso` and `libsql` crate deps behind `#[cfg(not(target_arch = "wasm32"))]` in foundation_db/Cargo.toml
2. [ ] Gate turso_backend and libsql_backend modules behind `#[cfg(not(target_arch = "wasm32"))]`
3. [ ] Create `d1_binding.rs` with wasm-bindgen types for D1Database, D1PreparedStatement, etc.
4. [ ] Create `r2_binding.rs` with wasm-bindgen types for R2Bucket
5. [ ] Ensure D1/R2 REST API backends compile on wasm (they use HTTP client)

## Verification

```bash
# Wasm compilation
cargo build -p foundation_db --target wasm32-unknown-unknown \
  --no-default-features --features d1,r2,foundation_core/ssl-rustls-awsrc,foundation_core/std \
  2>&1 | tee /tmp/wasm-db.log

# Native (no regression)
cargo build -p foundation_db --features turso,d1,r2 2>&1 | tee /tmp/native-db.log
```

---

_Created: 2026-05-15_
