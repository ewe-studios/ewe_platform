# Learnings: Foundation AI

## Overview

This file captures learnings, design decisions, challenges, and patterns discovered during the implementation of the `foundation_ai` llama.cpp integration.

## Design Decisions

### Valtron Async Runtime Integration

**Decision:** Use Valtron's TaskIterator/StreamIterator pattern for all async operations in foundation_ai.

**Why:**
- Valtron provides a unified executor framework that works across WASM (single-threaded) and native (multi-threaded) platforms
- The iterator-based approach avoids blocking and provides fine-grained control over async workflows
- Clear separation between task implementers (TaskIterator) and consumers (StreamIterator)

**How to apply:**
- All async tasks in foundation_ai should implement `TaskIterator`
- Use `execute()` from `foundation_core::valtron` to run tasks
- Apply combinators BEFORE `execute()` for TaskIterator transformations
- Apply combinators AFTER `execute()` for StreamIterator transformations

### PoolGuard Lifecycle Pattern

**Decision:** PoolGuard is initialized only in binary entry points (`main()`) and tests, NOT in library code.

**Why:**
- `PoolGuard::Drop` signals worker threads to shut down
- Library code should assume the pool is already available
- Prevents multiple pool initializations and premature shutdown

**How to apply:**
```rust
// In binary main() or tests:
let _guard = valtron::initialize_pool(100, None);

// In library code (foundation_ai):
// Just call execute() - pool should already be initialized
let stream = execute(task, None)?;
```

### State Machine Pattern for Complex Workflows

**Decision:** Use `StateMachine` trait for complex async workflows (OAuth, JWT validation, token generation).

**Why:**
- Provides explicit state tracking
- Makes retry logic and error handling clear
- Easier to test and reason about than nested futures

**How to apply:**
- Implement `StateMachine` trait with `StateTransition` enum
- Wrap with `StateMachineTask::new()` for execution
- Use `execute()` to run the state machine

## Valtron Capabilities Reference

### Core Types

| Type | Purpose | Location |
|------|---------|----------|
| `TaskStatus<D, P, S>` | Task state enum (Ready, Pending, Delayed, Init, Spawn, Ignore) | `valtron/task.rs` |
| `Stream<D, P>` | Stream state enum (Init, Ignore, Delayed, Pending, Next) | `synca/mpp.rs` |
| `TaskIterator` | Trait for implementing async tasks | `valtron/task.rs` |
| `StreamIterator` | Trait for consuming async results | `synca/mpp.rs` |

### Executor Functions

| Function | Purpose | Returns |
|----------|---------|---------|
| `execute()` | Execute single task | `DrivenStreamIterator` |
| `execute_as_task()` | Execute as TaskStatus iterator | `DrivenRecvIterator` |
| `execute_collect_all()` | Execute multiple tasks, collect when all complete | `CollectAllStream` |
| `execute_map_all()` | Execute multiple tasks, map when all complete | `MapAllDoneStream` |
| `execute_map_all_pending_and_done()` | Execute with state-aware mapping | `MapAllPendingAndDoneStream` |

### TaskIterator Combinators (BEFORE execute())

| Combinator | Purpose |
|------------|---------|
| `map_ready(f)` | Transform Ready values |
| `map_pending(f)` | Transform Pending values |
| `filter_ready(f)` | Filter Ready values (filtered items become Ignore) |
| `stream_collect()` | Collect all Ready values into Vec |
| `split_collector(predicate, size)` | Split into observer + continuation |
| `split_collect_one(predicate)` | Split for first match only |

### StreamIterator Combinators (AFTER execute())

| Combinator | Purpose |
|------------|---------|
| `map_done(f)` | Transform Next values |
| `map_pending(f)` | Transform Pending values |
| `map_delayed(f)` | Transform Delayed durations |
| `filter_done(f)` | Filter Next values |
| `collect()` | Collect all Next values |
| `split_collector(predicate, size)` | Split into observer + continuation |

### Platform Auto-Selection

| Platform | Feature | Executor Used |
|----------|---------|---------------|
| WASM | any | `single` |
| Native | none | `single` |
| Native | `multi` | `multi` |

## Tracing Best Practices

### Do: Generic Log Messages

```rust
// Good: No Debug/Display required on generics
tracing::trace!("TaskIterator: received Ready item");
tracing::debug!("StreamIterator: queue closed, no more items");
tracing::error!("Task failed during execution");
```

