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
    - weak-arc
has_features: false
has_fundamentals: false
builds_on:
  - "specifications/28-cloudflare-workers-readiness"
  - "specifications/21-http-framework"
related_specs:
  - "specifications/28-cloudflare-workers-readiness"
tasks:
  completed: 8
  uncompleted: 0
  total: 8
  completion_percentage: 100%
---

# CfServe / CfHttpApp — Idiomatic Cloudflare Workers Entry Point

## Problem

`CfHttpApp::handle_request(req, env: JsValue)` requires users to manually create the app, store it in an `OnceLock`, extract bindings, and wire up their `fetch()` handler. The current cf-login-app bypasses `CfServe`/`CfHttpApp` entirely because the plumbing was too heavy — handlers are free-standing async functions instead of `CfServe` trait implementations.

## Solution

Two additions to `foundation_http::wasm::bridge::cf`:

1. **`CfHttpApp::fetch()`** — idiomatic method name matching CF Workers convention
2. **`CfHttpAppSingleton`** — `Weak`/`Arc`-backed singleton with `get_or_init()` and `get_or_init_with_env()`

**Why `Weak`/`Arc`, not `OnceLock`:**

The same singleton pattern is used in spec 33 (valtron executors). Using `Weak`/`Arc` across all singletons gives a unified API and consistent lifecycle semantics:

- **Static holds a `Weak`** — never owns the app, only tracks its existence.
- **`CfHttpAppGuard` holds the `Arc<AppState>`** — strong ownership.
- When all guards drop → `AppState` drops → `CfHttpApp` is reclaimed naturally.
- The `Weak` becomes dangling → next `get_or_init` re-initializes cleanly.
- No `unsafe`, no explicit cleanup, drop-based reclamation is natural.

In CF Workers the V8 isolate lives for the duration of an invocation, so in practice the guard is never dropped before the isolate ends. But the pattern matters for:
- Consistency with valtron singleton APIs (single and multi executors).
- Local unit testing: dropping guards lets the app reclaim, enabling re-initialization.
- Future-proofing: if `CfHttpApp` gains a `Drop` (flushing connections, closing pools), the pattern already supports it.

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

### 2. `CfHttpAppSingleton` — Weak/Arc-Backed Lazy Init

**Storage:** a module-level static holding a `Weak`, not an `OnceLock`.

```rust
/// Static tracking of app existence. Never owns the app — only holds a Weak.
///
/// When all CfHttpAppGuard instances drop, the Arc refcount hits 0 and
/// the Weak becomes dangling. Next get_or_init detects this and re-initializes.
static STATE: Mutex<Weak<AppState>> = Mutex::new(Weak::new());
```

```rust
/// Internal app state shared between all guards.
///
/// Holds the Arc<CfHttpApp> and the ContextBag with bindings.
/// When all guards drop, AppState drops — in CF Workers this is a
/// no-op (the V8 isolate is shutting down anyway), but the structure
/// is ready for future cleanup hooks.
struct AppState {
    app: Arc<CfHttpApp>,
    bag: Arc<ContextBag>,
}

/// Lifecycle token for the CF HTTP app.
///
/// Holds an `Arc<AppState>` so the app stays alive as long as any guard
/// instance exists. When all guards drop, `AppState` drops naturally.
pub struct CfHttpAppGuard {
    state: Arc<AppState>,
}

impl CfHttpAppGuard {
    /// Access the underlying CfHttpApp.
    pub fn app(&self) -> &CfHttpApp {
        &self.state.app
    }

    /// Dispatch a request with CF env bindings through the app.
    pub async fn fetch(&self, req: Request, env: JsValue) -> Result<Response, JsError> {
        self.state.app.fetch(req, env).await
    }
}

impl Clone for CfHttpAppGuard {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

/// Zero-sized namespace for methods that manage the `STATE` static.
///
/// The app is initialized exactly once per isolate lifecycle. When all
/// CfHttpAppGuard instances drop, the app is reclaimed and can be
/// re-initialized.
///
/// # Usage
/// ```rust
/// // In your worker's fetch handler — one line:
/// CfHttpAppSingleton::get_app().fetch(req, env).await
///
/// // During app startup (or lazily on first request):
/// let guard = CfHttpAppSingleton::get_or_init(|bag| {
///     let mut app = HttpApp::new_cf();
///     app.route_cf::<HomeHandler>(SimpleMethod::GET, "/");
///     app.route_cf::<LoginHandler>(SimpleMethod::GET, "/login");
///     app.route_cf::<LoginHandler>(SimpleMethod::POST, "/login");
///     CfHttpApp::from_app(app)
/// });
/// ```
pub struct CfHttpAppSingleton;

