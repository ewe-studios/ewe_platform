---
feature: "ClientRequest Refactor with split_collect_until_map"
description: "Refactor ClientRequest to use split_collect_until_map() for intro/body separation instead of manual state machine"
status: "complete"
priority: "high"
depends_on: ["05-unified-executor-integration", "07-split-collector"]
estimated_effort: "large"
created: 2026-03-20
author: "Main Agent"
tasks:
  completed: 10
  uncompleted: 0
  total: 10
  completion_percentage: 100%
---

# ClientRequest Refactor Feature

## WHY: Problem Statement

Current `ClientRequest` in `wire/simple_http/client/api.rs` uses manual state machine loops:

```rust
// Current pattern in introduction_with_connection()
pub fn introduction_with_connection(&mut self) -> Result<(HttpClientConnection, ResponseIntro, SimpleHeaders), HttpClientError> {
    loop {
        if let Some(val) = self.task_state.take() {
            match val {
                ClientRequestState::NotStarted => {
                    self.start()?;
                    continue;
                }
                ClientRequestState::Executing(mut iter) => {
                    let Some(task_status) = iter.next() else {
                        return Err(HttpClientError::FailedExecution);
                    };
                    self.task_state = Some(ClientRequestState::Executing(iter));

                    match task_status {
                        Stream::Init | Stream::Ignore => continue,
                        Stream::Pending(v) => continue,
                        Stream::Delayed(dur) => continue,
                        Stream::Next(value) => {
                            match value {
                                RequestIntro::Success { stream, conn, intro, headers } => {
                                    self.task_state = Some(ClientRequestState::IntroReady(...));
                                    return Ok((conn, intro.into(), headers));
                                }
                                RequestIntro::Failed(err) => return Err(err),
                            }
                        }
                    }
                }
                // ... more manual states
            }
        }
    }
}
```

**Problems**:
1. **Manual state machine loops** - Explicit `loop { match state { ... } }` pattern
2. **Boilerplate state tracking** - `ClientRequestState` enum with `NotStarted`, `Executing`, `IntroReady`, `Completed`
3. **Intro/body coupling** - Must store intro in state to return it, then separately read body
4. **Hard to extend** - Adding new behaviors requires modifying the state machine

## Key Design Principle: TaskIterators Are Inputs, StreamIterators Are Outputs

The refactored pattern uses `split_collector()` to fork the iterator:
- **Observer branch**: Gets `RequestIntro::Success` immediately (for `introduction()` method)
- **Continuation branch**: Continues to body reader (for `body()` method)

```rust
// TaskIterator (input) → split_collector() → StreamIterator (output via execute())
let (observer, continuation) = task.split_collect_one(...);
let stream = execute(continuation)?;  // End user works with StreamIterator
```

End users never deal with TaskIterator directly - they receive `StreamIterator` from `execute()`.

## WHAT: Solution Overview

Refactor `ClientRequest` to use `split_collector()` to fork the iterator:
- **Observer branch**: Gets `RequestIntro::Success` immediately (for `introduction()` method)
- **Continuation branch**: Continues to body reader, then execute() returns StreamIterator

### Before: Manual State Machine

```rust
// Current: Manual loop with explicit state tracking
loop {
    match self.task_state.take() {
        ClientRequestState::NotStarted => { self.start()?; continue; }
        ClientRequestState::Executing(mut iter) => {
            match iter.next() {
                Some(Stream::Next(RequestIntro::Success { ... })) => return Ok(...),
                _ => continue,
            }
        }
        // ...
    }
}
```

### After: split_collector Pattern

```rust
// Refactored: Use split_collector to fork intro + body
pub fn start(&mut self) -> Result<(), HttpClientError> {
    let task = SendRequestTask::new(request, pool, config);

    // Split: observer gets intro, continuation handles body
    let (intro_observer, body_continuation) = task
        .split_collect_one(|item| matches!(item, RequestIntro::Success { .. }));

    // Store both branches for later use
    self.intro_observer = Some(intro_observer);
    self.body_continuation = Some(body_continuation);

    Ok(())
}

pub fn introduction_with_connection(&mut self) -> Result<(HttpClientConnection, ResponseIntro, SimpleHeaders), HttpClientError> {
    // Start execution if not started
    if self.intro_observer.is_none() {
        self.start()?;
    }

    // Pull from observer until we get intro
    let Some(intro_observer) = &mut self.intro_observer else {
        return Err(HttpClientError::InvalidRequestState);
    };

    for status in intro_observer {
        match status {
            Stream::Next(RequestIntro::Success { conn, intro, headers, .. }) => {
                return Ok((conn.clone(), intro.into(), headers.clone()));
            }
            Stream::Pending(_) => continue,
            Stream::Delayed(_) => continue,
            Stream::Done => break,
        }
    }

    Err(HttpClientError::FailedExecution)
}

pub fn body(&mut self) -> Result<SendSafeBody, HttpClientError> {
    // Use continuation branch for body reading
    let Some(body_continuation) = self.body_continuation.take() else {
        return Err(HttpClientError::InvalidRequestState);
    };

    // Execute continuation, get StreamIterator for body reading
    let stream = execute(body_continuation)?;
    for status in stream {
        match status {
            Stream::Next(ResponseComplete { body, .. }) => return Ok(body),
            Stream::Pending(_) => continue,
            _ => continue,
        }
    }

    Err(HttpClientError::FailedToReadBody)
}
```