### Don't: Format Generic Types

```rust
// Bad: Requires D: Debug
tracing::debug!("Got Ready({:?})", value);
```

### Handle Errors Properly

```rust
// Good: Log errors for debugging
if let Err(e) = self.queue.force_push(item) {
    tracing::error!("Failed to push to queue: {}", e);
} else {
    tracing::trace!("Copied item to observer queue");
}
```

### Iron Law: No tokio/async-trait in foundation_db and foundation_auth

**Decision:** `tokio` and `async-trait` are BANNED from `foundation_db` and `foundation_auth`. All async operations use Valtron only.

**Why:**
- Valtron provides a unified executor for WASM (single-threaded) and native (multi-threaded)
- Mixing tokio breaks cross-platform portability and creates competing runtimes
- The `#[async_trait]` pattern allocates on every call and hides the actual execution model

**How to apply:**
- Storage traits use synchronous methods or `TaskIterator` state machines
- DB operations are `TaskIterator` impls — consumers call `execute(task, None)` for a `StreamIterator`
- Memory backends use `std::sync::Mutex`, not `tokio::sync::Mutex`
- Tests use `valtron::initialize_pool` + `execute()`, not `#[tokio::test]`

### Turso Sync Backend

**Decision:** `foundation_db` uses the Turso crate exclusively with its sync API. No feature flags needed.

