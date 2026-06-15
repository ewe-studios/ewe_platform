# Valtron Async Runtime - Capabilities Reference

**Created:** 2026-03-25
**Purpose:** Reference guide for using valtron's async runtime capabilities in foundation_ai features

---

## Overview

Valtron is the async runtime in `foundation_core` that provides a unified executor framework with support for both single-threaded (WASM, no-thread) and multi-threaded (native) execution.

### Key Design Principles (from LEARNINGS.md)

1. **TaskIterator is Input, StreamIterator is Output** - `execute()` takes TaskIterator, returns StreamIterator
2. **Never block** - All iterator methods yield `Stream` states instead of waiting
3. **Clear separation** - TaskIterator for implementers, StreamIterator for end users
4. **execute() is the boundary** - Hides executor concerns (delays, actions, spawner)
5. **Combinators before execute()** - All TaskIterator combinators applied BEFORE calling execute()
6. **Standard iterators after execute()** - Use standard Iterator combinators on StreamIterator output

---

## Core Types

### TaskIterator Trait

For implementers defining async tasks. Yields `TaskStatus<Ready, Pending, Spawner>`:

```rust
pub trait TaskIterator {
    type Ready;      // Final result type
    type Pending;    // Intermediate state type
    type Spawner: ExecutionAction;  // For spawning child tasks

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>>;
}
```

### TaskStatus Enum

```rust
pub enum TaskStatus<D, P, S: ExecutionAction> {
    Init,                          // Task initializing
    Ready(D),                      // Task produced result
    Pending(P),                    // Task still processing
    Delayed(Duration),             // Task waiting (retry/backoff)
    Spawn(S),                      // Spawn child task
    Ignore,                        // Skip this item (filtering)
}
```

### Stream Enum (from synca::mpp)

```rust
pub enum Stream<D, P> {
    Init,
    Ignore,
    Delayed(Duration),
    Pending(P),
    Next(D),  // Final result
}
```

### StreamIterator Trait

For end users consuming results. What `execute()` returns:

```rust
pub trait StreamIterator {
    type Ready;
    type Pending;

    fn next_stream(&mut self) -> Option<Stream<Self::Ready, Self::Pending>>;
}
```

---

## Executor API (unified.rs)

### Platform Auto-Selection

| Platform | Feature | Executor Used |
|----------|---------|---------------|
| WASM     | any     | `single`      |
| Native   | none    | `single`      |
| Native   | `multi` | `multi`       |

### execute() Functions

```rust
/// Execute a task, returns StreamIterator (default, recommended)
pub fn execute<T>(
    task: T,
    wait_cycle: Option<Duration>,
) -> GenericResult<DrivenStreamIterator<T>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static;

/// Execute and collect all tasks, returns combined StreamIterator
pub fn execute_collect_all<T>(
    tasks: Vec<T>,
    wait_cycle: Option<Duration>,
) -> GenericResult<CollectAllStreamIterator<T::Ready, T::Pending>>;

/// Execute and map all tasks, returns mapped StreamIterator
pub fn execute_map_all<T, F, O>(
    tasks: Vec<T>,
    mapper: F,
    wait_cycle: Option<Duration>,
) -> GenericResult<MapAllStreamIterator<O, T::Pending>>;

/// Opt-in: Execute as TaskStatus iterator (for advanced use)
pub fn execute_as_task<T>(
    task: T,
    wait_cycle: Option<Duration>,
) -> GenericResult<DrivenRecvIterator<T>>;
```

### Multi-threaded Pool (multi/mod.rs)

```rust
/// Initialize multi-threaded pool, returns PoolGuard
/// PoolGuard MUST be kept alive for the entire duration that tasks are executing.
/// Typically called once at application startup in main() or in tests.
pub fn initialize_pool(seed_for_rng: u64, user_thread_num: Option<usize>) -> PoolGuard;

/// Block on with setup function
pub fn block_on<F>(seed_from_rng: u64, thread_num: Option<usize>, setup: F) -> PoolGuard
where
    F: FnOnce(LocalPoolHandle);

/// Spawn task to global queue
pub fn spawn<Task, Action>() -> ThreadPoolTaskBuilder<...>;
```

**Usage Pattern:**

```rust
// In main.rs (binary entry point)
fn main() -> Result<(), BoxedError> {
    // Initialize pool once at application startup
    let _guard = valtron::initialize_pool(100, None);

    // Now you can call execute() anywhere in your application
    // The pool stays alive as long as _guard is alive
    run_application()?;

    // Pool shuts down when _guard is dropped (at end of main)
    Ok(())
}

// In tests
#[test]
fn test_my_task() {
    let _guard = valtron::initialize_pool(4, None);
    // Pool stays alive for duration of test
    let task = MyTask::new();
    let stream = execute(task, None).unwrap();
    // ...
}

// In library code - NO PoolGuard needed!
// The library code uses execute() but doesn't initialize the pool.
// The pool is initialized by the binary that links against the library.
pub fn my_library_function() -> Result<(), BoxedError> {
    // Just call execute() - pool should already be initialized by caller
    let task = MyTask::new();
    let stream = execute(task, None)?;
    // ...
    Ok(())
}
```

