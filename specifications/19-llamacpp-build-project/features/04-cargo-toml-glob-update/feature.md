# Feature: cargo-toml-glob-update

## Description

Create a symlink so that the llama.cpp sources remain accessible to `infrastructure_llama_bindings` for `cargo publish`, preserving all existing `include` glob patterns.

## Problem

The current `include` array in `Cargo.toml` has ~50 glob patterns prefixed with `/llama.cpp/`, relative to the crate root (`infrastructure/llama-bindings/`). After moving the submodule to `tools/llama.cpp`, these paths no longer resolve within the crate directory.

## Decision: Symlink (Option A)

This crate **will be published** alongside the rest of the workspace. We preserve the existing `include` globs by creating a symlink:

```
infrastructure/llama-bindings/llama.cpp -> ../../tools/llama.cpp
```

This keeps every existing glob pattern valid and `cargo publish` follows symlinks during packaging.

## Implementation

- [ ] Remove the old submodule directory (after `git submodule deinit`)
- [ ] Create symlink: `ln -s ../../tools/llama.cpp infrastructure/llama-bindings/llama.cpp`
- [ ] Add symlink to `.gitignore` or ensure git tracks it as a symlink (git tracks symlinks as symlinks)
- [ ] Verify `cargo package --no-verify` succeeds
- [ ] No changes needed to the `include` array in `Cargo.toml`

## Windows Consideration

Windows requires developer mode or admin privileges for symlinks. If this becomes a blocker for Windows contributors, we can fall back to a build.rs pre-check that resolves the symlink target or uses a junction point on Windows specifically.

## Verification

- `ls -la infrastructure/llama-bindings/llama.cpp` shows symlink to `../../tools/llama.cpp`
- `cargo package --no-verify` succeeds
- `cargo build -p infrastructure_llama_bindings` compiles and links
- `git status` shows the symlink as a tracked file (mode 120000)
