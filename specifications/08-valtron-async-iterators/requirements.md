---
description: "Create Valtron-native async iterator traits (TaskStatusIterator, StreamIteratorExt) with combinators for parallel async operations, enabling non-blocking parallel fetches in gen_model_descriptors."
status: "pending"
priority: "high"
created: 2026-03-20
author: "Main Agent"
metadata:
  version: "2.0"
  last_updated: 2026-03-20
  estimated_effort: "large"
  tags:
    - valtron
    - async-iterators
    - task-status
    - stream-iterator
    - combinators
    - parallel-execution
  skills:
    - rust-patterns
    - valtron-executors
  tools:
    - Rust
    - cargo
has_features: true
has_fundamentals: false
builds_on: "specifications/02-build-http-client"
related_specs:
  - "specifications/07-foundation-ai"
features:
  completed: 0
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---

# Overview

This specification defines Valtron-native async iterator traits and combinators that work with `TaskStatus<Ready, Pending, Spawner>` and `Stream<Done, Pending>` types as first-class citizens. Unlike Rust's standard `Iterator` trait (synchronous and blocking), these traits enable non-blocking, state-aware iteration integrated with Valtron's executor system.

## The Problem

The current `gen_model_descriptors` implementation fetches model metadata from three upstream APIs **sequentially**:

```rust
let models_dev = fetch_models_dev(&client);  // Blocks ~500ms
let openrouter = fetch_openrouter(&client);  // Blocks ~500ms
let ai_gateway = fetch_ai_gateway(&client);  // Blocks ~500ms
// Total: ~1500ms
```

Each fetch function uses blocking HTTP calls via `http_get_json()`, causing:

1. **No parallelism** - Each fetch blocks until completion before the next begins
2. **Underutilized Valtron** - The execution engine's async capabilities exist but are not leveraged
3. **Wasted opportunity** - These are independent I/O operations that could run concurrently

**Root cause**: Rust's standard `Iterator` trait is fundamentally synchronous and blocking. Methods like `map()`, `filter()`, `collect()` all block waiting for each element. This conflicts with Valtron's `TaskStatus` flow where operations can be `Pending`, `Delayed`, `Init`, or `Ready`.

## Goals

1. **TaskStatusIterator trait** - Iterator-like operations over `TaskStatus<Ready, Pending, Spawner>` that never block
2. **TaskIteratorExt trait** - Builder-style combinators (map_ready, map_pending, stream_collect) that transform/filter TaskStatus variants
3. **StreamIteratorExt trait** - Extensions for `Stream<Done, Pending>` with state-aware methods
4. **Collection combinators** - `collect_all()` to aggregate multiple iterators in Valtron-friendly way
5. **Mapping combinators** - `map_all_done()`, `map_all_pending_and_done()` for state-aware transformations
6. **Executor integration** - `execute_collect_all()`, `execute_map_all()` helpers in unified.rs
7. **ClientRequest refactor** - Use TaskIteratorExt combinators instead of manual state loops (feature 06a)
8. **Parallel fetch demonstration** - gen_model_descriptors using execute_collect_all() for ~3x speedup (feature 06b)

## Key Design Principle: Leverage Existing Infrastructure

**CRITICAL**: This implementation must leverage existing Valtron infrastructure rather than creating parallel wrapper types:

### Existing Types in `drivers.rs`

| Type | Purpose | Use This |
|------|---------|----------|
| `DrivenRecvIterator<T>` | Wraps `RecvIterator<TaskStatus<...>>`, auto-drives execution | For task result collection |
| `DrivenStreamIterator<T>` | Wraps `StreamRecvIterator<...>`, auto-drives execution | For stream-style iteration |
| `DrivenSendTaskIterator<T>` | Wraps `TaskIterator`, auto-drives on next() | For direct TaskStatus iteration |
| `DrivenNonSendTaskIterator<T>` | Non-Send variant for WASM | For WASM contexts |

### Existing Functions in `unified.rs`

