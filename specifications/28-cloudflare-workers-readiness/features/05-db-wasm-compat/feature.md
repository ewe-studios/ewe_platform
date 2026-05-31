---
feature: "Database Wasm Compatibility"
description: "Restructure foundation_db into core/native/wasm modules, add wasm-bindgen bridge for CF D1/R2/KV"
status: "implemented"
priority: "high"
depends_on: ["01-core-wasm-compat"]
estimated_effort: "large"
created: 2026-05-15
last_updated: 2026-05-31
author: "Main Agent"
tasks:
  completed: 8
  uncompleted: 0
  total: 8
  completion_percentage: 100%
---

# Feature: Database Wasm Compatibility

## Overview

`foundation_db` restructures into shared core, native-specific, and wasm-specific modules. The wasm module provides a wasm-bindgen bridge for Cloudflare Workers storage services (D1, R2, KV).

## Module Restructure

```
foundation_db/src/
├── lib.rs                      # Always compiled — re-exports core + target module
├── core/                       # Shared between wasm and non-wasm
│   ├── mod.rs
│   ├── storage_provider.rs     # StorageProvider enum, KeyValueStore, BlobStore, QueryStore traits
│   ├── schema/                 # Schema types, migrations
│   ├── crypto/                 # Encryption utilities
│   ├── errors/                 # StorageError types
│   ├── cleanup/                # Cleanup utilities
│   ├── state/                  # State management
│   ├── backends/
│   │   ├── memory_kv.rs        # In-memory key-value store
│   │   ├── memory_blob.rs      # In-memory blob store
│   │   ├── json_file.rs        # JSON file key-value store (gated to native, needs filesystem)
│   │   ├── d1_kvstore.rs       # D1 via Cloudflare REST API (both targets, HTTP-based)
│   │   └── r2_blobstore.rs     # R2 via Cloudflare REST API (both targets, HTTP-based)
│   └── async_utils.rs          # Shared async utilities
├── native/                     # Only compiled on non-wasm targets
│   ├── mod.rs                  # Re-exports turso, libsql backends
│   ├── turso_backend.rs        # Turso embedded/remote
│   └── libsql_backend.rs       # libSQL backend
└── wasm/                       # Only compiled on wasm32 targets
    ├── mod.rs                  # Re-exports cf bindings
    └── cf/                     # wasm-bindgen bridge for Cloudflare services
        ├── mod.rs
        ├── d1.rs               # env.DB → D1Database, D1PreparedStatement
        ├── r2.rs               # env.BUCKET → R2Bucket
        └── kv.rs               # env.KV → KVNamespace
```

## Core Module (shared)

Storage traits, error types, schema management, memory backends, and the HTTP-based D1/R2 REST API backends (which use `SimpleHttpClient` and compile on any target).

`json_file.rs` gated to native (needs filesystem access).

## Native Module (non-wasm only)

### `turso_backend.rs`

Turso embedded/remote SQLite. The `turso` crate compiles to wasm32, but embedded mode needs filesystem — use in-memory mode on wasm or gate entirely to native.

### `libsql_backend.rs`

Check if `libsql` compiles on wasm32-unknown-unknown. If not, gate to native.

## Wasm Module (wasm32 only)

### Cloudflare Service Bindings

wasm-bindgen types for CF Workers storage services:

**D1 (`cf/d1.rs`):**

```rust
use wasm_bindgen::prelude::*;
use js_sys::{Object, Promise, Array};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = Object)]
    pub type D1Database;

    #[wasm_bindgen(method, js_name = prepare)]
    pub fn prepare(this: &D1Database, query: &str) -> D1PreparedStatement;
}

#[wasm_bindgen]
extern "C" {
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

    #[wasm_bindgen(method, js_name = batch)]
    pub fn batch(this: &D1Database, stmts: &Array) -> js_sys::Promise;
}

impl D1Database {
    /// Extract D1 database from CF env.
    pub fn from_env(env: &JsValue, binding: &str) -> Result<Self, JsError> {
        // env[binding] → D1Database
    }
}
```

**R2 (`cf/r2.rs`):**

```rust
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = Object)]
    pub type R2Bucket;

    #[wasm_bindgen(method, js_name = get)]
    pub fn get(this: &R2Bucket, key: &str) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = put)]
    pub fn put(this: &R2Bucket, key: &str, value: &JsValue, opts: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = delete)]
    pub fn delete(this: &R2Bucket, key: &str) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = head)]
    pub fn head(this: &R2Bucket, key: &str) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = list)]
    pub fn list(this: &R2Bucket, opts: &JsValue) -> js_sys::Promise;
}
```

**KV (`cf/kv.rs`):**

```rust
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = Object)]
    pub type KVNamespace;

    #[wasm_bindgen(method, js_name = get)]
    pub fn get(this: &KVNamespace, key: &str) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = put)]
    pub fn put(this: &KVNamespace, key: &str, value: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = delete)]
    pub fn delete(this: &KVNamespace, key: &str) -> js_sys::Promise;

    #[wasm_bindgen(method, js_name = list)]
    pub fn list(this: &KVNamespace, opts: &JsValue) -> js_sys::Promise;
}
```

### Integration with StorageProvider

The wasm bindings implement the existing `KeyValueStore`, `BlobStore`, `QueryStore` traits so they're drop-in replacements:

```rust
#[cfg(target_arch = "wasm32")]
pub struct WasmD1Store {
    db: D1Database,
}

impl KeyValueStore for WasmD1Store { ... }
impl QueryStore for WasmD1Store { ... }
```

### Tests

- `wasm/cf/tests.rs` — mock CF env bindings, verify D1/R2/KV extraction and basic operations

## Tasks

1. [ ] Create `core/` directory and move shared modules (traits, errors, schema, memory backends, HTTP-based D1/R2)
2. [ ] Create `native/` directory and move turso/libsql backends
3. [ ] Gate `json_file.rs` to native (filesystem dependency)
4. [ ] Verify `turso` crate compiles on wasm32 — add feature flags if needed
5. [ ] Check `libsql` crate wasm compatibility — gate if necessary
6. [ ] Create `wasm/cf/d1.rs` with wasm-bindgen types for D1
7. [ ] Create `wasm/cf/r2.rs` with wasm-bindgen types for R2
8. [ ] Create `wasm/cf/kv.rs` with wasm-bindgen types for KV

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
