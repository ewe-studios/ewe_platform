---
feature: "gen_model_descriptors Refactor"
description: "Refactor gen_model_descriptors to compose existing HTTP client tasks with TaskIteratorExt combinators for parallel API fetches"
status: "pending"
priority: "high"
depends_on: ["05-unified-executor-integration"]
estimated_effort: "large"
created: 2026-03-20
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0%
---

# gen_model_descriptors Refactor Feature

## WHY: Problem Statement

Current `gen_model_descriptors` fetches from 3 APIs **sequentially** with blocking HTTP calls:

```rust
// Current sequential pattern in bin/platform/src/gen_model_descriptors/mod.rs
fn run(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    valtron::initialize_pool(100, None);
    let client = SimpleHttpClient::from_system()...;

    // Sequential blocking calls - ~500ms each
    let models_dev = fetch_models_dev(&client);      // ~500ms, blocks
    let openrouter = fetch_openrouter(&client);      // ~500ms, blocks
    let ai_gateway = fetch_ai_gateway(&client);      // ~500ms, blocks

    // Total: ~1500ms
    let mut all_models = Vec::new();
    all_models.extend(models_dev);
    all_models.extend(openrouter);
    all_models.extend(ai_gateway);
    // ...
}
```

**Problems**:
1. **3x slower than needed** - Each fetch blocks, total time = sum of all fetch times
2. **Valtron initialized but unused** - Pool created, but fetches don't leverage it
3. **Poor user experience** - Running the generator takes unnecessarily long
4. **No progress visibility** - User has no idea how far along the fetch is

**Expected speedup**: Sequential ~1500ms → Parallel ~500ms (speed of slowest single fetch)

## WHAT: Solution Overview

**Key Insight**: We don't need custom `FetchModelsDevTask` structs. The existing `SendRequestTask` from the HTTP client already implements `TaskIterator`. We compose it with our new `TaskIteratorExt` combinators:

```rust
// OLD: Would create custom FetchModelsDevTask struct
// NEW: Compose existing SendRequestTask with combinators

fn fetch_models_dev_task(
    client: SimpleHttpClient,
) -> impl TaskStatusIterator<Ready = Vec<ModelEntry>, Pending = FetchPending> + Send + 'static {
    // 1. Build HTTP request
    let request = client.get("https://models.dev/api/models")
        .header("Accept", "application/json")
        .build()?;

    // 2. Use existing SendRequestTask (already implements TaskIterator)
    // 3. Chain combinators to transform output:
    SendRequestTask::new(request, pool, config)
        // Transform Ready(HttpResponse) → Ready(Vec<ModelEntry>)
        .map_ready(|response| {
            let json = response.body().as_json()?;
            parse_models_response(json)
        })
        // Transform HttpPending → FetchPending
        .map_pending(|http_pending| http_pending.to_fetch_pending())
        // Add stream_collect for progress reporting
        .stream_collect()
}
```

### Pattern 1: Compose Existing Tasks with TaskIteratorExt

```rust
// Build task by composing existing SendRequestTask with combinators
let models_dev_task = SendRequestTask::new(
    client.get("https://models.dev/api/models")?,
    pool.clone(),
    config.clone()
)
.map_ready(|response| {
    // Transform: HttpResponse → Vec<ModelEntry>
    let json = response.body().as_json()?;
    let models: ModelsResponse = serde_json::from_str(json)?;
    models.entries.into_iter()
        .filter(|m| m.is_enabled())
        .collect::<Vec<_>>()
})
.map_pending(|pending| {
    // Transform: HttpPending → FetchPending with source tracking
    FetchPending::from_http(pending, "models.dev")
});
```

### Pattern 2: Parallel Execution with execute_collect_all

```rust
// Build all three tasks
let tasks = vec![
    fetch_models_dev_task(client.clone()),
    fetch_openrouter_task(client.clone()),
    fetch_ai_gateway_task(client.clone()),
];

// Execute all in parallel
// Returns DrivenStreamIterator yielding Stream<Vec<Vec<ModelEntry>>, CombinedPending>
let collected = valtron::execute_collect_all(tasks, None)?;
```

