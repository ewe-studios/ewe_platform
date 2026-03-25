---
workspace_name: "ewe_platform"
spec_directory: "specifications/10-simple-http-client-enhancements"
feature_directory: "specifications/10-simple-http-client-enhancements/features/05-progress-tracking-design"
this_file: "specifications/10-simple-http-client-enhancements/features/05-progress-tracking-design/feature.md"

status: pending
priority: medium
created: "2026-03-25"
completed: null

depends_on: []

tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0
---

# Progress Tracking Design

## Overview

This feature documents the `FetchPending` pattern for observable parallel fetch operations with source identification.

## WHY: Problem Statement

When executing multiple HTTP fetches in parallel, users need visibility into each fetch's progress. The progress state should include source identification for observability and debugging.

Without source identification in progress states:
- Impossible to tell which URL is connecting vs. awaiting response
- Debug output is ambiguous ("Connecting..." for which request?)
- Progress reporting lacks context for logging/monitoring

### Source Pattern Analysis

From `gen_model_descriptors/mod.rs`:

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

### Usage in Context

```rust
let task = SendRequestTask::new(request, 5, pool, config)
    .map_ready(move |intro| {
        // Transform response to domain type
        parse_response(intro)
    })
    // Transform: HttpPending → FetchPending with source tracking
    .map_pending(move |p| FetchPending::from_http(p, source));

// Progress can be observed via StreamIterator
for item in valtron::execute(task, None)? {
    match item {
        Stream::Pending(p) => tracing::info!("Progress: {p}"),
        //                                    ^^^^^^^^^^^^^
        // Output: "models.dev: Connecting..." or "openrouter: Awaiting response..."
        Stream::Next(result) => { /* handle result */ }
        Stream::Ignore => {}
    }
}
```

## WHAT: Solution Overview

Provide a progress tracking pattern with:

1. Source identification in every progress state
2. Simple conversion from `HttpRequestPending`
3. Human-readable Display implementation
4. Integration with `.map_pending()` combinator

### Core API

```rust
/// Progress state for HTTP fetch operations with source identification.
///
/// # Type Parameters
/// * `S` - Source type (default: &'static str)
///
/// # Examples
///
/// ```
/// let progress = FetchPending::Connecting { source: "api.example.com" };
/// println!("Progress: {}", progress); // "api.example.com: Connecting..."
/// ```
#[derive(Debug, Clone)]
pub enum FetchPending<S = &'static str> {
    /// Establishing connection to source
    Connecting { source: S },
    /// Connection established, awaiting response headers
    AwaitingResponse { source: S },
}

impl<S: AsRef<str>> FetchPending<S> {
    /// Create a Connecting state
    pub fn connecting(source: S) -> Self {
        Self::Connecting { source }
    }

    /// Create an AwaitingResponse state
    pub fn awaiting_response(source: S) -> Self {
        Self::AwaitingResponse { source }
    }

    /// Convert from HttpRequestPending with source context
    pub fn from_http(p: HttpRequestPending, source: S) -> Self {
        match p {
            HttpRequestPending::WaitingForStream => Self::connecting(source),
            HttpRequestPending::WaitingIntroAndHeaders => Self::awaiting_response(source),
        }
    }
}

impl<S: AsRef<str>> std::fmt::Display for FetchPending<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchPending::Connecting { source } => {
                write!(f, "{}: Connecting...", source.as_ref())
            }
            FetchPending::AwaitingResponse { source } => {
                write!(f, "{}: Awaiting response...", source.as_ref())
            }
        }
    }
}
```

### Extended Progress States

For more granular tracking:

```rust
#[derive(Debug, Clone)]
pub enum FetchPending<S = &'static str> {
    /// Resolving DNS
    ResolvingDns { source: S, host: String },
    /// Establishing TCP connection
    Connecting { source: S },
    /// Performing TLS handshake
    TlsHandshake { source: S },
    /// Connection established, awaiting response headers
    AwaitingResponse { source: S },
    /// Receiving response body
    ReceivingBody { source: S, bytes_received: usize },
}
```

### Extended Progress States with Error Tracking

For tracking which source failed when parallel fetches complete:

```rust
use std::fmt;

