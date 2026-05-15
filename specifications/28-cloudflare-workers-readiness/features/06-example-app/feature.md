---
feature: "Example Login App"
description: "Working login application deployable to Cloudflare Workers — login form, auth guard, D1-backed sessions, protected dashboard route"
status: "pending"
priority: "high"
depends_on: ["05-wasm-bindings"]
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
  1. User visits /login → renders HTML form
  2. POST /login → validate credentials against D1 users table
  3. On success → create JWT session token → set cookie
  4. GET /dashboard → auth middleware checks JWT cookie
  5. If valid → render dashboard HTML
  6. If invalid → redirect to /login
```

## D1 Schema

```sql
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id),
    token TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);
```

## Project Structure

```
examples/cf-login-app/
├── Cargo.toml              # wasm-pack compatible
├── src/
│   ├── lib.rs              # wasm-bindgen entry point
│   ├── handlers/
│   │   ├── login.rs        # GET form + POST auth
│   │   ├── dashboard.rs    # Protected page
│   │   └── logout.rs       # Session clear
│   └── middleware/
│       └── auth_guard.rs   # JWT cookie check
├── wrangler.toml           # CF Workers config
├── migrations/
│   └── 001_init.sql        # D1 schema
└── package.json            # wasm-pack build scripts
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
2. [ ] Implement login handler (form + auth)
3. [ ] Implement dashboard handler with auth guard
4. [ ] Implement logout handler
5. [ ] Create `wrangler.toml` with D1 binding
6. [ ] Create D1 migration and seed script

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