### Pattern 3: Progress Reporting with stream_collect

```rust
// Each task wrapped with stream_collect() reports progress
let task = SendRequestTask::new(request, pool, config)
    .map_ready(parse_response)
    .stream_collect();

for status in execute_stream(task)? {
    match status {
        Stream::Pending(StreamCollectStatus::Pending { count, pending_info }) => {
            // Progress update: count items collected, pending_info shows fetch state
            tracing::info!("Fetched {} models, state: {:?}", count, pending_info);
        }
        Stream::Pending(StreamCollectStatus::Ready(models)) => {
            // Collection complete
            tracing::info!("Fetch complete! Got {} models", models.len());
        }
        Stream::Next(_) => { /* Final aggregated result */ }
    }
}
```

### Complete Refactored run() Function

```rust
use valtron::{TaskIteratorExt, execute_collect_all, execute_stream};
use crate::wire::simple_http::client::tasks::SendRequestTask;

fn run(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    // Initialize Valtron pool
    valtron::initialize_pool(100, None);
    let client = SimpleHttpClient::from_system()?;
    let pool = client.pool().clone();
    let config = client.config().clone();

    let start_time = Instant::now();
    tracing::info!("Starting model descriptor generation with parallel fetch...");

    // Build task iterators by composing SendRequestTask with combinators
    let tasks = vec![
        // Task 1: Fetch from models.dev
        SendRequestTask::new(
            client.get("https://models.dev/api/models")?,
            pool.clone(),
            config.clone()
        )
        .map_ready(|response| parse_models_response(response.body()))
        .map_pending(|p| FetchPending::from_http(p, "models.dev"))
        .stream_collect(),

        // Task 2: Fetch from OpenRouter
        SendRequestTask::new(
            client.get("https://openrouter.ai/api/v1/models")?,
            pool.clone(),
            config.clone()
        )
        .map_ready(|response| {
            let models = parse_openrouter_response(response.body())?;
            // Filter by capability
            models.into_iter()
                .filter(|m| m.supports("inference"))
                .collect::<Vec<_>>()
        })
        .map_pending(|p| FetchPending::from_http(p, "openrouter"))
        .stream_collect(),

        // Task 3: Fetch from AI Gateway
        SendRequestTask::new(
            client.get("https://ai-gateway.internal/api/models")?,
            pool.clone(),
            config.clone()
        )
        .map_ready(|response| parse_gateway_response(response.body()))
        .map_pending(|p| FetchPending::from_http(p, "ai_gateway"))
        .stream_collect(),
    ];

    // Execute all tasks in parallel
    let collected = execute_collect_all(tasks, None)
        .map_err(|e| format!("Failed to execute tasks: {}", e))?;

    // Process stream with progress reporting
    let mut merged: Vec<ModelEntry> = Vec::new();
    let mut progress_counter = 0u64;

    for stream_item in collected {
        match stream_item {
            Stream::Pending(_) => {
                progress_counter += 1;
                if progress_counter % 10 == 0 {
                    tracing::info!("Fetch in progress... ({} updates)", progress_counter);
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

### FetchPending Type

```rust
/// Progress information during fetch operations
#[derive(Clone, Debug, PartialEq)]
pub enum FetchPending {
    /// Connecting to endpoint
    Connecting { endpoint: String },
    /// Waiting for response
    AwaitingResponse { endpoint: String },
    /// Parsing JSON response
    ParsingJson { bytes_received: usize, endpoint: String },
    /// Processing models
    ProcessingModels { models_count: usize, endpoint: String },
}