/// Progress state for HTTP fetch operations with source identification and error tracking.
///
/// WHY: Tracks progress and errors of individual API fetches during parallel execution.
/// Error tracking allows users to identify which sources failed after completion.
///
/// WHAT: Progress states with source identification and optional error information.
///
/// HOW: Used as the `Pending` type in TaskIterator combinators.
/// Errors are stored for post-execpection inspection.
#[derive(Debug, Clone)]
pub enum FetchPending<S = &'static str, E = String> {
    /// Establishing connection to source
    Connecting { source: S },
    /// Connection established, awaiting response headers
    AwaitingResponse { source: S },
    /// Fetch completed with error - source and error preserved for reporting
    Failed { source: S, error: E },
    /// Fetch completed successfully
    Completed { source: S },
}

impl<S: AsRef<str>, E: fmt::Display> FetchPending<S, E> {
    /// Convert from HttpRequestPending with source context
    pub fn from_http(p: HttpRequestPending, source: S) -> Self {
        match p {
            HttpRequestPending::WaitingForStream => Self::Connecting { source },
            HttpRequestPending::WaitingIntroAndHeaders => Self::AwaitingResponse { source },
        }
    }

    /// Create a Failed state from an error
    pub fn failed(source: S, error: E) -> Self {
        Self::Failed { source, error }
    }

    /// Create a Completed state
    pub fn completed(source: S) -> Self {
        Self::Completed { source }
    }
}

impl<S: AsRef<str>, E: fmt::Display> std::fmt::Display for FetchPending<S, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchPending::Connecting { source } => {
                write!(f, "{}: Connecting...", source.as_ref())
            }
            FetchPending::AwaitingResponse { source } => {
                write!(f, "{}: Awaiting response...", source.as_ref())
            }
            FetchPending::Failed { source, error } => {
                write!(f, "{}: FAILED - {}", source.as_ref(), error)
            }
            FetchPending::Completed { source } => {
                write!(f, "{}: Completed", source.as_ref())
            }
        }
    }
}
```

### Usage with Error Tracking

```rust
use foundation_core::wire::simple_http::client::tasks::FetchPending;
use foundation_core::valtron::{execute, TaskIteratorExt, Stream};
use std::sync::{Arc, Mutex};

// Collect errors for post-execution reporting
let errors = Arc::new(Mutex::new(Vec::new()));
let errors_clone = Arc::clone(&errors);

let task = create_fetch_task(&mut client, "models.dev", URL, parser)?
    .map_pending(move |p| {
        FetchPending::from_http(p, "models.dev")
    })
    .map_ready(move |intro| {
        match intro {
            RequestIntro::Success { stream, .. } => {
                let body = read_body_from_stream(stream);
                let result = parse_response(&body, "models.dev");
                if result.is_empty() {
                    // Parse failed - report as error
                    errors_clone.lock().unwrap().push((
                        "models.dev".to_string(),
                        "Parse returned empty results".to_string()
                    ));
                }
                result
            }
            RequestIntro::Failed(e) => {
                // Request failed - record error
                errors_clone.lock().unwrap().push((
                    "models.dev".to_string(),
                    format!("Request failed: {}", e)
                ));
                Vec::new()
            }
        }
    });

// Execute and observe
for item in execute(task, None)? {
    match item {
        Stream::Pending(p) => {
            match &p {
                FetchPending::Failed { source, error } => {
                    tracing::error!("Fetch failed for {}: {}", source, error);
                }
                FetchPending::Completed { source } => {
                    tracing::info!("Fetch completed for {}", source);
                }
                _ => tracing::debug!("Progress: {p}"),
            }
        }
        Stream::Next(result) => { /* handle result */ }
        Stream::Ignore => {}
    }
}

