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
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# Feature: Example Login App

## Overview

A complete, deployable Cloudflare Workers login app demonstrating the full pipeline:
login page, authentication via `CfServe` handlers, D1-backed `SessionManager`,
protected dashboard route, and the `CfHttpApp` wasm-bindgen bridge.

## Architecture

```
Route Structure:
  GET  /login          → Login form (HTML)
  POST /login          → Authenticate (JSON or form)
  GET  /dashboard      → Protected route (requires auth)
  GET  /logout         → Clear session
  GET  /               → Redirect to /login or /dashboard

Data Flow:
  1. wasm_bindgen init: extract D1 binding from CF env → ContextBag
  2. On first request: run MigrationRunner::new(MIGRATIONS) against D1WasmStorage to create tables
  3. User visits /login → CfServe handler renders HTML form via CfConn
  4. POST /login → validate credentials against users table (migration 002)
  5. On success → SessionManager<D1WasmStorage> creates session, stored in kv_store table
  6. GET /dashboard → auth middleware checks JWT cookie; short-circuits with redirect if missing
  7. If valid → render dashboard HTML
  8. GET /logout → revoke session, clear cookies
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
    app.middleware(AuthMiddleware);
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
    let mut cf_app = CfHttpApp::new();
    // Routes are registered through cf_app.app() which returns &HttpApp<Arc<dyn CfServe>>
    // Then handlers call .route_cf::<Handler>(...) on a mutable copy
    cf_app
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

```rust
use foundation_auth::session::SessionManager;

// SessionManager is generic over CredentialStore — D1WasmStorage implements it:
let session_mgr = SessionManager::<D1WasmStorage>::new(d1_store);

// Create session on login:
let token = session_mgr.create_session(user_id, &bag)?;

// Verify on protected route:
let session = session_mgr.get_session(&token, &bag)?;
```

### Auth Middleware

```rust
use foundation_http::shared::middleware::{RequestMiddleware, MiddlewareResult};
use foundation_core::wire::simple_http::SimpleIncomingRequest;
use std::sync::Arc;

struct AuthMiddleware;

impl RequestMiddleware for AuthMiddleware {
    fn handle(&self, _ctx: &Arc<ContextBag>, req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        // Only protect /dashboard
        if !req.request_url.url.starts_with("/dashboard") {
            return MiddlewareResult::Continue;
        }

        // Check session cookie
        match req.headers.get(&SimpleHeader::from("Cookie".to_string())) {
            Some(cookies) if cookies.iter().any(|c| c.contains("session=")) => {
                MiddlewareResult::Continue
            }
            _ => MiddlewareResult::Response(SimpleOutgoingResponse {
                proto: Proto::HTTP11,
                status: Status::TemporaryRedirect,
                headers: {
                    let mut h = SimpleHeaders::new();
                    h.entry(SimpleHeader::from("Location".to_string()))
                        .or_default().push("/login".to_string());
                    h
                },
                body: None,
            }),
        }
    }
}
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
4. [ ] Implement login handler (GET form + POST auth against users table)
5. [ ] Implement dashboard handler with auth guard middleware
6. [ ] Implement logout handler (revoke session, clear cookies)

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
