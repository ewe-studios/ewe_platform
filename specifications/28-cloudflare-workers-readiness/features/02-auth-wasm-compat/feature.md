---
feature: "Auth Wasm Compatibility"
description: "Fix uuid/chrono wasm features in foundation_auth, verify pure-Rust auth logic compiles on wasm32-unknown-unknown"
status: "implemented"
priority: "high"
depends_on: ["01-core-wasm-compat"]
estimated_effort: "small"
created: 2026-05-15
last_updated: 2026-05-15
author: "Main Agent"
tasks:
  completed: 3
  uncompleted: 0
  total: 3
  completion_percentage: 100%
---

# Feature: Auth Wasm Compatibility

## Overview

Make `foundation_auth` compile on `wasm32-unknown-unknown`. The core auth logic (JwtManager, SessionManager, AuthToken, middleware guards) is already pure Rust — only dependency features need adjustment via a unified `wasm` feature flag.

## Requirements

### Single `wasm` Feature Flag

All wasm-specific features are consolidated behind one flag. The base `[dependencies]` section stays clean with no wasm-specific features:

```toml
[dependencies]
uuid = { version = "1.0", features = ["v4"] }
chrono = "0.4"
rand = "0.8"
getrandom = { version = "0.2", optional = true }

[features]
wasm = ["uuid/js", "chrono/wasmbind", "getrandom/js"]
```

**Why not target-conditional (`[target.'cfg(target_arch = "wasm32")'.dependencies]`)?**
- `uuid/js` pulls in `wasm-bindgen` which fails to compile on native targets
- `chrono/wasmbind` brings in web-sys dependencies
- A feature flag is cleaner — only `wasm` target users pass `--features wasm`

Build commands:
```bash
# Native — normal build
cargo build -p foundation_auth

# Wasm — explicitly enable wasm features
cargo build -p foundation_auth --target wasm32-unknown-unknown --features wasm
```

### What Each Feature Does

| Feature | Effect |
|---------|--------|
| `uuid/js` | Uses browser crypto APIs via getrandom's js backend for V4 UUID generation |
| `chrono/wasmbind` | Uses JavaScript `Date`-based time for `Utc::now()` |
| `getrandom/js` | Uses browser crypto APIs for randomness in `rand` |

### Verification

The auth crate's pure logic components should compile without changes:
- `JwtManager`, `Claims`, `JwtToken` — pure data types
- `SessionManager`, `Session`, `SessionConfig` — Arc/Mutex-based
- `require_auth`, `optional_auth`, `has_scope` — pure string parsing
- `extract_bearer_token`, `extract_session_token` — pure string parsing
- `CredentialStore`, `AuthToken` — pure data types

## Tasks

1. [ ] Add `wasm` feature to `foundation_auth/Cargo.toml` with `uuid/js`, `chrono/wasmbind`, `getrandom/js`
2. [ ] Add `getrandom = { version = "0.2", optional = true }` to dependencies
3. [ ] Verify `foundation_auth` compiles on wasm32 with `--features wasm`

## Verification

```bash
# Wasm compilation (requires foundation_core built with wasm features first)
cargo build -p foundation_auth --target wasm32-unknown-unknown \
  --features wasm,foundation_core/ssl-rustls-awsrc,foundation_core/std \
  2>&1 | tee /tmp/wasm-auth.log
```

---

_Created: 2026-05-15_
