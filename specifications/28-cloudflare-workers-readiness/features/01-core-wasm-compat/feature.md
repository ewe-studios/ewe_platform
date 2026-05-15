---
feature: "Core Wasm Compatibility"
description: "Remove dead ctrlc dependency, add wasm32 feature flags for uuid/chrono/rand, implement SSL backend switching (aws-lc-rs for wasm, ring for native)"
status: "pending"
priority: "high"
depends_on: []
estimated_effort: "medium"
created: 2026-05-15
last_updated: 2026-05-15
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# Feature: Core Wasm Compatibility

## Overview

Make `foundation_core` compile cleanly on `wasm32-unknown-unknown` target by removing dead dependencies, adding wasm-compatible feature flags, and implementing automatic SSL backend selection.

## Requirements

### 1. Remove Dead `ctrlc` Dependency

**Problem:** `ctrlc` is declared in `Cargo.toml` but never imported or used in any source file. It fails to compile on wasm32 (uses OS signal handling).

**Action:** Remove `ctrlc = { version = "3.4" }` from `foundation_core/Cargo.toml`.

### 2. SSL Backend Selection

**Problem:** `ring` (used by default via `ssl-rustls-ring`) doesn't support wasm32 — `SystemRandom` has no `SecureRandom` impl on that target.

**Current state:**
- SSL modules already gated: `#[cfg(not(target_arch = "wasm32"))]` on rustls.rs, openssl.rs, native_ttls.rs, ssl/mod.rs
- wasm stub exists: `#[cfg(target_arch = "wasm32")]` on netcap/wasm.rs
- Features already defined: `ssl-rustls-ring`, `ssl-rustls-awsrc` (aws-lc-rs), `ssl-openssl`, `ssl-native-tls`

**Action:** No code changes needed — SSL modules are already properly gated. The `ssl-rustls-awsrc` feature (aws-lc-rs) compiles on wasm32-unknown-unknown. Users building for wasm should use:
```
cargo build --target wasm32-unknown-unknown --no-default-features --features ssl-rustls-awsrc,std
```

**Verification:** `cargo build --target wasm32-unknown-unknown --no-default-features --features ssl-rustls-awsrc,std` succeeds with zero errors.

### 3. Feature Flag Defaults

**Problem:** Default features include `ssl` which maps to `ssl-rustls-ring`, breaking wasm builds.

**Action:** Document the wasm-compatible feature combination. No Cargo.toml change needed — defaults are fine for native, users override for wasm.

## Tasks

1. [ ] Remove `ctrlc` dependency from `foundation_core/Cargo.toml`
2. [ ] Verify `foundation_core` compiles on wasm32 with `--no-default-features --features ssl-rustls-awsrc,std`
3. [ ] Verify `foundation_core` still compiles on native with default features
4. [ ] Add `#[cfg(target_arch = "wasm32")]` gate to any remaining non-wasm modules (if found)
5. [ ] Run `cargo clippy --target wasm32-unknown-unknown` and fix all warnings

## Verification

```bash
# Wasm compilation
cargo build -p foundation_core --target wasm32-unknown-unknown \
  --no-default-features --features ssl-rustls-awsrc,std 2>&1 | tee /tmp/wasm-core.log

# Native compilation (no regression)
cargo build -p foundation_core 2>&1 | tee /tmp/native-core.log
```

---

_Created: 2026-05-15_
