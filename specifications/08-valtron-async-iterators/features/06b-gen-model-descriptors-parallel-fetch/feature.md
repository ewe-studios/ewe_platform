---
feature: "gen_model_descriptors Parallel Fetch"
description: "Use execute_collect_all() for parallel API fetches in gen_model_descriptors"
status: "pending"
priority: "high"
depends_on: ["05-unified-executor-integration"]
estimated_effort: "medium"
created: 2026-03-20
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# gen_model_descriptors Parallel Fetch Feature

## WHY: Problem Statement

The current `gen_model_descriptors` implementation fetches model metadata from three upstream APIs **sequentially**:

```rust
// Sequential blocking - each fetch blocks until complete
let mut all_models = Vec::new();
all_models.extend(fetch_models_dev(&client));      // ~500ms, blocks
all_models.extend(fetch_openrouter(&client));      // ~500ms, blocks
all_models.extend(fetch_ai_gateway(&client));      // ~500ms, blocks
// Total: ~1500ms
```

**Key Design Principle: execute_collect_all() for Parallel Execution**

```rust
// TaskIterators go into execute_collect_all()...
let tasks = vec![
    create_fetch_task(client.clone(), "models.dev")?,
    create_fetch_task(client.clone(), "openrouter")?,
    create_fetch_task(client.clone(), "ai_gateway")?,
];

// ...StreamIterator comes out
let collected = execute_collect_all(tasks, None)?;
// Parallel execution (~500ms total)

for stream_item in collected {
    match stream_item {
        Stream::Pending(count) => println!("{count} still pending"),
        Stream::Next(results) => process(results),
    }
}
```

End users work with `StreamIterator` from `execute_collect_all()` - never dealing with TaskIterator directly.

## WHAT: Solution Overview

Use `execute_collect_all()` from `unified.rs` to execute multiple fetch tasks in parallel:

### Pattern: create_fetch_task Helper

```rust
/// Create a fetch task for a specific API using TaskIterator combinators
fn create_fetch_task(
    client: SimpleHttpClient,
    api_name: &str,
    url: &str,
) -> Result<impl TaskStatusIterator<
    Ready = Vec<ModelEntry>,
    Pending = FetchPending,
    Spawner = BoxedSendExecutionAction,
> + Send + 'static, HttpClientError>
{
    // Build request
    let request = client.get(url)?;

    // Create SendRequestTask and apply combinators (BEFORE execute)
    let task = SendRequestTask::new(request, pool, config)
        // Transform: HttpResponse → Vec<ModelEntry>
        .map_ready(|response| {
            let json = response.body().as_json()?;
            parse_models_response(json, api_name)
        })
        // Transform: HttpPending → FetchPending with source tracking
        .map_pending(|p| FetchPending::from_http(p, api_name))
        // Add progress tracking
        .stream_collect();

    Ok(task)
}
```

### run() Function Using execute_collect_all()

```rust
fn run(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    valtron::initialize_pool(100, None);
    let client = SimpleHttpClient::from_system()?;
    let start_time = Instant::now();

    tracing::info!("Starting model descriptor generation with parallel fetch...");

    // Create tasks using helper (TaskIterators are inputs)
    let tasks = vec![
        create_fetch_task(client.clone(), "models.dev", "https://models.dev/api/models")?,
        create_fetch_task(client.clone(), "openrouter", "https://openrouter.ai/api/v1/models")?,
        create_fetch_task(client.clone(), "ai_gateway", "https://ai-gateway.internal/api/models")?,
    ];

    // Execute all in parallel - returns StreamIterator (end user type)
    let collected = execute_collect_all(tasks, None)?;

    // Process stream with progress reporting
    let mut merged: Vec<ModelEntry> = Vec::new();
    let mut progress_updates = 0u64;

    for stream_item in collected {
        match stream_item {
            Stream::Pending(_) => {
                progress_updates += 1;
                if progress_updates % 10 == 0 {
                    tracing::info!("Fetch in progress... ({} updates)", progress_updates);
                }
                continue;
            }
            Stream::Delayed(duration) => {
                tracing::debug!("Fetch delayed by {:?}", duration);
                continue;
            }
            Stream::Next(results_vec) => {
                tracing::info!("All API fetches complete!");
                merged = results_vec.into_iter().flatten().collect();
                break;
            }
            Stream::Init => {
                tracing::debug!("Fetch initializing...");
                continue;
            }
        }
    }

    let fetch_elapsed = start_time.elapsed();
    tracing::info!("Parallel fetch completed in {:?}", fetch_elapsed);
    tracing::info!("Estimated sequential time: ~{:?} (3x slower)", fetch_elapsed * 3);
    tracing::info!("Total models collected: {}", merged.len());

    // Existing processing pipeline
    apply_overrides(&mut merged);
    let providers = deduplicate(merged);
    let rust_source = generate_rust(&providers);

    std::fs::write("backends/foundation_ai/src/models/model_descriptors.rs", &rust_source)?;
    tracing::info!("Generated model descriptors");

    Ok(())
}
```

