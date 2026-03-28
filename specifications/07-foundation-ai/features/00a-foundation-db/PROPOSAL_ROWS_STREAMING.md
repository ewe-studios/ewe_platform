# Proposal: Streaming SQL Rows Without Loading All Into Memory

## Problem Statement

Currently, `query()` and `list_keys()` methods collect ALL rows into a `Vec<T>` inside the async block before returning. This is memory-inefficient for large result sets:

```rust
// CURRENT: Collects ALL rows into memory before streaming
fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<StorageItemStream<'_, Vec<SqlRow>>> {
    let rows: Vec<SqlRow> = exec_future(async move {
        let mut rows = stmt.query([]).await?;
        let mut collected = Vec::new();
        while let Some(row) = rows.next().await? {
            collected.push(row);  // ALL rows in memory at once
        }
        Ok::<_, Error>(collected)
    })?;
    Ok(Box::new(std::iter::once(Stream::Next(Ok(rows)))))
}
```

**Why this happens:** Database row iterators (`turso::Rows`, `libsql::Rows`) are `!Send` - they cannot cross thread boundaries. To use Valtron's executor (which schedules tasks on a thread pool), we must wrap the rows in a way that's `Send`.

## The Core Challenge

**The Send Requirement Chain:**
```
turso::Rows: !Send  (has thread-local state like file handles)
       ↓
RowsIterator { rows: turso::Rows }: !Send
       ↓
Arc<Mutex<RowsIterator>>: !Send  (Mutex<T> requires T: Send)
       ↓
Cannot return from async block (FutureTask requires F::Output: Send)
```

**Why wrapping doesn't help:** `Arc<Mutex<T>>` is only `Send` if `T: Send`. Wrapping `!Send` in `Arc<Mutex<>>` doesn't make it `Send` - the mutex can't protect thread-safety if the inner type itself can't cross threads.

**Valtron's constraint:** Combinator methods require `Send` bounds, so we can't just keep `!Send` types on one thread without a bridge.

## Proposed Solution: ThreadedFuture with Channel-Based Bridging

### Key Insight

Use a **dedicated worker thread** that owns the `!Send` type, with a **channel-based queue** to stream data across the thread boundary. The `!Send` type never crosses threads - only the extracted row data does.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         MAIN THREAD                                     │
│  ThreadedFuture::new(async {                                            │
│      let rows = get_rows().await;                                       │
│      RowsIterator::new(rows)  // !Send, created on worker               │
│  })                                                                     │
│                                                                         │
│  execute() → Receiver<RowValue<T>>                                      │
│      ↓                                                                  │
│  recv() → RowValue::Next(row)  // T crosses via queue                   │
│  recv() → RowValue::Waiting  // or block until available                │
└─────────────────────────────────────────────────────────────────────────┘
                              ↕ ConcurrentQueue (MPMC)
┌─────────────────────────────────────────────────────────────────────────┐
│                         WORKER THREAD                                   │
│  Spawns and owns:                                                       │
│  - async runtime (for the initial future)                               │
│  - RowsIterator (!Send, stays here forever)                             │
│  - Sender end of ConcurrentQueue                                        │
│                                                                         │
│  Loop:                                                                  │
│  1. Poll async future → get RowsIterator                                │
│  2. Call iterator.next() → get RowValue<T>                              │
│  3. queue.send(item)  // Blocks if queue full (backpressure)            │
│  4. Repeat until iterator exhausted                                     │
└─────────────────────────────────────────────────────────────────────────┘
```

### Architecture Overview

**ThreadedFuture** - A new executor type for `!Send` async operations:

```rust
pub struct ThreadedFuture<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = I>,
    I: Iterator<Item = Result<T, E>> + 'static,
{
    /// Async function that produces the Iterator
    /// This is called ONCE on the worker thread
    future_fn: F,

    /// Queue size (configurable, minimum 2)
    queue_size: usize,

    _phantom: PhantomData<(Fut, I, T, E)>,
}

/// The internal worker that runs on the spawned thread
struct ThreadedFutureWorker<T, E> {
    /// Receiving end - given to caller
    receiver: Receiver<ThreadedValue<T, E>>,

