---
workspace_name: "ewe_platform"
spec_directory: "specifications/10-simple-http-client-enhancements"
feature_directory: "specifications/10-simple-http-client-enhancements/features/01-parallel-fetch-pattern"
this_file: "specifications/10-simple-http-client-enhancements/features/01-parallel-fetch-pattern/feature.md"

status: completed
priority: high
created: "2026-03-25"
completed: "2026-03-26"

depends_on:
  - "08-valtron-async-iterators/06b-map-iter-combinator"
  - "08-valtron-async-iterators/07-split-collector"
  - "05-progress-tracking-design"

tasks:
  completed: 5
  uncompleted: 0
  total: 5
  completion_percentage: 100
---

# Parallel Fetch Pattern Feature

## Overview

This feature lifts the parallel fetch task composition pattern from `gen_model_descriptors::create_fetch_task()` into a reusable, documented pattern for the simple_http client.

The pattern enables users to:
1. Create composable fetch tasks with progress tracking
2. Execute multiple heterogeneous fetches in parallel
3. Aggregate results from different sources with graceful error handling

## WHY: Problem Statement

Users of the simple_http client often need to fetch data from multiple endpoints concurrently. The current API supports sequential requests but doesn't provide a documented pattern for:

1. Creating composable fetch tasks with progress tracking
2. Executing multiple heterogeneous fetches in parallel
3. Aggregating results from different sources with graceful error handling

The `gen_model_descriptors` implementation (at `bin/platform/src/gen_model_descriptors/mod.rs`) demonstrates a proven pattern using Valtron TaskIterators with combinators that has been battle-tested in production.

### Source Pattern Analysis

From `gen_model_descriptors::create_fetch_task()`:

```rust
fn create_fetch_task<F>(
    client: &mut SimpleHttpClient,
    source: &'static str,
    url: &'static str,
    parser: F,
) -> Result<
    Box<dyn TaskIterator<
        Ready = Vec<ModelEntry>,
        Pending = FetchPending,
        Spawner = foundation_core::valtron::BoxedSendExecutionAction,
    > + Send + 'static>,
    GenModelError,
>
where
    F: Fn(&str, &'static str) -> Vec<ModelEntry> + Send + Clone + 'static,
{
    let request = client.get(url)?.build()?;
    let pool = client.client_pool().ok_or(GenModelError::Http { ... })?;
    let config = client.client_config();

    SendRequestTask::new(request, 5, pool, config)
        .map_ready(move |intro| {
            match intro {
                RequestIntro::Success { stream, .. } => {
                    // Read body from stream
                    let mut body_text = String::new();
                    for part in stream {
                        match part {
                            Ok(IncomingResponseParts::SizedBody(body))
                            | Ok(IncomingResponseParts::StreamedBody(body)) => {
                                match body {
                                    SendSafeBody::Text(t) => body_text = t,
                                    SendSafeBody::Bytes(b) => {
                                        body_text = String::from_utf8(b.clone())
                                            .unwrap_or_else(|e| {
                                                tracing::warn!("Invalid UTF-8: {e}");
                                                String::new()
                                            });
                                    }
                                    // ... handle other variants
                                    _ => {}
                                }
                            }
                            _ => continue,
                        }
                    }
                    parser(&body_text, source)
                }
                RequestIntro::Failed(e) => {
                    tracing::warn!("Request failed: {e}");
                    Vec::new() // Graceful degradation
                }
            }
        })
        .map_pending(move |p| FetchPending::from_http(p, source))
}
```

## WHAT: Solution Overview

Provide a reusable parallel fetch pattern that:

1. Uses `SendRequestTask::new()` with `.map_ready()` and `.map_pending()` combinators
2. Tracks progress with source identification via `FetchPending` enum
3. Handles errors gracefully (log and return empty results vs. fail fast)
4. Supports `execute_collect_all()` for homogeneous task vectors
5. Provides helper functions for common body reading patterns

### Core API

```rust
/// Create a fetch task for parallel execution
///
/// # Arguments
/// * `client` - HTTP client instance
/// * `source` - Source identifier for progress tracking
/// * `url` - URL to fetch
/// * `parser` - Function to parse response body: `fn(&str, &str) -> T`
///
/// # Returns
/// A TaskIterator that yields `T` on success, with `FetchPending` progress states
pub fn create_fetch_task<F, T>(
    client: &mut SimpleHttpClient,
    source: &'static str,
    url: &'static str,
    parser: F,
) -> Result<
    impl TaskIterator<
        Ready = T,
        Pending = FetchPending,
        Spawner = BoxedSendExecutionAction,
    > + Send + 'static,
    HttpClientError,
>
where
    F: Fn(&str, &'static str) -> T + Send + Clone + 'static,
    T: Default + Send + 'static,
{
    // Implementation
}
```

