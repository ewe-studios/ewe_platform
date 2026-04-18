---
name: "cranelift-incremental"
description: "Enable Cranelift codegen backend and incremental compilation for faster dev builds"
status: "pending"
priority: "high"
created: "2026-04-18"
author: "Main Agent"
metadata:
  version: "1.0"
  estimated_effort: "low"
  tags:
    - cranelift
    - incremental
    - build-optimization
dependencies: []
features: []
---

# Cranelift + Incremental Compilation

## Overview

Enable faster compilation for development builds by:
1. Enabling incremental compilation (currently disabled)
2. Using Cranelift codegen backend instead of LLVM

## Current State

### Incremental Compilation

**Workspace `Cargo.toml`:**
```toml
[profile.dev]
opt-level = 0
debug = true
debug-assertions = true
incremental = false  # ❌ Forces full recompilation
```

### Cranelift Backend

Cranelift is already installed:
```
rustc-codegen-cranelift-x86_64-unknown-linux-gnu (installed)
```

But not enabled:
```toml
[profile.dev]
# codegen-backend = "cranelift"  # ❌ Commented out
```

### .cargo/config.toml

```toml
[unstable]
codegen-backend = true

# [profile.dev]
# codegen-backend = "cranelift"  # ❌ Not configured
```

## Target Configuration

### Workspace `Cargo.toml`

```toml
[profile.dev]
opt-level = 0
debug = true
debug-assertions = true
incremental = true              # ✅ Enable incremental
codegen-backend = "cranelift"   # ✅ Use Cranelift
```

### .cargo/config.toml

```toml
[unstable]
codegen-backend = true

[profile.dev]
codegen-backend = "cranelift"
```

## What Cranelift Changes

| Phase | LLVM | Cranelift | Speedup |
|-------|------|-----------|---------|
| Parsing | Same | Same | - |
| Name resolution | Same | Same | - |
| Type checking | Same | Same | - |
| MIR optimization | Same | Same | - |
| **Codegen (opt-level=0)** | Slow | Fast | 2-3x |
| **Memory usage** | High | Lower | 30-50% |

**Important:** Cranelift only affects the codegen phase. Type checking and macro expansion still run at full speed. This is why Phase 1 (unified generator with feature flags) has the biggest impact.

## Tasks

- [ ] Read current workspace `Cargo.toml` profile configuration
- [ ] Set `incremental = true` in `[profile.dev]`
- [ ] Add `codegen-backend = "cranelift"` to `[profile.dev]`
- [ ] Configure `.cargo/config.toml`
- [ ] Run `cargo clean` to clear old LLVM artifacts
- [ ] Build and verify Cranelift is active
- [ ] Benchmark clean build time
- [ ] Benchmark incremental build time
- [ ] Document Cranelift limitations (release builds still use LLVM)

## Verification

```bash
# Clean LLVM artifacts
cargo clean

# Build with Cranelift
time cargo check -p foundation_deployment --features "gcp_run"

# Should see faster codegen phase
# Output may mention "cranelift" in verbose mode

# Verify incremental works
touch src/lib.rs
time cargo check -p foundation_deployment --features "gcp_run"
# Should be < 5 seconds for incremental
```

## Expected Impact

| Scenario | Before | After |
|----------|--------|-------|
| Clean build (full GCP) | 2+ hours | 30-40 min |
| Clean build (gcp_run only) | 2 min | 45 sec |
| Incremental build | 5 min | 15-20 sec |

**Note:** These are compounding improvements:
- Feature flags (Phase 1): 95% reduction
- Cranelift + Incremental (Phase 4): Additional 2-3x codegen speedup

## Cranelift Limitations

| Limitation | Impact |
|------------|--------|
| Debug info less complete | Debugger may show less info |
| Some optimizations missing | Only affects dev builds |
| Nightly only | Required for `codegen-backend` flag |
| Not all targets supported | x86_64 Linux/Windows/Mac OK |

**Release builds** should continue using LLVM with LTO for optimal performance.

## Release Profile (Unchanged)

```toml
[profile.release]
opt-level = 3
lto = true
strip = "debuginfo"
# LLVM stays for release
```

---

_Version: 1.0 - Created: 2026-04-18_
