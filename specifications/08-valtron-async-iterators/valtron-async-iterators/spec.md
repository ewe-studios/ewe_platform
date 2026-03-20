# Valtron Async Iterator Traits

**Date:** 2026-03-20
**Status:** Proposed
**Related:** `backends/foundation_core/src/valtron/`

---

## WHY: Problem Statement

The current `gen_model_descriptors` implementation fetches model metadata from three upstream APIs sequentially:

```rust
let models_dev = fetch_models_dev(&client);
let openrouter = fetch_openrouter(&client);
let ai_gateway = fetch_ai_gateway(&client);
```

While Valtron's `unified.rs::execute()` and `execute_stream()` provide async task execution capabilities, the fetch functions use synchronous blocking HTTP calls via `http_get_json()`. This means:

1. **No parallelism**: Each fetch blocks until completion before the next begins
2. **Underutilized Valtron**: The execution engine's async capabilities are not leveraged
3. **Wasted opportunity**: These are independent I/O operations that could run concurrently

The root cause: Rust's standard `Iterator` trait is fundamentally synchronous and blocking. Methods like `map()`, `filter()`, `collect()` all block waiting for each element. This conflicts with Valtron's `TaskStatus` flow where operations can be `Pending`, `Delayed`, `Init`, or `Ready`.

## WHAT: Solution Overview

Create a parallel set of iterator traits and types that work natively with Valtron's `TaskStatus` and `Stream` types, enabling:

1. **TaskIterator** - Iterator-like operations over `TaskStatus<Ready, Pending, Spawner>` values
2. **StreamIterator** - Iterator-like operations over `Stream<Done, Pending>` values
3. **Conversion methods**:
   - `TaskIterator::into_task_streams()` - Convert to Stream-based processing
   - `TaskIterator::into_stream()` - Convert TaskIterator to StreamIterator via RecvIterator
4. **Collection utilities**:
   - `collect_all()` - Collect outputs from multiple iterators in Valtron-friendly way
   - `map_all()` - Map values from iterator groups with state-aware handling
5. **State-aware combinators**:
   - `map_all_done()` - Only process when all sources are Done
   - `map_all_pending_and_done()` - Process Pending and Done states together

### Key Design Principle

Unlike standard `Iterator`, these Valtron-native methods must:
- Never block waiting for completion
- Share `TaskStatus` or `Stream` state as first-class operations
- Handle `Pending` and `Delayed` states explicitly
- Integrate with Valtron's executor (spawn, delays, scheduling)

Only via `.into_task_streams()` and `.into_streams()` should iterator-style methods be available, ensuring proper TaskStatus/Stream handling throughout.

---

## HOW: Implementation Plan

### Phase 1: Core Trait Definitions

Create `backends/foundation_core/src/valtron/task_iterators.rs`:

```rust
/// Core trait for TaskStatus-aware iterator operations
pub trait TaskStatusIterator: Iterator<Item = TaskStatus<Self::Ready, Self::Pending, Self::Spawner>>
where
    Self: TaskIterator,
{
    type Ready;
    type Pending;
    type Spawner: ExecutionAction;

    /// Convert into TaskStreams for parallel async processing
    fn into_task_streams(self) -> TaskStreams<Self::Ready, Self::Pending, Self::Spawner>;

    /// Convert into StreamIterator via RecvIterator for execution engine integration
    fn into_stream(self) -> StreamIterator<Self::Ready, Self::Pending>;

    /// Collect all outputs from multiple TaskIterators
    fn collect_all<I>(iterators: Vec<I>) -> CollectAllTaskIterator<I::Ready, I::Pending, I::Spawner>
    where
        I: TaskStatusIterator;

    /// Map all values from iterator groups
    fn map_all<I, F, O>(iterators: Vec<I>, mapper: F) -> MapAllTaskIterator<O, I::Pending, I::Spawner>
    where
        I: TaskStatusIterator,
        F: Fn(Vec<I::Ready>) -> O + Send + 'static;
}
```

### Phase 2: Stream Iterator Extensions

Create `backends/foundation_core/src/valtron/stream_iterators.rs`:

```rust
/// Extension trait for Stream-aware iterator operations
pub trait StreamIteratorExt<D, P>: StreamIterator<D, P> {
    /// Convert underlying Stream iterator to TaskIterator
    fn into_task_iter(self) -> StreamAsTaskIterator<D, P>;

    /// Collect all outputs from multiple StreamIterators
    fn collect_all<I>(iterators: Vec<I>) -> CollectAllStreamIterator<I::Item, I::Pending>
    where
        I: StreamIteratorExt;

    /// Map all values with state awareness
    fn map_all<I, F, O>(iterators: Vec<I>, mapper: F) -> MapAllStreamIterator<O, I::Pending>
    where
        I: StreamIteratorExt,
        F: Fn(Vec<I::Item>) -> O + Send + 'static;

    /// Only process when all sources reach Done state
    fn map_all_done<I, F, O>(iterators: Vec<I>, mapper: F) -> MapAllDoneStreamIterator<O, I::Pending>
    where
        I: StreamIteratorExt,
        F: Fn(Vec<I::Item>) -> O + Send + 'static;

    /// Process both Pending and Done states together
    fn map_all_pending_and_done<I, F, O>(iterators: Vec<I>, mapper: F) -> MapAllPendingDoneStreamIterator<O, I::Pending>
    where
        I: StreamIteratorExt,
        F: Fn(Vec<StreamState<I::Item, I::Pending>>) -> O + Send + 'static;
}
```

### Phase 3: Collection Types

Implement collection iterators that aggregate multiple sources:

```rust
/// Collects outputs from multiple TaskIterators into single TaskStatus stream
pub struct CollectAllTaskIterator<D, P, S> {
    sources: Vec<Box<dyn TaskStatusIterator<Ready = D, Pending = P, Spawner = S>>,
    pending_count: usize,
    collected: Vec<D>,
}

impl<D, P, S> Iterator for CollectAllTaskIterator<D, P, S>
where
    S: ExecutionAction,
{
    type Item = TaskStatus<Vec<D>, P, S>;

    fn next(&mut self) -> Option<Self::Item> {
        // Poll all sources, track pending count
        // Return TaskStatus::Pending with count while any are pending
        // Return TaskStatus::Ready with collected Vec when all done
    }
}

/// Collects outputs from multiple StreamIterators
pub struct CollectAllStreamIterator<D, P> {
    sources: Vec<Box<dyn StreamIterator<D, P>>>,
    collected: Vec<D>,
}
```

### Phase 4: Mapping Combinators

Implement state-aware mapping using existing `Stream<D, P>`:

```rust
/// Maps values when all sources reach Ready state
pub struct MapAllDoneStreamIterator<D, O, P> {
    sources: Vec<Box<dyn StreamIterator<Item = Stream<D, P>>>>,
    mapper: Box<dyn Fn(Vec<D>) -> O + Send>,
    buffer: Vec<Option<D>>,
}

/// Maps values processing both Pending and Next states
pub struct MapAllPendingDoneStreamIterator<D, O, P> {
    sources: Vec<Box<dyn StreamIterator<Item = Stream<D, P>>>>,
    mapper: Box<dyn Fn(Vec<Stream<D, P>>) -> O + Send>,
}
```

**Note**: Use existing `Stream<D, P>` from `synca/mpp.rs`, NOT a new `StreamState` enum:

```rust
// Already exists in synca/mpp.rs
pub enum Stream<D, P> {
    Init,
    Ignore,
    Delayed(Duration),
    Pending(P),
    Next(D),  // This is "Done"
}
```

### Phase 5: Integration with unified.rs

Update `backends/foundation_core/src/valtron/executors/unified.rs`:

```rust
/// Execute multiple TaskIterators in parallel, collecting results
pub fn execute_collect_all<T>(
    tasks: Vec<T>,
    wait_cycle: Option<Duration>,
) -> GenericResult<CollectAllTaskIterator<T::Ready, T::Pending, T::Spawner>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    // Spawn all tasks via executor
    // Return CollectAllTaskIterator that aggregates results
}

/// Execute and map multiple TaskIterators
pub fn execute_map_all<T, F, O>(
    tasks: Vec<T>,
    mapper: F,
    wait_cycle: Option<Duration>,
) -> GenericResult<MapAllTaskIterator<O, T::Pending, T::Spawner>>
where
    T: TaskIterator + Send + 'static,
    F: Fn(Vec<T::Ready>) -> O + Send + 'static,
{
    // Spawn all tasks
    // Apply mapper when all complete
}
```

### Phase 6: Update gen_model_descriptors

Refactor `bin/platform/src/gen_model_descriptors/mod.rs`:

**Before:**
```rust
fn fetch_models_dev(client: &SimpleHttpClient) -> Vec<ModelEntry> {
    let data: serde_json::Value = http_get_json(client, "https://models.dev/api.json")?;
    // ... process synchronously ...
    models
}

fn run(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    valtron::initialize_pool(100, None);

    let client = SimpleHttpClient::from_system()...;

    // Sequential blocking calls
    let models_dev = fetch_models_dev(&client);
    let openrouter = fetch_openrouter(&client);
    let ai_gateway = fetch_ai_gateway(&client);

    // Merge results
    let mut all_models = Vec::new();
    all_models.extend(models_dev);
    all_models.extend(openrouter);
    all_models.extend(ai_gateway);
    // ...
}
```

**After:**
```rust
/// Returns a StreamRecvIterator that fetches models.dev data asynchronously
fn fetch_models_dev_stream(
    client: SimpleHttpClient,
) -> impl StreamIterator<Vec<ModelEntry>, FetchPending> + Send + 'static {
    // Wrap HTTP fetch in Valtron task
    // Return StreamRecvIterator via execute_stream()
}

/// Returns TaskIterator for parallel composition
fn fetch_models_dev_task(
    client: SimpleHttpClient,
) -> impl TaskStatusIterator<Ready = Vec<ModelEntry>, Pending = FetchPending> + Send + 'static {
    // Return TaskIterator that yields TaskStatus during fetch
}

fn run(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    valtron::initialize_pool(100, None);

    let client = SimpleHttpClient::from_system()...;

    // Create async fetch tasks
    let models_dev_task = fetch_models_dev_task(client.clone());
    let openrouter_task = fetch_openrouter_task(client.clone());
    let ai_gateway_task = fetch_ai_gateway_task(client.clone());

    // Execute all in parallel, collect when all complete
    let all_tasks = vec![models_dev_task, openrouter_task, ai_gateway_task];
    let collected = valtron::execute_collect_all(all_tasks, None)?;

    // Process collected results (still async, can map/filter before blocking)
    let merged: Vec<ModelEntry> = collected
        .into_stream()
        .map_all_done(|results| {
            results.into_iter().flatten().collect()
        })
        .collect();

    // Continue with apply_overrides, deduplicate, etc.
    apply_overrides(&mut merged);
    // ...
}
```

---

## Type Relationships

```
TaskIterator
    │
    ├── .into_task_streams() → TaskStreams (parallel processing)
    │
    ├── .into_stream() → StreamIterator (via RecvIterator)
    │       │
    │       └── .into_streams() → Stream operations
    │
    └── .collect_all(Vec<TaskIterator>) → CollectAllTaskIterator
            │
            └── Yields TaskStatus<Vec<Ready>, Pending, Spawner>

StreamIterator
    │
    ├── .into_task_iter() → StreamAsTaskIterator
    │
    ├── .collect_all(Vec<StreamIterator>) → CollectAllStreamIterator
    │
    ├── .map_all(...) → Map values with state awareness
    │
    ├── .map_all_done(...) → Only when all sources Done
    │
    └── .map_all_pending_and_done(...) → Process Pending + Done together
```

---

## Module Structure

```
backends/foundation_core/src/valtron/
├── task_iterators.rs          # TaskStatusIterator trait + combinators
├── stream_iterators.rs        # StreamIteratorExt trait + combinators
├── collect_all.rs             # CollectAllTaskIterator, CollectAllStreamIterator
├── map_all.rs                 # MapAll*, MapAllDone*, MapAllPendingDone* types
└── executors/
    └── unified.rs             # execute_collect_all, execute_map_all functions
```

---

## Testing Strategy

1. **Unit tests** for each iterator type:
   - Verify state transitions (Pending → Ready)
   - Test collect_all with mixed completion times
   - Test map_all variants with different state combinations

2. **Integration tests** in `tests/backends/foundation_core/units/valtron/`:
   - Parallel task execution with collect_all
   - Stream conversion and processing
   - End-to-end gen_model_descriptors refactoring

3. **Benchmarks**:
   - Compare sequential vs parallel fetch times
   - Measure overhead of TaskStatus/Stream wrapping

---

## Benefits

1. **True parallelism**: All three API fetches run concurrently
2. **Non-blocking**: Intermediate states visible, can apply transformations while waiting
3. **Executor integration**: Proper spawn/delay handling through Valtron
4. **Composable**: Build complex async pipelines from simple combinators
5. **Type-safe**: Compiler enforces proper state handling

---

## Migration Path

1. Implement core traits (Phases 1-2)
2. Implement collection types (Phase 3)
3. Implement mapping combinators (Phase 4)
4. Add unified.rs helpers (Phase 5)
5. Refactor gen_model_descriptors (Phase 6)
6. Remove old sequential implementation

Each phase can be tested independently before proceeding.
