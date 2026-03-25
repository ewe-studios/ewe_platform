---
workspace_name: "ewe_platform"
spec_directory: "specifications/10-simple-http-client-enhancements"
feature_directory: "specifications/10-simple-http-client-enhancements/features/09-valtron-executor-integration"
this_file: "specifications/10-simple-http-client-enhancements/features/09-valtron-executor-integration/feature.md"

status: pending
priority: high
created: "2026-03-25"
completed: null

depends_on:
  - "08-valtron-async-iterators/05-unified-executor-integration"

tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0
---

# Valtron Executor Integration

## Overview

This feature documents the complete lifecycle of Valtron executor usage: pool initialization, task execution with `execute()`, and parallel aggregation with `execute_collect_all()`.

## WHY: Problem Statement

Users need to understand the complete lifecycle of Valtron executor usage:
1. Pool initialization and guard lifecycle
2. Task execution with `execute()`
3. Parallel aggregation with `execute_collect_all()`

Without proper understanding:
- PoolGuard is dropped prematurely (threads shut down)
- Tasks never execute or complete
- Parallel execution benefits are not realized
- Confusion between `execute()` and `execute_collect_all()`

### Source Pattern Analysis

From `gen_model_descriptors/mod.rs`:

```rust
use foundation_core::valtron;

pub fn run(args: &clap::ArgMatches) -> std::result::Result<(), BoxedError> {
    // ... setup ...

    // CRITICAL: Keep the pool guard alive for the duration of the function
    let _guard = valtron::initialize_pool(100, None);

    let mut client = SimpleHttpClient::from_system()
        .max_body_size(None)
        .batch_size(8192 * 2)
        .read_timeout(Duration::from_secs(1))
        .max_retries(5)
        .enable_pool(10);

    tracing::info!("Starting model descriptor generation with PARALLEL fetch...");
    let start_time = Instant::now();

    // Create fetch tasks for each source
    let models_dev_task = create_fetch_task(&mut client, "models.dev", URL1, parser1)?;
    let openrouter_task = create_fetch_task(&mut client, "openrouter", URL2, parser2)?;
    let ai_gateway_task = create_fetch_task(&mut client, "ai-gateway", URL3, parser3)?;

    // Execute all tasks and collect their results
    let mut all_models = Vec::new();

    let model_report_stream = valtron::execute_collect_all(
        vec![models_dev_task, openrouter_task, ai_gateway_task],
        None,
    )
    .expect("return stream");

    for stream_item in model_report_stream {
        if let Stream::Next(models) = stream_item {
            all_models.extend(models.into_iter().flatten());
        }
    }

    let fetch_elapsed = start_time.elapsed();
    tracing::info!("Parallel fetch completed in {:?}", fetch_elapsed);
    tracing::info!(
        "Estimated sequential time: ~{:?} (3x slower)",
        fetch_elapsed * 3
    );

    // ... process results ...

    Ok(())
    // _guard dropped here, threads shut down gracefully
}
```

## WHAT: Solution Overview

### Pool Initialization

```rust
use foundation_core::valtron;

// Initialize thread pool with 100 threads
// The returned guard MUST be kept alive
let _guard = valtron::initialize_pool(100, None);

// _guard must stay in scope for task execution
// When _guard drops, threads shut down gracefully
```

### Task Execution with execute()

```rust
use foundation_core::valtron::execute;
use foundation_core::synca::mpp::Stream;

// Create a task
let task = create_fetch_task(&mut client, "source", URL, parser)?;

// Execute the task
let mut result_stream = execute(task, None)?;

// Process results
for item in result_stream {
    match item {
        Stream::Next(result) => {
            // Handle successful result
            println!("Got result: {:?}", result);
        }
        Stream::Pending(p) => {
            // Handle progress update
            tracing::debug!("Progress: {p}");
        }
        Stream::Ignore => {
            // Ignore (task yielded without value)
        }
    }
}
```

### Parallel Aggregation with execute_collect_all()

```rust
use foundation_core::valtron::execute_collect_all;

// Create homogeneous task vector
let tasks: Vec<Box<dyn TaskIterator<
    Ready = Vec<ModelEntry>,
    Pending = FetchPending,
    Spawner = BoxedSendExecutionAction,
> + Send>> = vec![
    create_fetch_task(&mut client, "source1", URL1, parser1)?,
    create_fetch_task(&mut client, "source2", URL2, parser2)?,
    create_fetch_task(&mut client, "source3", URL3, parser3)?,
];

// Execute all tasks in parallel
let aggregated = execute_collect_all(tasks, None)?;

// Collect results (returned as they complete)
let mut all_results = Vec::new();
for item in aggregated {
    if let Stream::Next(results) = item {
        all_results.extend(results);
    }
}
```

## HOW: Critical Patterns

### PoolGuard Lifecycle - CRITICAL

**IMPORTANT**: The `PoolGuard` returned by `initialize_pool()` **must be kept alive**:

```rust
// CORRECT: Keep guard alive for duration of task execution
fn run() -> Result<(), BoxedError> {
    let _guard = valtron::initialize_pool(100, None);

    // Create and execute tasks
    let task = create_task()?;
    let mut stream = valtron::execute(task, None)?;

    for item in stream {
        // Process items
    }

    // Guard dropped here when function returns
    // Threads shut down gracefully after all tasks complete
    Ok(())
}

// WRONG: Discard guard immediately
fn run() -> Result<(), BoxedError> {
    valtron::initialize_pool(100, None);
    // Guard dropped immediately!
    // Threads shut down before tasks can run

    let task = create_task()?;
    let mut stream = valtron::execute(task, None)?;
    // Tasks may never complete - pool is already shut down

    for item in stream {
        // May hang or never receive items
    }
    Ok(())
}

// CORRECT: Store guard in struct for long-lived applications
struct App {
    _pool_guard: PoolGuard,
    client: SimpleHttpClient,
}

impl App {
    fn new() -> Self {
        let _guard = valtron::initialize_pool(100, None);
        Self {
            _pool_guard: _guard,
            client: SimpleHttpClient::from_system()
                .enable_pool(10),
        }
    }

    fn run(&self) -> Result<(), BoxedError> {
        // Tasks can execute because guard is stored in struct
        let task = create_task()?;
        // ...
        Ok(())
    }
}
```

### Parallel Execution Semantics

Tasks passed to `execute_collect_all()` run in parallel:

```rust
use std::time::{Instant, Duration};

let start = Instant::now();

let tasks: Vec<Box<dyn TaskIterator<Ready = (), Pending = _, Spawner = _> + Send>> = vec![
    Box::new(sleep_task(1000)), // 1 second task
    Box::new(sleep_task(1000)), // 1 second task
    Box::new(sleep_task(1000)), // 1 second task
];

let _results = execute_collect_all(tasks, None)?;

let elapsed = start.elapsed();
// elapsed ≈ 1 second (parallel), NOT 3 seconds (sequential)

// Each task runs on a separate thread from the pool
// Tasks start as soon as a thread is available
```

### Result Ordering

Results are returned as they complete, NOT in task order:

```rust
let tasks: Vec<Box<dyn TaskIterator<Ready = String, Pending = _, Spawner = _> + Send>> = vec![
    Box::new(delayed_task("first", 3000)),  // Completes 3rd
    Box::new(delayed_task("second", 1000)), // Completes 1st
    Box::new(delayed_task("third", 2000)),  // Completes 2nd
];

let results: Vec<String> = execute_collect_all(tasks, None)?
    .filter_map(|item| match item {
        Stream::Next(result) => Some(result),
        _ => None,
    })
    .collect();

// results = ["second", "third", "first"] (order of completion)
// NOT ["first", "second", "third"] (task order)
```

### Error Handling in Tasks

Errors should be handled in the task's `.map_ready()` closure:

```rust
let task = SendRequestTask::new(request, 5, pool, config)
    .map_ready(|intro| {
        match intro {
            RequestIntro::Success { stream, .. } => {
                match parse_response(stream) {
                    Ok(result) => result,
                    Err(e) => {
                        tracing::warn!("Parse error: {e}");
                        MyType::default() // Graceful degradation
                    }
                }
            }
            RequestIntro::Failed(e) => {
                tracing::warn!("Request failed: {e}");
                MyType::default()
            }
        }
    });
```

## Implementation Location

Valtron executor functions exist in:
```
backends/foundation_core/src/valtron/executors/
```

Documentation should be added to:
```
documentation/valtron/doc.md (MODIFY - Add executor lifecycle guide)
```

## Success Criteria

- [ ] Pool initialization pattern documented
- [ ] PoolGuard lifecycle emphasized (CRITICAL)
- [ ] execute() usage shown with examples
- [ ] execute_collect_all() usage shown with examples
- [ ] Common pitfall (dropping guard) highlighted
- [ ] Parallel execution semantics demonstrated
- [ ] Result ordering behavior explained
- [ ] Error handling pattern shown

## Verification Commands

```bash
# Build the module
cargo build --package foundation_core

# Run tests
cargo test --package foundation_core -- valtron::executors

# Check formatting
cargo fmt -- --check

# Run clippy
cargo clippy --package foundation_core -- -D warnings
```

## Notes for Agents

### Critical: PoolGuard Lifecycle

The most common mistake is dropping the `PoolGuard` prematurely:

```rust
// BUG: Guard is dropped at end of this block
{
    let _guard = valtron::initialize_pool(100, None);
} // _guard dropped here, threads shut down

// Tasks will never execute
let task = create_task()?;
let mut stream = valtron::execute(task, None)?;
```

**Always** keep the guard alive for the duration of task execution.

### Thread Pool Sizing

- Default: 100 threads is usually excessive
- For I/O-bound tasks (HTTP fetches): 10-20 threads is often sufficient
- For CPU-bound tasks: Match thread count to CPU cores

```rust
// For HTTP fetching
let _guard = valtron::initialize_pool(20, None);

// For CPU-bound work
let num_cpus = std::thread::available_parallelism()
    .map(|n| n.get())
    .unwrap_or(4);
let _guard = valtron::initialize_pool(num_cpus, None);
```

### Common Pitfalls

1. **Dropping PoolGuard**: Most common and critical error
2. **Expecting ordered results**: Results come back in completion order
3. **Too few threads**: I/O-bound tasks need more threads than CPU count
4. **Not handling errors in tasks**: Errors in tasks should be caught and handled
5. **Blocking the executor**: Don't do long blocking operations in task closures

---

_Created: 2026-03-25_
_Source: gen_model_descriptors valtron integration analysis_
