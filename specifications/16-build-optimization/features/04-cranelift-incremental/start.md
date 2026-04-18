# Cranelift + Incremental - Start Here

## Goal

Enable Cranelift codegen backend and incremental compilation for faster development builds.

## Why

Current configuration:
- `incremental = false` - full recompilation every time
- Cranelift available but unused - LLVM does all codegen

## Changes Required

### 1. Workspace Cargo.toml

```toml
[profile.dev]
incremental = true              # Enable incremental
codegen-backend = "cranelift"   # Use Cranelift
```

### 2. .cargo/config.toml

```toml
[unstable]
codegen-backend = true

[profile.dev]
codegen-backend = "cranelift"
```

## First Steps

1. Read current `Cargo.toml` profile configuration
2. Update `incremental` and `codegen-backend` settings
3. Run `cargo clean` to clear old artifacts
4. Verify Cranelift is active

## Verification

```bash
# Clean and rebuild
cargo clean
time cargo check -p foundation_deployment --features "gcp_run"

# Should see:
# - Faster codegen phase
# - Lower memory usage
```

## Expected Impact

| Scenario | Before | After |
|----------|--------|-------|
| Clean build (gcp_run) | 2 min | 45 sec |
| Incremental | 5 min | 15-20 sec |
