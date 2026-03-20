---
feature: "Mapping Combinators"
description: "MapAllDone and MapAllPendingDone custom types that hold sources + mapper function"
status: "pending"
priority: "high"
depends_on: ["02-stream-iterators", "03-collection-combinators"]
estimated_effort: "medium"
created: 2026-03-20
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# Mapping Combinators Feature

## WHY: Problem Statement

After collecting multiple async sources, we often need to transform results. Different use cases need different state awareness:

- **map_all_done()** - Transform only when ALL sources reach Done state (e.g., merge results when all APIs respond)
- **map_all_pending_and_done()** - Transform with visibility into Pending states (e.g., progress reporting, partial results)

**Use case example**:

```rust
// map_all_done - only transform when all complete
let tasks = vec![fetch_models_dev_task(client), fetch_openrouter_task(client), fetch_ai_gateway_task(client)];
let merged = execute_map_all(tasks, |results| {
    // results: Vec<Vec<ModelEntry>> from 3 APIs
    results.into_iter().flatten().collect::<Vec<_>>()
});

// map_all_pending_and_done - track progress
let progress = streams.map_all_pending_and_done(|states| {
    let done_count = states.iter().filter(|s| s.is_done()).count();
    let pending_count = states.len() - done_count;
    ProgressUpdate { done: done_count, pending: pending_count }
});
```

## WHAT: Solution Overview

Implement state-aware mapping combinators as custom types that hold sources + mapper:

### MapAllDoneStreamIterator<I, F>

```rust
/// Maps values only when all sources reach Done state
///
/// This custom type holds both the source iterators and the mapper function.
/// It buffers values from all sources and applies mapper when all complete.
pub struct MapAllDoneStreamIterator<I, F>
where
    I: TaskIterator + Send + 'static,
    F: Fn(Vec<I::Ready>) -> O + Send + 'static,
{
    sources: Vec<DrivenStreamIterator<I>>,
    mapper: F,
    buffer: Vec<Option<I::Ready>>,
    pending_count: usize,
}

impl<I, F, O> Iterator for MapAllDoneStreamIterator<I, F>
where
    I: TaskIterator + Send + 'static,
    F: Fn(Vec<I::Ready>) -> O + Send + 'static,
    O: Send + 'static,
{
    type Item = Stream<O, I::Pending>;

    fn next(&mut self) -> Option<Self::Item> {
        // Poll all sources round-robin
        // Buffer values as they arrive
        // Return Stream::Pending while any source pending
        // When all done: apply self.mapper(buffer), return Stream::Next(mapped_result)
    }
}
```

### MapAllPendingDoneStreamIterator<I, F>

```rust
/// Maps values with Pending + Next state visibility
///
/// This custom type holds sources and mapper, providing full state
/// information to the mapper for progress tracking.
///
/// Note: Uses existing Stream<D, P> enum, NOT a custom StreamState.
pub struct MapAllPendingDoneStreamIterator<I, F>
where
    I: TaskIterator + Send + 'static,
    F: Fn(Vec<Stream<I::Ready, I::Pending>>) -> O + Send + 'static,
{
    sources: Vec<DrivenStreamIterator<I>>,
    mapper: F,
    last_states: Vec<Stream<I::Ready, I::Pending>>,
}

impl<I, F, O> Iterator for MapAllPendingDoneStreamIterator<I, F>
where
    I: TaskIterator + Send + 'static,
    F: Fn(Vec<Stream<I::Ready, I::Pending>>) -> O + Send + 'static,
    O: Send + 'static,
{
    type Item = Stream<O, I::Pending>;

    fn next(&mut self) -> Option<Self::Item> {
        // Poll all sources
        // Update last_states with current Stream variant from each source
        // Apply self.mapper(&last_states)
        // Return mapped result with appropriate Stream variant
    }
}
```

### Use Existing `Stream<D, P>` Enum

**Do NOT create `StreamState`** - use the existing `Stream<D, P>` from `synca/mpp.rs`:

```rust
pub enum Stream<D, P> {
    Init,
    Ignore,
    Delayed(Duration),
    Pending(P),
    Next(D),  // This is "Done"
}
```

## HOW: Implementation Approach

1. Define `StreamState<D, P>` enum with helper methods
2. Implement `MapAllDoneStreamIterator<I, F>` that holds sources + mapper
3. Implement `MapAllPendingDoneStreamIterator<I, F>` that holds sources + mapper with state tracking
4. Add tests for state-aware mapping behavior

## Requirements

1. **MapAllDoneStreamIterator<I, F>** - Generic struct holding sources + mapper, buffers values
2. **MapAllPendingDoneStreamIterator<I, F>** - Generic struct holding sources + mapper, tracks `Stream<D, P>` states
3. **State propagation** - Preserve Pending/Delayed/Next states from sources
4. **Mapper application** - Apply mapper function when appropriate (all-done vs each-poll)
5. **Use existing Stream<D, P>** - No new state enum needed

## Tasks

1. [ ] Define `StreamState<D, P>` enum with all variants and helpers
2. [ ] Define `MapAllDoneStreamIterator<I, F>` struct with generic mapper
3. [ ] Implement `Iterator` for `MapAllDoneStreamIterator`
4. [ ] Define `MapAllPendingDoneStreamIterator<I, F>` struct with generic mapper
5. [ ] Implement `Iterator` for `MapAllPendingDoneStreamIterator`
6. [ ] Write unit tests for map_all_done behavior
7. [ ] Write unit tests for map_all_pending_and_done behavior
8. [ ] Run clippy and fmt checks

## Verification

```bash
cargo test -p foundation_core -- valtron::map_all
cargo clippy -p foundation_core -- -D warnings
cargo fmt -p foundation_core -- --check
```

## Success Criteria

- All 8 tasks completed
- Both mapping iterators compile with zero errors
- Buffering works correctly in map_all_done
- State tracking accurate in map_all_pending_and_done
- Unit tests pass for state-aware mapping
- Zero clippy warnings

---

_Created: 2026-03-20_
_Updated: 2026-03-20 (Generic wrapper types with embedded mapper functions)_
