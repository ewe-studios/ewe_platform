# Fundamentals 01 — Valtron: the async executor

Zero-to-expert on the executor that drives every async operation in the platform.
If you're calling `*_async`, using `DocumentStore`, or running an agentic loop,
you're going through valtron.

---

## 1. Why a custom executor

Rust's `tokio` and `async-std` assume OS threads. On `wasm32-unknown-unknown`
there are no threads — only a single-threaded event loop. The platform's design
requires **one async surface that works on both native (multi-threaded) and wasm
(single-threaded)**. Valtron achieves this through a cooperative `TaskIterator`
model: instead of OS-level preemption, tasks drive themselves to completion by
returning their own progress.

## 2. The TaskIterator trait

Every unit of work implements `TaskIterator`:

```rust
trait TaskIterator {
    type Ready;  // The value yielded on completion
    type Pending; // Thin status emitted during work
    type Spawner; // Action the executor can take (spawn new tasks)

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>>;
}
```

`TaskStatus` has four variants:
- **`Ready(T)`** — task completed, yield the value
- **`Pending(P)`** — still working, emit a thin progress signal
- **`Ignore`** — skip this turn, try again later
- **`Wait`** — park this task until something wakes it

This is **cooperative multitasking**: the executor calls `next_status()` in a
loop until the task returns `Ready`. The task itself decides when it's blocked
and what signal to emit.

## 3. FutureTask: bridging `async fn` to `TaskIterator`

Most code is written as `async fn`. `from_future()` wraps a `Future` into a
`FutureTask`:

```rust
pub fn from_future<F>(future: F) -> FutureTask<F>
where F: Future + Send + 'static
```

`FutureTask` polls the inner future using a no-op waker. If the future is ready,
it returns `Ready`. If it returns `Poll::Pending`, the task emits a `Pending`
signal and the executor parks it.

## 4. The Send gap on wasm

JS `JsFuture` (from `wasm-bindgen-futures`) is `!Send` because it holds a
reference to a JS Promise bound to the single-threaded isolate. But valtron's
`from_future()` requires `Send`. The bridge is `SendWrapper`:

```rust
#[repr(transparent)]
pub struct SendWrapper<T>(T);

#[cfg(target_family = "wasm")]
unsafe impl<T> Send for SendWrapper<T> {}

impl<F: Future> Future for SendWrapper<F> { ... }
```

On single-threaded wasm there are no other threads, so `Send` is vacuously
sound. Usage:

```rust
// In wasm storage code (e.g., D1WasmStorage):
let task = from_future(SendWrapper::new(JsFuture::from(promise)));
let stream = execute(task, None)?;
let result = collect_one(stream)?;
```

**On native/emscripten**, `SendWrapper` does NOT implement `Send` — the type
system prevents you from accidentally wrapping a `!Send` future where a real
`Send` future is needed.

## 5. Pool architecture

Valtron runs multiple **pools**, each with its own work queue and (on native)
worker threads:

```
Pool 0 (default)  ──→ workers 0..N
Pool 1 (priority)   ──→ workers N..M
Pool 2 (background) ──→ workers M..K
```

- **`execute(task, config)`** — schedules a task, returns a `DrivenStreamIterator`
  that yields progress as the task runs.
- **`execute_with_config`** — same but with explicit pool selection, max turns,
  and park duration.
- **`initialize_pool(thread_count, config)`** — starts the executor. Must be
  called once before any `execute()` call. Returns a `PoolGuard` that shuts down
  the pools on drop.

## 6. Stream integration: DrivenStreamIterator

`execute()` returns a `DrivenStreamIterator<T>` — an iterator that drives the
task forward each time you call `next()`. It implements the `StreamIteratorExt`
trait with helpers:

```rust
let stream = execute(task, None)?;

// Collect all Next values into a Vec (blocks until task completes)
let results = stream.collect_all().await?;

// Collect just the first completed value
let first = stream.collect_first().await?;

// Map over Done/Pending items
let mapped = stream.map_done(|r| transform(r)).map_pending(|p| thin(p));
```

The stream yields `Stream<Result<T, E>, P>` items:
- `Stream::Next(Ok(T))` — task completed with value
- `Stream::Next(Err(E))` — task failed
- `Stream::Pending(P)` — progress signal
- `Stream::Ignore` / `Stream::Wait` — control signals

## 7. Scheduling futures through valtron

The common pattern in wasm storage code:

```rust
fn schedule_future<T, E, F>(future: F) -> StorageResult<StorageItemStream<'static, T>>
where
    F: Future<Output = Result<T, E>> + 'static,
    E: Into<StorageError>,
{
    let task = from_future(SendWrapper::new(future));
    let stream = execute(task, None)
        .map_err(|e| StorageError::Backend(format!("Valtron scheduling failed: {e}")))?;
    Ok(Box::new(
        stream
            .map_done(|r: Result<T, E>| r.map_err(Into::into))
            .map_pending(|_| ()),
    ))
}

fn exec_future<T, E, F>(future: F) -> StorageResult<T>
where
    F: Future<Output = Result<T, E>> + 'static,
    E: Into<StorageError>,
{
    let task = from_future(SendWrapper::new(future));
    let stream = execute(task, None)
        .map_err(|e| StorageError::Backend(format!("Valtron execution failed: {e}")))?;
    let result: Result<Option<T>, StorageError> = collect_one(stream)
        .map(|r| r.map_err(Into::into))
        .transpose();
    result?.ok_or_else(|| StorageError::Generic("No result from future execution".into()))
}
```

`schedule_future` returns a lazy stream (caller drives execution). `exec_future`
blocks until completion (used by sync trait methods that delegate to async impls).

## 8. Testing

Tests that need the executor use `#[valtron_test]` instead of `#[test]`. The
attribute initializes a single-threaded pool before running the test body.

**Common hang**: creating multiple pools in parallel tests. Use a global
`Mutex<Option<PoolGuard>>` to share a single pool across tests:

```rust
static POOL: Mutex<Option<PoolGuard>> = Mutex::new(None);

fn init_valtron() {
    let mut guard = POOL.lock().unwrap();
    if guard.is_none() {
        *guard = Some(valtron::initialize_pool(42, None));
    }
}
```

See `valtron/docs/debugging_multi_pool_test_hangs.md` for troubleshooting.

## 9. When to NOT use valtron

- **Pure CPU work** — use `rayon` or `std::thread::spawn` on native.
- **IO-bound with native tokio** — if you're already in a tokio runtime, use
  `tokio::spawn` and `tokio::fs`. Valtron is for cross-platform code that must
  also run on wasm.
- **Simple one-shot futures on native** — `pollster::block_on(future)` is simpler
  than setting up a valtron pool for a single future.
