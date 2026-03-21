---
feature: "Foundation"
description: "Module structure, core type imports, and trait foundation for Valtron async iterators"
status: "pending"
priority: "high"
depends_on: []
estimated_effort: "small"
created: 2026-03-20
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# Foundation Feature

## WHY: Problem Statement

Before implementing TaskStatusIterator and StreamIteratorExt traits, we need to establish:

1. **Module structure** - Where new types live within `backends/foundation_core/src/valtron/`
2. **Core type imports** - Ensure `TaskStatus`, `Stream`, `ExecutionAction` types are properly accessible
3. **Design alignment** - Document how new traits compose with existing `DrivenRecvIterator`, `DrivenStreamIterator` types

The foundation ensures all subsequent features have consistent structure and proper imports, leveraging existing Valtron infrastructure rather than duplicating it.

## WHAT: Solution Overview

Create the module structure and document how new combinators compose with existing types:

### New Module Files

| File | Purpose |
|------|---------|
| `backends/foundation_core/src/valtron/task_iterators.rs` | TaskStatusIterator trait and conversions |
| `backends/foundation_core/src/valtron/stream_iterators.rs` | StreamIteratorExt trait and state-aware methods |
| `backends/foundation_core/src/valtron/collect_all.rs` | collect_all() collection combinators |
| `backends/foundation_core/src/valtron/map_all.rs` | map_all_done(), map_all_pending_and_done() mapping combinators |
| `backends/foundation_core/src/valtron/mod.rs` | Update to export new modules |

### Key Design Principle: Extension Traits for Builder Pattern

**Extension traits provide combinator methods for ANY `TaskIterator` or `StreamIterator`:**

```rust
// Combinators accept any TaskIterator/StreamIterator
// This includes both raw iterators AND driven wrappers (they implement the traits too)
let result = execute(task)?       // DrivenRecvIterator implements TaskIterator
    .map_ready(|x| x * 2)         // Works - DrivenRecvIterator is a TaskIterator
    .map_pending(|p| p + 1);      // Works - still a TaskIterator

// Also works with raw iterators
let raw = MyRawTaskIterator::new();
let mapped = raw.map_ready(|x| x * 2);  // Works - raw is also a TaskIterator
```

**Why this design:**

| Approach | Result |
|----------|--------|
| Accept `DrivenRecvIterator` specifically | Only works with driven types |
| Accept `TaskIterator` trait | Works with raw AND driven (DrivenRecvIterator implements TaskIterator) |

**The trait bounds do the work:**

```rust
// Combinators accept anything implementing TaskIterator
pub trait TaskIteratorExt: TaskStatusIterator + Sized {
    fn map_ready<F, R>(self, f: F) -> MapReady<Self, F>
    where
        F: Fn(Self::Ready) -> R + Send + 'static;
}

// Blanket impl - ANY TaskStatusIterator gets these methods
impl<I> TaskIteratorExt for I
where
    I: TaskStatusIterator + Send + 'static,
{
    fn map_ready<F, R>(self, f: F) -> MapReady<Self, F> {
        MapReady(self, f)
    }
}

// DrivenRecvIterator implements TaskStatusIterator, so it gets TaskIteratorExt methods
// Raw TaskIterator also implements TaskStatusIterator, so it also gets the methods
// Same trait, same methods - works for both!
```

**Type Flow:**

```
execute(task)                    raw TaskIterator
    │                                 │
    ▼                                 ▼
DrivenRecvIterator              MyRawTaskIterator
    │                                 │
    │ implements TaskStatusIterator   │ implements TaskStatusIterator
    └──────────────┬──────────────────┘
                   │
                   │ Both get TaskIteratorExt via blanket impl
                   ▼
        ┌──────────────────────────────┐
        │  TaskIteratorExt methods:    │
        │  - map_ready()               │
        │  - map_pending()             │
        │  - stream_collect()          │
        └──────────────────────────────┘
                   │
                   ▼
        Returns wrapper types (MapReady, etc.)
        that implement TaskStatusIterator too
```

### Type Flow Diagram

```
Raw TaskIterator          DrivenSendTaskIterator (wrapper with auto-drive)
     │                           │
     │  drive_iterator()         │ already driven
     ├──────────────────────────►│
     │                           │
     │  Both implement           │ Both get TaskIteratorExt
     │  TaskStatusIterator       │ combinators (map_ready, etc.)
     ▼                           ▼
┌────────────────────────────────────────┐
│     TaskIteratorExt combinators        │
│     - map_ready()                      │
│     - map_pending()                    │
│     - stream_collect()                 │
└────────────────────────────────────────┘
                    │
                    │  eventually needs driving
                    ▼
            execute(task) or drive_iterator()
```

## HOW: Implementation Approach