    /// Worker handle - joins on drop
    handle: Option<std::thread::JoinHandle<()>>,
}
```

### How It Works

**Step 1: Create ThreadedFuture**
```rust
let threaded = ThreadedFuture::new(|| async {
    // This async block runs on WORKER thread
    let rows = db.query("SELECT * FROM large_table").await?;
    Ok::<_, Error>(RowsIterator::new(rows))  // Returns !Send Iterator
});
```

**Step 2: Execute and spawn worker thread**
```rust
let (worker, queue_size) = threaded.execute();

// Internally, execute():
// 1. Creates ConcurrentQueue with configured size
// 2. Spawns worker thread that:
//    a. Runs the async function (gets RowsIterator)
//    b. Loops: iterator.next() → queue.send(result)
//    c. Blocks on send() when queue full (backpressure)
// 3. Returns Receiver to caller
```

**Step 3: Receive data on main thread**
```rust
let receiver = worker.receiver();

// Receiver implements Iterator<Item = RowValue<T>>
for value in receiver {
    match value {
        RowValue::Next(row) => { /* process row */ }
        RowValue::Waiting => { /* skip or handle */ }
    }
}
```

### ThreadedValue Type

```rust
/// Result of polling an iterator - sent across thread boundary.
///
/// Note: Iterator loops internally until Ready, so we always get Value.
/// The Waiting variant is kept for future flexibility but currently unused.
pub enum ThreadedValue<T, E> {
    Value(Result<T, E>),  // Ok(row) or Err(error)
}
```


### ThreadedFuture Implementation

```rust
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::marker::PhantomData;
use foundation_core::synca::mpp::{self, Sender, Receiver, SenderError};

/// Create a no-op waker for blocking poll loops (same as in RowsIterator)
fn create_noop_waker() -> Waker {
    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        |_| RawWaker::new(std::ptr::null(), &VTABLE),
        |_| {},
        |_| {},
        |_| {},
    );
    unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}

/// A future executor that spawns a dedicated thread for !Send operations.
pub struct ThreadedFuture<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<I, E>> + 'static,
    I: Iterator<Item = Result<T, E>> + 'static,
    T: Send + 'static,
    E: Send + 'static,
{
    future_fn: F,
    queue_size: usize,
    /// Backpressure sleep duration when queue is full (std feature)
    /// Falls back to spin_loop if not std or Duration is None
    backpressure_sleep: Option<Duration>,
    _phantom: PhantomData<(Fut, I, T, E)>,
}

impl<F, Fut, I, T, E> ThreadedFuture<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<I, E>> + 'static,
    I: Iterator<Item = Result<T, E>> + 'static,
    T: Send + 'static,
    E: Send + 'static,
{
    pub fn new(future_fn: F) -> Self {
        Self {
            future_fn,
            queue_size: 16,
            backpressure_sleep: Some(Duration::from_millis(10)),
            _phantom: PhantomData,
        }
    }

    pub fn with_queue_size(future_fn: F, queue_size: usize) -> Self {
        Self {
            future_fn,
            queue_size: queue_size.max(2),
            backpressure_sleep: Some(Duration::from_millis(10)),
            _phantom: PhantomData,
        }
    }

    pub fn with_backpressure_sleep(
        future_fn: F,
        queue_size: usize,
        backpressure_sleep: Option<Duration>,
    ) -> Self {
        Self {
            future_fn,
            queue_size: queue_size.max(2),
            backpressure_sleep,
            _phantom: PhantomData,
        }
    }

    pub fn execute(self) -> (WorkerHandle, Receiver<ThreadedValue<T, E>>) {
        let (sender, receiver) = mpp::bounded::<ThreadedValue<T, E>>(self.queue_size);
        let backpressure_sleep = self.backpressure_sleep;

        let handle = thread::spawn(move || {
            let waker = create_noop_waker();
            let cx = Context::from_waker(&waker);
            let mut future = Pin::new((self.future_fn)());

            let mut iterator = loop {
                match future.as_mut().poll(&cx) {
                    Poll::Ready(Ok(iter)) => break Some(iter),
                    Poll::Ready(Err(e)) => {
                        // Try to send error, but receiver might be dropped
                        let _ = sender.send(ThreadedValue::Value(Err(e)));
                        break None;
                    }
                    Poll::Pending => continue,
                }
            };

            // Stream iterator results through the queue
            if let Some(mut iter) = iterator {
                loop {
                    match iter.next() {
                        Some(result) => {
                            let value = ThreadedValue::Value(result);

                            // Try to send with backpressure handling
                            loop {
                                match sender.send(value) {
                                    Ok(()) => break,
                                    Err(SenderError::Full(_)) => {
                                        // Queue full - apply backpressure
                                        if let Some(sleep_dur) = backpressure_sleep {
                                            thread::sleep(sleep_dur);
                                        } else {
                                            std::hint::spin_loop();
                                        }
                                    }
                                    Err(SenderError::Closed(_)) => {
                                        // Receiver dropped, queue closed - exit cleanly
                                        return;
                                    }
                                }
                            }
                        }
                        None => break,  // Iterator exhausted
                    }
                }
            }
            // Worker exits cleanly - sender dropped, queue closed
        });

        (WorkerHandle { handle: Some(handle) }, receiver)
    }
}

