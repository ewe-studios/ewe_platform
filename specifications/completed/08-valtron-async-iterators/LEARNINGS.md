# Learnings

This file captures learnings, discoveries, and design decisions made during implementation of Valtron Async Iterators.

## Feature 07: Split Collector Implementation Learnings

### Queue Closing: Use `ConcurrentQueue::close()` Instead of `AtomicBool`

**Initial approach**: Used `Arc<AtomicBool>` to signal when the continuation iterator finished, so the observer could stop waiting.

**Problem**: Extra state management, and the observer had to poll the flag.

**Solution**: Share the `Arc<ConcurrentQueue>` directly and call `queue.close()` when the continuation iterator naturally completes:

```rust
fn next(&mut self) -> Option<Self::Item> {
    let item = match self.inner.next() {
        Some(item) => item,
        None => {
            // Source iterator is naturally exhausted, close the queue
            self.queue.close();
            tracing::debug!("Continuation: source exhausted, queue closed");
            return None;
        }
    };
    // ... push matched items to queue ...
    Some(item)
}
```

The observer then checks `queue.is_closed()` instead of an `AtomicBool`:

```rust
fn next(&mut self) -> Option<Self::Item> {
    match self.queue.pop() {
        Ok(item) => Some(item),
        Err(PopError::Empty) => {
            if self.queue.is_closed() {
                None  // Queue closed, no more items coming
            } else {
                Some(Stream::Ignore)  // Queue open, keep waiting
            }
        }
        Err(PopError::Closed) => None,
    }
}
```

**Why this is better**:
- No extra `AtomicBool` state to manage
- The queue itself is the source of truth for completion
- `is_closed()` is a clean, atomic check
- Drop becomes a backup for abnormal termination, not the primary signal

---

### Close Queue on Natural Completion, Not Just on Drop

**Initial approach**: Relied on `Drop` impl to close the queue when the continuation iterator was dropped.

**Problem**: Tests hung indefinitely because the observer was waiting for items that never arrived. The continuation needed to be explicitly dropped (via scoped block) for the observer to complete.

**User feedback**: "Why are we depending on dropping to close the queue, just close the queue when the main task that feeds the queue is done."

**Solution**: Close the queue when `inner.next()` returns `None` (natural iterator completion). `Drop` is only a backup for abnormal termination:

```rust
// Primary: close when iterator naturally completes
fn next(&mut self) -> Option<Self::Item> {
    match self.inner.next() {
        Some(item) => { /* ... */ }
        None => {
            self.queue.close();  // <-- Close here, not in Drop
            return None;
        }
    }
}

// Backup: close on abnormal termination (panic, early return, etc.)
impl<I, D, P> Drop for SplitCollectorContinuation<I, D, P>
where
    I: TaskIterator<D, P>,
{
    fn drop(&mut self) {
        if !self.queue.is_closed() {
            tracing::debug!("SplitCollectorContinuation: dropped before completion, closing queue");
            self.queue.close();
        }
    }
}
```

**Key insight**: `Drop` should handle abnormal termination only. Normal completion should close the queue explicitly when the iterator returns `None`.

---

### Tracing Best Practices for Generic Types

**Problem**: Using `{:?}` formatting in tracing logs requires `Debug` trait bounds on generic types:

```rust
// DON'T DO THIS - requires D: Debug
tracing::debug!("Got Ready({:?})", value);
```

**User feedback**: "The Ready(value) logging will be problematic if value is not Display or Debug, so might be just good to always just say, i got something in ready state."

**Solution**: Use generic log messages that don't require trait bounds:

```rust
// DO THIS - no Debug/Display required
tracing::trace!("SplitCollectorContinuation: received Ready item");
tracing::trace!("SplitCollectorContinuation: received Pending item");
tracing::debug!("SCollectorStreamIterator: received item from queue");
```

**Pattern**:
- Use `tracing::trace!` for fine-grained flow details
- Use `tracing::debug!` for state changes (queue closed, items received)
- Use `tracing::info!` for high-level events
- Use `tracing::error!` for errors
- Avoid `{:?}` or `{}` formatting on generic type parameters

---

### Error Handling: Don't Ignore Errors with `let _ =`

**Problem**: Silently ignoring errors makes debugging harder:

```rust
// DON'T DO THIS
let _ = self.queue.force_push(item);
```

**Solution**: Handle and log errors properly:

```rust
// DO THIS
if let Err(e) = self.queue.force_push(item) {
    tracing::error!("SplitCollectorContinuation: failed to push to queue: {}", e);
} else {
    tracing::trace!("SplitCollectorContinuation: copied item to observer queue");
}
```

