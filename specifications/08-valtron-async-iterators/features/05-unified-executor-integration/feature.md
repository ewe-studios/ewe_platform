---
feature: "Unified Executor Integration"
description: "execute_collect_all and execute_map_all helper functions - TaskIterators in, StreamIterators out"
status: "complete"
priority: "high"
depends_on: ["03-collection-combinators", "04-mapping-combinators"]
estimated_effort: "small"
created: 2026-03-20
author: "Main Agent"
tasks:
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100%
---

# Unified Executor Integration Feature

## WHY: Problem Statement

Users need helper functions that:
- Execute multiple TaskIterators in parallel
- Return StreamIterators (not TaskStatus iterators)
- Hide executor complexity (delays, actions, spawner concerns)
- Use existing `execute()` pattern from `unified.rs`

**Key Design: TaskIterators Are Inputs, StreamIterators Are Outputs**

```rust
// TaskIterators go in...
let tasks = vec![task1, task2, task3];

// ...StreamIterator comes out
let collected = execute_collect_all(tasks, None)?;
// Type: impl StreamIterator<Vec<D>, P>

for stream_item in collected {
    match stream_item {
        Stream::Pending(count) => println!("{count} still pending"),
        Stream::Next(results) => process(results),
    }
}
```

**Opt-in for TaskIterator output** (rare cases):

```rust
// Only use execute_as_task() when you specifically need TaskStatus output
let driven = execute_as_task(task, None)?;
// Type: DrivenRecvIterator<T> which yields TaskStatus
```

## WHAT: Solution Overview

Add helper functions to `backends/foundation_core/src/valtron/executors/unified.rs`:

### execute_collect_all