### Key Changes

1. **Remove `ClientRequestState` enum** - No longer needed; combinators handle state
2. **Remove manual `loop { match }`** - Replaced with iterator chaining
3. **`start()` returns composed iterator** - Instead of storing state
4. **Methods consume iterator** - `introduction()` pulls from iterator until first useful result
5. **execute() returns StreamIterator** - body() works with Stream results

## HOW: Refactoring Approach

### Phase 1: Add split_collector to SendRequestTask

```rust
// In start() method:
let task = SendRequestTask::new(request, pool, config);

// Split: observer gets RequestIntro::Success, continuation handles rest
let (intro_observer, body_continuation) = task
    .split_collect_one(|item| matches!(item, RequestIntro::Success { .. }));

// Store both branches
self.intro_observer = Some(intro_observer);
self.body_continuation = Some(body_continuation);
```

### Phase 2: Refactor introduction_with_connection

```rust
pub fn introduction_with_connection(&mut self) -> Result<(HttpClientConnection, ResponseIntro, SimpleHeaders), HttpClientError> {
    // Start execution if not started
    if self.intro_observer.is_none() {
        self.start()?;
    }

    // Pull from observer until we get intro
    let Some(intro_observer) = &mut self.intro_observer else {
        return Err(HttpClientError::InvalidRequestState);
    };

    for status in intro_observer.by_ref() {
        match status {
            Stream::Next(RequestIntro::Success { conn, intro, headers, stream }) => {
                // Store stream for body reading
                self.response_stream = Some(stream);
                return Ok((conn.clone(), intro.into(), headers.clone()));
            }
            Stream::Pending(_) => continue,
            Stream::Delayed(_) => continue,
            Stream::Done => break,
        }
    }

    Err(HttpClientError::FailedExecution)
}
```

### Phase 3: Simplify body() Method

```rust
pub fn body(&mut self) -> Result<SendSafeBody, HttpClientError> {
    // Use continuation branch for body reading
    let Some(body_continuation) = self.body_continuation.take() else {
        return Err(HttpClientError::InvalidRequestState);
    };

    // Execute continuation, get StreamIterator for body reading
    let stream = execute(body_continuation)?;

    // Body reading is simple iteration over StreamIterator
    for status in stream {
        match status {
            Stream::Next(ResponseComplete { body, .. }) => return Ok(body),
            Stream::Pending(_) => continue,
            _ => continue,
        }
    }

    Err(HttpClientError::FailedToReadBody)
}
```

## Requirements

1. **split_collector() implementation** - Forks iterator into observer + continuation
2. **Clone bounds** - Ready and Pending must be Clone for split_collector
3. **ConcurrentQueue** - Size-configurable queue between branches
4. **Refactor start()** - Use split_collector to fork intro/body
5. **Refactor introduction_with_connection()** - Pull from observer branch
6. **Refactor body()** - Execute continuation, iterate StreamIterator
7. **Keep API compatible** - Public methods unchanged
8. **Remove ClientRequestState** - Replace with observer/continuation storage
9. **execute() returns StreamIterator** - End users work with Stream results

## Tasks

1. [ ] Read existing `wire/simple_http/client/api.rs` completely
2. [ ] Implement `split_collector()` in feature 07 first
3. [ ] Add `Clone` bounds to `RequestIntro` variants
4. [ ] Refactor `start()` to use `split_collector()` for intro/body fork
5. [ ] Remove `ClientRequestState` enum
6. [ ] Add `intro_observer` and `body_continuation` fields to `ClientRequest`
7. [ ] Refactor `introduction_with_connection()` to pull from observer
8. [ ] Refactor `body()` to execute continuation and iterate StreamIterator
9. [ ] Write tests verifying API compatibility
10. [ ] Run clippy and fmt checks

## Verification

```bash
cargo test -p foundation_core -- wire::simple_http::client
cargo clippy -p foundation_core -- -D warnings
cargo fmt -p foundation_core -- --check
```

## Success Criteria

- All 10 tasks completed
- `ClientRequestState` enum removed
- No manual `loop { match state }` patterns
- Public API unchanged (backward compatible)
- Tests pass demonstrating same behavior
- Code is simpler and more composable
- `execute()` returns StreamIterator for body reading
- Zero clippy warnings

## Benefits

| Before | After |
|--------|-------|
| Manual state enum (`ClientRequestState`) | State handled by split_collector branches |
| Explicit `loop { match }` | Observer/continuation pattern |
| Intro/body coupling via state storage | Clean separation via fork |
| Hard to extend | Easy to add more split points |
| No progress visibility | Can add progress observers |
| Body reading from stored stream | Body reading via execute() → StreamIterator |

---

_Created: 2026-03-20_
_Updated: 2026-03-20 (v3.0: execute() returns StreamIterator, TaskIterator is input)_