impl CfHttpAppSingleton {
    /// Get a guard to the singleton app.
    ///
    /// Internally: `STATE.lock().unwrap().upgrade()
    ///   .expect("CfHttpApp not initialized — call get_or_init() or get_or_init_with_env() first")`
    ///
    /// Returns `CfHttpAppGuard` (wraps a clone of the Arc, not a deep copy).
    pub fn get_app() -> CfHttpAppGuard { ... }

    /// Initialize the app if not already initialized.
    ///
    /// Internally:
    ///   1. Lock STATE, check if existing Weak can be upgraded.
    ///   2. If yes, return a new CfHttpAppGuard wrapping the existing Arc.
    ///   3. If no, create a fresh `Arc<ContextBag>` (no bindings auto-extracted),
    ///      build the `CfHttpApp`, wrap in `AppState` → `Arc` → `CfHttpAppGuard`,
    ///      store Weak in STATE, return guard.
    ///
    /// The closure receives an `Arc<ContextBag>` so you can store typed
    /// bindings (D1, R2, KV, secrets) before registering routes.
    pub fn get_or_init<F: FnOnce(Arc<ContextBag>) -> CfHttpApp>(
        builder: F,
    ) -> CfHttpAppGuard { ... }

    /// Initialize the app from a raw CF env `JsValue`.
    ///
    /// Internally:
    ///   1. Lock STATE, check if existing Weak can be upgraded.
    ///   2. If yes, return a new CfHttpAppGuard wrapping the existing Arc.
    ///   3. If no, create a fresh `Arc<ContextBag>`.
    ///      Auto-extract bindings from `env` by name:
    ///      - `env.get("DB")`     → D1Database  → bag.store()
    ///      - `env.get("BUCKET")` → R2Bucket    → bag.store()
    ///      - `env.get("KV")`     → KVNamespace → bag.store()
    ///      Build the `CfHttpApp`, wrap in `AppState` → `Arc` → `CfHttpAppGuard`,
    ///      store Weak in STATE, return guard.
    ///
    /// If already initialized, returns the existing guard and ignores
    /// the closure (env is not re-parsed).
    pub fn get_or_init_with_env<F: FnOnce(Arc<ContextBag>) -> CfHttpApp>(
        env: &JsValue,
        builder: F,
    ) -> CfHttpAppGuard { ... }

    /// Check if the app has been initialized.
    pub fn is_initialized() -> bool { ... }

    /// Force-reset the singleton state. cfg-gated to #[cfg(test)].
    ///
    /// Drops any existing app by clearing the Weak. In tests this
    /// effectively resets state immediately.
    #[cfg(test)]
    pub fn reset() {
        let mut weak = STATE.lock().unwrap();
        *weak = Weak::new();
    }
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
| `foundation_http/src/wasm/bridge/cf.rs` | Add `fetch()` to `CfHttpApp`; add `AppState`, `CfHttpAppGuard`, `static STATE: Mutex<Weak<AppState>>`, and `CfHttpAppSingleton` impl |
| `foundation_http/src/wasm/mod.rs` | Re-export `CfHttpAppSingleton` and `CfHttpAppGuard` |

No new dependencies. No changes outside `foundation_http`.

## Tasks

1. [ ] Add `fetch()` method to `CfHttpApp` (alias for `handle_request`)
2. [ ] Add `AppState` struct with `Arc<CfHttpApp>` + `Arc<ContextBag>`
3. [ ] Add `CfHttpAppGuard` holding `Arc<AppState>` with `fetch()` method
4. [ ] Add `static STATE: Mutex<Weak<AppState>>` and `CfHttpAppSingleton` struct
5. [ ] Implement `get_app()`, `get_or_init()`, `get_or_init_with_env()`, `is_initialized()`, `reset()` on `CfHttpAppSingleton`
6. [ ] Export `CfHttpAppSingleton` and `CfHttpAppGuard` from `wasm/mod.rs`
7. [ ] Verify wasm32 compilation: `cargo check --target wasm32-unknown-unknown --features wasm-bindgen-http`
8. [ ] Verify native compilation: `cargo check -p foundation_http`

## Success Criteria

- [ ] `CfHttpApp::fetch()` compiles and delegates to `handle_request`
- [ ] `CfHttpAppGuard` holds `Arc<AppState>` and provides `fetch()` method
- [ ] `CfHttpAppSingleton::get_or_init()` initializes app once via Weak/Arc pattern
- [ ] `CfHttpAppSingleton::get_or_init_with_env()` auto-extracts D1/R2/KV bindings
- [ ] `CfHttpAppSingleton::get_app()` returns guard via `Weak::upgrade`, panics if not initialized
- [ ] `CfHttpAppSingleton::is_initialized()` returns `bool`
- [ ] `CfHttpAppSingleton::reset()` clears state in tests only (`#[cfg(test)]`)
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