| Function | Returns | Use Case |
|----------|---------|----------|
| `execute(task)` | `DrivenRecvIterator<T>` | Execute single task, get results |
| `execute_stream(task)` | `DrivenStreamIterator<T>` | Execute task as stream |

### New Combinators Should

1. **Accept both driven and raw types** - Combinators should be generic over `TaskIterator` and `StreamIterator`, accepting both raw and driven versions
2. **Use `execute_*` at entry points only** - `execute()` and `execute_stream()` are used at the extremes (when entering the execution engine)
3. **Compose, don't wrap** - Internal combinators compose driven iterators without creating new wrapper structs
4. **Pass through `TaskStatus`** - Preserve state information through all operations

### Design: Entry Points vs Internal Combinators

```
Entry Points (use execute/execute_stream):
  ┌─────────────────────────────────────┐
  │  execute(task) → DrivenRecvIterator │
  │  execute_stream(task) → DrivenStreamIterator │
  └─────────────────────────────────────┘
                    │
                    ▼
Internal Combinators (accept both driven and raw):
  ┌─────────────────────────────────────┐
  │  collect_all<I>(iterators: Vec<I>)  │
  │    where I: TaskIterator            │
  │         or I: StreamIterator        │
  │                                     │
  │  map_all_done<I, F>(...)            │
  │    where I: TaskIterator            │
  │         or I: StreamIterator        │
  └─────────────────────────────────────┘
```

**Key insight**: A `TaskIterator` must go into the execution engine eventually, but combinators should work with both:
- `DrivenSendTaskIterator` / `DrivenStreamIterator` (already driven, auto-executes)
- Raw `TaskIterator` / `StreamIterator` (can be driven later)

This allows users to:
1. Start with driven iterators from `execute()` → chain combinators → consume
2. Build combinator pipelines first → drive later

## Feature Index

| # | Feature | Description | Dependencies |
|---|---------|-------------|--------------|
| 1 | [foundation](./features/00-foundation/feature.md) | Module structure, core types, trait foundations | None |
| 2 | [task-iterators](./features/01-task-iterators/feature.md) | TaskStatusIterator trait, TaskIteratorExt combinators (map_ready, map_pending, stream_collect) | #0 |
| 3 | [stream-iterators](./features/02-stream-iterators/feature.md) | StreamIteratorExt trait, StreamState, state-aware combinators | #0 |
| 4 | [collection-combinators](./features/03-collection-combinators/feature.md) | collect_all() function using existing DrivenRecvIterator types | #1, #2 |
| 5 | [mapping-combinators](./features/04-mapping-combinators/feature.md) | map_all_done(), map_all_pending_and_done() using existing DrivenStreamIterator | #3 |
| 6 | [unified-executor-integration](./features/05-unified-executor-integration/feature.md) | execute_collect_all, execute_map_all helper functions | #4 |
| 7 | [split-collector](./features/07-split-collector/feature.md) | split_collector() and split_collect_one() forking combinators for observer + continuation | #1, #2 |
| 8 | [client-request-refactor](./features/06a-client-request-refactor/feature.md) | Refactor ClientRequest to use split_collector for intro/body separation | #5, #7 |
| 9 | [gen-model-descriptors-parallel-fetch](./features/06b-gen-model-descriptors-parallel-fetch/feature.md) | Use refactored ClientRequest patterns for parallel API fetches | #6, #8 |

## High-Level Architecture

```mermaid
graph TD
    subgraph Task Creation
        A[Task Iterator] -->|execute()| B[DrivenRecvIterator]
        A -->|execute_stream()| C[DrivenStreamIterator]
    end

    subgraph Collection Layer
        B -->|collect_all()| D[Aggregated Results]
        C -->|collect_all()| E[Aggregated Stream]
    end

    subgraph Mapping Layer
        D -->|map_all_done()| F[Mapped Results]
        E -->|map_all_done()| G[Mapped Stream]
    end

    subgraph Executor Layer
        H[Valtron Executor] -->|single/multi| A
    end

    subgraph Application Layer
        I[gen_model_descriptors] -->|uses| H
    end
```