/// Worker handle - joins the worker thread on drop
pub struct WorkerHandle {
    handle: Option<thread::JoinHandle<()>>,
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        self.handle.take().map(|h| h.join());
    }
}
```

**Key improvements:**
1. **Handles closed queue**: When `sender.send()` fails because receiver is dropped (queue closed), the worker exits cleanly
2. **Configurable backpressure**: `backpressure_sleep: Option<Duration>` lets caller tune the sleep duration when queue is full
3. **std vs no_std**: Uses `thread::sleep()` when std is enabled and sleep duration is set, falls back to `spin_loop()` otherwise

**Why this is cleaner:** `execute()` returns `(WorkerHandle, Receiver)` directly using the existing `mpp` module. `Receiver` wraps the `ConcurrentQueue` and provides `recv()`, `recv_timeout()`, and iterator adapters.

### Usage in query()

```rust
fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<StorageItemStream<'_, SqlRow>> {
    let turso_params = Self::to_turso_params(params);
    let sql = sql.to_string();
    let conn = Arc::clone(&self.conn);

    let threaded = ThreadedFuture::new(move || async move {
        let mut stmt = conn.prepare(&sql).await?;
        let rows = stmt.query(turso_params).await?;
        Ok::<_, turso::Error>(RowsIterator::new(rows))
    });

    // Execute returns (WorkerHandle, Receiver<ThreadedValue<SqlRow, StorageError>>)
    let (_handle, receiver) = threaded.execute();

    // Convert Receiver to Iterator using RecvIter + RecvIterator
    let recv_iter = receiver.into_iter_recv();
    
    // Convert to Stream via Valtron combinator
    let stream = foundation_core::valtron::map_iter_with_status(recv_iter);

    Ok(Box::new(stream))
}
```

**Note:** Added `receiver.chan()` method to `Receiver` to get the underlying `Arc<ConcurrentQueue>`. The `mpp::RecvIter::block_iter()` provides the blocking iterator behavior we need.

**Error flow:**
1. **Future errors** (e.g., `prepare()` fails): Error sent through queue, worker exits, receiver gets `Err(e)` then `None`
2. **Iterator errors** (e.g., `convert_row()` fails): `Err(e)` sent through queue, worker continues to next row
3. **Receiver gets all errors** - nothing is swallowed

### RowsIterator - Owned on Worker Thread

```rust
/// Iterator that yields SqlRow values one at a time from a database query.
///
/// WHY direct ownership: This iterator lives ONLY on the worker thread
/// (spawned by ThreadedFuture). It never crosses threads, so !Send is fine.
///
/// WHY Iterator<Item = Result<T, E>>: Errors are yielded as Err(E) directly,
/// not wrapped in an extra enum. ThreadedValue handles the Waiting state.
pub struct RowsIterator {
    rows: turso::Rows,  // Direct ownership - !Send but stays on worker thread
    current_future: Option<Pin<Box<dyn Future<Output = Result<Option<turso::Row>, turso::Error>>>>>,
}

impl RowsIterator {
    pub fn new(rows: turso::Rows) -> Self {
        Self {
            rows,
            current_future: None,
        }
    }

    fn turso_value_to_data_value(value: turso::Value) -> DataValue {
        match value {
            turso::Value::Null => DataValue::Null,
            turso::Value::Integer(i) => DataValue::Integer(i),
            turso::Value::Real(r) => DataValue::Real(r),
            turso::Value::Text(s) => DataValue::Text(s),
            turso::Value::Blob(b) => DataValue::Blob(b),
        }
    }