1. **Read existing modules** to understand current structure:
   - `backends/foundation_core/src/valtron/mod.rs` - Current exports
   - `backends/foundation_core/src/valtron/task.rs` - TaskStatus definition
   - `backends/foundation_core/src/synca/mpp.rs` - Stream and StreamIterator definitions
   - `backends/foundation_core/src/valtron/executors/unified.rs` - execute() and execute_stream() patterns
   - `backends/foundation_core/src/valtron/executors/drivers.rs` - DrivenRecvIterator, DrivenStreamIterator types

2. **Create stub module files** with `//!` level documentation

3. **Update parent mod.rs** to export new modules

4. **Verify compilation** with `cargo check -p foundation_core`

## Requirements

### Module Structure

1. **Create `task_iterators.rs`** - Stub file for TaskStatusIterator trait with module-level docs
2. **Create `stream_iterators.rs`** - Stub file for StreamIteratorExt trait with module-level docs
3. **Create `collect_all.rs`** - Stub file for collection combinators with module-level docs
4. **Create `map_all.rs`** - Stub file for mapping combinators with module-level docs
5. **Update `mod.rs`** - Add `pub mod` declarations for all new modules
6. **Verify imports** - Ensure TaskStatus, Stream, ExecutionAction are importable from new modules

### Documentation

7. **Module-level docs** - Add comprehensive `//!` documentation to each new module explaining:
   - Purpose of the module
   - Key types and traits
   - Relationship to drivers.rs and unified.rs
   - Usage examples showing composition with existing types

## Tasks

### Task 1: Read existing valtron modules

Read these files to understand current structure and patterns:

- [ ] `backends/foundation_core/src/valtron/mod.rs` - Module exports
- [ ] `backends/foundation_core/src/valtron/task.rs` - TaskStatus enum and TaskIterator trait
- [ ] `backends/foundation_core/src/synca/mpp.rs` - Stream enum and StreamIterator trait
- [ ] `backends/foundation_core/src/valtron/executors/unified.rs` - execute() and execute_stream() patterns
- [ ] `backends/foundation_core/src/valtron/executors/drivers.rs` - DrivenRecvIterator, DrivenStreamIterator types

### Task 2: Create task_iterators.rs module

- [ ] Create `backends/foundation_core/src/valtron/task_iterators.rs`
- [ ] Add `//!` module-level documentation explaining composition with execute()
- [ ] Add trait signature stubs (can use `todo!()` for method bodies)
- [ ] Import required types: `TaskStatus`, `ExecutionAction`, `StreamIterator`, `DrivenRecvIterator`

### Task 3: Create stream_iterators.rs module

- [ ] Create `backends/foundation_core/src/valtron/stream_iterators.rs`
- [ ] Add `//!` module-level documentation explaining composition with execute_stream()
- [ ] Add trait signature stubs
- [ ] Import required types: `Stream`, `StreamIterator`, `DrivenStreamIterator`

### Task 4: Create collect_all.rs module

- [ ] Create `backends/foundation_core/src/valtron/collect_all.rs`
- [ ] Add `//!` module-level documentation
- [ ] Add `collect_all()` function signatures that accept Vec<DrivenRecvIterator> and Vec<DrivenStreamIterator>
- [ ] Import required types

### Task 5: Create map_all.rs module

- [ ] Create `backends/foundation_core/src/valtron/map_all.rs`
- [ ] Add `//!` module-level documentation
- [ ] Add `map_all_done()` and `map_all_pending_and_done()` function signatures
- [ ] Import required types

### Task 6: Update valtron/mod.rs

- [ ] Add `pub mod task_iterators;`
- [ ] Add `pub mod stream_iterators;`
- [ ] Add `pub mod collect_all;`
- [ ] Add `pub mod map_all;`
- [ ] Add re-exports as needed

### Task 7: Verify compilation

- [ ] Run `cargo check -p foundation_core`
- [ ] Fix any import errors or unresolved types
- [ ] Ensure zero warnings

### Task 8: Add comprehensive documentation

- [ ] Document trait relationships in module-level comments
- [ ] Add usage examples showing composition with DrivenRecvIterator/DrivenStreamIterator
- [ ] Cross-reference related modules (drivers.rs, unified.rs)

## Verification

```bash
# Build check
cargo check -p foundation_core

# Lint check
cargo clippy -p foundation_core -- -D warnings

# Format check
cargo fmt -p foundation_core -- --check
```

## Success Criteria

- All tasks completed
- `cargo check -p foundation_core` passes with zero errors
- All new modules have `//!` level documentation
- Trait signatures show composition with existing Driven* types
- Zero clippy warnings

---

_Created: 2026-03-20_
_Updated: 2026-03-20 (Aligned with existing Valtron infrastructure)_