### Single-threaded Executor (local.rs)

```rust
/// Local thread executor with block_on()
pub struct LocalThreadExecutor<T: ProcessController + Clone> {
    // ...
}

impl<T> LocalThreadExecutor<T> {
    pub fn block_on(&self);  // Main event loop
}
```

---

## TaskIterator Combinators (BEFORE execute())

All combinators must be applied BEFORE calling `execute()`:

```rust
// Builder pattern via ExecutionTaskIteratorBuilder
let task = MyTask::new()
    .with_mappers(mapper)      // Add status mapper
    .with_resolver(resolver)   // Add custom resolver
    .with_parent(entry);       // Set parent for tracking

// Or using blanket impl TaskIteratorExt
let task = my_task
    .map_ready(|value| transform(value))           // Transform Ready values
    .map_pending(|state| log(state))               // Transform Pending values
    .filter_ready(|value| predicate(value))        // Filter Ready values
    .stream_collect();                             // Convert to stream collection
```

### Available Combinators

| Combinator | Purpose | Applied Before/After execute() |
|------------|---------|-------------------------------|
| `map_ready(f)` | Transform Ready values | Before |
| `map_pending(f)` | Transform Pending values | Before |
| `filter_ready(f)` | Filter Ready values | Before |
| `stream_collect()` | Convert to stream collection | Before |
| `split_collector()` | Observer + continuation pattern | Both |
| `split_collect_until()` | Split until predicate matches | Both |
| `map_iter()` | Nested iterator patterns | Before |

---

## StreamIterator Combinators (AFTER execute())

After `execute()` returns StreamIterator, use standard Iterator combinators:

```rust
let stream = execute(task, None)?;

// Standard iterator combinators
let results: Vec<_> = stream
    .filter_map(|item| match item {
        Stream::Next(value) => Some(value),
        _ => None,
    })
    .collect();

// Or use StreamIteratorExt combinators
let stream = stream
    .map_all_done(|values| combine(values))
    .collect();
```

### Available Combinators

| Combinator | Purpose | Returns |
|------------|---------|---------|
| `collect_all()` | Collect from multiple streams | Combined StreamIterator |
| `map_all_done(f)` | Map when all sources complete | Mapped StreamIterator |
| `map_all_pending_and_done(f)` | Map with state awareness | Mapped StreamIterator |
| `split_collector()` | Observer + continuation | (observer, continuation) |
| `split_collect_until(f)` | Split until predicate | (observer, continuation) |

---

## State Machine Pattern (state_machine.rs)

For complex async workflows, implement `StateMachine` trait:

```rust
pub trait StateMachine {
    type State: Clone;
    type Output;
    type Error;
    type Action: ExecutionAction;

    fn transition(&mut self, state: Self::State) -> StateTransition<Self::State, Self::Output, Self::Error, Self::Action>;
    fn initial_state(&self) -> Self::State;
}

pub enum StateTransition<S, O, E, A: ExecutionAction> {
    Continue(S),           // Continue processing
    Yield(O, S),           // Yield value, new state
    Complete(O),           // Task complete
    Error(E),              // Task failed
    Delay(Duration, S),    // Wait before continuing
    Spawn(A, S),           // Spawn child task
}

// Wrap any StateMachine as TaskIterator
pub struct StateMachineTask<M: StateMachine> { /* ... */ }
```

### Example State Machine

```rust
#[derive(Clone)]
enum FetchState {
    Initializing,
    Fetching { url: String },
    Parsing { body: String },
    Complete(Vec<ModelEntry>),
}

struct FetchMachine {
    state: FetchState,
    client: SimpleHttpClient,
}

impl StateMachine for FetchMachine {
    type State = FetchState;
    type Output = Vec<ModelEntry>;
    type Error = FetchError;
    type Action = NoAction;

    fn transition(&mut self, state: FetchState) -> StateTransition<...> {
        match state {
            FetchState::Initializing => {
                StateTransition::Continue(FetchState::Fetching { url: self.url.clone() })
            }
            FetchState::Fetching { url } => {
                // In real impl, this would be async with Pending state
                match self.client.get(&url) {
                    Ok(body) => StateTransition::Yield(body, FetchState::Parsing { body }),
                    Err(e) => StateTransition::Error(e),
                }
            }
            FetchState::Parsing { body } => {
                match parse_models(&body) {
                    Ok(models) => StateTransition::Complete(models),
                    Err(e) => StateTransition::Error(e),
                }
            }
            FetchState::Complete(models) => StateTransition::Complete(models),
        }
    }

    fn initial_state(&self) -> Self::State {
        FetchState::Initializing
    }
}

// Execute the state machine
let task = StateMachineTask::new(FetchMachine { ... });
let stream = execute(task, None)?;
```