    fn convert_row(row: &turso::Row) -> StorageResult<SqlRow> {
        let column_count = row.column_count();
        let mut columns = Vec::with_capacity(column_count);
        for i in 0..column_count {
            let name = format!("col{i}");
            let value = Self::turso_value_to_data_value(
                row.get_value(i).map_err(|e| {
                    StorageError::SqlConversion(format!("Column {i} error: {e}"))
                })?
            );
            columns.push((name, value));
        }
        Ok(SqlRow::new(columns))
    }
}

impl Iterator for RowsIterator {
    type Item = Result<SqlRow, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        let waker = create_noop_waker();
        let cx = Context::from_waker(&waker);

        // Step 1: Get or create the future from rows.next()
        if self.current_future.is_none() {
            if let Some(future) = self.rows.next() {
                self.current_future = Some(Box::pin(future));
            } else {
                return None;  // No more rows - exhausted
            }
        }

        // Step 2: Poll the stored (pinned) future once
        let future = self.current_future.as_mut().unwrap();
        match future.as_mut().poll(&cx) {
            Poll::Ready(Some(Ok(row))) => {
                self.current_future = None;
                Some(Ok(Self::convert_row(&row)))
            }
            Poll::Ready(Some(Err(e))) => {
                self.current_future = None;
                Some(Err(StorageError::Backend(e.to_string())))
            }
            Poll::Ready(None) => {
                self.current_future = None;
                None  // Exhausted
            }
            Poll::Pending => {
                // Can't return Pending from Iterator::next()
                // Instead, we return a sentinel value or use a different approach
                // See ThreadedValue::Waiting for how this is handled
                Some(Err(StorageError::Backend("Row fetch returned Pending".to_string())))
            }
        }
    }
}
```

**Wait - issue:** `Iterator::next()` can't signal "Waiting" - it must return `Some(T)` or `None`. For local SQLite, the future is typically ready immediately, but if it returns `Pending`, we need to handle it.

**Solution:** The worker thread in `ThreadedFuture` should handle `Pending` by looping until ready, not the iterator itself. Let me update the design:

```rust
impl Iterator for RowsIterator {
    type Item = Result<SqlRow, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        let waker = create_noop_waker();
        let cx = Context::from_waker(&waker);

        loop {
            // Get or create the future
            if self.current_future.is_none() {
                if let Some(future) = self.rows.next() {
                    self.current_future = Some(Box::pin(future));
                } else {
                    return None;  // Exhausted
                }
            }

            // Poll the future
            let future = self.current_future.as_mut().unwrap();
            match future.as_mut().poll(&cx) {
                Poll::Ready(Some(Ok(row))) => {
                    self.current_future = None;
                    return Some(Ok(Self::convert_row(&row)));
                }
                Poll::Ready(Some(Err(e))) => {
                    self.current_future = None;
                    return Some(Err(StorageError::Backend(e.to_string())));
                }
                Poll::Ready(None) => {
                    self.current_future = None;
                    return None;  // Exhausted
                }
                Poll::Pending => {
                    // Loop and poll again - for local SQLite, will be ready immediately
                    continue;
                }
            }
        }
    }
}
```

**Why the loop:** Since `Iterator::next()` can't return `Pending`, we loop internally until the future is ready. For local SQLite, this is acceptable - the row data is already in memory, so `Pending` is rare/transient.

The `ThreadedValue::Waiting` variant is then used when the WORKER thread hasn't produced a result yet (e.g., first poll before any data is ready).

### Why Pin Is Required

```rust
// rows.next() returns impl Future that must be pinned before polling
// We store Pin<Box<dyn Future>> to continue polling across next() calls

let future = self.rows.next()?;  // Get future from iterator
let pinned: Pin<Box<dyn Future<...>>> = Box::pin(future);  // Box then pin
pinned.as_mut().poll(&cx);  // Now we can poll through Pin
```

**Why this matters:**
- Rust futures are not `Unpin` by default - they may contain self-references
- Polling requires the future to be at a stable memory location
- `Box::pin()` heap-allocates and creates a `Pin<Box<T>>` wrapper
- We store this pinned future to continue polling across `Iterator::next()` calls

### LibsqlRowsIterator

Same pattern for libsql backend:

```rust
pub struct LibsqlRowsIterator {
    rows: libsql::Rows,
    current_future: Option<Pin<Box<dyn Future<Output = Result<Option<libsql::Row>, libsql::Error>>>>>,
}

