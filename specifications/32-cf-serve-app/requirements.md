---
description: "Add CfHttpApp::fetch() and CfHttpAppSingleton for idiomatic one-liner Cloudflare Workers entry points. Workers call CfHttpAppSingleton::get_app().fetch(req, env) — exercises CfServe handlers, CfConn structured responses, full middleware chain."
status: "pending"
priority: "high"
created: 2026-05-31
updated: 2026-05-31
author: "Main Agent"
metadata:
  version: "1.0"
  estimated_effort: "small"
  tags:
    - cloudflare-workers
    - wasm
    - cf-serve
    - cf-http-app
    - singleton
has_features: false
has_fundamentals: false
builds_on:
  - "specifications/28-cloudflare-workers-readiness"
  - "specifications/21-http-framework"
related_specs:
  - "specifications/28-cloudflare-workers-readiness"
tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# CfServe / CfHttpApp — Idiomatic Cloudflare Workers Entry Point

## Problem

`CfHttpApp::handle_request(req, env: JsValue)` requires users to manually create the app, store it in an `OnceLock`, extract bindings, and wire up their `fetch()` handler. The current cf-login-app bypasses `CfServe`/`CfHttpApp` entirely because the plumbing was too heavy — handlers are free-standing async functions instead of `CfServe` trait implementations.

## Solution

Two additions to `foundation_http::wasm::bridge::cf`:

1. **`CfHttpApp::fetch()`** — idiomatic method name matching CF Workers convention
2. **`CfHttpAppSingleton`** — `OnceLock`-backed singleton with `get_or_init()` and `get_or_init_with_env()`

The result: a CF Worker is a one-liner that exercises the full `CfServe` stack.

```rust
#[wasm_bindgen]
pub async fn fetch(req: Request, env: worker::Env) -> Response {
    CfHttpAppSingleton::get_app().fetch(req, env).await
}
```

## Architecture

```
┌────────────────────────────────────────────────────────────────────┐
│                    Cloudflare Worker (V8 isolate)                   │
│                                                                     │
│  fetch(req, env)                                                    │
│    │                                                                │
│    ▼                                                                │
│  CfHttpAppSingleton::get_app()                                      │
│    │                                                                │
│    ▼                                                                │
│  CfHttpApp::fetch(req, env) ────> dispatch_cf()                     │
│    │                              │                                 │
│    │                              ▼                                 │
│    │                         middleware chain                       │
│    │                              │                                 │
│    │                              ▼                                 │
│    │                         router.dispatch()                      │
│    │                              │                                 │
│    │                              ▼                                 │
│    │                         handler.serve_cf(bag, req, conn)       │
│    │                              │                                 │
│    │                              ▼                                 │
│    │                         CfConn → into_response()               │
│    │                              │                                 │
│    ▼                              ▼                                 │
│  web_sys::Response ◄─────────────┘                                  │
│                                                                     │
│  ContextBag (populated by init_with_env):                           │
│    - D1Database::from_env(env, "DB")                                │
│    - R2Bucket::from_env(env, "BUCKET")                              │
│    - KVNamespace::from_env(env, "KV")                               │
│    - secrets, custom config, etc.                                   │
└────────────────────────────────────────────────────────────────────┘
```

## Implementation Detail

### 1. `CfHttpApp::fetch()` — Idiomatic Method Name

Add `fetch` as the primary method on `CfHttpApp`. `handle_request` remains as an alias.

```rust
impl CfHttpApp {
    /// Cloudflare Workers entry point — dispatches a Request with CF env bindings
    /// through the app and returns a Response.
    pub async fn fetch(&self, req: Request, env: JsValue) -> Result<Response, JsError> {
        self.handle_request(req, env).await
    }
}
```

### 2. `CfHttpAppSingleton` — OnceLock-Backed Lazy Init

**Storage:** a module-level static, not a struct field.

```rust
/// The actual OnceLock storage — a module-level static that holds the
/// initialized CfHttpApp for the lifetime of the V8 isolate.
static HTTP_APP: OnceLock<Arc<CfHttpApp>> = OnceLock::new();
```

**Why static, not a field:**
- CF Workers run in a single V8 isolate per invocation. No thread contention.
- `OnceLock` exists only for its "initialize at most once" semantics.
- A zero-sized wrapper struct provides a clean public API surface without exposing the raw static.
- `Arc<>` allows the singleton to be cloned (cheap ref-count bump) and returned by value from `get_app()` without moving out of the `OnceLock`.

```rust
/// Zero-sized namespace for methods that manage the `HTTP_APP` static.
/// The app is initialized exactly once on first request, then reused
/// for all subsequent requests in this isolate.
///
/// # Usage
/// ```rust
/// // In your worker's fetch handler — one line:
/// CfHttpAppSingleton::get_app().fetch(req, env).await
///
/// // During app startup (or lazily on first request):
/// CfHttpAppSingleton::get_or_init(|bag| {
///     let mut app = HttpApp::new_cf();
///     app.route_cf::<HomeHandler>(SimpleMethod::GET, "/");
///     app.route_cf::<LoginHandler>(SimpleMethod::GET, "/login");
///     app.route_cf::<LoginHandler>(SimpleMethod::POST, "/login");
///     CfHttpApp::from_app(app)
/// });
/// ```
pub struct CfHttpAppSingleton;

impl CfHttpAppSingleton {
    /// Get the singleton app instance from the HTTP_APP static.
    ///
    /// Internally: `HTTP_APP.get().expect("CfHttpApp not initialized — \
    ///   call get_or_init() or get_or_init_with_env() first")`
    ///
    /// Returns `Arc<CfHttpApp>` (cheap clone of the Arc, not a deep copy).
    pub fn get_app() -> Arc<CfHttpApp> { ... }