---

## Common Patterns

### Pattern 0: Application Structure (PoolGuard Usage)

**PoolGuard is only needed in binary entry points and tests** - library code does NOT initialize the pool:

```rust
// ===== main.rs (Binary Entry Point) =====
fn main() -> Result<(), BoxedError> {
    // Initialize pool ONCE at application startup
    let _guard = valtron::initialize_pool(100, None);

    // Run application - pool stays alive
    run_app()?;

    // Pool shuts down when _guard is dropped
    Ok(())
}

fn run_app() -> Result<(), BoxedError> {
    // Library code - NO PoolGuard here!
    // Just use execute() - pool is already initialized
    let task = MyTask::new();
    let stream = execute(task, None)?;
    // ...
    Ok(())
}

// ===== In tests =====
#[test]
fn test_my_feature() {
    // Initialize pool for this test
    let _guard = valtron::initialize_pool(4, None);

    let task = MyTask::new();
    let stream = execute(task, None).unwrap();
    // ...
}

// ===== Library code (foundation_ai, foundation_db, etc.) =====
// NO PoolGuard - the binary that uses your library will initialize the pool
pub fn library_function() -> Result<(), BoxedError> {
    // Just call execute() - assumes caller has initialized pool
    let task = MyTask::new();
    let stream = execute(task, None)?;
    // ...
    Ok(())
}
```

### Pattern 1: Simple Task Execution

```rust
// 1. Define task (implements TaskIterator)
struct MyTask { /* ... */ }

impl TaskIterator for MyTask {
    type Ready = MyResult;
    type Pending = MyState;
    type Spawner = NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<...>> {
        // Your async logic here
    }
}

// 2. Execute and consume
let task = MyTask::new();
let stream = execute(task, None)?;

for item in stream {
    match item {
        Stream::Next(result) => { /* handle result */ }
        Stream::Pending(state) => { /* handle intermediate state */ }
        _ => {}
    }
}
```

### Pattern 2: Parallel Fetches

```rust
// Create multiple fetch tasks
let task1 = FetchTask::new("api1.example.com");
let task2 = FetchTask::new("api2.example.com");
let task3 = FetchTask::new("api3.example.com");

// Execute all in parallel, collect when all complete
let tasks = vec![task1, task2, task3];
let collected = execute_collect_all(tasks, None)?;

// Process combined results
for item in collected {
    match item {
        Stream::Next(all_results) => {
            // all_results is Vec<FetchResult>
            let merged: Vec<_> = all_results.into_iter().flatten().collect();
        }
        _ => {}
    }
}
```

### Pattern 3: Observer/Continuation (split_collector)

```rust
// Split stream into observer + continuation
let (observer, continuation) = task.split_collector(|item| {
    // Predicate: which items to observe?
    matches!(item, TaskStatus::Pending(_))
});

// Observer receives matching items
let headers: Vec<_> = observer.collect();

// Continue with main stream
let stream = execute(continuation, None)?;
let body = stream.collect();
```

### Pattern 4: State Machine with Retry

```rust
struct RetryMachine {
    attempts: u32,
    max_attempts: u32,
}

impl StateMachine for RetryMachine {
    fn transition(&mut self, state: Self::State) -> StateTransition<...> {
        match self.fetch_data() {
            Ok(data) => StateTransition::Complete(data),
            Err(e) if self.attempts < self.max_attempts => {
                self.attempts += 1;
                StateTransition::Delay(Duration::from_secs(2), state)
            }
            Err(e) => StateTransition::Error(e),
        }
    }
}
```

### Pattern 5: PoolGuard Lifecycle

**PoolGuard is for binary entry points and tests only** - library code does NOT need to handle PoolGuard:

```rust
// CORRECT: Binary entry point keeps PoolGuard alive
fn main() -> Result<(), BoxedError> {
    let _guard = initialize_pool(100, None);
    run_app()?;  // All execute() calls happen here
    // Guard dropped at end of main, pool shuts down
    Ok(())
}

// CORRECT: Tests initialize their own pool
#[test]
fn test_my_task() {
    let _guard = initialize_pool(4, None);
    // Test code with execute() calls
}

// CORRECT: Library code does NOT initialize pool
pub fn library_function() -> Result<(), BoxedError> {
    // Just call execute() - pool should be initialized by binary
    let task = MyTask::new();
    let stream = execute(task, None)?;
    Ok(())
}

// WRONG: Library code initializing pool
pub fn wrong_library_function() -> Result<(), BoxedError> {
    let _guard = initialize_pool(100, None);  // Don't do this in libraries!
    let task = MyTask::new();
    let stream = execute(task, None)?;
    Ok(())
}
```

