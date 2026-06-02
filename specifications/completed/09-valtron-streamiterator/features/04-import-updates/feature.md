---
description: "Update all imports across codebase, ensure clean compilation, add backward-compatibility re-exports"
status: "completed"
priority: "high"
created: 2026-04-04
updated: 2026-04-04
author: "Main Agent"
feature_number: 4
depends_on: ["01-stream-migration", "03-replace-streamrecv-iterator"]
metadata:
  estimated_effort: "medium"
  files_to_search:
    - backends/**/*.rs
    - bin/**/*.rs
  files_to_modify: TBD (based on search results)
---

# Feature 04: Import Updates

## Overview

This feature updates all imports across the codebase to use the new `valtron::streams` location for `Stream`, `StreamIterator`, and `StreamRecvIterator`, and optionally adds backward-compatibility re-exports in `synca::mpp`.

## Scope

### Files to Search

Search the entire codebase for:
1. `use.*mpp::Stream`
2. `use.*mpp::StreamIterator`
3. `use.*mpp::StreamRecvIterator`
4. `synca::mpp::Stream`
5. `synca::mpp::StreamIterator`
6. `synca::mpp::StreamRecvIterator`
7. `crate::synca::mpp::Stream`
8. `crate::synca::mpp::StreamIterator`
9. `crate::synca::mpp::StreamRecvIterator`

### Directories to Check

- `backends/foundation_core/src/`
- `backends/foundation_deployment/src/`
- `bin/platform/src/`
- Any other Rust source directories

## Tasks

### Task 1: Search for All Stream/StreamIterator/StreamRecvIterator Imports

Run comprehensive search:

```bash
# Search in backends
grep -rn "synca::mpp::Stream" backends/
grep -rn "mpp::Stream" backends/
grep -rn "StreamIterator" backends/ | grep -v "StreamIteratorExt"
grep -rn "synca::mpp::StreamRecvIterator" backends/
grep -rn "StreamRecvIterator" backends/

# Search in bin
grep -rn "synca::mpp::Stream" bin/
grep -rn "mpp::Stream" bin/
grep -rn "StreamRecvIterator" bin/

# Alternative: use Grep tool
```

**Deliverable:** Create a list of all files that import or reference `Stream`, `StreamIterator`, or `StreamRecvIterator` from `synca::mpp`.

**Acceptance Criteria:**
- [ ] Complete list of files identified
- [ ] Categorized by: must update, optional update, re-export only

### Task 2: Categorize Files

Sort identified files into categories:

#### Category A: Valtron Internal (Must Update)
- `backends/foundation_core/src/valtron/*`
- These MUST be updated to use `crate::valtron::streams`

#### Category B: Other Foundation Core (Must Update)
- `backends/foundation_core/src/*` (non-valtron)
- These should be updated to use `crate::valtron::streams`

#### Category C: Foundation Deployment (Should Update)
- `backends/foundation_deployment/src/*`
- These should be updated to use `foundation_core::valtron::streams`

#### Category D: Bin/Platform (Should Update)
- `bin/platform/src/*`
- These should be updated if they directly import

#### Category E: External/Third-party (Cannot Update)
- Any external crates or dependencies
- Document these for manual handling

**Acceptance Criteria:**
- [ ] All files categorized
- [ ] Update strategy defined for each category

### Task 3: Update Category A (Valtron Internal)

For each file in Category A:

**Before:**
```rust
use crate::synca::mpp::{Stream, StreamIterator};
```

**After:**
```rust
use crate::valtron::streams::{Stream, StreamIterator};
// OR (if within valtron module)
use super::streams::{Stream, StreamIterator};
```

**Files to Update (example list - verify with search results):**
- `backends/foundation_core/src/valtron/stream_iterators.rs`
- `backends/foundation_core/src/valtron/task_iterators.rs`
- `backends/foundation_core/src/valtron/executors/drivers.rs`
- `backends/foundation_core/src/valtron/executors/unified.rs`
- `backends/foundation_core/src/valtron/branches.rs`
- `backends/foundation_core/src/valtron/iterators.rs`
- Any other valtron files

**Acceptance Criteria:**
- [ ] All Category A files updated
- [ ] `cargo check -p foundation_core` passes after each file
- [ ] No remaining references to `synca::mpp::Stream` in valtron/

### Task 4: Update Category B (Other Foundation Core)

For each file in Category B:

**Before:**
```rust
use ewe_platform::synca::mpp::Stream;
```

**After:**
```rust
use ewe_platform::valtron::streams::Stream;
```

**Acceptance Criteria:**
- [ ] All Category B files updated
- [ ] `cargo check -p foundation_core` passes

### Task 5: Update Category C (Foundation Deployment)

For each file in Category C:

**Before:**
```rust
use foundation_core::synca::mpp::Stream;
```

**After:**
```rust
use foundation_core::valtron::streams::Stream;
```

**Acceptance Criteria:**
- [ ] All Category C files updated
- [ ] `cargo check -p foundation_deployment` passes

### Task 6: Update Category D (Bin/Platform)

For each file in Category D:

**Before:**
```rust
use ewe_platform::synca::mpp::Stream;
```

**After:**
```rust
use ewe_platform::valtron::streams::Stream;
```

**Acceptance Criteria:**
- [ ] All Category D files updated
- [ ] `cargo check -p platform` (or equivalent) passes

### Task 7: Add Backward-Compatibility Re-exports

**File:** `backends/foundation_core/src/synca/mpp.rs`

