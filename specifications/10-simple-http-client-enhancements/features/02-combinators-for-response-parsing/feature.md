---
workspace_name: "ewe_platform"
spec_directory: "specifications/10-simple-http-client-enhancements"
feature_directory: "specifications/10-simple-http-client-enhancements/features/02-combinators-for-response-parsing"
this_file: "specifications/10-simple-http-client-enhancements/features/02-combinators-for-response-parsing/feature.md"

status: pending
priority: high
created: "2026-03-25"
completed: null

depends_on:
  - "08-valtron-async-iterators/06b-map-iter-combinator"

tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0
---

# TaskIterator Combinators for Response Parsing

## Overview

This feature documents and formalizes the use of `.map_ready()` and `.map_pending()` combinators for transforming HTTP responses into domain-specific types.

## WHY: Problem Statement

Users need to transform raw HTTP responses (`RequestIntro`) into domain-specific types. The current API returns `RequestIntro` which requires manual processing in every call site. The `gen_model_descriptors` implementation shows a cleaner pattern using combinators that:

1. Transform the `Ready` type from `RequestIntro` to a domain type `T`
2. Transform the `Pending` type from `HttpRequestPending` to a domain-specific progress type
3. Enable composition with other TaskIterator operations

### Source Pattern Analysis

From `gen_model_descriptors/mod.rs`:

```rust
SendRequestTask::new(request, 5, pool, config)
    // Transform Ready: RequestIntro → Vec<ModelEntry>
    .map_ready(move |intro| {
        match intro {
            RequestIntro::Success { stream, .. } => {
                let body_text = read_body_from_stream(stream);
                parser(&body_text, source)
            }
            RequestIntro::Failed(e) => {
                tracing::warn!("Request failed: {e}");
                Vec::new() // Graceful degradation
            }
        }
    })
    // Transform Pending: HttpRequestPending → FetchPending
    .map_pending(move |p| FetchPending::from_http(p, source))
```

## WHAT: Solution Overview

Document and export combinator patterns for response transformation:

### Combinator Signatures

```rust
/// Transform the Ready type of a TaskIterator.
///
/// # Type Parameters
/// * `F` - Mapper function: `Self::Ready → R`
/// * `R` - New Ready type
///
/// # Returns
/// A new TaskIterator with Ready type changed to R
pub fn map_ready<F, R>(self, f: F) -> MapReady<Self, F>
where
    F: Fn(Self::Ready) -> R + Send,
{
    MapReady {
        inner: self,
        mapper: f,
    }
}

/// Transform the Pending type of a TaskIterator.
///
/// # Type Parameters
/// * `F` - Mapper function: `Self::Pending → P`
/// * `P` - New Pending type
///
/// # Returns
/// A new TaskIterator with Pending type changed to P
pub fn map_pending<F, P>(self, f: F) -> MapPending<Self, F>
where
    F: Fn(Self::Pending) -> P + Send,
{
    MapPending {
        inner: self,
        mapper: f,
    }
}
```

### Complete Response Transformation Pattern

```rust
use foundation_core::valtron::TaskIteratorExt;
use foundation_core::wire::simple_http::client::{SendRequestTask, RequestIntro};

// Create base task
let task = SendRequestTask::new(request, 5, pool, config);

// Apply combinators for domain-specific transformation
let transformed = task
    // Transform Ready: RequestIntro → DomainType
    .map_ready(|intro| {
        match intro {
            RequestIntro::Success { stream, conn, intro, headers } => {
                // Read body from stream
                let body = read_body_from_stream(stream);

                // Parse into domain type
                parse_response(&body, &headers, intro)
            }
            RequestIntro::Failed(e) => {
                tracing::warn!("Request failed: {e}");
                DomainType::default() // Graceful degradation
            }
        }
    })
    // Transform Pending: HttpRequestPending → DomainPending
    .map_pending(|p| DomainPending::from_http(p, "my-source"));
```

## HOW: Implementation

### Pattern 1: Basic Response to Domain Type

```rust
use foundation_core::valtron::{TaskIteratorExt, execute};
use foundation_core::wire::simple_http::client::{SendRequestTask, RequestIntro};

// Define domain type
#[derive(Debug, Default)]
struct ApiResponse {
    status: u16,
    body: String,
}

// Create and transform task
let task = SendRequestTask::new(request, 5, pool, config)
    .map_ready(|intro| {
        match intro {
            RequestIntro::Success { stream, intro, .. } => {
                let body = read_body_from_stream(stream);
                ApiResponse {
                    status: intro.status.as_u16(),
                    body,
                }
            }
            RequestIntro::Failed(_) => ApiResponse::default(),
        }
    });

// Execute
let mut stream = execute(task, None)?;
for item in stream {
    if let Stream::Next(response) = item {
        println!("Status: {}, Body: {}", response.status, response.body);
    }
}
```

