---
name: "Cleanup and Verification"
description: "Remove migrated test files from tests/, update module declarations, and verify no regressions"
status: "completed"
priority: "high"
dependencies: ["01-migrate-foundation-core-tests", "02-migrate-foundation-nostd-tests", "03-migrate-foundation-macros-tests", "04-migrate-foundation-testing-tests"]
estimated_effort: "medium"
---

# Feature: Cleanup and Verification

## Overview

Final cleanup phase: remove migrated files, update module declarations, and verify the entire test suite passes.

## Prerequisites

All previous features must be complete:
- [ ] foundation_core tests migrated
- [ ] foundation_nostd tests migrated
- [ ] foundation_macros tests migrated
- [ ] foundation_testing tests migrated

## Cleanup Tasks

### 1. Update tests/backends/mod.rs

Remove migrated modules:
```rust
// Before:
pub mod foundation_core;
pub mod foundation_nostd;

// After:
// Keep only cross-crate integration tests
// (if any remain)
```

### 2. Delete Empty Directories

Remove from `tests/backends/`:
- `foundation_core/` (entire directory)
- `foundation_nostd/` (entire directory)
- `test_macros_embeddeders.rs`
- `tests.rs`

### 3. Update tests/mod.rs

If `backends/mod.rs` becomes empty, update `tests/mod.rs`:
```rust
// Remove or comment out if no tests remain
// mod backends;
```

### 4. Verify tests/Cargo.toml

Review dependencies - may be able to remove some:
- Keep: `foundation_core`, `foundation_testing` (for integration tests)
- Review: other dependencies that were only for migrated tests

## Verification Steps

### Full Test Suite

Run all tests to ensure no regressions:

```bash
# Test each migrated crate
cargo test -p foundation_core
cargo test -p foundation_nostd
cargo test -p foundation_macros
cargo test -p foundation_testing

# Test the integration test crate (if any tests remain)
cargo test -p ewe_platform_tests

# Full workspace test
cargo test --workspace
```

### Dependency Check

Verify no circular dependencies:
```bash
cargo tree -p foundation_core
cargo tree -p foundation_testing
cargo tree -p ewe_platform_tests
```

### Compilation Check

```bash
cargo check --workspace
cargo check --workspace --tests
```

## Rollback Plan

If critical issues found:
1. Restore files from git: `git checkout tests/backends/`
2. Revert changes to `backends/*/tests/mod.rs`
3. Re-run tests to confirm restoration

## Success Criteria

- [ ] All migrated tests pass in new locations
- [ ] Original test files removed from `tests/`
- [ ] No circular dependency errors
- [ ] CI passes
- [ ] Documentation updated (if any references old test locations)

## Final State

### tests/ Directory
```
tests/
├── Cargo.toml          # Keep, but may have fewer deps
├── mod.rs              # May be simplified or removed
└── [cross-crate integration tests if any]
```

### backends/*/tests/ Directories
```
backends/foundation_core/tests/       # ~20 test files
backends/foundation_nostd/tests/      # wasm tests
backends/foundation_macros/tests/     # embedder tests
backends/foundation_testing/tests/    # self-tests
```

## Notes

- The `ewe_platform_tests` crate may be removed entirely if no cross-crate tests remain
- If cross-crate tests exist, they justify keeping the separate test crate
- Document the new test organization in AGENTS.md or README
