---
feature: "Example Login App"
description: "Working login application deployable to Cloudflare Workers — login form, auth guard, D1-backed sessions, protected-dashboard route"
status: "pending"
priority: "high"
depends_on: ["03-http-wasm-compat", "02-auth-wasm-compat", "05-db-wasm-compat"]
estimated_effort: "medium"
created: 2026-05-15
last_updated: 2026-05-19
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---

# Feature: Example Login App

## Overview

A complete, deployable Cloudflare Workers login app demonstrating the full pipeline:
login page, user registration with argon2 password hashing, authentication
via `CfServe` handlers, D1-backed `SessionManager`,
protected dashboard route, and the `CfHttpApp` wasm-bindgen bridge.

## Architecture

```
Route Structure:
  GET  /              → Redirect to /login or /dashboard
  GET  /register      → Registration form (HTML)
  POST /register      → Create user (JSON or form), hash password with argon2, insert into users table
  GET  /login         → Login form (HTML)
  POST /login         → Authenticate against users table (migration 002)
  GET  /dashboard     → Protected route (requires auth)
  GET  /logout        → Clear session

Data Flow:
  1. wasm_bindgen init: extract D1 binding from CF env → ContextBag
  2. On first request: run MigrationRunner::new(MIGRATIONS) against D1WasmStorage to create tables
  3. User visits /register → fills form → POST creates account with argon2-hashed password in users table
  4. User visits /login → fills credentials → validated against users table (migration 002)
  5. On success → SessionManager<D1CredentialStore> creates session, stored in kv_store table
  6. GET /dashboard → session cookie validated via SessionManager.get_session()
  7. If valid → render dashboard HTML
  8. If invalid → redirect to /login
  9. GET /logout → revoke session, clear cookies
```

## Key API Patterns

### `HttpApp<Arc<dyn CfServe>>` with `CfConn`

Handlers implement `CfServe` + `CfServeFactory`, receiving a `&mut CfConn` for
structured responses — no HTTP wire format:

```rust
use foundation_http::shared::context::ContextBag;
use foundation_http::wasm::serve_cf::{CfServe, CfServeFactory};
use foundation_http::wasm::cf_conn::{CfConn, CfConnectionResult};
use foundation_core::wire::simple_http::SimpleIncomingRequest;
use std::sync::Arc;

struct LoginHandler;

impl CfServeFactory for LoginHandler {
    fn create(_bag: &ContextBag) -> Self { LoginHandler }
}

impl CfServe for LoginHandler {
    fn serve_cf(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: &mut CfConn,
    ) -> CfConnectionResult {
        conn.set_status(200);
        conn.set_header("Content-Type", "text/html");
        conn.set_body(b"<h1>Login</h1><form method='POST' action='/login'>...</form>".to_vec());
        CfConnectionResult::Ok
    }
}
```

### App Assembly

```rust
use foundation_http::shared::app::HttpApp;
use foundation_http::wasm::serve_cf::CfServe;
use std::sync::Arc;

fn create_app() -> Arc<HttpApp<Arc<dyn CfServe>>> {
    let mut app = HttpApp::new_cf();
    app.route_cf::<RegisterHandler>(SimpleMethod::GET, "/register");
    app.route_cf::<RegisterHandler>(SimpleMethod::POST, "/register");
    app.route_cf::<LoginHandler>(SimpleMethod::GET, "/login");
    app.route_cf::<LoginHandler>(SimpleMethod::POST, "/login");
    app.route_cf::<DashboardHandler>(SimpleMethod::GET, "/dashboard");
    app.route_cf::<LogoutHandler>(SimpleMethod::GET, "/logout");
    Arc::new(app)
}
```

### `CfHttpApp` Bridge

The wasm-bindgen entry point wraps `HttpApp<Arc<dyn CfServe>>` in a `CfHttpApp`:

```rust
use foundation_http::wasm::bridge::cf::CfHttpApp;

#[wasm_bindgen]
pub fn create_worker() -> CfHttpApp {
    let mut app = HttpApp::new_cf();
    app.route_cf::<HomeHandler>(SimpleMethod::GET, "/");
    app.route_cf::<RegisterHandler>(SimpleMethod::GET, "/register");
    app.route_cf::<RegisterHandler>(SimpleMethod::POST, "/register");
    app.route_cf::<LoginHandler>(SimpleMethod::GET, "/login");
    app.route_cf::<LoginHandler>(SimpleMethod::POST, "/login");
    app.route_cf::<DashboardHandler>(SimpleMethod::GET, "/dashboard");
    app.route_cf::<LogoutHandler>(SimpleMethod::GET, "/logout");
    CfHttpApp::from_app(app)
}

// In JS/Worker:
// export default {
//   async fetch(request, env, ctx) {
//     const worker = create_worker();
//     return worker.handleRequest(request, env);
//   }
// }
```

The bridge (`CfHttpApp::handle_request`) automatically:
- Converts `web_sys::Request` → `SimpleIncomingRequest`
- Extracts CF bindings (`DB`, `BUCKET`, `KV`) from `env` into `ContextBag`
- Dispatches through the middleware chain inline in `dispatch_cf()`
- Converts `CfConn` fields → `web_sys::Response`

### D1 Storage + Migrations

