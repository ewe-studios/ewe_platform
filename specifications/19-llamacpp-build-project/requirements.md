---
description: "Relocate llama.cpp git submodule from infrastructure/llama-bindings/llama.cpp to tools/llama.cpp and fix the build system include paths, Cargo.toml globs, and all config references"
status: "pending"
priority: "high"
created: 2026-05-02
updated: 2026-05-02
author: "Main Agent"
metadata:
  version: "1.0"
  estimated_effort: "medium"
  tags:
    - build-system
    - submodule-relocation
    - rust
    - llama.cpp
    - bindgen
  skills: []
  tools:
    - Rust
    - cargo
    - cmake
    - bindgen
    - git submodules
has_features: true
has_fundamentals: false
builds_on: ""
related_specs: []
features:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# 19: llama.cpp Build Project Relocation

## Overview

This specification covers moving the llama.cpp git submodule from `infrastructure/llama-bindings/llama.cpp` to `tools/llama.cpp` and fixing all build system references so that compilation continues to work correctly on all target platforms (linux, macos, android).

### Current State

The llama.cpp submodule lives at `infrastructure/llama-bindings/llama.cpp` (a sibling directory to `build.rs`, `wrapper.h`, and `wrapper_mtmd.h`). The build works because:

1. **build.rs** (`infrastructure/llama-bindings/build.rs`) reads `LLAMA_DIR` from the environment and passes two `-I` include paths to bindgen: `{llama_src}/include` and `{llama_src}/ggml/include`.
2. **wrapper.h** contains `#include "llama.cpp/include/llama.h"` — this resolves relative to the `wrapper.h` directory, finding `infrastructure/llama-bindings/llama.cpp/include/llama.h`.
3. **wrapper_mtmd.h** contains `#include "llama.cpp/tools/mtmd/mtmd.h"` and `#include "llama.cpp/tools/mtmd/mtmd-helper.h"` — same sibling-directory resolution.
4. **Cargo.toml** has `include = ["/llama.cpp/..."]` glob patterns for crate publishing.
5. **mise.toml** sets `LLAMA_DIR = "infrastructure/llama-bindings/llama.cpp"`.
6. **.cargo/config.toml** sets `LLAMA_DIR = "infrastructure/llama-bindings/llama.cpp"`.
7. **.gitmodules** declares the submodule at `infrastructure/llama-bindings/llama.cpp`.

### Problem

After moving the submodule to `tools/llama.cpp`, the `llama.cpp/` sibling directory next to `wrapper.h` and `wrapper_mtmd.h` no longer exists, so the `#include "llama.cpp/..."` directives break. Additionally, `LLAMA_DIR` and the Cargo.toml globs point to the old path.

### Solution

1. Add the llama.cpp root directory as a `-I` include path in `build.rs` so that includes in the wrapper headers resolve relative to the llama.cpp root instead of relying on a sibling directory.
2. Update the wrapper headers to use `-I`-relative paths (strip the `llama.cpp/` prefix).
3. Update all configuration references (`LLAMA_DIR`, `.gitmodules`, `Cargo.toml` globs) to point to the new location.

---

## Feature Index

### Pending Features (0/5 completed)

1. **[include-path-refactor](./features/01-include-path-refactor/feature.md)** - Fix wrapper.h and wrapper_mtmd.h to use -I-relative paths
2. **[build-rs-include-paths](./features/02-build-rs-include-paths/feature.md)** - Add llama_src root as -I path in build.rs
3. **[submodule-relocation](./features/03-submodule-relocation/feature.md)** - Move git submodule and update all config references
4. **[cargo-toml-glob-update](./features/04-cargo-toml-glob-update/feature.md)** - Update include globs in Cargo.toml
5. **[build-verification](./features/05-build-verification/feature.md)** - Verify build works on all platforms (linux, macos, android)

---

## Tasks

### Phase 1: build.rs Include Path Fix

- [ ] In `infrastructure/llama-bindings/build.rs`, around line 308-311, add a new clang_arg for the llama.cpp root directory: `clang_arg(format!("-I{}", llama_src.display()))`
- [ ] This must be added before the existing `llama_src.join("include")` and `llama_src.join("ggml/include")` entries
- [ ] Verify the order: root `-I` first, then more specific subdirectories

### Phase 2: Wrapper Header Updates

- [ ] In `infrastructure/llama-bindings/wrapper.h`, change:
  - `#include "llama.cpp/include/llama.h"` to `#include "include/llama.h"`
- [ ] In `infrastructure/llama-bindings/wrapper_mtmd.h`, change:
  - `#include "llama.cpp/tools/mtmd/mtmd.h"` to `#include "tools/mtmd/mtmd.h"`
  - `#include "llama.cpp/tools/mtmd/mtmd-helper.h"` to `#include "tools/mtmd/mtmd-helper.h"`

### Phase 3: Submodule Relocation