**Why:**
- Turso is a ground-up SQLite rewrite with MVCC, concurrent writes, and sync/async I/O APIs
- libsql has hard sync dependencies that conflict with our Valtron-only async model
- Turso provides a clean sync API (https://github.com/tursodatabase/turso/blob/main/sdk-kit/README.md)
- Turso supports edge sync capabilities for distributed deployments

**How to apply:**
- Turso backend is always available (no feature flag)
- Use Turso sync API exclusively — no async/await in storage traits
- Public trait interfaces must NOT leak `turso::Value`, `turso::Row`, etc. — use crate-owned `DataValue`/`DataRow`

### Error Convention: `derive_more::From` + Manual Display

**Decision:** All error types use `#[derive(From, Debug)]` from `derive_more` for automatic `From<T>` conversions and manual `impl Display`. No `thiserror`.

**Why:**
- This is the established convention across `foundation_core`, `foundation_auth`, and the entire workspace
- `derive_more::From` auto-generates `From<T>` for each variant with a typed field
- `#[from(ignore)]` prevents conflicts when multiple variants wrap `String`
- Manual `Display` gives full control over error messages
- `thiserror` is redundant when `derive_more::From` already handles conversions

**How to apply:**
- Central `src/errors.rs` per crate with all error enums
- `#[derive(From, Debug)]` on error enums
- `#[from(ignore)]` on all `(String)` variants
- Manual `impl core::fmt::Display` with match arms
- Simple `impl std::error::Error for ... {}`
- `pub type FooResult<T> = Result<T, FooError>;`

## Wrapping Async Libraries with `from_future` + `execute` (Learned from Feature 00a)

**Discovery:** Many Rust crates (Turso, libsql, hf-hub, HTTP clients) only expose async APIs. Valtron's `from_future` bridges these into the Valtron executor without needing tokio.

**IMPORTANT:** Read `.agents/skills/rust-valtron-usage/skill.md` for the full guidance on when and how to use these patterns.

### Primary Pattern: Return Streams (Non-Blocking)

Methods that perform I/O should schedule work via `execute()` and **return the stream** to the caller. This preserves composability and enables parallelism:

```rust
fn get<V: DeserializeOwned + Send + 'static>(
    &self, key: &str,
) -> StorageResult<impl Iterator<Item = Stream<Option<V>, ()>>> {
    let key = key.to_string();
    let conn = Arc::clone(&self.conn);
    let task = from_future(async move {
        // ... async DB work, collect results inside async block ...
        Ok::<_, BackendError>(result)
    });
    let stream = execute(task, None)
        .map_err(|e| StorageError::Backend(format!("Valtron scheduling failed: {e}")))?;
    Ok(stream)
}
```

Callers collect at boundaries using `collect_result()`, `sync_one()`, or standard `Iterator` methods.

### Sync Boundary Helpers (Available in Valtron)

These are real functions in `foundation_core::valtron`:

- **`collect_result(stream)`** — Drains a stream, collects all `Next` values into `Vec<D>`. Use at sync boundaries only.
- **`sync_one(task)`** — Execute a single task, block until complete. Escape hatch — use sparingly.
- **`sync_all(tasks)`** — Execute multiple tasks in parallel, block until all complete.

### Legacy Pattern: `exec_future` (Blocking — Use Sparingly)

The `exec_future` helper wraps a future and blocks immediately. This turns async into sync at the leaf and **prevents parallelism**. Use only for one-shot initialization or migrations:

```rust
pub fn exec_future<T, E, F>(future: F) -> Result<T, YourError> { /* ... */ }
```

### Critical Constraint: `Send + 'static`

All data captured by the async block must be `Send + 'static`:
- Convert `&str` to `String` before the async block
- Clone `Arc<T>` before moving into the async block
- The future itself must be `Send + 'static`

### Critical Constraint: `!Send` Iterators

Many database crates return row iterators that are `!Send`. These **cannot cross the Valtron execution boundary**. All iteration must happen inside the async block, collecting into `Vec<T>` before returning.

```rust
// CORRECT: Collect inside async, return Vec
let rows: Vec<MyRow> = exec_future(async move {
    let mut rows = stmt.query([]).await?;
    let mut result = Vec::new();
    while let Some(row) = rows.next().await? {
        result.push(convert_row(&row)?);
    }
    Ok::<_, BackendError>(result)
})?;

// WRONG: Cannot return !Send row iterators from exec_future
```

### When Valtron Is NOT Needed

Purely synchronous/CPU-bound operations do NOT need Valtron wrapping:
- In-memory storage (HashMap + Mutex)
- Password hashing (Argon2id)
- Token signing/verification (JWT, HMAC)
- PKCE code generation
- Tokenization and model inference (llama.cpp, Candle)

Only use `from_future`/`exec_future` for genuinely async I/O: database queries, HTTP requests, file downloads.

### Three-Level Error Handling

Every `exec_future` call handles errors at three distinct levels:
1. **Valtron execution failure** — `execute()` itself fails (pool not initialized, runtime error)
2. **Empty stream** — Future ran but produced no `Stream::Next` item
3. **Backend error** — The future's `Result` was `Err` (SQL error, HTTP error, etc.)

### Multi-Value Returns: `StorageItemStream`

For operations returning multiple items, collect into `Vec<T>` then wrap:

```rust
type ItemStream<'a, T> = Box<dyn Iterator<Item = Stream<T, ()>> + Send + 'a>;

// After exec_future collects Vec<T>:
Ok(Box::new(results.into_iter().map(Stream::Next)))

// Consumer side:
let items: Vec<T> = stream
    .filter_map(|s| match s { Stream::Next(v) => Some(v), _ => None })
    .collect();
```

### Turbo-fish for Async Block Error Types

When the compiler can't infer the error type in an async block, annotate explicitly:

```rust
exec_future(async move {
    conn.execute_batch(&sql).await?;
    Ok::<_, turso::Error>(true)  // Turbo-fish needed
})?;
```

## Key Takeaways

1. **TaskIterator is Input, StreamIterator is Output** - `execute()` takes TaskIterator, returns StreamIterator
2. **Never block** - All iterator methods yield `Stream` states instead of waiting
3. **Clear separation** - TaskIterator for implementers, StreamIterator for end users
4. **execute() is the boundary** - Hides executor concerns (delays, actions, spawner)
5. **Combinators before execute()** - All TaskIterator combinators applied BEFORE calling execute()
6. **Standard iterators after execute()** - Use standard Iterator combinators on StreamIterator output
7. **Queue Closing** - Use `ConcurrentQueue::close()` on natural completion, not just in Drop
8. **Use #[traced_test]** - Invaluable for debugging async iterator behavior
9. **Heterogeneous Closures** - Cannot use `execute_collect_all()` for heterogeneous tasks; execute each individually
10. **`from_future` bridges async crates** - Any async-only library can be used via `from_future` + `execute` without tokio
11. **Collect !Send types inside async blocks** - Database row iterators are typically `!Send` and must be consumed before crossing the Valtron boundary
12. **Sync code stays sync** - CPU-bound operations (hashing, signing, inference) should NOT be wrapped in Valtron

---

_Last Updated: 2026-03-28 (Added: from_future/exec_future patterns from foundation_db implementation)_
