---
description: "Migrate single-crate tests from ./tests (ewe_platform_tests) into their respective crate test directories to eliminate circular dependency concerns and follow Rust testing best practices."
status: "pending"
priority: "medium"
created: "2026-05-13"
author: "Claude"
metadata:
  version: "1.0"
  last_updated: "2026-05-13"
  estimated_effort: "medium"
  tags:
    - testing
    - refactoring
    - crate-structure
    - rust-best-practices
  skills: []
  tools:
    - Rust
    - cargo
has_features: true
has_fundamentals: true
builds_on: null
related_specs: []
features:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0
---

# Overview

This specification defines the migration of single-crate tests from the `ewe_platform_tests` crate (located at `./tests/`) into their respective crate-specific test directories. The goal is to:

1. **Eliminate circular dependency concerns**: Tests that only test `foundation_core` should live in `foundation_core/tests/`
2. **Follow Rust best practices**: Tests belong with the code they test
3. **Clarify test ownership**: Single-crate vs. cross-crate integration tests
4. **Simplify dependency management**: Dev-dependencies don't propagate to regular dependencies

## The Problem

Currently, `ewe_platform_tests` is a separate test crate that:
- Depends on `foundation_core` (regular dependency)
- Depends on `foundation_testing` (regular dependency)
- `foundation_testing` depends on `foundation_core`

This creates a chain: `ewe_platform_tests` → `foundation_testing` → `foundation_core`

While this isn't technically circular, it creates confusion about where tests should live.

## The Solution

Move tests that only test a single crate into that crate's `tests/` directory, using `foundation_testing` as a **dev-dependency** (which doesn't cause circular dependencies).

## Migration Categories

### Tests to Move (Single-Crate Tests)

| Source | Target Crate | Test Files |
|--------|--------------|------------|
| `tests/backends/foundation_core/*` | `foundation_core` | ~20 test files |
| `tests/backends/foundation_nostd/*` | `foundation_nostd` | 4 test files |
| `tests/backends/test_macros_embeddeders.rs` | `foundation_macros` | 1 test file |
| `tests/backends/tests.rs` | `foundation_testing` | 1 test file |

### Tests to Keep (Cross-Crate Integration)

Any tests that verify interaction between multiple crates should remain in `ewe_platform_tests`.

## Implementation Location

- Specification: `specifications/22-test-crate-migration/`
- Target directories:
  - `backends/foundation_core/tests/`
  - `backends/foundation_nostd/tests/`
  - `backends/foundation_macros/tests/`
  - `backends/foundation_testing/tests/`

## Why Dev-Dependencies Are Safe

In Rust, dev-dependencies are **only used when testing the declaring crate**:

```
foundation_core (dev-deps)
    └── foundation_testing ──► foundation_core (regular deps, dev-deps stripped)
```

When building `foundation_testing`:
- `foundation_core` is fetched as a regular dependency
- `foundation_core`'s dev-dependencies are **NOT** activated

When running `cargo test -p foundation_core`:
- Only then is `foundation_testing` added
- This is a separate build context

## Verification Criteria

- [ ] All single-crate tests moved to appropriate crate `tests/` directories
- [ ] `foundation_testing` remains a dev-dependency in target crates (already configured)
- [ ] Tests compile and pass in new locations
- [ ] Cross-crate integration tests remain in `./tests/`
- [ ] No circular dependency errors introduced
- [ ] Original files removed from `./tests/` (after verification)

## Risk Mitigation

1. **No file deletion until verified**: Keep original files until tests pass in new location
2. **Incremental migration**: Move one crate at a time
3. **CI verification**: Ensure all tests pass after each migration step
