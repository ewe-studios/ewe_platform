---
feature: "Wasm Bindings Layer"
description: "wasm-bindgen entry point for Cloudflare Workers — Request/Response conversion, CF env bindings (DB, BUCKET, secrets), fetch handler"
status: "pending"
priority: "high"
depends_on: ["02-auth-wasm-compat", "03-http-wasm-compat", "04-db-wasm-compat"]
estimated_effort: "large"
created: 2026-05-15
last_updated: 2026-05-15
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# Feature: Wasm Bindings Layer

## Overview

This feature creates the `wasm-bindgen` glue layer that connects Cloudflare Workers' JavaScript runtime to our Rust foundation crates. This is the bridge that makes everything work together.

## Architecture

### Entry Point

```rust
use wasm_bindgen::prelude::*;
use web_sys::{Request, Response, ResponseInit};

#[wasm_bindgen(start)]
pub fn init() {
    // Initialize logging, panic hook, etc.
    console_error_panic_hook::set_once();
}

#[wasm_bindgen(js_name = handleRequest)]
pub async fn handle_request(req: Request, env: JsValue) -> Result<Response, JsError> {
    // 1. Convert JS Request → SimpleIncomingRequest
    let incoming = js_request_to_incoming(&req)?;

    // 2. Build ContextBag from env bindings
    let bag = build_context_from_env(&env)?;

    // 3. Route the request
    let handler = router.match_route(&incoming)?;

    // 4. Serve via ResponseSink
    let mut sink = WasmResponseSink::new();
    let result = handler.serve(bag, incoming, &mut sink);

    // 5. Convert ResponseSink → JS Response
    js_response_from_sink(&sink, &result)
}
```

### Request Conversion

```rust
fn js_request_to_incoming(req: &web_sys::Request) -> Result<SimpleIncomingRequest, JsError> {
    let method = req.method()?;
    let url = req.url()?;
    let headers = req.headers()?;

    // Parse into SimpleIncomingRequest format
    // ...
}
```

### Response Conversion

```rust
fn js_response_from_sink(
    sink: &WasmResponseSink,
    result: &ConnectionResult,
) -> Result<Response, JsError> {
    let init = ResponseInit::new();
    init.set_status(sink.status_code());
    // set headers from sink

    let body = sink.body_bytes();
    Response::new_with_opt_u8_array_and_init(Some(&body), &init)
        .map_err(|e| JsError::from(e))
}
```

### Cloudflare Bindings

```rust
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = js_sys::Object)]
    pub type CloudflareEnv;

    #[wasm_bindgen(method, getter)]
    pub fn db(this: &CloudflareEnv, name: &str) -> D1Database;

    #[wasm_bindgen(method, getter)]
    pub fn bucket(this: &CloudflareEnv, name: &str) -> R2Bucket;

    #[wasm_bindgen(method, js_name = SECRET)]
    pub fn secret(this: &CloudflareEnv, name: &str) -> String;
}
```

### ContextBag Population

The `build_context_from_env` function extracts D1, R2, and secrets from the CF env object and inserts them into the `ContextBag`:

```rust
fn build_context_from_env(env: &JsValue) -> Result<Arc<ContextBag>, JsError> {
    let bag = ContextBag::new();

    // Extract D1 database
    let cf_env: CloudflareEnv = env.clone().unchecked_into();
    let db = cf_env.db("DB");
    bag.insert(db);

    // Extract R2 bucket
    let bucket = cf_env.bucket("BUCKET");
    bag.insert(bucket);

    // Extract secrets for JWT signing, etc.
    let jwt_secret = cf_env.secret("JWT_SECRET");
    bag.insert(JwtSecret::new(&jwt_secret));

    Ok(Arc::new(bag))
}
```

## Tasks

1. [ ] Create `foundation_cf` crate (or module) for Cloudflare-specific bindings
2. [ ] Implement `js_request_to_incoming` conversion
3. [ ] Implement `js_response_from_sink` conversion
4. [ ] Implement `build_context_from_env` for CF bindings
5. [ ] Add `wasm-pack` configuration for `web` target
6. [ ] Test locally with `wrangler dev`

## Verification

```bash
# Build with wasm-pack
wasm-pack build --target web

# Local dev
wrangler dev
```

---

_Created: 2026-05-15_