**Why this matters**:
- Errors are visible in logs for debugging
- `#[traced_test]` marker shows exactly what failed
- Helps identify edge cases (queue full, queue closed, etc.)

---

### Testing Async Iterators with `#[traced_test]`

The `#[traced_test]` test marker is invaluable for debugging async iterator behavior:

```rust
#[traced_test]
#[test]
fn test_split_collector_observer_receives_matched_items() {
    // Test implementation...
}
```

**What you get**:
- Full trace logs in test output
- See exactly when items are pushed/popped from the queue
- See when the queue is closed
- Understand why tests hang (observer waiting, queue never closed, etc.)

**Pattern for testing split_collector**:
1. Create test iterator with known states
2. Call `split_collector()` to get observer + continuation
3. Iterate continuation in a loop, collecting observer items
4. Verify observer received the expected matched items
5. Use `#[traced_test]` logs to debug timing/completion issues

---

---

## Design Decisions

### Why TaskStatusIterator instead of modifying Iterator?

The standard `Iterator` trait cannot be modified and is fundamentally synchronous. By creating a new `TaskStatusIterator` trait, we:
- Keep `std::iter::Iterator` unchanged for synchronous cases
- Make async state (`Pending`, `Delayed`, `Ready`) explicit in the type system
- Enable Valtron executor integration without blocking

### Why collect_all() returns TaskStatus<Vec<T>> not Vec<T>?

Returning `TaskStatus<Vec<T>>` instead of `Vec<T>` allows the collection itself to be async-aware:
- Caller sees `Pending` while any sources are still pending
- Caller sees `Ready(vec)` only when all sources complete
- Enables further composition (map, filter) on the collected result

---

_Created: 2026-03-20_
_Updated: 2026-03-20 (v3.0: TaskIterator/StreamIterator separation)_

---

## Feature 06c: Parallel Fetch in gen_model_descriptors Learnings

### PoolGuard Lifecycle is Critical

**Problem**: Thread pool was shutting down immediately after initialization, causing tasks to never execute.

**Root Cause**: The `PoolGuard` returned by `valtron::initialize_pool()` was being discarded:

```rust
// WRONG: Guard dropped immediately, threads shut down
valtron::initialize_pool(100, None);
// PoolGuard goes out of scope here, Drop impl kills all threads!

// Tasks try to spawn into dead pool
let task = valtron::execute(my_task, None)?; // Panics or hangs
```

**Solution**: Keep the `PoolGuard` alive for the entire duration of task execution:

```rust
// CORRECT: Keep guard alive via binding
let _guard = valtron::initialize_pool(100, None);
// ... execute tasks ...
// Guard dropped here, after tasks complete
```

**Why this matters**: `PoolGuard::Drop` signals all worker threads to shut down and waits for them to exit. This is intentional - the guard is the lifecycle manager for the thread pool. If you don't hold it, the pool dies.

**Pattern for binaries**:
```rust
pub fn run() -> Result<(), BoxedError> {
    let _guard = valtron::initialize_pool(100, None);
    // All valtron::execute() calls happen here
    Ok(())
}
// Guard dropped when function returns
```

**Pattern for libraries**:
```rust
pub struct MyExecutor {
    _guard: PoolGuard,
}

impl MyExecutor {
    pub fn new() -> Self {
        Self {
            _guard: valtron::initialize_pool(100, None),
        }
    }

    pub fn execute<T>(&self, task: T) -> Result<...> {
        valtron::execute(task, None)
    }
}
```

---

### Heterogeneous Closures Can't Use execute_collect_all()

**Initial plan**: Use `execute_collect_all(vec![task1, task2, task3])` to run all fetches in parallel.

**Problem**: `execute_collect_all()` requires `Vec<TaskIterator<...>>`, but each fetch task has a different closure type for its parser:

```rust
// Each closure is a unique type, even with same signature
let task1 = create_fetch_task(client, "models.dev", parse_dev)?;  // Closure type A
let task2 = create_fetch_task(client, "openrouter", parse_or)?;   // Closure type B
let task3 = create_fetch_task(client, "ai-gateway", parse_ai)?;   // Closure type C

// Can't store in Vec - different types!
let tasks: Vec<_> = vec![task1, task2, task3]; // Compilation error
```

**Solution**: Execute each task individually via `valtron::execute()` - they still run in parallel on the thread pool:

```rust
// Execute separately - each returns StreamIterator
let mut stream1 = valtron::execute(task1, None)?;
let mut stream2 = valtron::execute(task2, None)?;
let mut stream3 = valtron::execute(task3, None)?;

// Tasks run concurrently on the thread pool
// Collect results as they complete
```

**Key insight**: The executor schedules tasks concurrently regardless of whether you call `execute_collect_all()` or individual `execute()` calls. The thread pool is shared, so all tasks run in parallel.