// After execution, report all errors
let collected_errors = errors.lock().unwrap();
if !collected_errors.is_empty() {
    tracing::warn!("{} sources failed:", collected_errors.len());
    for (source, error) in collected_errors.iter() {
        tracing::warn!("  - {}: {}", source, error);
    }
}
```

### FetchResult Pattern for Complete Tracking

For comprehensive tracking with result aggregation:

```rust
/// Result of a single fetch operation with source and outcome.
#[derive(Debug, Clone)]
pub struct FetchResult<S = &'static str, T = Vec<ModelEntry>, E = String> {
    pub source: S,
    pub outcome: FetchOutcome<T, E>,
}

#[derive(Debug, Clone)]
pub enum FetchOutcome<T, E> {
    Success(T),
    Error(E),
}

impl<S: AsRef<str>, T, E: std::fmt::Display> FetchResult<S, T, E> {
    /// Log the result with appropriate level
    pub fn log(&self) {
        match &self.outcome {
            FetchOutcome::Success(result) => {
                tracing::info!("{}: Success", self.source.as_ref());
            }
            FetchOutcome::Error(err) => {
                tracing::error!("{}: Error - {}", self.source.as_ref(), err);
            }
        }
    }

    /// Get success result if available
    pub fn ok(self) -> Option<T> {
        match self.outcome {
            FetchOutcome::Success(v) => Some(v),
            FetchOutcome::Error(_) => None,
        }
    }

    /// Get error if available
    pub fn err(self) -> Option<E> {
        match self.outcome {
            FetchOutcome::Error(e) => Some(e),
            FetchOutcome::Success(_) => None,
        }
    }
}

// Usage with parallel fetch
let task = create_fetch_task(&mut client, "models.dev", URL, parser)?
    .map_ready(|intro| {
        let result = match intro {
            RequestIntro::Success { stream, .. } => {
                let body = read_body_from_stream(stream);
                let parsed = parse_response(&body, "models.dev");
                FetchOutcome::Success(parsed)
            }
            RequestIntro::Failed(e) => {
                FetchOutcome::Error(format!("Request failed: {}", e))
            }
        };
        FetchResult {
            source: "models.dev",
            outcome: result,
        }
    })
    .map_pending(|p| FetchPending::from_http(p, "models.dev"));

// Collect results and report failures
let mut results = execute(task, None)?;
let mut successes = Vec::new();
let mut failures = Vec::new();

for item in results {
    if let Stream::Next(fetch_result) = item {
        match fetch_result.outcome {
            FetchOutcome::Success(data) => successes.push(data),
            FetchOutcome::Error(err) => {
                failures.push((fetch_result.source, err));
                tracing::warn!("Fetch failed for {}: {}", fetch_result.source, err);
            }
        }
    }
}

// Summary report
tracing::info!("Fetch summary: {} succeeded, {} failed", successes.len(), failures.len());
for (source, error) in &failures {
    tracing::warn!("  Failed: {} - {}", source, error);
}
```

## HOW: Implementation

### Step 1: Define FetchPending Enum

```rust
// File: backends/foundation_core/src/wire/simple_http/client/tasks/state.rs

use crate::wire::simple_http::client::HttpRequestPending;

/// Progress state for HTTP fetch operations with source identification.
///
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
    ///
    /// # Arguments
    /// * `p` - The HTTP pending state to convert
    /// * `source` - Source identifier (typically a string literal)
    ///
    /// # Examples
    ///
    /// ```
    /// let progress = FetchPending::from_http(
    ///     HttpRequestPending::WaitingForStream,
    ///     "api.example.com"
    /// );
    /// assert!(matches!(progress, FetchPending::Connecting { .. }));
    /// ```
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

### Step 2: Integration with map_pending