    /// Initialize the HTTP_APP static with a builder closure.
    ///
    /// Internally: `HTTP_APP.get_or_init(|| builder(bag))`
    /// where `bag` is a fresh `Arc<ContextBag>` (no bindings auto-extracted).
    ///
    /// If already initialized, returns the existing instance and ignores
    /// the closure.
    ///
    /// The closure receives an `Arc<ContextBag>` so you can store typed
    /// bindings (D1, R2, KV, secrets) before registering routes.
    pub fn get_or_init<F: FnOnce(Arc<ContextBag>) -> CfHttpApp>(builder: F) -> Arc<CfHttpApp> { ... }

    /// Initialize the HTTP_APP static from a raw CF env `JsValue`.
    ///
    /// Internally:
    ///   1. Create a fresh `Arc<ContextBag>`.
    ///   2. Auto-extract bindings from `env` by name:
    ///      - `env.get("DB")`     → D1Database  → bag.store()
    ///      - `env.get("BUCKET")` → R2Bucket    → bag.store()
    ///      - `env.get("KV")`     → KVNamespace → bag.store()
    ///   3. `HTTP_APP.get_or_init(|| builder(bag))`
    ///
    /// If already initialized, returns the existing instance and ignores
    /// the closure (env is not re-parsed).
    pub fn get_or_init_with_env<F: FnOnce(Arc<ContextBag>) -> CfHttpApp>(
        env: &JsValue,
        builder: F,
    ) -> Arc<CfHttpApp> { ... }
}
```

### 3. How Users Write a Worker

**Minimal (no DB):**

```rust
#[wasm_bindgen]
pub async fn fetch(req: Request, _env: worker::Env) -> Response {
    CfHttpAppSingleton::get_or_init(|_bag| {
        let mut app = HttpApp::new_cf();
        app.route_cf::<HomeHandler>(SimpleMethod::GET, "/");
        CfHttpApp::from_app(app)
    }).fetch(req, JsValue::NULL).await
}
```

**Full (D1 + auth):**

```rust
#[wasm_bindgen]
pub async fn fetch(req: Request, env: worker::Env) -> Response {
    let env_val: &JsValue = env.as_ref();

    CfHttpAppSingleton::get_or_init_with_env(env_val, |bag| {
        // D1 binding is auto-extracted by get_or_init_with_env
        let db = bag.get::<D1Database>().unwrap();
        let storage = Arc::new(D1WasmStorage::new(Arc::new(db.clone()), "app"));
        bag.store(storage);

        let mut app = HttpApp::new_cf();
        app.route_cf::<LoginHandler>(SimpleMethod::GET, "/login");
        app.route_cf::<LoginHandler>(SimpleMethod::POST, "/login");
        app.route_cf::<DashboardHandler>(SimpleMethod::GET, "/dashboard");
        app.middleware(AuthMiddleware);
        CfHttpApp::from_app(app)
    }).fetch(req, env_val.clone()).await
}
```

### 4. Auto-Extracted Bindings

`get_or_init_with_env` extracts these from `env` by convention:

| Binding Name | Type | ContextBag Key |
|-------------|------|----------------|
| `DB` | `D1Database` | `D1Database` |
| `BUCKET` | `R2Bucket` | `R2Bucket` |
| `KV` | `KVNamespace` | `KVNamespace` |
| `SECRETS` | `js_sys::Object` | `KVNamespace` (for secret key lookups) |

Custom bindings can be added by the user inside the builder closure.

## File Changes

| File | Change |
|------|--------|
| `foundation_http/src/wasm/bridge/cf.rs` | Add `fetch()` to `CfHttpApp`; add `static HTTP_APP` and `CfHttpAppSingleton` impl |
| `foundation_http/src/wasm/mod.rs` | Re-export `CfHttpAppSingleton` |

No new dependencies. No changes outside `foundation_http`.

## Tasks

1. [ ] Add `fetch()` method to `CfHttpApp` (alias for `handle_request`)
2. [ ] Add `static HTTP_APP: OnceLock<Arc<CfHttpApp>>` and `CfHttpAppSingleton` struct
3. [ ] Implement `get_app()`, `get_or_init()`, `get_or_init_with_env()` on `CfHttpAppSingleton`
4. [ ] Export `CfHttpAppSingleton` from `wasm/mod.rs`
5. [ ] Verify wasm32 compilation: `cargo check --target wasm32-unknown-unknown --features wasm-bindgen-http`
6. [ ] Verify native compilation: `cargo check -p foundation_http`

## Success Criteria

- [ ] `CfHttpApp::fetch()` compiles and delegates to `handle_request`
- [ ] `CfHttpAppSingleton::get_or_init()` initializes app once
- [ ] `CfHttpAppSingleton::get_or_init_with_env()` auto-extracts D1/R2/KV bindings
- [ ] `cargo check --target wasm32-unknown-unknown --features wasm-bindgen-http` passes
- [ ] `cargo check -p foundation_http` (native) passes

## Dependencies

This spec builds on foundation types from [spec 28](../28-cloudflare-workers-readiness/requirements.md):
- `CfServe`, `CfConn`, `CfHttpApp` — from feature 03-http-wasm-compat
- `HttpApp<Arc<dyn CfServe>>` — from feature 10-generic-serve-traits
- `D1Database`, `R2Bucket`, `KVNamespace` — from feature 05-db-wasm-compat
- `CfHttpAppDispatch` — from feature 03-http-wasm-compat

---

_Created: 2026-05-31_