### Type Relationships (Updated)

```
TaskIterator
    │
    ├── execute(task) → DrivenRecvIterator<T> (boundary type)
    │       │
    │       └── collect_all(vec!) → combines multiple DrivenRecvIterator
    │
    └── execute_stream(task) → DrivenStreamIterator<T> (boundary type)
            │
            ├── collect_all(vec!) → combines multiple DrivenStreamIterator
            │
            ├── map_all_done(mapper) → transforms when all complete
            │
            └── map_all_pending_and_done(mapper) → transforms with state visibility
```

### Key Design Principles

1. **Never block** - All iterator methods yield states (`Pending`, `Delayed`, `Ready`) instead of waiting
2. **State as first-class** - `TaskStatus` and `Stream` are the primary types, not wrapped/hidden
3. **Composability** - Small combinators build complex async pipelines
4. **Executor integration** - Use `execute()` and `execute_stream()` as entry points
5. **Explicit state handling** - `Pending` and `Delayed` states are visible and actionable
6. **Reuse existing types** - Compose `DrivenRecvIterator` and `DrivenStreamIterator` instead of new wrappers

## Known Issues/Limitations

None currently identified. This is a greenfield implementation.

## Requirements Conversation Summary

This specification was created through collaborative requirements gathering and updated to align with existing Valtron infrastructure:

- **Scope**: Core traits (TaskStatusIterator, TaskIteratorExt, StreamIteratorExt) plus combinators AND real-world application (ClientRequest refactor + gen_model_descriptors) as features
- **Structure**: Feature-based with 8 features covering foundation through application
- **Detail level**: Comprehensive - clear "what we are doing" and "how we are doing it", not just code copying
- **Priority**: High priority, large effort investment
- **Critical Update**: Leverage existing `drivers.rs` driven iterator types (`DrivenRecvIterator`, `DrivenStreamIterator`) and `unified.rs` execute functions instead of creating new wrapper types

## Success Criteria (Spec-Wide)

This specification is considered complete when:

### Functionality
- All 8 features completed and verified
- TaskStatusIterator and TaskIteratorExt traits implemented with all combinators (map_ready, map_pending, stream_collect)
- StreamIteratorExt trait implemented with state-aware combinators
- `execute_collect_all()` and `execute_map_all()` helpers functional in unified.rs
- ClientRequest refactored to use TaskIteratorExt combinators (feature 06a)
- gen_model_descriptors using execute_collect_all() for parallel fetches (feature 06b)
- Demonstrated 2-3x speedup in fetch time (sequential ~1500ms → parallel ~500ms)

### Code Quality
- Zero warnings from `cargo clippy -p foundation_core -- -D warnings`
- `cargo fmt -p foundation_core -- --check` passes for all modified files
- All unit and integration tests pass
- Integration test demonstrates parallel execution with multiple TaskIterators

### Documentation
- Module-level docs (`//!`) in all new modules
- `LEARNINGS.md` captures design decisions and trade-offs
- `VERIFICATION.md` produced with all verification checks passing
- `REPORT.md` created documenting final implementation

## Module References

Agents implementing features should read these files:

- `backends/foundation_core/src/valtron/task.rs` - TaskStatus definition
- `backends/foundation_core/src/synca/mpp.rs` - Stream and StreamIterator definitions
- `backends/foundation_core/src/valtron/executors/unified.rs` - Executor integration patterns (execute, execute_stream)
- `backends/foundation_core/src/valtron/executors/drivers.rs` - Driven iterator types (DrivenRecvIterator, DrivenStreamIterator)
- `backends/foundation_core/src/valtron/iterators.rs` - Existing iterator patterns (if exists)
- `documentation/valtron/doc.md` - Valtron executor patterns (if exists)

---

_Created: 2026-03-20_
_Last Updated: 2026-03-20 (v2.0: Aligned with existing Valtron infrastructure)_
_Structure: Feature-based (has_features: true)_
