---
feature: "ClientRequest Refactor with split_collect_one_map"
description: "Refactor ClientRequest to use split_collect_one_map() for intro/body separation - direct iterator access via send() and start() methods"
status: "complete"
priority: "high"
depends_on: ["05-unified-executor-integration", "07-split-collector"]
estimated_effort: "medium"
created: 2026-03-20
updated: 2026-03-23
author: "Main Agent"
tasks:
  completed: 8
  uncompleted: 0
  total: 8
  completion_percentage: 100%
---

# ClientRequest Refactor Feature

## WHY: Problem Statement

The original `ClientRequest` in `wire/simple_http/client/api.rs` used manual state machine loops with unnecessary complexity:

```rust
// Old pattern - manual state machine
pub enum ClientRequestState {
    NotStarted,
    Executing { iter: ... },
    IntroReady { ... },
    BodyReady { ... },
    Completed,
}

pub fn introduction_with_connection(&mut self) -> Result<...> {
    loop {
        if let Some(val) = self.task_state.take() {
            match val {
                ClientRequestState::NotStarted => { self.start()?; continue; }
                ClientRequestState::Executing(mut iter) => {
                    // Manual iteration logic
                }
                // ... more states
            }
        }
    }
}
```

**Problems**:
1. **Manual state enum** - `ClientRequestState` with multiple variants
2. **Boilerplate state tracking** - Storing iterator, intro, headers in state
3. **Complex loop logic** - `loop { match state { ... } }` patterns
4. **Unnecessary methods** - `collect()`, `into_iter()` that added no value

## WHAT: Solution Overview

Refactor `ClientRequest` to use `split_collect_one_map()` for clean intro/body separation:

### New API

```rust
// send() - One-shot execution, returns full response
pub fn send(self) -> Result<FinalizedResponse<SendSafeBody, R>, HttpClientError>

// start() - Direct access to intro observer and body stream
pub fn start(&mut self) -> Result<(RequestIntroStream, MappedDrivenBodyStream<R>), HttpClientError>
```

### Implementation Pattern

```rust
pub fn start(&mut self) -> Result<(RequestIntroStream, MappedDrivenBodyStream<R>), HttpClientError> {
    // Transition state
    self.task_state = ClientRequestState::Executing;

    // Split: observer gets intro, continuation handles body
    let (observer, task) = SendRequestTask::new(request, config, pool)
        .split_collect_one_map(|ready| match ready {
            RequestIntro::Success { intro, headers, .. } => {
                (true, Some((intro, headers)))
            }
            RequestIntro::Failed(err) => (false, None),
        });

    // Execute continuation, map body result
    let body_stream = valtron::execute(task, None)?
        .map_done(|done| match done {
            RequestIntro::Success { stream, conn, .. } => {
                // Use map_iter to flatten nested stream
                stream.map_done(|parts| match parts {
                    IncomingResponseParts::SizedBody(inner) => Ok((conn, inner)),
                    // ... handle other cases
                })
            }
            RequestIntro::Failed(err) => std::iter::once(Err(err)),
        });

    Ok((observer, body_stream))
}

pub fn send(mut self) -> Result<FinalizedResponse<SendSafeBody, R>, HttpClientError> {
    let (intro_stream, body_stream) = self.start()?;

    // Collect intro
    let intro_data = intro_stream
        .find_map(|s| match s {
            Stream::Next(value) => Some(value),
            _ => None,
        })
        .ok_or(HttpClientError::InvalidRequestState)?;

    // Collect body
    let (conn, body) = body_stream
        .find_map(|s| match s {
            Stream::Next(Ok(res)) => Some(res),
            Stream::Next(Err(err)) => return Err(err),
            _ => None,
        })
        .ok_or(HttpClientError::InvalidRequestState)?;

    Ok(FinalizedResponse::new(response, conn, pool))
}
```

### Key Changes

1. **Simplified `ClientRequestState`** - Just `NotStarted`, `Executing`, `Failed` (no data storage)
2. **Removed `collect()` and `into_iter()`** - Not needed, direct iteration works
3. **`send()` returns `FinalizedResponse`** - Wraps `SimpleResponse` with connection pooling
4. **`start()` returns tuple directly** - `(RequestIntroStream, MappedDrivenBodyStream<R>)`
5. **Use `map_iter()` for body** - Flattens nested stream from `RequestIntro::Success`

## HOW: Implementation Details

### Type Aliases

```rust
pub type DrivenBodyStream<R> = DrivenStreamIterator<
    SplitCollectorMapContinuation<SendRequestTask<R>, (ResponseIntro, SimpleHeaders)>,
>;

pub type MappedDrivenBodyStream<R> = MapDone<
    DrivenBodyStream<R>,
    RequestIntro,
    HttpRequestPending,
    Result<(HttpClientConnection, SendSafeBody), HttpClientError>,
>;

pub type RequestIntroStream = SplitCollectorMapObserver<(ResponseIntro, SimpleHeaders), HttpRequestPending>;
```

### Simplified State Enum