**When to use execute_collect_all()**: When you have homogeneous task types (same closure types, often via `Box<dyn TaskIterator>` or when tasks share the exact same type).

**When to use individual execute() calls**: When tasks have heterogeneous types (different closures, different Mapper/Resolver types).

---

### Streaming Body Handling for SendSafeBody

**Problem**: HTTP responses can have various body types depending on the server's transfer encoding, and the `.map_ready()` closure must handle all variants:

- `Text` - Simple string body
- `Bytes` - Raw bytes
- `Stream` - Streaming body via iterator
- `ChunkedStream` - Chunked transfer encoding
- `LineFeedStream` - Line-by-line streaming
- `None` - No body

**Solution**: Pattern match on all variants in the `.map_ready()` closure:

```rust
.map_ready(move |intro| {
    match intro {
        RequestIntro::Success { stream, .. } => {
            let body_text = match stream {
                IncomingResponseParts::SizedBody(body)
                | IncomingResponseParts::StreamedBody(body) => {
                    match body {
                        SendSafeBody::Text(t) => t,
                        SendSafeBody::Bytes(b) => {
                            String::from_utf8(b).unwrap_or_default()
                        }
                        SendSafeBody::Stream(mut opt_iter) => {
                            // Consume stream iterator
                            if let Some(iter) = opt_iter.take() {
                                let mut bytes = Vec::new();
                                for chunk_result in iter {
                                    match chunk_result {
                                        Ok(data) => bytes.extend(data),
                                        Err(e) => {
                                            tracing::warn!("Stream error: {e}");
                                            break;
                                        }
                                    }
                                }
                                String::from_utf8(bytes).unwrap_or_default()
                            } else {
                                String::new()
                            }
                        }
                        SendSafeBody::ChunkedStream(mut opt_iter) => {
                            // Handle ChunkedData variants
                            if let Some(iter) = opt_iter.take() {
                                let mut bytes = Vec::new();
                                for chunk_result in iter {
                                    match chunk_result {
                                        Ok(ChunkedData::Data(data, _)) => bytes.extend(data),
                                        Ok(ChunkedData::Trailers(_)) => {}
                                        Ok(ChunkedData::DataEnded) => break,
                                        Err(e) => {
                                            tracing::warn!("Chunked error: {e}");
                                            break;
                                        }
                                    }
                                }
                                String::from_utf8(bytes).unwrap_or_default()
                            } else {
                                String::new()
                            }
                        }
                        SendSafeBody::None => {
                            tracing::warn!("No body in response");
                            String::new()
                        }
                        SendSafeBody::LineFeedStream(_) => {
                            tracing::warn!("LineFeedStream not supported");
                            String::new()
                        }
                    }
                }
                _ => String::new(),
            };
            parser(&body_text, source)
        }
        RequestIntro::Failed(e) => {
            tracing::warn!("Request failed: {e}");
            Vec::new()
        }
    }
})
```

**Key patterns**:
1. Use `opt_iter.take()` to consume `Option<StreamIterator>` - takes ownership
2. Collect bytes into `Vec<u8>` first, then convert to `String`
3. Handle errors gracefully with `unwrap_or_default()` or logging
4. Use `tracing::warn!` for unexpected cases, don't panic

**Why this complexity**: The HTTP client uses non-blocking I/O with connection pooling. Streaming bodies allow processing large responses without buffering everything in memory at once.

---

### TaskIterator Combinators Apply Before execute()

**Pattern**: All combinators (`.map_ready()`, `.map_pending()`, `.filter_ready()`, etc.) must be applied BEFORE calling `valtron::execute()`:

```rust
// CORRECT: Combinators before execute
let task = SendRequestTask::new(request, pool, config)
    .map_ready(|response| transform(response))
    .map_pending(|pending| log(pending));

let stream = valtron::execute(task, None)?;

// WRONG: Can't apply combinators after execute
let stream = valtron::execute(task, None)?;
let stream = stream.map_ready(...); // Doesn't exist - StreamIterator has different API
```

**Why**: `execute()` is the boundary between TaskIterator (implementer space) and StreamIterator (end-user space). Combinators are trait methods on `TaskIteratorExt`, not `StreamIteratorExt`.

**For StreamIterator processing**: Use standard iterator combinators after execute:

```rust
let stream = valtron::execute(task, None)?;
let results: Vec<_> = stream
    .filter_map(|item| match item {
        Stream::Next(value) => Some(value),
        _ => None,
    })
    .collect();
```

---

## Design Decisions

### Why TaskStatusIterator instead of modifying Iterator?

