# Progress - Zero Warnings Workspace

_Last updated: 2026-04-12 (spec created, cleanup not yet started)_

## Overview

Workspace-wide cleanup to eliminate all Rust warnings (clippy, cargo, doc).
Target: **zero warnings** with **zero suppression** except for llama.cpp FFI.

## Phase Status

| Phase | Crates | Status | Warnings Eliminated |
|-------|--------|--------|---------------------|
| 1: Small | `foundation_auth`, `foundation_testing`, `foundation_macros`, `foundation_db`, `foundation_nostd` | 🔄 In Progress | 1 / 52 |
| 2: Medium | `foundation_codegen`, `foundation_deployment`, `foundation_openapi`, `ewe_platform` bin | ⬜ Pending | 0 / 299 |
| 3: Large | `foundation_ai`, `ewe_platform_tests`, `foundation_core` | ⬜ Pending | 0 / 1,145 |
| 4: Verification | Workspace-wide checks | ⬜ Pending | — |

**Total:** 1 / ~1,500 warnings eliminated.

Status key: ⬜ Pending 🔄 In Progress ✅ Complete

## Completed Work

None yet — spec freshly created.

## In Progress

None yet.

## Next Up

**Phase 1, Crate 1: `foundation_auth`** (1 warning)

```bash
cargo clippy --package foundation_auth --all-targets -- -D warnings
```

Fix the single test warning, verify clean, commit, move to `foundation_testing`.

---

## Workspace Baseline (Pre-Cleanup)

```
cargo check --workspace                    # 12 warnings (ewe_platform bin dead code)
cargo clippy --workspace --all-targets     # ~1,500 warnings across 15 crates
cargo doc --workspace --no-deps            # 0 warnings (clean baseline)
cargo fmt --workspace --check              # Some drift in jwt.rs, oauth.rs
```

---

## Suppression Removal Log

| Location | Suppression Removed | Date | Notes |
|----------|---------------------|------|-------|
| — | — | — | None yet |

---

## Generated Code Fixes

| Generator | Fix Applied | Regenerated | Date | Notes |
|-----------|-------------|-------------|------|-------|
| — | — | — | None yet |

---

## llama.cpp FFI Exceptions (Authorized Suppressions)

These are the **only** allowed suppressions in the workspace:

| File | Suppression | Reason |
|------|-------------|--------|
| `infrastructure/llama-bindings/src/lib.rs` | `not_unsafe_ptr_arg_deref`, `missing_safety_doc` | FFI boundary, C API patterns |
| `infrastructure/llama-cpp/src/*.rs` | TBD | Safe wrapper — minimize scope |

**Note:** Any new `#[allow(...)]` outside these files must be justified in
`requirements.md` and approved.

---

## Blockers / Decisions Needed

None yet.