```rust
#[derive(PartialEq, Eq, PartialOrd, Clone)]
pub enum ClientRequestState {
    NotStarted,
    Executing,
    Failed,
}
```

### start() Method

```rust
pub fn start(&mut self) -> Result<(RequestIntroStream, MappedDrivenBodyStream<R>), HttpClientError> {
    // Validate state
    if self.pool.is_none() {
        return Err(HttpClientError::NoPool);
    }
    if self.task_state != ClientRequestState::NotStarted {
        return Err(HttpClientError::InvalidReadState);
    }

    // Take prepared request
    let Some(mut request) = self.prepared_request.take() else {
        return Err(HttpClientError::NoRequestToSend);
    };

    // Apply middleware
    self.middleware_chain.process_request(&mut request)?;

    // Store for response middleware
    self.original_request = Some(PreparedRequest {
        method: request.method.clone(),
        url: request.url.clone(),
        headers: request.headers.clone(),
        body: SendSafeBody::None,
        extensions: std::mem::take(&mut request.extensions),
    });

    // Transition state
    self.task_state = ClientRequestState::Executing;

    // Split and execute
    let (observer, task) = SendRequestTask::new(request, config, pool)
        .split_collect_one_map(|ready| match ready {
            RequestIntro::Success { intro, headers, .. } => {
                (true, Some((intro, headers)))
            }
            RequestIntro::Failed(err) => (false, None),
        });

    let body_stream = valtron::execute(task, None)?
        .map_done(|done| { /* map to body result */ });

    Ok((observer, body_stream))
}
```

### send() Method

```rust
pub fn send(mut self) -> Result<FinalizedResponse<SendSafeBody, R>, HttpClientError> {
    let (intro_stream, body_stream) = self.start()?;

    // Collect intro
    let mut intro_data: Option<(ResponseIntro, SimpleHeaders)> = None;
    for intro_element in intro_stream {
        if let Stream::Next(value) = intro_element {
            intro_data = Some(value);
            break;
        }
    }

    // Collect body
    let mut response_body: Option<(HttpClientConnection, SendSafeBody)> = None;
    for body_element in body_stream {
        if let Stream::Next(Ok(res)) = body_element {
            response_body = Some(res);
            break;
        }
    }

    // Build response
    let (intro, headers) = intro_data.ok_or(HttpClientError::InvalidRequestState)?;
    let (conn, body) = response_body.ok_or(HttpClientError::InvalidRequestState)?;
    let response = SimpleResponse::new(intro.status, headers, body);

    // Apply response middleware
    if let Some(request) = &self.original_request {
        self.middleware_chain.process_response(request, &mut response)?;
    }

    Ok(FinalizedResponse::new(response, conn, self.pool.take().ok_or(HttpClientError::NoPool)?))
}
```

## Requirements

1. **split_collect_one_map()** - Forks iterator, observer gets matched items
2. **Clone bounds** - `RequestIntro` and `HttpRequestPending` must be Clone
3. **Type aliases** - `RequestIntroStream`, `MappedDrivenBodyStream<R>`, `DrivenBodyStream<R>`
4. **Simplified ClientRequestState** - No data storage, just lifecycle states
5. **Removed collect()/into_iter()** - Direct iteration only
6. **send() returns FinalizedResponse** - Wraps SimpleResponse with connection pooling
7. **start() returns tuple** - Direct access to observer and body stream
8. **map_iter() for body** - Flattens nested stream from RequestIntro::Success

## Tasks

1. [x] Read existing `wire/simple_http/client/api.rs` completely
2. [x] Implement `split_collect_one_map()` in feature 07
3. [x] Add `Clone` bounds to `RequestIntro` variants
4. [x] Simplify `ClientRequestState` enum - remove data storage
5. [x] Refactor `start()` to return `(RequestIntroStream, MappedDrivenBodyStream<R>)`
6. [x] Refactor `send()` to use direct iteration over streams
7. [x] Remove `collect()` and `into_iter()` methods
8. [x] Run clippy and fmt checks

## Verification

```bash
cargo test -p foundation_core -- wire::simple_http::client
cargo clippy -p foundation_core -- -D warnings
cargo fmt -p foundation_core -- --check
```

## Success Criteria

- All 8 tasks completed
- `ClientRequestState` simplified to just lifecycle states
- No manual `loop { match state }` patterns
- `send()` and `start()` methods working correctly
- Tests pass demonstrating same behavior
- Code is simpler and more composable
- Zero clippy warnings

## Benefits

| Before | After |
|--------|-------|
| Complex state enum with data storage | Simple lifecycle states only |
| Manual `loop { match }` patterns | Direct iteration with `for` loops |
| `collect()`, `into_iter()` methods | Just `send()` and `start()` |
| Intro/body coupling via state | Clean separation via split_collector |
| Body reading from stored stream | Body reading via execute() → StreamIterator |

---

_Created: 2026-03-20_
_Updated: 2026-03-23 (Simplified API: send() and start() only, removed collect/into_iter)_