Add deprecated re-exports for gradual migration:

```rust
// ============================================================================
// Backward Compatibility Re-exports (Deprecated)
// ============================================================================
//
// Stream, StreamIterator, and StreamRecvIterator have moved to valtron::streams.
// These re-exports are provided for backward compatibility during migration.
//
// Deprecated since: 0.9.0
// Migration: Use `valtron::streams::Stream`, `valtron::streams::StreamIterator`, 
//            and `valtron::streams::StreamRecvIterator` instead

/// Re-export of valtron::streams::Stream for backward compatibility.
///
/// # Deprecated
/// Use [`valtron::streams::Stream`] instead.
#[deprecated(
    since = "0.9.0",
    note = "Use valtron::streams::Stream instead. Stream types have moved to the valtron module."
)]
pub use crate::valtron::streams::Stream;

/// Re-export of valtron::streams::StreamIterator for backward compatibility.
///
/// # Deprecated
/// Use [`valtron::streams::StreamIterator`] instead.
#[deprecated(
    since = "0.9.0",
    note = "Use valtron::streams::StreamIterator instead. StreamIterator has moved to the valtron module."
)]
pub use crate::valtron::streams::StreamIterator;

/// Re-export of valtron::streams::StreamRecvIterator for backward compatibility.
///
/// # Deprecated
/// Use [`valtron::streams::StreamRecvIterator`] instead.
#[deprecated(
    since = "0.9.0",
    note = "Use valtron::streams::StreamRecvIterator instead. StreamRecvIterator has moved to the valtron module."
)]
pub use crate::valtron::streams::StreamRecvIterator;
```

**Alternative: Silent Re-export (No Deprecation)**

If deprecation warnings are too disruptive, use silent re-export:

```rust
/// Re-export for backward compatibility.
/// Stream has moved to valtron::streams, but this re-export maintains compatibility.
pub use crate::valtron::streams::Stream;

/// Re-export for backward compatibility.
/// StreamIterator has moved to valtron::streams, but this re-export maintains compatibility.
pub use crate::valtron::streams::StreamIterator;

/// Re-export for backward compatibility.
/// StreamRecvIterator has moved to valtron::streams, but this re-export maintains compatibility.
pub use crate::valtron::streams::StreamRecvIterator;
```

**Decision:** Use deprecated re-exports (first option) to encourage migration.

**Acceptance Criteria:**
- [ ] Re-exports added to `synca::mpp.rs`
- [ ] Deprecated attribute applied
- [ ] Old code still compiles (with warnings)

### Task 8: Full Test Suite

Run comprehensive test suite:

```bash
# Foundation core
cargo check -p foundation_core
cargo clippy -p foundation_core -- -D warnings
cargo test -p foundation_core

# Foundation deployment
cargo check -p foundation_deployment
cargo clippy -p foundation_deployment -- -D warnings
cargo test -p foundation_deployment

# Platform binary
cargo check -p platform
cargo clippy -p platform -- -D warnings
cargo test -p platform

# Format check
cargo fmt --all -- --check
```

**Acceptance Criteria:**
- [ ] All packages compile without errors
- [ ] All clippy checks pass
- [ ] All tests pass
- [ ] Code is properly formatted

### Task 9: Documentation Audit

Verify documentation is consistent:

1. Check module-level docs reference correct paths
2. Update any examples using old import paths
3. Run doc tests

```bash
cargo doc -p foundation_core --no-deps
cargo test -p foundation_core --doc
```

**Acceptance Criteria:**
- [ ] Generated docs show correct paths
- [ ] Doc tests pass
- [ ] No broken links in documentation

## Verification Checklist

After implementation:

- [ ] `grep -rn "synca::mpp::Stream " backends/foundation_core/src/valtron/` returns no results (except re-exports)
- [ ] `grep -rn "synca::mpp::StreamIterator" backends/foundation_core/src/valtron/` returns no results (except re-exports)
- [ ] `grep -rn "synca::mpp::StreamRecvIterator" backends/foundation_core/src/valtron/` returns no results (except re-exports)
- [ ] All new imports use `valtron::streams::Stream`, `valtron::streams::StreamIterator`, and `valtron::streams::StreamRecvIterator`
- [ ] `cargo test --all` passes
- [ ] `cargo clippy --all -- -D warnings` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] Deprecation warnings appear for old import paths (if using deprecated re-exports)

## Rollback Plan

If issues arise:

1. **Keep re-exports indefinitely** - Old code continues to work
2. **Revert import changes** - Restore original imports temporarily
3. **Debug in isolation** - Test changes in separate branch

## Files Modified Summary

Expected files to modify (verify with search):

| File | Change Type |
|------|-------------|
| `backends/foundation_core/src/valtron/mod.rs` | Add streams module export |
| `backends/foundation_core/src/valtron/iterators.rs` | Update imports |
| `backends/foundation_core/src/valtron/stream_iterators.rs` | Update imports |
| `backends/foundation_core/src/valtron/task_iterators.rs` | Update imports |
| `backends/foundation_core/src/valtron/executors/drivers.rs` | Update imports |
| `backends/foundation_core/src/valtron/executors/unified.rs` | Update imports |
| `backends/foundation_core/src/valtron/branches.rs` | Update imports |
| `backends/foundation_core/src/synca/mpp.rs` | Add re-exports for Stream, StreamIterator, StreamRecvIterator |
| [Other files] | TBD based on search |

---

_Feature 04 of 04 | Part of specification 09-valtron-streamiterator_