impl FetchPending {
    /// Convert from HttpPending with source endpoint
    pub fn from_http(http: HttpPending, endpoint: &str) -> Self {
        match http {
            HttpPending::Connecting { .. } => FetchPending::Connecting { endpoint: endpoint.to_string() },
            HttpPending::AwaitingResponse { .. } => FetchPending::AwaitingResponse { endpoint: endpoint.to_string() },
            HttpPending::ReadingBody { bytes_read } => FetchPending::ParsingJson {
                bytes_received: bytes_read,
                endpoint: endpoint.to_string()
            },
        }
    }
}
```

## HOW: Refactoring Steps

1. **Read existing code** - `bin/platform/src/gen_model_descriptors/mod.rs`
2. **Read existing HTTP client tasks** - `wire/simple_http/client/tasks/send_request.rs`
3. **Create FetchPending enum** - Progress states with endpoint tracking, conversion from HttpPending
4. **Create parser functions** - `parse_models_response()`, `parse_openrouter_response()`, `parse_gateway_response()`
5. **Update run() function** - Compose SendRequestTask with TaskIteratorExt combinators
6. **Add benchmark timing** - Log fetch elapsed time and estimated sequential equivalent
7. **Test** - Ensure generated output matches original

## Requirements

1. **FetchPending enum** - Progress states with endpoint tracking, `from_http()` conversion
2. **Parser functions** - Transform HttpResponse → Vec<ModelEntry> for each API
3. **TaskIteratorExt combinators** - map_ready, map_pending, stream_collect working correctly
4. **Refactored run()** - Uses execute_collect_all(), processes Stream results with progress logging
5. **Benchmark output** - Log fetch elapsed time, estimated sequential time, speedup factor
6. **Output parity** - Generated Rust code must match original output

## Tasks

1. [ ] Read existing `bin/platform/src/gen_model_descriptors/mod.rs`
2. [ ] Read `backends/foundation_core/src/wire/simple_http/client/tasks/send_request.rs`
3. [ ] Define `FetchPending` enum with Connecting, AwaitingResponse, ParsingJson, ProcessingModels variants
4. [ ] Implement `FetchPending::from_http()` conversion from HttpPending
5. [ ] Implement parser functions for each API response format
6. [ ] Refactor `run()` to compose SendRequestTask with TaskIteratorExt combinators
7. [ ] Add progress reporting and benchmark timing to run()
8. [ ] Test: Run generator, compare output with original
9. [ ] Test: Verify execution time is ~3x faster
10. [ ] Run clippy and fmt checks

## Verification

```bash
# Build check
cargo check -p ewe_platform

# Run generator (new parallel way)
cargo run --bin ewe_platform gen_model_descriptors

# Verify output matches expected
diff backends/foundation_ai/src/models/model_descriptors.rs expected_output.rs

# Lint check
cargo clippy -p ewe_platform -- -D warnings

# Format check
cargo fmt -p ewe_platform -- --check
```

## Success Criteria

- All 10 tasks completed
- `cargo check -p ewe_platform` passes with zero errors
- Generated `model_descriptors.rs` matches original output
- Execution time reduced by ~3x (1500ms → 500ms)
- Progress logging shows fetch state updates during execution
- Benchmark logging shows speedup comparison
- Zero clippy warnings

## Benchmark and Progress Logging Example

```
[INFO] Starting model descriptor generation with parallel fetch...
[DEBUG] Fetch initializing...
[INFO] Fetch in progress... (10 status updates)
[DEBUG] Fetch delayed by 50ms
[INFO] Fetch in progress... (20 status updates)
[INFO] All API fetches complete!
[INFO] Parallel fetch completed in 523ms
[INFO] Estimated sequential time: ~1569ms (3x slower)
[INFO] Total models collected: 423
[INFO] Generated backends/foundation_ai/src/models/model_descriptors.rs
```

## Design Principles Demonstrated

| Pattern | Usage in This Feature |
|---------|----------------------|
| **Compose existing TaskIterators** | SendRequestTask (existing) + TaskIteratorExt combinators (new) |
| **No custom TaskIterator structs** | Avoids FetchModelsDevTask, FetchOpenRouterTask, etc. |
| **map_ready** | Transforms HttpResponse → Vec<ModelEntry> |
| **map_pending** | Transforms HttpPending → FetchPending with source tracking |
| **stream_collect** | Wraps tasks for progress reporting via StreamCollectStatus |
| **execute_collect_all** | Parallel execution of multiple composed tasks |
| **Forwarding unknown states** | Combinators forward Delayed/Init/Spawn unchanged |

---

_Created: 2026-03-20_
_Updated: 2026-03-20 (Composes existing SendRequestTask with TaskIteratorExt combinators - no custom TaskIterator structs needed)_
