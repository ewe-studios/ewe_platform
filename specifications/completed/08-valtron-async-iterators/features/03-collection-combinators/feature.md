---
feature: "Collection Combinators"
description: "CollectAll types that aggregate multiple async sources via execute() returning StreamIterator"
status: "complete"
priority: "high"
depends_on: ["01-task-iterators", "02-stream-iterators"]
estimated_effort: "medium"
created: 2026-03-20
author: "Main Agent"
tasks:
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100%
---

# Collection Combinators Feature

## WHY: Problem Statement

When executing multiple async tasks in parallel (e.g., fetching from 3 APIs), we need to collect results without blocking. Standard `collect()` waits for all items sequentially and blocks. Need a combinator that:

- Aggregates outputs from multiple TaskIterators via `execute()`
- Returns `Stream<Vec<D>, P>` types only (not TaskStatus)
- Tracks pending count and yields `Stream::Pending` while any sources incomplete
- Yields `Stream::Next(Vec<D>)` only when all sources complete

**Current problem in gen_model_descriptors**:

```rust
// Sequential blocking - each extend() blocks until complete
let mut all_models = Vec::new();
all_models.extend(fetch_models_dev(&client));      // Blocks
all_models.extend(fetch_openrouter(&client));      // Blocks
all_models.extend(fetch_ai_gateway(&client));      // Blocks
```

**Desired parallel pattern**:

```rust
// TaskIterators are inputs only - execute() returns StreamIterator
let tasks = vec![fetch_models_dev_task(client), fetch_openrouter_task(client), fetch_ai_gateway_task(client)];
let collected: impl StreamIterator<Vec<ModelEntry>, _> = execute_collect_all(tasks)?;

for stream_item in collected {
    match stream_item {
        Stream::Pending(count) => { /* count sources still pending */ }
        Stream::Next(all_models) => { /* all models collected! */ }
        Stream::Delayed(_) => continue,
    }
}
```

## WHAT: Solution Overview

### Key Design: TaskIterators Are Inputs, StreamIterators Are Outputs

```
┌─────────────────┐     execute()      ┌──────────────────┐
│  TaskIterator   │ ─────────────────► │  StreamIterator  │
│  (input only)   │                    │  (output)        │
└─────────────────┘                    └──────────────────┘
```

**Design principle**:
- `execute(tasks)` - Takes `TaskIterator` inputs, returns `StreamIterator`
- `execute_as_task(tasks)` - Opt-in when you specifically need `TaskIterator` output (rare)
- All combinators work with `Stream<D, P>` after execution

### collect_all() Function

```rust
/// Execute multiple TaskIterators in parallel and collect results.
///
/// Uses execute() internally which:
/// 1. Takes TaskIterator inputs
/// 2. Hands them off to executor engine (hides delays, actions, spawner concerns)
/// 3. Returns StreamIterator that yields Stream<D, P> variants
///
/// # Arguments
///
/// * `tasks` - Vector of TaskIterators to execute in parallel
/// * `wait_cycle` - Optional polling duration (defaults to DEFAULT_WAIT_CYCLE)
///
/// # Returns
///
/// StreamIterator that yields:
/// - Stream::Pending(count) - while sources are pending
/// - Stream::Next(Vec<D>) - when all sources complete
/// - Stream::Delayed(duration) - if any source is delayed
///
/// # Example
///
/// ```rust
/// // TaskIterators are inputs - execute() returns StreamIterator
/// let tasks = vec![task1, task2, task3];
/// let collected = execute_collect_all(tasks, None)?;
///
/// for stream_item in collected {
///     match stream_item {
///         Stream::Pending(count) => println!("{count} still pending..."),
///         Stream::Next(results) => process(results),
///         Stream::Delayed(dur) => continue,
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
    // Use execute() for each task - returns StreamIterator
    let streams: Vec<DrivenStreamIterator<T>> = tasks
        .into_iter()
        .map(|t| execute(t, wait_cycle))
        .collect::<GenericResult<_>>()?;

    Ok(CollectAllStream::new(streams))
}
```

### CollectAllStream Type

```rust
/// Collects outputs from multiple TaskIterators executed via execute().
///
/// This type holds the StreamIterators returned from execute() and polls them,
/// yielding Stream::Pending while any are pending,
/// and Stream::Next(Vec<D>) when all complete.
pub struct CollectAllStream<T>
where
    T: TaskIterator + Send + 'static,
{
    sources: Vec<DrivenStreamIterator<T>>,
    pending_count: usize,
    collected: Vec<T::Ready>,
}

impl<T> Iterator for CollectAllStream<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    type Item = Stream<Vec<T::Ready>, T::Pending>;

    fn next(&mut self) -> Option<Self::Item> {
        // Poll all sources round-robin
        // Track pending_count, decrement as sources complete
        // Return Stream::Pending while any pending
        // Return Stream::Next(collected_vec) when all done
        // Return Stream::Delayed if any source delayed
    }
}
```

### execute_as_task() - Opt-in for TaskIterator Output

```rust
/// Execute and return as TaskIterator instead of StreamIterator.
///
/// Use this ONLY when you specifically need TaskIterator output
/// (e.g., further TaskStatus-aware transformations).
///
/// This is intentionally separate from execute() to make
/// TaskIterator output an opt-in choice.
pub fn execute_as_task<T>(
    task: T,
    wait_cycle: Option<Duration>,
) -> GenericResult<DrivenRecvIterator<T>>
where
    T: TaskIterator + Send + 'static,
{
    // Returns DrivenRecvIterator which yields TaskStatus
    execute(task, wait_cycle)
}
```

## HOW: Implementation Approach

1. Implement `CollectAllStream<T>` that holds `Vec<DrivenStreamIterator<T>>` (returned from `execute()`)
2. Round-robin polling logic, tracking pending count
3. Yield `Stream<Vec<D>, P>` variants (not `TaskStatus`)
4. Use `execute()` internally which hides executor concerns (delays, actions, spawner)

## Requirements

1. **CollectAllStream** - Aggregates StreamIterators from `execute()`, yields `Stream<Vec<D>, P>`
2. **Round-robin polling** - Poll all sources fairly
3. **Pending tracking** - Track count of pending sources
4. **Completion detection** - Detect when all sources complete
5. **Stream output only** - All results are `Stream<D, P>` variants, not `TaskStatus`
6. **execute() as entry point** - TaskIterators handed off to executor, which returns StreamIterator

## Tasks

1. [x] Define `CollectAllStream<T>` struct holding `Vec<DrivenStreamIterator<T>>`
2. [x] Implement `Iterator` for `CollectAllStream<T>` yielding `Stream<Vec<T::Ready>, T::Pending>`
3. [x] Implement round-robin polling logic
4. [x] Track pending_count, detect completion
5. [x] Write unit tests for collection behavior
6. [x] Run clippy and fmt checks

## Verification

```bash
cargo test -p foundation_core -- valtron::collect_all
cargo clippy -p foundation_core -- -D warnings
cargo fmt -p foundation_core -- --check
```

## Success Criteria

- All 6 tasks completed
- `CollectAllStream<T>` compiles with zero errors
- Yields `Stream<Vec<D>, P>` variants (not `TaskStatus`)
- Round-robin polling works correctly
- Pending tracking accurate
- Unit tests pass for completion detection
- Zero clippy warnings

---

_Created: 2026-03-20_
_Updated: 2026-03-20 (TaskIterators are inputs, execute() returns StreamIterator)_