```rust
use foundation_db::core::schema::{MIGRATIONS, MigrationRunner};
use foundation_db::wasm::wasm_storage::D1WasmStorage;
use foundation_db::wasm::bindgen::cf::D1Database;

// D1Database is extracted from env by the bridge into ContextBag
// Retrieve it and wrap in D1WasmStorage:
let db: D1Database = bag.get::<D1Database>()?;
let d1_store = D1WasmStorage::new(db, "app");

// Run migrations once on init:
let runner = MigrationRunner::new(MIGRATIONS);
let applied = runner.run(&d1_store)?;
```

### Session Manager with D1

`D1WasmStorage` implements `KeyValueStore` (Valtron stream-returning), while
`SessionManager<S>` requires `CredentialStore` (sync `Result`-returning). A
`D1CredentialStore` wrapper drains streams to bridge the gap:

```rust
use foundation_auth::{CredentialStore, SessionManager, SessionConfig};
use foundation_db::{D1WasmStorage, KeyValueStore};
use foundation_core::valtron::Stream;

struct D1CredentialStore(D1WasmStorage);

impl CredentialStore for D1CredentialStore {
    fn get<V: serde::de::DeserializeOwned + Send + 'static>(
        &self, key: &str,
    ) -> Result<Option<V>, CredentialStoreError> {
        let stream = self.0.get(key).map_err(CredentialStoreError::Storage)?;
        for item in stream {
            if let Stream::Next(result) = item {
                return result.map_err(CredentialStoreError::Storage);
            }
        }
        Err(CredentialStoreError::NotFound(key.to_string()))
    }
    // ... set, delete, exists, list_keys similarly
}
```

`D1WasmStorage` wraps a JS `D1Database` (`!Send` by default). On wasm32 there is
only one thread, so `unsafe impl Send + Sync` is safe:

```rust
// In foundation_db/src/wasm/wasm_storage/d1_wasm.rs:
unsafe impl Send for D1WasmStorage {}
unsafe impl Sync for D1WasmStorage {}
```

Now `SessionManager<D1CredentialStore>` compiles and works:

```rust
let session_mgr = SessionManager::new(
    D1CredentialStore(d1_store),
    SessionConfig::default(),
    &signing_key,
)?;

// Create session on login:
let (session, cookies) = session_mgr.create_session(user_id, ip, ua)?;

// Verify on protected route:
let session = session_mgr.get_session(&token)?;
```

### Auth Guard (inline in handlers)

Each protected handler checks the session cookie inline rather than using
middleware — `extract_session_token` parses the `Cookie` header, then
`SessionManager::get_session()` validates it:

```rust
let token = extract_session_token_from_headers(&req.headers, "session_token");
let Some(token) = token else {
    conn.set_status(302);
    conn.set_header("Location", "/login");
    return CfConnectionResult::Ok;
};
let session = session_mgr.get_session(&token)?;
```

## Schema

The app uses existing migrations from `foundation_db::schema`. No custom schema needed.

```rust
use foundation_db::schema::{MIGRATIONS, MigrationRunner};
use foundation_db::core::storage_provider::QueryStore;

// On app initialization, run migrations through D1:
let runner = MigrationRunner::new(MIGRATIONS);
let applied = runner.run(&d1_store)?;
```

This applies all 15 existing migrations including:
- `001_create_kv_store` — key-value store for `SessionManager`
- `002_create_users` — users table (email, username, password_hash, etc.)
- `003_create_sessions` — sessions table (token, expires_at, user_id FK)
- Plus OAuth, JWT, 2FA, rate limits, audit logs (available for extension)

## Project Structure

```
examples/cf-login-app/
├── Cargo.toml              # wasm-pack compatible, depends on foundation_* crates
├── src/
│   └── lib.rs              # wasm-bindgen entry point:
│                           #   - create_worker() -> CfHttpApp
│                           #   - route registration (HttpApp::new_cf(), route_cf)
│                           #   - handler impls (CfServe + CfServeFactory)
│                           #   - AuthMiddleware impl
├── wrangler.toml           # CF Workers config with D1 binding
└── package.json            # wasm-pack build scripts
```

Unlike a traditional web app, handlers are registered directly in `lib.rs` rather
than in separate files — the entire app is a single wasm module. The `CfHttpApp`
bridge handles all request/response conversion, so handlers work with structured
`CfConn` fields instead of HTTP wire bytes.

## wrangler.toml

```toml
name = "cf-login-app"
main = "build/worker/shim.mjs"
compatibility_date = "2024-01-01"

[[d1_databases]]
binding = "DB"
database_name = "cf-login-db"
database_id = "<database-id>"
```

## Tasks

1. [ ] Create `examples/cf-login-app/Cargo.toml` with foundation_* crate dependencies
2. [ ] Implement `src/lib.rs` with `CfHttpApp` bridge entry point and route registration
3. [ ] Wire up `MigrationRunner::new(MIGRATIONS)` against `D1WasmStorage` on first request
4. [ ] Implement registration handler (GET form + POST create user with argon2 password hash)
5. [ ] Implement login handler (GET form + POST auth against users table)
6. [ ] Implement dashboard handler with session validation
7. [ ] Implement logout handler (revoke session, clear cookies)

## Verification

```bash
# Local dev
cd examples/cf-login-app
wasm-pack build --target web
wrangler dev

# Deploy
wrangler deploy
```

---

_Created: 2026-05-15, Updated: 2026-05-19_
