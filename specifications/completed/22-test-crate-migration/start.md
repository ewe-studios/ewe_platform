# Test Crate Migration - Start Here

**Status:** Pending Review  
**Created:** 2026-05-13  
**Author:** Claude

## Quick Start

1. **Read the requirements**: [requirements.md](./requirements.md)
2. **Review features** (in dependency order):
   - [01-migrate-foundation-core-tests](./features/01-migrate-foundation-core-tests/feature.md)
   - [02-migrate-foundation-nostd-tests](./features/02-migrate-foundation-nostd-tests/feature.md)
   - [03-migrate-foundation-macros-tests](./features/03-migrate-foundation-macros-tests/feature.md)
   - [04-migrate-foundation-testing-tests](./features/04-migrate-foundation-testing-tests/feature.md)
   - [05-cleanup-and-verify](./features/05-cleanup-and-verify/feature.md)

## The Problem

Tests that only test a single crate (`foundation_core`, `foundation_nostd`, etc.) currently live in a separate `ewe_platform_tests` crate. This creates:
- Confusion about test ownership
- Unnecessary dependency chains
- Violation of Rust best practices (tests should live with code)

## The Solution

Move single-crate tests into their respective crate's `tests/` directory, using `foundation_testing` as a dev-dependency.

## Why Dev-Dependencies Work

Dev-dependencies are only active when testing the declaring crate:

```
foundation_core (dev-deps)
    └── foundation_testing ──► foundation_core (regular deps)
         
# When building foundation_testing:
# - foundation_core is fetched WITHOUT its dev-dependencies
# - No circular dependency!
```

## Migration Summary

| Crate | Files | Status |
|-------|-------|--------|
| foundation_core | ~20 | Pending |
| foundation_nostd | 4 | Pending |
| foundation_macros | 1 | Pending |
| foundation_testing | 1 | Pending |

## Decision Points

### 1. Cross-Crate Tests
Some tests verify interaction between crates. **Keep these in `ewe_platform_tests`**:
- Integration tests spanning multiple crates
- End-to-end platform tests

### 2. foundation_nostd Tests
Most tests in `tests/backends/foundation_nostd/` actually test `foundation_testing` scenarios.
- `barrier_debug.rs` → `foundation_testing/tests/`
- `integration_tests.rs` → `foundation_testing/tests/`
- `wasm_tests.rs` → Verify if it tests nostd or testing

### 3. foundation_macros Asset Paths
The macro tests have relative paths (`#[source = "../../assets/..."]`).
- May need adjustment for new location
- Check if macro supports workspace-relative paths

## Verification Checklist

Before starting migration:
- [ ] Review this spec
- [ ] Verify dev-dependencies are configured in target crates
- [ ] Identify any cross-crate integration tests to keep
- [ ] Decide on asset path handling for macro tests

After each migration:
- [ ] Tests compile in new location
- [ ] Tests pass in new location
- [ ] No circular dependency errors

Final verification:
- [ ] All single-crate tests migrated
- [ ] Original files removed
- [ ] Full test suite passes

## Questions?

Review the detailed feature specifications for:
- File-by-file breakdown
- Import changes needed
- Specific verification steps
- Rollback procedures
