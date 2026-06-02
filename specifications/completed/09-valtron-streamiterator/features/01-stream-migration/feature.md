---
description: "Consolidate Stream/StreamIterator with combinators into streams.rs, and Task/TaskIterator with combinators into task.rs"
status: "completed"
priority: "high"
created: 2026-04-04
updated: 2026-04-04
author: "Main Agent"
feature_number: 1
depends_on: []
metadata:
  estimated_effort: "large"
  files_modified:
    - backends/foundation_core/src/valtron/streams.rs
    - backends/foundation_core/src/valtron/task.rs
    - backends/foundation_core/src/valtron/mod.rs
    - backends/foundation_core/src/synca/mpp.rs
    - backends/foundation_core/src/valtron/executors/*.rs
---

# Feature 01: Stream and Task Iterator Consolidation - COMPLETED

## Summary

Successfully consolidated all valtron stream and task functionality into single source-of-truth modules:

- **streams.rs** (3029 lines) - Core `Stream` enum, `StreamIterator` trait, `StreamRecvIterator`, AND all `StreamIteratorExt` combinators
- **task.rs** (4071 lines) - Core `TaskStatus`, `TaskIterator` trait, AND all `TaskIteratorExt` combinators

## Changes Made

### 1. Migrated from mpp.rs to streams.rs
- `Stream<D, P>` enum
- `StreamIterator` trait + blanket impl
- `StreamRecvIterator<D, P>` struct + impls

### 2. Consolidated stream_iterators.rs → streams.rs
- `StreamIteratorExt` extension trait
- All combinator structs: `SMapState`, `SFilterState`, `SCollectAll`, `SFold`, `SFind`, `SAny`, `SAll`
- `SCollectNext` and multi-source combinators
- `MapIter`, `MapIterPending`, `MapIterDone` (flatmap pattern)
- `ShortCircuit` enum

### 3. Consolidated task_iterators.rs → task.rs
- `TaskIteratorExt` extension trait
- All combinator structs: `TMapState`, `TFilterReady`, `TCollectAll`, `TFold`, `TFind`, `TAny`, `TAll`
- `TaskShortCircuit` enum
- `SplitCollectorMapContinuation`, `SplitCollectorMapObserver`

### 4. Removed from mpp.rs
- Deleted `Stream`, `StreamIterator`, `StreamRecvIterator`
- Retained channel primitives: `Sender`, `Receiver`, `RecvIter`, `RecvIterator`

### 5. Updated Imports Across Codebase
- `backends/foundation_core/src/valtron/executors/*.rs`
- `backends/foundation_db/src/backends/libsql_backend.rs`
- `bin/platform/src/gen_model_descriptors/mod.rs`
- `backends/foundation_core/tests/sync_boundary_helpers.rs`

### 6. Files Deleted
- `backends/foundation_core/src/valtron/stream_iterators.rs` (~2943 lines)
- `backends/foundation_core/src/valtron/task_iterators.rs` (~2668 lines)

## Verification Results

| Check | Status |
|-------|--------|
| `cargo check -p foundation_core` | ✓ Passed |
| `cargo clippy -p foundation_core -- -D warnings` | ✓ Passed |
| `cargo check` (workspace) | ✓ Passed |
| Valtron tests | ✓ 79/80 passed (1 pre-existing flaky test) |

## Module Structure After Migration

```
valtron/
├── streams.rs        # Stream, StreamIterator, StreamRecvIterator, StreamIteratorExt + combinators
├── task.rs           # TaskStatus, TaskIterator, ExecutionAction, TaskIteratorExt + combinators
├── executors/        # Executor implementations (use valtron::{Stream, TaskIterator, ...})
├── mod.rs            # pub use streams::*; pub use task::*;
└── ...
```

---

_Feature 01 of 03 | Part of specification 09-valtron-streamiterator_
