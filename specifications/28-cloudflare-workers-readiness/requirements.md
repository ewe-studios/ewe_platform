---
description: "Make foundation crates wasm32-unknown-unknown compatible for Cloudflare Workers deployment. Covers SSL backend switching, dead dependency removal, wasm feature flags, wasm-bindgen bindings layer, and a working login example app deployable via wrangler."
status: "pending"
priority: "high"
created: 2026-05-15
updated: 2026-05-15
author: "Main Agent"
metadata:
  version: "1.0"
  estimated_effort: "large"
  tags:
    - wasm
    - cloudflare-workers
    - deployment
    - wasm32-unknown-unknown
    - aws-lc-rs
    - turso
    - wrangler
  skills: []
  tools:
    - Rust
    - cargo
    - wrangler
    - wasm-pack
has_features: true
has_fundamentals: false
builds_on: "specifications/11-foundation-deployment"
related_specs:
  - "specifications/03-wasm-friendly-sync-primitives"
  - "specifications/21-http-framework"
features:
  completed: 0
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---

# Cloudflare Workers Readiness Specification

## Overview

This specification makes the foundation crates compile and run on `wasm32-unknown-unknown` target, specifically for deployment to Cloudflare Workers via `wasm-bindgen` + `wrangler`.

**Scope:** The crates in scope are:
1. `foundation_core` — core runtime, SSL, networking primitives
2. `foundation_auth` — authentication, JWT, sessions, middleware guards
3. `foundation_http` — HTTP serving framework (Serve trait, Router, HttpApp)
4. `foundation_db` — database abstraction (Turso, D1, R2 backends)
5. `foundation_wasm` — existing custom FFI layer (augment with wasm-bindgen)

**Out of scope:** `foundation_deployment` (spec 11 covers that separately).

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│              Cloudflare Worker (V8 isolate)             │
│                                                         │
│  ┌───────────────────────────────────────────────────┐  │
│  │  wasm-bindgen entry point (lib.rs)                │  │
│  │  ┌─────────────────────────────────────────────┐  │  │
│  │  │  foundation_auth logic (pure Rust)          │  │  │
│  │  │  - JwtManager, SessionManager               │  │  │
│  │  │  - Auth guards (require_auth, has_scope)    │  │  │
│  │  └─────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────┐  │  │
│  │  │  foundation_db via Cloudflare bindings      │  │  │
│  │  │  - D1 via JS env.DB binding (wasm-bindgen)  │  │  │
│  │  │  - R2 via JS env.BUCKET binding             │  │  │
│  │  │  - Turso via libSQL WASI runtime (optional) │  │  │
│  │  └─────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────┐  │  │
│  │  │  foundation_http handler logic (Serve impl) │  │  │
│  │  │  - Request parsing from CF FetchEvent       │  │  │
│  │  │  - Response building → JS Response object   │  │  │
│  │  └─────────────────────────────────────────────┘  │  │
│  └─────────────────────────────────────────────────┘  │
│                                                         │
│  Bindings layer (wasm-bindgen glue):                   │
│  - Request/Response ↔ SimpleIncoming/SimpleOutgoing    │
│  - env.DB → D1 prepared statements                     │
│  - env.BUCKET → R2 get/put                             │
└─────────────────────────────────────────────────────────┘
```

### Wasm Compatibility Strategy

Each crate follows a three-tier approach:

| Tier | Approach | Examples |
|------|----------|----------|
| **Tier 1: Pure logic** | Already wasm-compatible — no changes needed | JwtManager, SessionManager, auth guards, data types |
| **Tier 2: Feature-flagged deps** | Add `js`/`wasmbind` features to deps; gate non-wasm deps | uuid (js), chrono (wasmbind), ctrlc (remove), rand (getrandom/js) |
| **Tier 3: Conditional compilation** | `#[cfg(target_arch = "wasm32")]` gates for TCP/threading | HttpServer, RawStream, SSL backends |

### SSL Backend Selection

- **wasm32**: `aws-lc-rs` via `ssl-rustls-awsrc` feature (pure Rust, compiles to wasm32)
- **native**: `ring` via `ssl-rustls-ring` feature (default for x86_64/aarch64)
- Already partially implemented: SSL modules gated with `#[cfg(not(target_arch = "wasm32"))]`