The standard `Iterator` trait cannot be modified and is fundamentally synchronous. By creating a new `TaskStatusIterator` trait, we:
- Keep `std::iter::Iterator` unchanged for synchronous cases
- Make async state (`Pending`, `Delayed`, `Ready`) explicit in the type system
- Enable Valtron executor integration without blocking

### Why collect_all() Returns TaskStatus<Vec<T>> Not Vec<T>?

Returning `TaskStatus<Vec<T>>` instead of `Vec<T>` allows the collection itself to be async-aware:
- Caller sees `Pending` while any sources are still pending
- Caller sees `Ready(vec)` only when all sources complete
- Enables further composition (map, filter) on the collected result

### Why execute() Returns StreamIterator Not TaskIterator? (v3.0 Architecture)

**Critical Design Decision**: `execute()` takes `TaskIterator` as input and returns `StreamIterator` as output. This separation provides:

1. **Clear Boundary**: TaskIterators are for implementers defining async tasks; StreamIterators are for end users consuming results
2. **Hidden Complexity**: The executor engine (delays, actions, spawner) is hidden inside `execute()`
3. **Simplified End User Experience**: End users work with `Stream<D, P>` variants (Init, Pending, Delayed, Next) without TaskStatus complexity
4. **Opt-in for TaskStatus**: `execute_as_task()` exists for rare cases needing TaskStatus output

```rust
// Implementer defines task with combinators (BEFORE execute)
let task = fetch_task().map_ready(transform).map_pending(log);

// execute() is the boundary - returns StreamIterator for end users
let stream = execute(task)?;  // StreamIterator, not TaskIterator

// End user consumes results (AFTER execute)
for item in stream {
    match item {
        Stream::Pending(_) => { /* still waiting */ }
        Stream::Next(value) => { /* got result */ }
    }
}
```

### Why Extension Traits for Combinators?

Extension traits (`TaskIteratorExt`, `StreamIteratorExt`) provide builder-style combinators:

1. **Automatic Availability**: Any type implementing `TaskStatusIterator` automatically gets all combinator methods
2. **Builder Pattern**: Enables chaining like `.map_ready(f).map_pending(g).filter_ready(h)`
3. **Internal Wrappers**: Each combinator wraps the iterator in a custom struct (MapReady, FilterReady, etc.)
4. **Type Safety**: Compiler enforces correct usage through trait bounds

### Why Wrapper Types for Combinators?

Each combinator (MapReady, MapPending, FilterReady, etc.) is a custom struct that:
1. **Holds the Source**: `inner: I` stores the wrapped iterator
2. **Holds the Operation**: `mapper: F` or `predicate: F` stores the transformation
3. **Forwards Unknown States**: `Pending`, `Delayed`, `Init`, `Spawn` pass through unchanged
4. **Transforms Target States**: Only the targeted variant (e.g., `Ready` for `MapReady`) is transformed

This pattern enables:
- **Composability**: Multiple combinators can be chained
- **Lazy Evaluation**: Transformations happen on `next()`, not at construction
- **Zero-Cost Abstraction**: No runtime overhead beyond the transformation itself

### Why Stream<D, P> Instead of StreamState?

We use the existing `Stream<D, P>` enum from `synca/mpp.rs` rather than creating a new `StreamState` type:

```rust
// Already exists - use it!
pub enum Stream<D, P> {
    Init,
    Ignore,
    Delayed(Duration),
    Pending(P),
    Next(D),  // This is "Done"
}
```

This avoids:
- Duplicate type definitions
- Confusion about which type to use
- Unnecessary trait conversions

### Why split_collector() Is Available on Both Traits?

The `split_collector()` combinator works on both `TaskIterator` (before `execute()`) and `StreamIterator` (after `execute()`):

1. **TaskIterator Context**: Split before execution, then continue with `execute(continuation)`
2. **StreamIterator Context**: Split after execution for pure stream processing
3. **Observer Pattern**: One branch observes intermediate values, the other continues the chain
4. **ClientRequest Use Case**: Get intro/headers first (observer), then read body (continuation)

```rust
// On TaskIterator (before execute)
let (observer, continuation) = task.split_collect_one(predicate);
let stream = execute(continuation)?;

// On StreamIterator (after execute)
let (observer, continuation) = stream.split_collect_one(predicate);
```

### Why Clone Bounds for split_collector()?

The `split_collector()` method requires `Ready: Clone` and `Pending: Clone` because:

1. **Observer Gets a Copy**: The observer branch receives cloned items via `ConcurrentQueue`
2. **Continuation Gets Original**: The continuation continues with the original iterator
3. **Trade-off Accepted**: Clone requirement is acceptable for the clean observer/continuation pattern

This is a deliberate trade-off that enables the ClientRequest intro/body pattern without manual state machines.
