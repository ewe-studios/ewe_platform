---
description: "Create Valtron-native async iterator traits (TaskStatusIterator, StreamIteratorExt) with combinators for parallel async operations, enabling non-blocking parallel fetches in gen_model_descriptors."
status: "pending"
priority: "high"
created: 2026-03-20
author: "Main Agent"
metadata:
  version: "3.0"
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
  completed: 7
  uncompleted: 2
  total: 9
  completion_percentage: 78%
---

# Overview

This specification defines Valtron-native async iterator traits and combinators with a clear separation:

- **`TaskIterator`** - For implementers defining async tasks; builder combinators applied BEFORE execution
- **`StreamIterator`** - For end users; what `execute()` returns; all combinators work here
- **`execute()`** - Takes `TaskIterator`, returns `StreamIterator` (hides executor concerns)

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

1. **TaskIteratorExt trait** - Builder-style combinators (map_ready, map_pending, stream_collect) for any `T: TaskIterator`
2. **StreamIteratorExt trait** - Extensions for `Stream<Done, Pending>` applied AFTER execute() for any `T: StreamIterator`
4. **Collection combinators** - `execute_collect_all()` to aggregate multiple TaskIterators
5. **Mapping combinators** - `execute_map_all()` for state-aware transformations
6. **Executor integration** - `execute()` returns `StreamIterator` only (hides delays, actions, spawner)
7. **ClientRequest refactor** - Use StreamIterator combinators after execute (feature 06a)
8. **Parallel fetch demonstration** - gen_model_descriptors using execute_collect_all() for ~3x speedup (feature 06b)

## Key Design Principle: TaskIterators Are Inputs, StreamIterators Are Outputs

```
┌─────────────────────────────────────────────────────────────────┐
│                    TaskIterator (Input)                         │
│  - For implementers defining async tasks                        │
│  - Builder combinators applied BEFORE execute()                 │
│  - Examples: map_ready(), map_pending(), stream_collect()       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ execute()
                              │ (hands off to executor engine)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    StreamIterator (Output)                      │
│  - For end users consuming results                              │
│  - Combinators applied AFTER execute()                          │
│  - Examples: collect_all(), map_all_done(), etc.                │
│  - Yields Stream<D, P> variants (Init, Pending, Delayed, Next)  │
└─────────────────────────────────────────────────────────────────┘
```

### execute() Function (Entry Point)

| Function | Returns | Use Case |
|----------|---------|----------|
| `execute(task)` | `StreamIterator` (via `DrivenStreamIterator`) | **Default** - Execute task, get Stream results |
| `execute_as_task(task)` | `TaskStatusIterator` (via `DrivenRecvIterator`) | **Opt-in** - When you need TaskStatus for further transformations |

### Design Philosophy

1. **End users never deal with TaskIterator directly** - They call `execute()` and work with `StreamIterator`
2. **TaskIterator combinators are for implementers** - Applied before handing off to executor
3. **StreamIterator combinators are for everyone** - Applied after `execute()` returns
4. **`execute()` hides executor concerns** - Delays, actions, spawner are all internal
- Raw `TaskIterator` / `StreamIterator` (can be driven later)

This allows users to:
1. Start with driven iterators from `execute()` → chain combinators → consume
2. Build combinator pipelines first → drive later

## Feature Index

| # | Feature | Description | Dependencies |
|---|---------|-------------|--------------|
| 1 | [foundation](./features/00-foundation/feature.md) | Module structure, core types, trait foundations | None |
| 2 | [task-iterators](./features/01-task-iterators/feature.md) | ✅ COMPLETE - TaskIteratorExt combinators for implementers (map_ready, map_pending, filter_ready, stream_collect) | #0 |
| 3 | [stream-iterators](./features/02-stream-iterators/feature.md) | ✅ COMPLETE - StreamIteratorExt trait, state-aware combinators for end users | #0 |
| 4 | [collection-combinators](./features/03-collection-combinators/feature.md) | ✅ COMPLETE - execute_collect_all() returns StreamIterator | #2, #3 |
| 5 | [mapping-combinators](./features/04-mapping-combinators/feature.md) | ✅ COMPLETE - execute_map_all() returns StreamIterator | #3, #4 |
| 6 | [unified-executor-integration](./features/05-unified-executor-integration/feature.md) | ✅ COMPLETE - execute() returns StreamIterator, execute_as_task() opt-in | #4, #5 |
| 6a | [client-request-refactor](./features/06a-client-request-refactor/feature.md) | ✅ COMPLETE - Refactor ClientRequest to use split_collect_until_map() | #6, #7 |
| 6b | [map-iter-combinator](./features/06b-map-iter-combinator/feature.md) | PENDING - map_iter() for nested iterator patterns (outer yields inners) | #2 |
| 6c | [gen-model-descriptors-parallel-fetch](./features/06c-gen-model-descriptors-parallel-fetch/feature.md) | PENDING - Use execute_collect_all() for parallel API fetches | #5 |
| 7 | [split-collector](./features/07-split-collector/feature.md) | ✅ COMPLETE - split_collector() and split_collect_until() for observer + continuation pattern | #2, #3 |