### Turso in wasm

Turso's libSQL engine is SQLite-compatible and supports WASM. The embedded SQLite engine can run in WASI environments. For Cloudflare Workers specifically:
- Workers has **no filesystem** and **no WASI** — embedded SQLite won't work directly
- Turso's **remote/libSQL over HTTP** mode works on wasm (uses HTTP, not filesystem)
- Turso's JavaScript bindings (`@tursodatabase/api`) can be accessed via wasm-bindgen

**Decision:** For wasm32-unknown-unknown, use Turso's HTTP remote mode or JavaScript bindings via wasm-bindgen. The embedded local mode requires WASI and is not available on Cloudflare Workers.

## Known Issues

1. **`ctrlc` dead dependency** — declared in `foundation_core/Cargo.toml` but never used. Blocks wasm compilation.
2. **`uuid` needs `js` feature** — `compile_error!` on wasm32 without it.
3. **`chrono` needs `wasmbind` feature** — `Utc::now()` unavailable on wasm32.
4. **`ring` doesn't support wasm32** — `SystemRandom` missing `SecureRandom` impl.
5. **`rand::thread_rng()`** — needs `getrandom` with `js` feature on wasm32.
6. **`foundation_http` Serve trait** — uses `SharedByteBufferStream<RawStream>` which requires TCP sockets.

## Feature Index

### Pending Features (0/7 completed)

1. **[core-wasm-compat](./features/01-core-wasm-compat/feature.md)** — Remove dead ctrlc dep, add wasm feature flags, SSL backend switching
2. **[auth-wasm-compat](./features/02-auth-wasm-compat/feature.md)** — Fix uuid/chrono wasm features, verify auth logic compiles
3. **[http-wasm-compat](./features/03-http-wasm-compat/feature.md)** — Gate TCP-dependent code, make Serve trait usable on wasm
4. **[db-wasm-compat](./features/04-db-wasm-compat/feature.md)** — Gate turso/libsql embedded, enable D1/R2 wasm bindings
5. **[wasm-bindings](./features/05-wasm-bindings/feature.md)** — wasm-bindgen layer for CF Workers (Request/Response/D1/R2)
6. **[example-app](./features/06-example-app/feature.md)** — Working login app deployable to Cloudflare Workers
7. **[ci-wasm-checks](./features/07-ci-wasm-checks/feature.md)** — CI pipeline for wasm32 compilation checks

---

## Feature Dependencies

```
01-core-wasm-compat (base)
    |
    +----+----+----+
    |    |    |    |
    v    v    v    v
02-auth 03-http 04-db  (parallel)
    |    |    |
    +----+----+
         |
         v
    05-wasm-bindings
         |
         v
    06-example-app
         |
         v
    07-ci-wasm-checks
```

---

## Success Criteria (Spec-Wide)

### Compilation
- [ ] `cargo build --target wasm32-unknown-unknown` succeeds for foundation_core
- [ ] `cargo build --target wasm32-unknown-unknown` succeeds for foundation_auth
- [ ] `cargo build --target wasm32-unknown-unknown` succeeds for foundation_http (wasm-compatible subset)
- [ ] `cargo build --target wasm32-unknown-unknown` succeeds for foundation_db (d1 feature only)

### Deployment
- [ ] Example login app builds with `wasm-pack build --target web`
- [ ] Example app deploys with `wrangler deploy`
- [ ] Login page renders and accepts credentials
- [ ] Auth guard protects protected routes
- [ ] D1 database operations work in production

### Code Quality
- [ ] `cargo clippy --target wasm32-unknown-unknown -- -D warnings` passes
- [ ] No regressions on native target (`cargo clippy -- -D warnings` on x86_64)
- [ ] All existing tests pass on native target

---

## Prerequisites

- `wasm-pack` installed (`cargo install wasm-pack`)
- `wrangler` installed (`npm install -g wrangler`)
- Cloudflare account with API token
- D1 database created in Cloudflare dashboard

---

_Created: 2026-05-15_
_Structure: Feature-based (has_features: true)_