### Usage Example

```rust
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_core::wire::simple_http::client::tasks::create_fetch_task;
use foundation_core::valtron;

fn fetch_from_multiple_sources() -> Result<Vec<ModelEntry>, Box<dyn std::error::Error>> {
    // Initialize thread pool - MUST keep guard alive
    let _guard = valtron::initialize_pool(100, None);

    // Configure client for parallel fetch
    let mut client = SimpleHttpClient::from_system()
        .max_body_size(None)
        .batch_size(8192 * 2)
        .read_timeout(Duration::from_secs(5))
        .max_retries(3)
        .enable_pool(10);

    // Create fetch tasks for each source
    // Note: Each closure has a unique type, so we need type erasure
    let tasks: Vec<Box<dyn TaskIterator<
        Ready = Vec<ModelEntry>,
        Pending = FetchPending,
        Spawner = BoxedSendExecutionAction,
    > + Send>> = vec![
        create_fetch_task(&mut client, "models.dev", "https://models.dev/api.json", parse_models_dev)?,
        create_fetch_task(&mut client, "openrouter", "https://openrouter.ai/api/v1/models", parse_openrouter)?,
        create_fetch_task(&mut client, "ai-gateway", "https://ai-gateway.vercel.sh/v1/models", parse_ai_gateway)?,
    ];

    // Execute all tasks in parallel and collect results
    let result_stream = valtron::execute_collect_all(tasks, None)?;

    let mut all_models = Vec::new();
    for stream_item in result_stream {
        if let Stream::Next(models) = stream_item {
            all_models.extend(models);
        }
    }

    Ok(all_models)
}
```

## HOW: Implementation

### Step 1: Define FetchPending Enum (Feature 05)

First, define the progress tracking enum in `client::tasks::state`:

```rust
/// WHY: Tracks progress of individual API fetches during parallel execution.
///
/// WHAT: Progress states with source identification for observability.
///
/// HOW: Used as the `Pending` type in TaskIterator combinators.
#[derive(Debug, Clone)]
pub enum FetchPending {
    Connecting { source: &'static str },
    AwaitingResponse { source: &'static str },
}

impl FetchPending {
    /// Convert from HttpRequestPending with source context
    pub fn from_http(p: HttpRequestPending, source: &'static str) -> Self {
        match p {
            HttpRequestPending::WaitingForStream => Self::Connecting { source },
            HttpRequestPending::WaitingIntroAndHeaders => Self::AwaitingResponse { source },
        }
    }
}

impl std::fmt::Display for FetchPending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchPending::Connecting { source } => write!(f, "{source}: Connecting..."),
            FetchPending::AwaitingResponse { source } => {
                write!(f, "{source}: Awaiting response...")
            }
        }
    }
}
```

### Step 2: Implement Body Reading Helper

Create a helper function for reading response bodies:

```rust
/// Read response body from stream, handling all SendSafeBody variants
///
/// # Arguments
/// * `stream` - Iterator over IncomingResponseParts
///
/// # Returns
/// The response body as a String, or empty string on error
fn read_body_from_stream(
    mut stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> String {
    let mut body_text = String::new();

    for part in stream {
        match part {
            Ok(IncomingResponseParts::SizedBody(body))
            | Ok(IncomingResponseParts::StreamedBody(body)) => {
                match body {
                    SendSafeBody::Text(t) => return t,
                    SendSafeBody::Bytes(b) => {
                        return String::from_utf8(b.clone()).unwrap_or_else(|e| {
                            tracing::warn!("Invalid UTF-8 in response: {e}");
                            String::new()
                        });
                    }
                    SendSafeBody::Stream(mut opt_iter) => {
                        if let Some(iter) = opt_iter.take() {
                            let mut bytes = Vec::new();
                            for chunk_result in iter {
                                match chunk_result {
                                    Ok(data) => bytes.extend_from_slice(&data),
                                    Err(e) => {
                                        tracing::warn!("Stream error: {e}");
                                        break;
                                    }
                                }
                            }
                            return String::from_utf8(bytes).unwrap_or_else(|e| {
                                tracing::warn!("Invalid UTF-8 in streamed response: {e}");
                                String::new()
                            });
                        }
                    }
                    SendSafeBody::ChunkedStream(mut opt_iter) => {
                        if let Some(iter) = opt_iter.take() {
                            let mut bytes = Vec::new();
                            for chunk_result in iter {
                                match chunk_result {
                                    Ok(ChunkedData::Data(data, _)) => bytes.extend_from_slice(&data),
                                    Ok(ChunkedData::DataEnded) => break,
                                    Ok(ChunkedData::Trailers(_)) => {}
                                    Err(e) => {
                                        tracing::warn!("Chunked stream error: {e}");
                                        break;
                                    }
                                }
                            }
                            return String::from_utf8(bytes).unwrap_or_else(|e| {
                                tracing::warn!("Invalid UTF-8 in chunked response: {e}");
                                String::new()
                            });
                        }
                    }
                    SendSafeBody::None => {
                        tracing::debug!("No body in response");
                        return String::new();
                    }
                    SendSafeBody::LineFeedStream(_) => {
                        tracing::warn!("LineFeedStream not supported for this use case");
                        return String::new();
                    }
                }
            }
            Ok(IncomingResponseParts::Intro(_, _, _)
            | IncomingResponseParts::Headers(_)
            | IncomingResponseParts::SKIP) => continue,
            Ok(IncomingResponseParts::NoBody) => return String::new(),
            Err(e) => {
                tracing::warn!("Error reading response body: {e}");
                return String::new();
            }
        }
    }

    body_text
}
```

### Step 3: Implement create_fetch_task

