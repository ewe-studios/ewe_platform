---
feature: "Example Login App"
description: "Working login application deployable to Cloudflare Workers — login form, auth guard, D1-backed sessions, protected dashboard route"
status: "pending"
priority: "high"
depends_on: ["03-http-wasm-compat", "02-auth-wasm-compat", "04-db-wasm-compat"]
estimated_effort: "medium"
created: 2026-05-15
last_updated: 2026-05-15
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# Feature: Example Login App

## Overview

A complete, deployable example app demonstrating the full Cloudflare Workers deployment pipeline: login page, authentication, protected routes, and D1-backed session storage.

## Architecture

```
Route Structure:
  GET  /login          → Login form (HTML)
  POST /login          → Authenticate (JSON or form)
  GET  /dashboard      → Protected route (requires auth)
  GET  /logout         → Clear session
  GET  /               → Redirect to /login or /dashboard

Data Flow:
  1. On init: run MigrationRunner::new(MIGRATIONS) to create users, sessions, kv_store tables in D1
  2. User visits /login → renders HTML form
  3. POST /login → validate credentials against existing users table (migration 002)
  4. On success → create JWT session via SessionManager (stored in kv_store, migration 001)
  5. GET /dashboard → auth middleware checks JWT cookie (foundation_auth guards)
  6. If valid → render dashboard HTML
  7. If invalid → redirect to /login
```

## Schema

The app uses existing migrations from `foundation_db::schema`. No custom schema needed.

```rust
use foundation_db::schema::{MIGRATIONS, MigrationRunner};

// On app initialization, run migrations through D1:
let runner = MigrationRunner::new(MIGRATIONS);
let applied = runner.run(&d1_store)?;
```

This applies all 15 existing migrations including:
- `002_create_users` — users table (email, username, password_hash, etc.)
- `003_create_sessions` — sessions table (token, expires_at, user_id FK)
- `001_create_kv_store` — key-value store for SessionManager
- Plus OAuth, JWT, 2FA, rate limits, audit logs (available for extension)

## Project Structure

```
examples/cf-login-app/
├── Cargo.toml              # wasm-pack compatible
├── src/
│   ├── lib.rs              # wasm-bindgen entry point + init
│   ├── handlers/
│   │   ├── login.rs        # GET form + POST auth
│   │   ├── dashboard.rs    # Protected page
│   │   ├── logout.rs       # Session clear
│   │   └── register.rs     # User registration (optional)
│   └── middleware/
│       └── auth_guard.rs   # JWT cookie check
├── wrangler.toml           # CF Workers config
├── package.json            # wasm-pack build scripts
└── worker/                 # wasm-bindgen output
    └── ...
```

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

1. [ ] Create `examples/cf-login-app/` project structure
2. [ ] Wire up `foundation_db::schema::MIGRATIONS` to D1 storage on init
3. [ ] Implement login handler (form + auth against existing users table)
4. [ ] Implement dashboard handler with auth guard
5. [ ] Implement logout handler
6. [ ] Create `wrangler.toml` with D1 binding

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

_Created: 2026-05-15_