### Pattern 2: JSON Response Parsing

```rust
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct GitHubRepo {
    name: String,
    stargazers_count: u32,
    description: Option<String>,
}

fn parse_github_repo(response: &str) -> Option<GitHubRepo> {
    serde_json::from_str(response).ok()
}

let task = SendRequestTask::new(request, 5, pool, config)
    .map_ready(|intro| {
        match intro {
            RequestIntro::Success { stream, .. } => {
                let body = read_body_from_stream(stream);
                parse_github_repo(&body)
            }
            RequestIntro::Failed(e) => {
                tracing::warn!("Request failed: {e}");
                None
            }
        }
    });
```

### Pattern 3: Streaming Response with Progress

```rust
use foundation_core::wire::simple_http::client::HttpRequestPending;

#[derive(Debug, Clone)]
enum DownloadProgress {
    Connecting,
    AwaitingResponse,
    Downloading { bytes: usize },
    Complete { total: usize },
}

impl DownloadProgress {
    fn from_http(p: HttpRequestPending) -> Self {
        match p {
            HttpRequestPending::WaitingForStream => Self::Connecting,
            HttpRequestPending::WaitingIntroAndHeaders => Self::AwaitingResponse,
        }
    }
}

let task = SendRequestTask::new(request, 5, pool, config)
    .map_ready(|intro| {
        match intro {
            RequestIntro::Success { stream, .. } => {
                let mut total = 0;
                for part in stream {
                    if let Ok(IncomingResponseParts::StreamedBody(SendSafeBody::Bytes(data))) = part {
                        total += data.len();
                    }
                }
                DownloadProgress::Complete { total }
            }
            RequestIntro::Failed(_) => DownloadProgress::Complete { total: 0 },
        }
    })
    .map_pending(DownloadProgress::from_http);
```

### Pattern 4: Chaining Multiple Combinators

```rust
let task = SendRequestTask::new(request, 5, pool, config)
    // First: Transform response to raw body
    .map_ready(|intro| {
        match intro {
            RequestIntro::Success { stream, .. } => read_body_from_stream(stream),
            RequestIntro::Failed(_) => String::new(),
        }
    })
    // Second: Parse JSON
    .map_ready(|body| {
        serde_json::from_str::<MyType>(&body).unwrap_or_default()
    })
    // Third: Transform to domain type
    .map_ready(|json| MyDomainType::from(json))
    // Finally: Add progress tracking
    .map_pending(|p| MyProgress::from_http(p, "api-source"));
```

### Pattern 5: Error Propagation with Result

```rust
#[derive(Debug)]
enum FetchError {
    RequestFailed(String),
    ParseError(String),
}

let task = SendRequestTask::new(request, 5, pool, config)
    .map_ready(|intro| {
        match intro {
            RequestIntro::Success { stream, .. } => {
                let body = read_body_from_stream(stream);
                serde_json::from_str::<MyType>(&body)
                    .map_err(|e| FetchError::ParseError(e.to_string()))
            }
            RequestIntro::Failed(e) => {
                Err(FetchError::RequestFailed(e.to_string()))
            }
        }
    });
```

## Implementation Location

The combinators exist in:
```
backends/foundation_core/src/valtron/executors/task_iterators.rs
```

Documentation should be added to:
```
documentation/valtron/doc.md (MODIFY - Add response parsing examples)
```

## Success Criteria

- [ ] `map_ready()` usage documented with HTTP response examples
- [ ] `map_pending()` usage documented with progress tracking examples
- [ ] Body reading patterns covered in examples
- [ ] Error handling patterns shown (graceful vs. propagation)
- [ ] JSON parsing example provided
- [ ] Chaining multiple combinators demonstrated
- [ ] Integration with progress tracking shown
- [ ] All examples compile and run

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

1. **Combinator Order**: The order of combinator application matters. Apply `.map_ready()` before `.map_pending()` for clarity, though either order works.

2. **Closure Capture**: Closures in combinators capture by reference by default. Use `move` to transfer ownership when needed.

3. **Type Inference**: Rust's type inference can struggle with chained combinators. Explicit type annotations may be needed.

4. **Performance**: Each combinator adds a thin wrapper. The compiler typically inlines these away, but be aware when chaining many combinators.

### Common Pitfalls

1. Not handling both `Success` and `Failed` variants of `RequestIntro`
2. Forgetting to read the body from the stream before parsing
3. Blocking the executor in combinator closures (keep them async-friendly)
4. Not considering what happens when the response has no body

---

_Created: 2026-03-25_
_Source: gen_model_descriptors combinator usage analysis_