```rust
/// Create a fetch task for parallel execution
///
/// # Type Parameters
/// * `F` - Parser function type: `fn(&str, &'static str) -> T`
/// * `T` - Result type, must implement Default for error handling
///
/// # Arguments
/// * `client` - HTTP client instance
/// * `source` - Source identifier for progress tracking and error messages
/// * `url` - URL to fetch
/// * `parser` - Function to parse response body
///
/// # Returns
/// A TaskIterator that yields `T` on success, with `FetchPending` progress states
///
/// # Errors
/// Returns HttpClientError if request construction fails.
/// Individual fetch errors are logged and return T::default() (graceful degradation).
pub fn create_fetch_task<F, T>(
    client: &mut SimpleHttpClient,
    source: &'static str,
    url: &'static str,
    parser: F,
) -> Result<
    impl TaskIterator<
        Ready = T,
        Pending = FetchPending,
        Spawner = BoxedSendExecutionAction,
    > + Send + 'static,
    HttpClientError,
>
where
    F: Fn(&str, &'static str) -> T + Send + Clone + 'static,
    T: Default + Send + 'static,
{
    // Build the HTTP request
    let request = client.get(url).map_err(|e| {
        tracing::error!("Failed to build request for {source}: {e}");
        e
    })?;

    // Extract pool and config before moving request
    let pool = client.client_pool().ok_or_else(|| {
        tracing::error!("No connection pool available");
        HttpClientError::NoPool
    })?;

    let config = client.client_config();

    // Create SendRequestTask and apply combinators
    let task = SendRequestTask::new(request, 5, pool, config)
        .map_ready(move |intro| {
            match intro {
                RequestIntro::Success { stream, .. } => {
                    // Read body from stream using helper
                    let body_text = read_body_from_stream(stream);

                    // Parse and return (or default on error)
                    parser(&body_text, source)
                }
                RequestIntro::Failed(e) => {
                    tracing::warn!("Request failed for {source}: {e}");
                    T::default() // Graceful degradation
                }
            }
        })
        .map_pending(move |p| FetchPending::from_http(p, source));

    Ok(task)
}
```

### Step 4: Usage with execute_collect_all

```rust
// Initialize pool
let _guard = valtron::initialize_pool(100, None);

// Create client
let mut client = SimpleHttpClient::from_system()
    .max_body_size(None)
    .read_timeout(Duration::from_secs(5))
    .enable_pool(10);

// Create homogeneous task vector
// Note: All parsers must return the same type (Vec<ModelEntry>)
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
let result_stream = valtron::execute_collect_all(tasks, None)?;

// Collect results
let mut all_results = Vec::new();
for stream_item in result_stream {
    if let Stream::Next(models) = stream_item {
        all_results.extend(models);
    }
}
```

## Implementation Location

```
backends/foundation_core/src/wire/simple_http/client/tasks/
├── fetch.rs       (NEW - Parallel fetch pattern with create_fetch_task)
└── state.rs       (MODIFY - Add FetchPending enum)
```

### File: fetch.rs

```rust
//! Parallel fetch pattern for simple_http client.
//!
//! WHY: Provides reusable pattern for parallel HTTP fetches with progress tracking.
//!
//! WHAT: Exports create_fetch_task() and read_body_from_stream() helpers.
//!
//! HOW: Uses TaskIterator combinators (.map_ready, .map_pending) for composition.

use crate::valtron::{TaskIterator, TaskIteratorExt, BoxedSendExecutionAction};
use crate::wire::simple_http::client::{
    HttpRequestPending, RequestIntro, SendRequestTask, SimpleHttpClient,
};
use crate::wire::simple_http::{ChunkedData, HttpClientError, IncomingResponseParts, SendSafeBody};

mod state;
pub use state::FetchPending;

/// Read response body from stream, handling all SendSafeBody variants.
pub fn read_body_from_stream(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> String {
    // Implementation as shown above
}

/// Create a fetch task for parallel execution.
pub fn create_fetch_task<F, T>(
    client: &mut SimpleHttpClient,
    source: &'static str,
    url: &'static str,
    parser: F,
) -> Result<
    impl TaskIterator<
        Ready = T,
        Pending = FetchPending,
        Spawner = BoxedSendExecutionAction,
    > + Send + 'static,
    HttpClientError,
>
where
    F: Fn(&str, &'static str) -> T + Send + Clone + 'static,
    T: Default + Send + 'static,
{
    // Implementation as shown above
}
```

### File: state.rs

```rust
//! Progress tracking for parallel fetch operations.

use crate::wire::simple_http::client::HttpRequestPending;

/// WHY: Tracks progress of individual API fetches during parallel execution.
///
/// WHAT: Progress states with source identification for observability.
///
/// HOW: Used as the `Pending` type in TaskIterator combinators.
#[derive(Debug, Clone)]
pub enum FetchPending {
    Connecting { source: &'static str },
    AwaitingResponse { source: &'static str },
}

impl FetchPending {
    /// Convert from HttpRequestPending with source context.
    pub fn from_http(p: HttpRequestPending, source: &'static str) -> Self {
        match p {
            HttpRequestPending::WaitingForStream => Self::Connecting { source },
            HttpRequestPending::WaitingIntroAndHeaders => Self::AwaitingResponse { source },
        }
    }
}

impl std::fmt::Display for FetchPending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchPending::Connecting { source } => write!(f, "{source}: Connecting..."),
            FetchPending::AwaitingResponse { source } => {
                write!(f, "{source}: Awaiting response...")
            }
        }
    }
}
```

## Success Criteria

- [ ] `FetchPending` enum defined in `client::tasks::state` with source tracking
- [ ] `read_body_from_stream()` helper function implemented and tested
- [ ] `create_fetch_task()` function implemented and documented
- [ ] Example code compiles and runs
- [ ] Progress tracking shows source identification
- [ ] Graceful error handling demonstrated (logs warning, returns default)
- [ ] Integration test with multiple parallel fetches passing
- [ ] Module exported in `client::tasks::mod.rs`

## Verification Commands

```bash
# Build the module
cargo build --package foundation_core

# Run tests
cargo test --package foundation_core -- wire::simple_http::client::tasks::fetch

# Check formatting
cargo fmt -- --check

# Run clippy
cargo clippy --package foundation_core -- -D warnings
```

## Notes for Agents

### Important Considerations

1. **PoolGuard Lifecycle**: The guard returned by `valtron::initialize_pool()` MUST be kept alive for the duration of task execution. If dropped, threads shut down immediately.

2. **Type Erasure**: Each closure in Rust has a unique type. To store multiple tasks in a Vec, use `Box<dyn TaskIterator<...>>` for type erasure.

3. **Graceful Degradation**: The pattern uses `T::default()` on error rather than propagating errors. This allows parallel fetch to continue even if individual sources fail.

4. **Source Identification**: The `source` parameter is `'static` because it's typically a string literal identifying the API endpoint.

### Common Pitfalls

1. Dropping PoolGuard before tasks complete
2. Not handling all SendSafeBody variants
3. Forgetting to clone the parser closure
4. Not providing source identification for progress tracking

---

_Created: 2026-03-25_
_Source: gen_model_descriptors::create_fetch_task() analysis_