- [ ] Remove old submodule: `git submodule deinit -f infrastructure/llama-bindings/llama.cpp`
- [ ] Remove old directory: `rm -rf infrastructure/llama-bindings/llama.cpp`
- [ ] Add new submodule: `git submodule add https://github.com/ggml-org/llama.cpp tools/llama.cpp`
- [ ] Update `.gitmodules`: change path and section from `infrastructure/llama-bindings/llama.cpp` to `tools/llama.cpp`
- [ ] Update `mise.toml`:
  - `[env]` section: `LLAMA_DIR = { value = "tools/llama.cpp", relative=true, force = true }`
  - `tasks."git:update-submodules"`: change `infrastructure/llama-bindings/llama.cpp/` to `tools/llama.cpp/`
  - `tasks."llama:server:clean"`: change `$PROJECT_ROOT/infrastructure/llama-bindings/llama.cpp` to `$PROJECT_ROOT/tools/llama.cpp`
- [ ] Update `.cargo/config.toml`: change `LLAMA_DIR` value from `infrastructure/llama-bindings/llama.cpp` to `tools/llama.cpp`

### Phase 4: Cargo.toml Glob Updates

- [ ] In `infrastructure/llama-bindings/Cargo.toml`, the `include` array already uses `/llama.cpp/...` paths. Since these are relative to the crate root (where the submodule lives), after the move the submodule is no longer a child of the crate directory.
- [ ] **Decision needed**: The `/llama.cpp/` prefix in the include globs is relative to the crate directory. After relocation, llama.cpp is no longer inside `infrastructure/llama-bindings/`. These globs will break for `cargo publish`.
- [ ] **Options**:
  - Option A: Use a symlink `infrastructure/llama-bindings/llama.cpp -> ../../tools/llama.cpp` (simplest, preserves globs)
  - Option B: Update all globs to reference a relative path from workspace root (may not work for `cargo publish` which requires paths under the crate)
  - Option C: Remove llama.cpp sources from the `include` list entirely (the crate is not intended to be published standalone)
- [ ] Implement the chosen approach and verify `cargo package --no-verify` succeeds or is intentionally skipped

### Phase 5: Build Verification

- [ ] Run `cargo check -p infrastructure_llama_bindings` on linux — must succeed
- [ ] Run `cargo check -p infrastructure_llama_bindings --features mtmd` on linux — must succeed
- [ ] Run `cargo build -p infrastructure_llama_bindings` on macos — must succeed
- [ ] Run `cargo build -p infrastructure_llama_bindings --features metal` on macos — must succeed
- [ ] Run cross-build for android: `cargo check -p infrastructure_llama_bindings --target aarch64-linux-android` — must succeed
- [ ] Verify `mise run check` passes
- [ ] Verify `mise run llama:server:build` still works (uses `LLAMA_DIR`)

---

## Success Criteria

- [ ] `cargo check -p infrastructure_llama_bindings` succeeds without errors
- [ ] `cargo check -p infrastructure_llama_bindings --features mtmd` succeeds
- [ ] `cargo build -p infrastructure_llama_bindings` compiles and links
- [ ] `git submodule status` shows `tools/llama.cpp` at correct commit
- [ ] `.gitmodules` has no references to `infrastructure/llama-bindings/llama.cpp`
- [ ] `mise run check` passes
- [ ] No stale references to old path in `mise.toml`, `.cargo/config.toml`, or any other config files
- [ ] bindgen generates correct bindings (verified by checking `target/` output or running a downstream crate that uses the bindings)

---

## Agent Rules Reference

### Mandatory Rules for All Agents

Load these rules from `.agents/rules/`:

| Rule | File | Purpose |
|------|------|---------|
| 01 | `.agents/rules/01-rule-naming-and-structure.md` | File naming conventions |
| 02 | `.agents/rules/02-rules-directory-policy.md` | Directory policies |
| 03 | `.agents/rules/03-dangerous-operations-safety.md` | Dangerous operations safety |
| 04 | `.agents/rules/04-work-commit-and-push-rules.md` | Work commit and push rules |

### Role-Specific Rules

| Agent Type | Additional Rules to Load |
|------------|--------------------------|
| **Review Agent** | `.agents/rules/06-specifications-and-requirements.md` |
| **Implementation Agent** | `.agents/rules/13-implementation-agent-guide.md`, stack file |
| **Verification Agent** | `.agents/rules/08-verification-workflow-complete-guide.md`, stack file |
| **Documentation Agents** | `.agents/rules/06-specifications-and-requirements.md` |

### Stack Files

Load from `.agents/stacks/`:
- **Language**: Rust -> `.agents/stacks/rust.md`

---

## File Organization Reminder

ONLY these files allowed:
1. requirements.md - Requirements with tasks
2. LEARNINGS.md - All learnings
3. REPORT.md - All reports
4. VERIFICATION.md - Verification
5. PROGRESS.md - Current status (delete at 100%)
6. fundamentals/, features/, templates/ (optional)

FORBIDDEN: Separate learning/report/verification files

Consolidation: All learnings -> LEARNINGS.md, All reports -> REPORT.md

See Rule 06 "File Organization" for complete policy.
