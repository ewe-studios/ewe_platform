# Feature: cargo-toml-glob-update

## Description

Update the `include` array in `infrastructure/llama-bindings/Cargo.toml` to account for the llama.cpp submodule moving out of the crate directory.

## Problem

The current `include` array has ~50 glob patterns all prefixed with `/llama.cpp/`. These paths are relative to the crate root (`infrastructure/llama-bindings/`). After the submodule moves to `tools/llama.cpp`, these paths no longer resolve within the crate directory.

`cargo publish` and `cargo package` only allow files that are under the crate directory, so globs pointing outside the crate will cause packaging failures.

## Options

### Option A: Symlink (Recommended for preserving publish)

Create a symlink at `infrastructure/llama-bindings/llama.cpp` pointing to `../../tools/llama.cpp`:

```
infrastructure/llama-bindings/llama.cpp -> ../../tools/llama.cpp
```

This preserves all existing glob patterns without changes. Cargo follows symlinks during packaging.

**Pros:** No glob changes needed, `cargo publish` continues to work.
**Cons:** Symlink may not work on all platforms (Windows requires developer mode or admin).

### Option B: Update globs to workspace-root relative (Does NOT work for publish)

Change globs to reference the workspace root path. However, `cargo publish` does not support paths outside the crate directory, so this breaks publishing.

### Option C: Remove llama.cpp from include list

If `infrastructure_llama_bindings` is never intended to be published to crates.io independently (it depends on the llama.cpp sources being present), the include list can be stripped to only include the crate's own sources:

```toml
include = [
    "wrapper.h",
    "wrapper_mtmd.h",
    "build.rs",
    "/src",
]
```

**Pros:** Clean, no symlinks, no cross-directory references.
**Cons:** Crate cannot be published standalone.

## Decision

The implementation should evaluate whether `infrastructure_llama_bindings` is intended for crates.io publication:
- If yes: use Option A (symlink)
- If no: use Option C (strip include list)

Given the crate name includes `infrastructure_` and it lives in an infrastructure directory, Option C is likely appropriate. The workspace publishes as a whole, not individual infrastructure crates.

## Verification

- If Option A: `ls -la infrastructure/llama-bindings/llama.cpp` shows symlink, `cargo package --no-verify` succeeds
- If Option C: `cargo package --no-verify` succeeds with minimal include list
- Build (`cargo build -p infrastructure_llama_bindings`) is unaffected by include list changes
