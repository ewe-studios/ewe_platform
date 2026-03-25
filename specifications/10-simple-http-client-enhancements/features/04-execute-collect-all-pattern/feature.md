---
workspace_name: "ewe_platform"
spec_directory: "specifications/10-simple-http-client-enhancements"
feature_directory: "specifications/10-simple-http-client-enhancements/features/04-execute-collect-all-pattern"
this_file: "specifications/10-simple-http-client-enhancements/features/04-execute-collect-all-pattern/feature.md"

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

# execute_collect_all Pattern

## Overview

This feature documents the parallel aggregation pattern using `valtron::execute_collect_all()` for executing multiple TaskIterators concurrently and collecting their results.

## WHY: Problem Statement

Users need to execute multiple TaskIterators in parallel and aggregate results. The pattern differs based on whether tasks are homogeneous (same types) or heterogeneous (different closure types).

Key challenges:
1. Understanding when `execute_collect_all()` can be used vs. individual `execute()` calls
2. Handling type erasure for homogeneous task vectors
3. Understanding parallel execution semantics

### Source Pattern Analysis

From `gen_model_descriptors/mod.rs`:

```rust
// Create fetch tasks for each source
// Note: Each closure has a unique type - can't store in homogeneous Vec
let models_dev_task = create_fetch_task(&mut client, "models.dev", URL1, parser1)?;
let openrouter_task = create_fetch_task(&mut client, "openrouter", URL2, parser2)?;
let ai_gateway_task = create_fetch_task(&mut client, "ai-gateway", URL3, parser3)?;

// Execute all tasks and collect their results
// Each task is executed separately but they run in parallel on the thread pool
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
```

**Key Insight**: The code comment reveals an important detail: "Each task is executed separately but they run in parallel on the thread pool." This is possible because `create_fetch_task` returns `Box<dyn TaskIterator<...>>` - using type erasure to create a homogeneous vector.

## WHAT: Solution Overview

### Pattern 1: Homogeneous Tasks with execute_collect_all

When all tasks have the same `Ready`, `Pending`, and `Spawner` types:

```rust
use foundation_core::valtron::{execute_collect_all, TaskIterator};
use foundation_core::wire::simple_http::client::tasks::{create_fetch_task, FetchPending};

// Create homogeneous task vector using type erasure (Box)
let tasks: Vec<Box<dyn TaskIterator<
    Ready = Vec<ModelEntry>,
    Pending = FetchPending,
    Spawner = BoxedSendExecutionAction,
> + Send>> = vec![
    create_fetch_task(&mut client, "source1", URL1, parser1)?,
    create_fetch_task(&mut client, "source2", URL2, parser2)?,
    create_fetch_task(&mut client, "source3", URL3, parser3)?,
];

// Execute all in parallel
let result_stream = execute_collect_all(tasks, None)?;

// Collect results
let mut all_results = Vec::new();
for stream_item in result_stream {
    if let Stream::Next(models) = stream_item {
        all_results.extend(models);
    }
}
```

### Pattern 2: Individual execute() for Heterogeneous Tasks

When tasks have different types (not using type erasure):

```rust
use foundation_core::valtron::execute;

// Each task has a unique type - execute individually
let task1 = create_fetch_task(&mut client, "source1", URL1, parser1)?;
let task2 = create_fetch_task(&mut client, "source2", URL2, parser2)?;
let task3 = create_fetch_task(&mut client, "source3", URL3, parser3)?;

// Execute each task - they still run in parallel on the thread pool
let mut stream1 = execute(task1, None)?;
let mut stream2 = execute(task2, None)?;
let mut stream3 = execute(task3, None)?;

// Collect from each stream
let mut all_results = Vec::new();
for stream in [&mut stream1, &mut stream2, &mut stream3] {
    for item in stream {
        if let Stream::Next(result) = item {
            all_results.extend(result);
        }
    }
}
```

### Pattern 3: Scoped Task Vector with Type Inference

Using `vec!` macro with explicit type annotation:

```rust
// Type annotation on the vector helps inference
let tasks: Vec<Box<dyn TaskIterator<Ready = Vec<MyType>, Pending = _, Spawner = _> + Send>> = vec![
    Box::new(create_fetch_task(&mut client, "s1", URL1, parser1)?),
    Box::new(create_fetch_task(&mut client, "s2", URL2, parser2)?),
];

let results = execute_collect_all(tasks, None)?;
```

