---
feature: "Auth Wasm Compatibility"
description: "Fix uuid/chrono wasm features in foundation_auth, verify pure-Rust auth logic compiles on wasm32-unknown-unknown"
status: "pending"
priority: "high"
depends_on: ["01-core-wasm-compat"]
estimated_effort: "small"
created: 2026-05-15
last_updated: 2026-05-15
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0%
---

# Feature: Auth Wasm Compatibility

## Overview

Make `foundation_auth` compile on `wasm32-unknown-unknown`. The core auth logic (JwtManager, SessionManager, AuthToken, middleware guards) is already pure Rust — only dependency features need adjustment.

## Requirements

### 1. Fix `uuid` Feature

**Problem:** `uuid = { version = "1.0", features = ["v4"] }` triggers `compile_error!` on wasm32 without a randomness source.

**Action:** Add `js` feature for wasm target:
```toml
uuid = { version = "1.0", features = ["v4", "js"] }
```
The `js` feature uses browser crypto APIs via `getrandom`'s `js` backend on wasm32. On native targets, `js` is a no-op.

### 2. Fix `chrono` Feature

**Problem:** `chrono::Utc::now()` uses `std::time::SystemTime` which is unavailable on wasm32-unknown-unknown.

**Action:** Add `wasmbind` feature:
```toml
chrono = { version = "0.4", features = ["wasmbind"] }
```
This enables JavaScript `Date`-based time for wasm32 targets.

### 3. Fix `rand` Feature

**Problem:** `rand = "0.8"` uses `thread_rng()` which requires OS entropy on wasm32.

**Action:** Add `getrandom` with `js` feature:
```toml
getrandom = { version = "0.2", features = ["js"] }
```
This makes `rand` use browser crypto APIs for randomness on wasm32.

### 4. Verify Compilation

The auth crate's pure logic components should compile without changes:
- `JwtManager`, `Claims`, `JwtToken` — pure data types
- `SessionManager`, `Session`, `SessionConfig` — Arc/Mutex-based
- `require_auth`, `optional_auth`, `has_scope` — pure string parsing
- `extract_bearer_token`, `extract_session_token` — pure string parsing
- `CredentialStore`, `AuthToken` — pure data types

The only changes needed are in `Cargo.toml` feature flags.

## Tasks

1. [ ] Add `js` feature to `uuid` in `foundation_auth/Cargo.toml`
2. [ ] Add `wasmbind` feature to `chrono` in `foundation_auth/Cargo.toml`
3. [ ] Add `getrandom = { version = "0.2", features = ["js"] }` to `foundation_auth/Cargo.toml`
4. [ ] Verify `foundation_auth` compiles on wasm32 with compatible foundation_core

## Verification

```bash
# Wasm compilation (requires foundation_core built with wasm features first)
cargo build -p foundation_auth --target wasm32-unknown-unknown \
  --no-default-features --features foundation_core/ssl-rustls-awsrc,foundation_core/std \
  2>&1 | tee /tmp/wasm-auth.log
```

---

_Created: 2026-05-15_
