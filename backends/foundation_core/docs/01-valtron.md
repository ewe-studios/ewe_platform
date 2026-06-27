# Valtron — Multi-pool async executor

## What
A work-stealing async executor with multiple thread pools, designed for both
native multi-threaded and wasm single-threaded execution.

## Architecture
- **ThreadPool** — Each pool has its own queue and worker threads. Tasks are
  scheduled to pools by priority or type.
- **TaskIterator** — Tasks implement this trait to drive their own execution
  (cooperative multitasking, no preemption).
- **Stream integration** — `execute()` returns a `DrivenStreamIterator` that
  yields task progress (`Next`, `Pending`, `Ignore`, `Wait`).
- **Send wrapper** — `SendWrapper` bridges `!Send` futures (e.g. JS `JsFuture`)
  to the `Send`-requiring executor on single-threaded wasm.

## Key APIs
```rust
// Initialize a pool (usually at app startup)
let guard = valtron::initialize_pool(42, None); // 42 worker threads

// Execute a future
let task = valtron::from_future(my_async_fn());
let stream = valtron::execute(task, None)?;

// Drive to completion (sync wrapper for async)
let result = valtron::collect_one(stream)?;
```

## Testing
Use `#[valtron_test]` instead of `#[test]` for tests that need the executor.
See `valtron/docs/debugging_multi_pool_test_hangs.md` for hang debugging.