```rust
use foundation_core::wire::simple_http::client::tasks::{create_fetch_task, FetchPending};
use foundation_core::valtron::{execute, TaskIteratorExt};
use foundation_core::synca::mpp::Stream;

let task = create_fetch_task(&mut client, "models.dev", URL, parser)?
    .map_pending(|p| FetchPending::from_http(p, "models.dev"));

// Observe progress during execution
for item in execute(task, None)? {
    match item {
        Stream::Pending(p) => {
            // Progress output: "models.dev: Connecting..." or "models.dev: Awaiting response..."
            tracing::debug!("Fetch progress: {p}");
        }
        Stream::Next(result) => {
            tracing::info!("Fetch completed, got {} items", result.len());
        }
        Stream::Ignore => {}
    }
}
```

### Step 3: Parallel Fetch with Progress Reporting

```rust
use foundation_core::valtron::execute_collect_all;
use foundation_core::wire::simple_http::client::tasks::FetchPending;

let tasks: Vec<Box<dyn TaskIterator<
    Ready = Vec<ModelEntry>,
    Pending = FetchPending,
    Spawner = BoxedSendExecutionAction,
> + Send>> = vec![
    create_fetch_task(&mut client, "models.dev", URL1, parser1)?,
    create_fetch_task(&mut client, "openrouter", URL2, parser2)?,
    create_fetch_task(&mut client, "ai-gateway", URL3, parser3)?,
];

let result_stream = execute_collect_all(tasks, None)?;

for stream_item in result_stream {
    match stream_item {
        Stream::Pending(p) => {
            // Progress output identifies which source:
            // "models.dev: Connecting..."
            // "openrouter: Awaiting response..."
            // "ai-gateway: Connecting..."
            tracing::info!("Progress: {p}");
        }
        Stream::Next(models) => {
            tracing::info!("Completed: got {} models", models.len());
        }
        _ => {}
    }
}
```

### Step 4: Progress Callback Pattern

For UI updates or external monitoring:

```rust
use std::sync::{Arc, Mutex};

let progress_log = Arc::new(Mutex::new(Vec::new()));
let progress_clone = Arc::clone(&progress_log);

let task = create_fetch_task(&mut client, "api.example.com", URL, parser)?
    .map_pending(move |p| {
        // Log progress externally
        progress_clone.lock().unwrap().push(format!("{p}"));
        FetchPending::from_http(p, "api.example.com")
    });
```

## Implementation Location

```
backends/foundation_core/src/wire/simple_http/client/tasks/
└── state.rs (NEW/MODIFY - Add FetchPending enum)
```

## Success Criteria

- [ ] `FetchPending` enum defined with source tracking
- [ ] Display impl provides human-readable output
- [ ] Conversion from `HttpRequestPending` implemented
- [ ] Integration with `map_pending()` demonstrated
- [ ] Parallel fetch progress reporting shown
- [ ] Progress callback pattern documented
- [ ] **Error tracking states added (`Failed`, `Completed`)**
- [ ] **Error collection pattern for parallel fetches documented**
- [ ] **`FetchResult` pattern for outcome aggregation documented**
- [ ] Module exported in `client::tasks::mod.rs`

## Verification Commands

```bash
# Build the module
cargo build --package foundation_core

# Run tests
cargo test --package foundation_core -- wire::simple_http::client::tasks::state

# Check formatting
cargo fmt -- --check

# Run clippy
cargo clippy --package foundation_core -- -D warnings
```

## Notes for Agents

### Important Considerations

1. **Source Type**: Using `&'static str` is most common (string literals). For dynamic sources, consider `FetchPending<String>`.

2. **Generic Source Type**: The enum can be generic over the source type (`FetchPending<S>`) for flexibility.

3. **Display for Observability**: The `Display` impl is critical for logging and debugging. Include all relevant context.

4. **Minimal States**: Start with minimal states (Connecting, AwaitingResponse). Add more states only if needed.

### Common Pitfalls

1. Not including source identification (defeats the purpose)
2. Forgetting to implement Display (can't log progress meaningfully)
3. Too many granular states (makes the enum complex)
4. Not documenting what each state means

---

_Created: 2026-03-25_
_Source: gen_model_descriptors FetchPending analysis_
