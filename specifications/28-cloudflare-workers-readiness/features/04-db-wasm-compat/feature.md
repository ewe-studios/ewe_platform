---
feature: "Database Wasm Compatibility"
description: "Enable turso, D1, R2 backends for wasm32 — turso compiles to wasm, D1/R2 via CF JS bindings"
status: "pending"
priority: "high"
depends_on: ["01-core-wasm-compat"]
estimated_effort: "medium"
created: 2026-05-15
last_updated: 2026-05-15
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0%
---

# Feature: Database Wasm Compatibility

## Overview

`foundation_db` has multiple backends. Turso provides first-class wasm support — the Rust `turso` crate compiles to wasm32, and the JavaScript `@tursodatabase/database-wasm` package provides a full embedded SQLite engine for browsers via `wasm32-wasip1-threads`.

For wasm32-unknown-unknown (Cloudflare Workers):
- **Turso**: The Rust `turso` crate compiles to wasm — in-memory storage works without filesystem
- **D1**: Cloudflare's `env.DB` JS binding via wasm-bindgen (preferred) or REST API fallback
- **R2**: Cloudflare's `env.BUCKET` JS binding via wasm-bindgen (preferred) or REST API fallback
- **Memory/JSON file**: Memory works; JSON file needs virtualized storage on wasm

## Architecture

### Turso Wasm Support

The `turso` Rust crate (`turso_core`) compiles to `wasm32` targets. When used in a wasm context:
- **In-memory mode**: `connect(":memory:")` — no filesystem needed, works on all wasm targets
- **Virtualized storage**: When running with `@tursodatabase/database-wasm`, the JS layer handles persistence (IDB, OPFS, etc.)
- **Same Rust API**: `turso::Database`, `turso::Connection`, `turso::Statement` all work on wasm

No `#[cfg]` gating needed for turso — it compiles on wasm32 out of the box.

### D1/R2 JS Bindings via wasm-bindgen

On wasm32, prefer Cloudflare's native JS bindings over REST API calls:

```rust
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use js_sys::{Object, Promise};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = Object)]
    pub type D1Database;

    #[wasm_bindgen(method, js_name = prepare)]
    pub fn prepare(this: &D1Database, query: &str) -> D1PreparedStatement;

    #[wasm_bindgen(extends = Object)]
    pub type D1PreparedStatement;

    #[wasm_bindgen(method, js_name = all)]
    pub fn all(this: &D1PreparedStatement) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = run)]
    pub fn run(this: &D1PreparedStatement) -> js_sys::Promise;
}

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

### libsql

The `libsql` crate is the upstream of turso. Check if it compiles on wasm32-unknown-unknown. If not, gate it and use turso for wasm.

## Tasks

1. [ ] Verify `turso` crate compiles on wasm32 — add feature flags if needed
2. [ ] Check `libsql` crate wasm compatibility — gate if necessary
3. [ ] Create `d1_binding.rs` with wasm-bindgen types for Cloudflare D1
4. [ ] Create `r2_binding.rs` with wasm-bindgen types for Cloudflare R2
5. [ ] Ensure D1/R2 REST API backends compile on wasm (they use HTTP client)

## Verification

```bash
# Wasm compilation (turso enabled)
cargo build -p foundation_db --target wasm32-unknown-unknown \
  --no-default-features --features turso,d1,r2,foundation_core/ssl-rustls-awsrc,foundation_core/std \
  2>&1 | tee /tmp/wasm-db.log

# Native (no regression)
cargo build -p foundation_db --features turso,d1,r2 2>&1 | tee /tmp/native-db.log
```

---

_Created: 2026-05-15_