## High-Level Architecture

```mermaid
graph TD
    subgraph TaskIterator "TaskIterator (Input - Implementers)"
        A[Task Iterator with combinators]
    end

    subgraph Execute "execute() - Entry Point"
        A -->|execute()| B[StreamIterator]
    end

    subgraph StreamIterator "StreamIterator (Output - End Users)"
        B -->|collect_all()| C[Combined Stream]
        B -->|map_all_done()| D[Mapped Stream]
    end

    subgraph Executor "Executor Layer"
        E[Valtron Executor] -->|single/multi| A
    end

    subgraph Application "Application Layer"
        F[gen_model_descriptors] -->|uses| E
    end
```

### Type Relationships

```
TaskIterator (implementers define tasks)
    │
    ├── TaskIteratorExt combinators (blanket impl for T: TaskIterator)
    │   - map_ready(f)
    │   - map_pending(f)
    │   - stream_collect()
    │
    └── execute(task) ──────────────────────► StreamIterator (end users consume)
                                                │
                                                ├── StreamIteratorExt combinators (blanket impl for T: StreamIterator)
                                                │   - collect_all()
                                                │   - map_all_done()
                                                │   - map_all_pending_and_done()
                                                │
                                                └── Yields Stream<D, P> variants
                                                    - Init, Pending, Delayed, Next
```

### Key Design Principles

1. **Never block** - All iterator methods yield `Stream` states instead of waiting
2. **Clear separation** - TaskIterator for implementers, StreamIterator for end users
3. **execute() is the boundary** - Hides executor concerns (delays, actions, spawner)
4. **Composability** - Small combinators build complex async pipelines
5. **Stream output by default** - execute() returns StreamIterator; execute_as_task() is opt-in

## Known Issues/Limitations

None currently identified. This is a greenfield implementation.

## Requirements Conversation Summary

This specification was created through collaborative requirements gathering and updated to align with existing Valtron infrastructure:

- **Scope**: Core extension traits (TaskIteratorExt, StreamIteratorExt) plus combinators AND real-world application (ClientRequest refactor + gen_model_descriptors) as features
- **Structure**: Feature-based with 8 features covering foundation through application
- **Detail level**: Comprehensive - clear "what we are doing" and "how we are doing it", not just code copying
- **Priority**: High priority, large effort investment
- **Critical Update**: Leverage existing `drivers.rs` driven iterator types (`DrivenRecvIterator`, `DrivenStreamIterator`) and `unified.rs` execute functions instead of creating new wrapper types

## Success Criteria (Spec-Wide)

This specification is considered complete when:

### Functionality
- All features completed and verified
- TaskIteratorExt trait implemented for any `T: TaskIterator` (for implementers, before execute)
- StreamIteratorExt trait implemented for any `T: StreamIterator` (for end users, after execute)
- `execute()` returns `StreamIterator` (hides executor concerns)
- `execute_as_task()` available as opt-in for TaskStatus output
- `execute_collect_all()` and `execute_map_all()` functional in unified.rs
- ClientRequest refactored to use StreamIterator combinators (feature 06a)
- `map_iter()` combinator for nested iterator patterns (feature 06b)
- ClientRequest body read (lines 393-420) refactored to use map_iter()
- gen_model_descriptors using execute_collect_all() for parallel fetches (feature 06c)
- Demonstrated 2-3x speedup in fetch time (sequential ~1500ms → parallel ~500ms)

### Code Quality
- Zero warnings from `cargo clippy -p foundation_core -- -D warnings`
- `cargo fmt -p foundation_core -- --check` passes for all modified files
- All unit and integration tests pass
- Integration test demonstrates parallel execution with execute_collect_all()

### Documentation
- Module-level docs (`//!`) in all new modules
- `LEARNINGS.md` captures design decisions and trade-offs
- `VERIFICATION.md` produced with all verification checks passing
- `REPORT.md` created documenting final implementation

## Module References

Agents implementing features should read these files:

- `backends/foundation_core/src/valtron/task.rs` - TaskStatus definition
- `backends/foundation_core/src/synca/mpp.rs` - Stream and StreamIterator definitions
- `backends/foundation_core/src/valtron/executors/unified.rs` - execute() returns StreamIterator
- `backends/foundation_core/src/valtron/executors/drivers.rs` - DrivenStreamIterator, DrivenRecvIterator
- `backends/foundation_core/src/valtron/iterators.rs` - Existing iterator patterns

---

_Created: 2026-03-20_
_Last Updated: 2026-03-20 (v3.0: execute() returns StreamIterator, TaskIterator is input only)_
_Structure: Feature-based (has_features: true)_