## HOW: Key Insights

### Type Erasure is Required for execute_collect_all

The fundamental challenge is that each closure in Rust has a unique, anonymous type. Even if two closures have the same signature:

```rust
let parser1 = |s: &str, src: &str| Vec::<ModelEntry>::new();
let parser2 = |s: &str, src: &str| Vec::<ModelEntry>::new();
// parser1 and parser2 have DIFFERENT types!
```

This means `create_fetch_task(&mut client, "s1", URL1, parser1)` and `create_fetch_task(&mut client, "s2", URL2, parser2)` return different types.

**Solution**: Use trait objects (`Box<dyn TaskIterator<...>>`) for type erasure:

```rust
// The function returns impl TraitIterator, which we box for type erasure
fn create_fetch_task<F, T>(...) -> Result<impl TaskIterator<...>, Error> {
    // ...
}

// Box each task to erase the concrete type
let tasks: Vec<Box<dyn TaskIterator<...>>> = vec![
    Box::new(create_fetch_task(...)?),
    Box::new(create_fetch_task(...)?),
];
```

### Parallel Execution Semantics

Important: Tasks passed to `execute_collect_all()` run in parallel on the thread pool, NOT sequentially.

```rust
let start = Instant::now();

let tasks: Vec<Box<dyn TaskIterator<Ready = (), Pending = _, Spawner = _> + Send>> = vec![
    Box::new(slow_task(1000)), // 1 second task
    Box::new(slow_task(1000)), // 1 second task
    Box::new(slow_task(1000)), // 1 second task
];

let _results = execute_collect_all(tasks, None)?;

let elapsed = start.elapsed();
// elapsed ≈ 1 second (parallel), NOT 3 seconds (sequential)
```

### Stream Item Collection Pattern

The `execute_collect_all()` function returns a stream where each item is the `Ready` type from the tasks. For tasks returning `Vec<T>`:

```rust
let result_stream = execute_collect_all(tasks, None)?;

// Pattern 1: Flatten nested vectors
let mut all_results = Vec::new();
for stream_item in result_stream {
    if let Stream::Next(models) = stream_item {
        all_results.extend(models); // models is Vec<T>
    }
}

// Pattern 2: Collect then flatten
let all_results: Vec<Vec<T>> = result_stream
    .filter_map(|item| match item {
        Stream::Next(result) => Some(result),
        _ => None,
    })
    .collect();
let flattened: Vec<T> = all_results.into_iter().flatten().collect();
```

## Implementation Location

The `execute_collect_all()` function exists in:
```
backends/foundation_core/src/valtron/executors/
```

Documentation should reference this pattern in:
```
documentation/valtron/doc.md (MODIFY - Add parallel aggregation examples)
```

## Success Criteria

- [ ] Homogeneous vs heterogeneous task pattern documented
- [ ] Type erasure with Box explained clearly
- [ ] Parallel execution semantics demonstrated
- [ ] Stream item collection patterns shown
- [ ] Code examples compile and run
- [ ] Performance benefit of parallelism shown (timing example)

## Verification Commands

```bash
# Build the module
cargo build --package foundation_core

# Run tests (if examples include tests)
cargo test --package foundation_core -- valtron::executors

# Check formatting
cargo fmt -- --check

# Run clippy
cargo clippy --package foundation_core -- -D warnings
```

## Notes for Agents

### Important Considerations

1. **Type Erasure Trade-off**: Using `Box<dyn Trait>` adds a small heap allocation and indirection. For most HTTP fetch operations, this is negligible compared to network latency.

2. **Task Spawning**: Each task in the vector is spawned independently. They begin executing as soon as threads are available from the pool.

3. **Result Ordering**: Results from `execute_collect_all()` are returned as they complete, NOT in task order. Don't rely on result ordering.

4. **Error Handling**: If one task fails, other tasks continue executing. Handle errors in each task's `.map_ready()` closure for graceful degradation.

### Common Pitfalls

1. Trying to store tasks without type erasure (compiler error: "expected struct X, found struct Y")
2. Expecting results in task order (results are returned as they complete)
3. Not understanding that tasks run in parallel (document this clearly)
4. Forgetting that `execute_collect_all()` needs homogeneous types

---

_Created: 2026-03-25_
_Source: gen_model_descriptors execute_collect_all usage analysis_