**Why**: `PoolGuard::Drop` signals all worker threads to shut down. The binary that links against your libraries should initialize the pool once at startup and keep it alive for the application's lifetime. Library code should assume the pool is already available.

---

## Testing Patterns

### Using #[traced_test]

```rust
#[traced_test]
#[test]
fn test_my_task() {
    // Test implementation with full trace logs
    let task = MyTask::new();
    let stream = execute(task, None).unwrap();

    for item in stream {
        match item {
            Stream::Next(result) => {
                assert_eq!(result, expected);
            }
            _ => {}
        }
    }

    // Logs show exactly what happened during execution
}
```

### Testing with PoolGuard

```rust
#[test]
fn test_parallel_execution() {
    let _guard = initialize_pool(4, None);

    let task1 = CounterTask::new(10);
    let task2 = CounterTask::new(10);
    let collected = execute_collect_all(vec![task1, task2], None).unwrap();

    for item in collected {
        match item {
            Stream::Next(results) => {
                assert_eq!(results.len(), 2);
            }
            _ => {}
        }
    }
}
```

---

## Tracing Best Practices (from LEARNINGS.md)

### Do: Generic Log Messages

```rust
// Good: No Debug/Display required on generics
tracing::trace!("TaskIterator: received Ready item");
tracing::debug!("StreamIterator: queue closed, no more items");
tracing::error!("Task failed during execution");
```

### Don't: Format Generic Types

```rust
// Bad: Requires D: Debug
tracing::debug!("Got Ready({:?})", value);

// Bad: Silently ignoring errors
let _ = self.queue.force_push(item);
```

### Handle Errors Properly

```rust
// Good: Log errors for debugging
if let Err(e) = self.queue.force_push(item) {
    tracing::error!("Failed to push to queue: {}", e);
} else {
    tracing::trace!("Copied item to observer queue");
}
```

---

## Key Takeaways from LEARNINGS.md

1. **Queue Closing**: Use `ConcurrentQueue::close()` on natural completion, not just in Drop
2. **Close on Natural Completion**: Close queue when `inner.next()` returns `None`, not in Drop
3. **Tracing for Generics**: Use generic log messages without `{:?}` formatting
4. **Don't Ignore Errors**: Handle and log errors, don't use `let _ =`
5. **Use #[traced_test]**: Invaluable for debugging async iterator behavior
6. **PoolGuard Lifecycle**: Initialize once in `main()` or tests; library code does NOT initialize the pool
7. **Heterogeneous Closures**: Can't use `execute_collect_all()` - execute each task individually
8. **Combinators Before execute()**: All TaskIterator combinators applied BEFORE calling execute()
9. **StreamIterator After execute()**: Use standard Iterator combinators on StreamIterator output

---

## Quick Reference

### Before execute() (TaskIterator)

```rust
let task = MyTask::new()
    .map_ready(|v| transform(v))
    .map_pending(|s| log(s))
    .filter_ready(|v| predicate(v));

let stream = execute(task, None)?;
```

### After execute() (StreamIterator)

```rust
let results: Vec<_> = stream
    .filter_map(|item| match item {
        Stream::Next(v) => Some(v),
        _ => None,
    })
    .collect();
```

### Parallel Execution

```rust
let tasks = vec![task1, task2, task3];
let collected = execute_collect_all(tasks, None)?;
```

### State Machine

```rust
let machine = MyStateMachine::new();
let task = StateMachineTask::new(machine);
let stream = execute(task, None)?;
```

---

## Related Files

- `backends/foundation_core/src/valtron/task.rs` - TaskStatus, Stream definitions
- `backends/foundation_core/src/valtron/executors/unified.rs` - execute() functions
- `backends/foundation_core/src/valtron/executors/multi/mod.rs` - Multi-threaded executor
- `backends/foundation_core/src/valtron/executors/local.rs` - Single-threaded executor
- `backends/foundation_core/src/valtron/executors/state_machine.rs` - State machine helpers
- `backends/foundation_core/src/valtron/executors/builders.rs` - Task builders
- `backends/foundation_core/src/synca/mpp.rs` - Stream, StreamIterator traits
- `specifications/08-valtron-async-iterators/LEARNINGS.md` - Implementation learnings