```rust
/// Execute multiple TaskIterators in parallel, collecting results.
///
/// Uses existing execute() for each task, returns StreamIterator.
///
/// # Arguments
///
/// * `tasks` - Vector of TaskIterators to execute in parallel
/// * `wait_cycle` - Optional polling duration (defaults to DEFAULT_WAIT_CYCLE)
///
/// # Returns
///
/// GenericResult wrapping CollectAllStream that yields Stream<Vec<D>, P>
///
/// # Example
///
/// ```rust
/// let tasks = vec![fetch_task1(), fetch_task2(), fetch_task3()];
/// let collected = execute_collect_all(tasks, None)?;
///
/// for stream_item in collected {
///     match stream_item {
///         Stream::Pending(count) => println!("{count} still pending"),
///         Stream::Next(results) => process(results),
///     }
/// }
/// ```
pub fn execute_collect_all<T>(
    tasks: Vec<T>,
    wait_cycle: Option<Duration>,
) -> GenericResult<CollectAllStream<T>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    // Use existing execute() for each task - returns StreamIterator
    let streams: Vec<DrivenStreamIterator<T>> = tasks
        .into_iter()
        .map(|t| execute(t, wait_cycle))
        .collect::<GenericResult<_>>()?;

    Ok(CollectAllStream::new(streams))
}
```

### execute_map_all

```rust
/// Execute multiple TaskIterators and apply mapper when all complete.
///
/// Uses execute() for each task, wraps with MapAllDoneStreamIterator.
///
/// # Arguments
///
/// * `tasks` - Vector of TaskIterators to execute in parallel
/// * `mapper` - Function to apply to collected results: Fn(Vec<T::Ready>) -> O
/// * `wait_cycle` - Optional polling duration
///
/// # Returns
///
/// GenericResult wrapping MapAllDoneStreamIterator that yields Stream<O, P>
pub fn execute_map_all<T, F, O>(
    tasks: Vec<T>,
    mapper: F,
    wait_cycle: Option<Duration>,
) -> GenericResult<MapAllDoneStreamIterator<T, F>>
where
    T: TaskIterator + Send + 'static,
    F: Fn(Vec<T::Ready>) -> O + Send + 'static,
    O: Send + 'static,
{
    // Use execute() for each task, wrap in MapAllDone
    let streams: Vec<DrivenStreamIterator<T>> = tasks
        .into_iter()
        .map(|t| execute(t, wait_cycle))
        .collect::<GenericResult<_>>()?;

    Ok(MapAllDoneStreamIterator::new(streams, mapper))
}
```

### execute_as_task() - Opt-in for TaskStatus Output

```rust
/// Execute and return as TaskIterator (yields TaskStatus).
///
/// Use this ONLY when you specifically need TaskStatus output
/// for further TaskStatus-aware transformations.
///
/// This is intentionally separate from execute() to make
/// TaskStatus output an opt-in choice.
pub fn execute_as_task<T>(
    task: T,
    wait_cycle: Option<Duration>,
) -> GenericResult<DrivenRecvIterator<T>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    execute(task, wait_cycle)
}
```

## WHAT: Solution Overview

Add helper functions to `backends/foundation_core/src/valtron/executors/unified.rs` that compose existing types:

### execute_collect_all

```rust
/// Execute multiple TaskIterators in parallel, collecting results.
///
/// Uses existing execute_stream() for each task, then wraps results
/// in CollectAllStreamDriven for aggregated collection.
///
/// # Arguments
///
/// * `tasks` - Vector of TaskIterators to execute in parallel
/// * `wait_cycle` - Optional polling duration (defaults to DEFAULT_WAIT_CYCLE)
///
/// # Returns
///
/// GenericResult wrapping CollectAllStreamDriven that yields Stream<Vec<D>, P>
///
/// # Example
///
/// ```rust
/// let tasks = vec![fetch_task1(), fetch_task2(), fetch_task3()];
/// let collected = execute_collect_all(tasks, None)?;
///
/// for stream_item in collected {
///     match stream_item {
///         Stream::Pending(count) => println!("{count} still pending"),
///         Stream::Next(results) => process(results),
///     }
/// }
/// ```
pub fn execute_collect_all<T>(
    tasks: Vec<T>,
    wait_cycle: Option<Duration>,
) -> GenericResult<CollectAllStreamDriven<T>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    // Use existing execute_stream() for each task
    let streams: Vec<DrivenStreamIterator<T>> = tasks
        .into_iter()
        .map(|t| execute_stream(t, wait_cycle))
        .collect::<GenericResult<_>>()?;

    Ok(CollectAllStreamDriven::new(streams))
}
```

### execute_map_all

```rust
/// Execute multiple TaskIterators and apply mapper when all complete.
///
/// Uses execute_collect_all() internally, then wraps with MapAllDoneStreamIterator.
///
/// # Arguments
///
/// * `tasks` - Vector of TaskIterators to execute in parallel
/// * `mapper` - Function to apply to collected results: Fn(Vec<T::Ready>) -> O
/// * `wait_cycle` - Optional polling duration
///
/// # Returns
///
/// GenericResult wrapping MapAllDoneStreamIterator that yields mapped results
pub fn execute_map_all<T, F, O>(
    tasks: Vec<T>,
    mapper: F,
    wait_cycle: Option<Duration>,
) -> GenericResult<MapAllDoneStreamIterator<T, F>>
where
    T: TaskIterator + Send + 'static,
    F: Fn(Vec<T::Ready>) -> O + Send + 'static,
    O: Send + 'static,
{
    // Use execute_stream() for each task, wrap in MapAllDone
    let streams: Vec<DrivenStreamIterator<T>> = tasks
        .into_iter()
        .map(|t| execute_stream(t, wait_cycle))
        .collect::<GenericResult<_>>()?;

    Ok(MapAllDoneStreamIterator::new(streams, mapper))
}
```

## HOW: Implementation Approach

1. Implement `execute_collect_all()` using existing `execute_stream()` for each task
2. Implement `execute_map_all()` using `execute_stream()` + `MapAllDoneStreamIterator`
3. Both functions compose existing types from `drivers.rs` and combinator modules
4. Add re-exports in `valtron/mod.rs`
5. Write integration tests demonstrating parallel execution

## Requirements

1. **execute_collect_all()** - Use execute_stream() for each task, return CollectAllStreamDriven
2. **execute_map_all()** - Use execute_stream() for each task, return MapAllDoneStreamIterator
3. **Platform detection** - Handled by existing execute_stream() function
4. **Proper trait bounds** - Send + 'static for all generics
5. **Integration tests** - Demonstrate parallel task execution with real tasks

## Tasks

1. [ ] Read `backends/foundation_core/src/valtron/executors/unified.rs` to understand existing execute_stream() pattern
2. [ ] Implement `execute_collect_all()` using execute_stream() for each task
3. [ ] Implement `execute_map_all()` using execute_stream() + MapAllDoneStreamIterator
4. [ ] Add re-exports in `valtron/mod.rs` if needed
5. [ ] Write integration test with mock parallel tasks demonstrating speedup
6. [ ] Run clippy and fmt checks

## Verification

```bash
cargo test -p foundation_core -- valtron::executors::unified
cargo clippy -p foundation_core -- -D warnings
cargo fmt -p foundation_core -- --check
```

## Success Criteria

- All 6 tasks completed
- Both helper functions compile with zero errors
- Platform detection works (WASM vs native) via existing execute_stream()
- Integration test demonstrates parallel execution
- Zero clippy warnings

---

_Created: 2026-03-20_
_Updated: 2026-03-20 (Composes existing execute_stream() function)_