// Same Iterator impl pattern as RowsIterator
```

## Implementation Checklist

- [ ] Create `RowValue<T>` enum in `foundation_db/src/rows_stream.rs`
  - `Next(T)` - row ready (Ok or Err)
  - `Waiting` - future still pending
- [ ] Create `RowsIterator` struct
  - [ ] Owns `turso::Rows` directly (no Arc<Mutex<>>)
  - [ ] Stores `Pin<Box<dyn Future>>` from `rows.next()`
  - [ ] Implements `Iterator<Item = RowValue<StorageResult<SqlRow>>>`
- [ ] Create `LibsqlRowsIterator` for libsql backend (same pattern)
- [ ] Create `ThreadedFuture<F, Fut, I, T>` struct
  - [ ] Takes async function returning Iterator
  - [ ] Spawns worker thread with dedicated runtime
  - [ ] Uses `ArrayQueue` for MPSC communication
  - [ ] Returns `ThreadedFutureReceiver<T>` that implements Iterator
- [ ] Create `ThreadedFutureReceiver<T>` struct
  - [ ] Holds `Arc<ArrayQueue>` and `JoinHandle`
  - [ ] Implements `Iterator<Item = RowValue<T>>`
  - [ ] Spins until item available or worker done
- [ ] Update `query()` in `turso_backend.rs` to use `ThreadedFuture`
- [ ] Update `query()` in `libsql_backend.rs` to use `ThreadedFuture`
- [ ] Add module to `backends/mod.rs`
- [ ] Update tests

## Why This Works

1. **Worker thread owns !Send type**: `RowsIterator` with `turso::Rows` lives entirely on worker thread, never crosses boundaries

2. **Channel bridges the thread boundary**: `ArrayQueue` is `Send + Sync` - only the `RowValue<SqlRow>` results (which are `Send`) cross threads

3. **Backpressure via blocking queue**: When queue is full, worker blocks on `send()`, preventing memory buildup

4. **O(1) memory per row on main thread**: Main thread only holds the queue buffer (configurable size), not all rows

5. **Clean separation**: Worker thread handles async + !Send complexity, main thread receives `Send` data via standard Iterator

## Key Architecture Pattern

```rust
// 1. ThreadedFuture wraps the !Send operation
let threaded = ThreadedFuture::new(|| async {
    // Runs on worker thread - !Send types OK here
    let rows = db.query().await?;
    Ok(RowsIterator::new(rows))  // !Send Iterator
});

// 2. Execute spawns worker, returns Receiver
let receiver = threaded.execute();
// receiver: impl Iterator<Item = RowValue<SqlRow>> + Send

// 3. Use with Valtron combinators
let stream = valtron::map_iter_with_status(receiver);
```

## Risks / Considerations

1. **Thread overhead**: Spawning a worker thread has cost (~8KB stack + OS scheduling). For small result sets (<100 rows), the Vec approach may be more efficient.

2. **Queue size tuning**: Default queue size (16) balances memory vs throughput. Larger queues reduce blocking but use more memory. Consider making configurable per-query.

3. **Spin-loop contention**: Receiver spins when queue empty, worker spins when queue full. For high-throughput scenarios, consider adding `std::thread::yield_now()` or using a smarter wait strategy.

4. **Worker thread panic**: If worker panics, `JoinHandle` propagates the panic. Receiver should handle this gracefully (currently just marks `done = true`).

5. **Shutdown behavior**: On `Drop`, receiver joins the worker thread. If worker is blocked on `send()`, it may take time to unwind. Consider adding a shutdown signal channel.

6. **Runtime dependency**: Uses `tokio::runtime::Builder::new_current_thread()` for async execution on worker. Adds tokio dependency (already present in project?).

## Alternative Considered (Rejected)

**Direct `Arc<Mutex<Rows>>` approach**: Would not work because `Mutex<T>` requires `T: Send`. Wrapping `!Send` in `Arc<Mutex<>>` doesn't help.

**`exec_future` + direct iterator**: Would work if Valtron iterators never crossed threads, but combinator methods require `Send` bounds.

**Channel-based with explicit thread**: This is the chosen approach - clean separation, handles !Send correctly, provides backpressure.

## Approval

Please review and approve this approach before implementation proceeds.

---
Created: 2026-03-28
Updated: 2026-03-28 (ThreadedFuture with channel-based bridging)
Status: pending_approval