## HOW: Implementation Steps

1. **Read unified.rs** - Understand execute_collect_all() pattern
2. **Create `FetchPending` enum** - Progress states with source tracking
3. **Create `create_fetch_task()` helper** - Compose SendRequestTask with combinators
4. **Create parser functions** - `parse_models_response()`, `parse_openrouter_response()`, etc.
5. **Update `run()` function** - Use `execute_collect_all()` with composed tasks
6. **Add benchmark timing** - Log fetch elapsed time

## Requirements

1. **FetchPending enum** - Progress states with source tracking, `from_http()` conversion
2. **create_fetch_task() helper** - Returns composed TaskIterator with combinators (input to execute)
3. **Parser functions** - Transform HttpResponse → Vec<ModelEntry> for each API
4. **run() function** - Uses `execute_collect_all()` returning StreamIterator
5. **Benchmark output** - Log elapsed time and estimated sequential equivalent
6. **execute_collect_all()** - Takes TaskIterators, returns StreamIterator

## Tasks

1. [ ] Define `FetchPending` enum with Connecting, AwaitingResponse, ParsingJson, ProcessingModels
2. [ ] Implement `FetchPending::from_http()` conversion
3. [ ] Create `create_fetch_task()` helper function
4. [ ] Implement parser functions for each API format
5. [ ] Refactor `run()` to use `execute_collect_all()` with composed tasks
6. [ ] Test: Run generator, verify output matches original
7. [ ] Test: Verify ~3x speedup (1500ms → 500ms)
8. [ ] Run clippy and fmt checks

## Verification

```bash
cargo check -p ewe_platform
cargo run --bin ewe_platform gen_model_descriptors
diff backends/foundation_ai/src/models/model_descriptors.rs expected_output.rs
cargo clippy -p ewe_platform -- -D warnings
cargo fmt -p ewe_platform -- --check
```

## Success Criteria

- All 8 tasks completed
- `gen_model_descriptors` uses `execute_collect_all()` pattern
- Generated output matches original
- Execution time reduced by ~3x
- Progress logging shows fetch states
- `execute_collect_all()` takes TaskIterators, returns StreamIterator
- Zero clippy warnings

## Relationship to Other Features

| Feature | Role |
|---------|------|
| 05-unified-executor-integration | Provides `execute_collect_all()` helper |
| 01-task-iterators | TaskIterator combinators for create_fetch_task() |
| 02-stream-iterators | StreamIterator combinators for processing results |
| 06a-client-request-refactor | Optional: Can use refactored ClientRequest |

## Architecture Flow

```
create_fetch_task() → TaskIterator (input)
                         │
                         │ execute_collect_all()
                         ▼
              StreamIterator (output to end users)
                         │
                         │ for stream_item in collected
                         ▼
              Stream::Pending, Stream::Next, etc.
```

---

_Created: 2026-03-20_
_Updated: 2026-03-20 (v3.0: execute_collect_all() returns StreamIterator)_
