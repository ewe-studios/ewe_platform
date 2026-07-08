# Smart Notification and the Stream / Task Split

## Two execution paths

Valtron has two distinct execution paths, and they have fundamentally different
relationships with the executor:

| | Task path | Stream path |
|---|---|---|
| **Core type** | `TaskIterator` → yields `TaskStatus` | `StreamIterator` → yields `Stream<D, P>` |
| **Relationship to executor** | **Inside** — the executor directly runs tasks | **Outside** — a consumer pulls output from a channel filled by tasks |
| **Driven by** | `ExecutorState::do_work()` calls `ExecutionIterator::next()` | `DrivenStreamIterator::next()` calls `run_until_next_state()` then pops the output queue |
| **Smart parking** | Yes — `TaskStatus::Depends(signal)` | No — `Stream::Wait` can only yield and retry |

## Why `TaskStatus::Depends` works

When a task returns `TaskStatus::Depends(signal)`:

1. The executor registers the task as a `Sleepable::Readiness(signal, entry)`.
2. The task is removed from the processing queue — it is **parked**.
3. The executor moves on to the **next task** in the queue.
4. That other task (or a different task entirely) runs, produces data, pushes it into
   a `ConcurrentQueue`.
5. The queue's `CondVar` fires → `QueueReadiness::is_ready()` returns `true` →
   the sleeper matures → the parked task is re-scheduled.

The executor **never stops**. It just parks *one task* while running *others*.
The other tasks are what eventually fire the readiness signal. This is the same
pattern Tokio, smol, and rayon use: park a worker, not the whole pool.

```
┌──────────────────────────────────────────────────┐
│ Executor                                         │
│                                                  │
│  Task A: returns Depends(queue_readiness) ──┐    │
│    → parked (Sleepable::Readiness)           │    │
│                                              │    │
│  Task B: runs, produces data                 │    │
│    → pushes to ConcurrentQueue ──fires───────┘    │
│    → CondVar notifies → Task A wakes              │
│                                                  │
│  Task C: runs independently                      │
└──────────────────────────────────────────────────┘
```

## Why `Stream::Wait` can't do the same

The `StreamIterator` is the **consumer side** of a task's output:

```
[Executor running Task A] → output queue → [DrivenStreamIterator pulling Stream<D,P>]
```

When `DrivenStreamIterator` calls `next()` and the output queue is empty, it
yields `Stream::Wait`. At that point the consumer calls `run_until_next_state()`
to advance the executor — because the executor **is** the producer that will
fill that queue.

If the consumer were to park the executor (register a sleeper and stop driving
it), **nobody would run the producer task** that fills the queue and fires the
readiness signal. You'd deadlock:

```
Consumer: "queue is empty, park the executor until data arrives"
Executor: [parked, no tasks running]
Producer: [can't run — executor is parked]
→ Deadlock. Nobody wakes up.
```

The consumer *cannot* park the executor because the consumer **depends on the
executor to produce the data it's waiting for**. The producer and the executor
are the same thing.

## The WASM / JS exception

In a JS environment (wasm32 with `js-wasmbindgen` or `js-foundation-wasm`),
yielding does **not** block the only thread — it returns control to the
browser's event loop:

```rust
// In LocalThreadExecutor::run_until and block_until_finished:
#[cfg(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))]
if matches!(&response, ProgressIndicator::NoWork) {
    break;  // Return to JS event loop — browser can schedule timers, resume us later
}
```

The browser's event loop is **external** to valtron. The JS runtime can fire a
`setTimeout`, process a fetch response, or handle a DOM event, then call back
into the valtron executor. From valtron's perspective, this looks like a
cooperative yield to an external scheduler — not parking while waiting for
itself.

This is the **only case** where a stream consumer can "park" without deadlock,
because the thing that will wake it is outside the executor (the browser's timer
or I/O system). In native code, there is no external event loop — the valtron
executor *is* the event loop.

## The `readiness()` bridge

Several `StreamIterator` types expose a `readiness()` method that returns a
`QueueReadiness` signal:

- `SCollectorStreamIterator::readiness()`
- `SSplitUntilObserver::readiness()`
- `SSplitCollectorMapObserver::readiness()`

These are **not** for use by stream combinators. They are hooks for the **task
path**. The intended usage:

```rust
// Wrap a stream consumer as a TaskIterator that uses Depends:
struct StreamAsTask<S: StreamIterator> {
    stream: S,
}

impl<S: StreamIterator> TaskIterator for StreamAsTask<S> {
    type Ready = S::D;
    type Pending = S::P;
    type Spawner = NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.stream.next()? {
            Stream::Next(v) => Some(TaskStatus::Ready(v)),
            Stream::Pending(p) => Some(TaskStatus::Pending(p)),
            Stream::Delayed(d) => Some(TaskStatus::Delayed(d)),
            Stream::Wait => {
                // Park THIS TASK on the observer's queue readiness.
                // The executor keeps running other tasks that will fill the queue.
                Some(TaskStatus::Depends(self.stream.readiness()))
            }
            Stream::Init | Stream::Ignore => Some(TaskStatus::Pending(/* ... */)),
            Stream::Spread(items) => { /* ... */ }
        }
    }
}
```

This works because the stream consumer is now **inside the executor** as a task.
When it parks on `Depends`, the executor is still running other tasks — including
the producer that fills the queue. The bridge converts the outer (stream) position
into an inner (task) position.

## What `NotifyQueueStreamIterator` already does

`NotifyQueueStreamIterator` (used under `feature = "multi"`) wraps the output
`ConcurrentQueue` with a `CondVar` and blocks efficiently:

```rust
// In NotifyQueueStreamIterator::next():
match self.chan.wait_for_item(self.park_duration) {
    Some(NotificationItem::Ready(value)) => Some(value),
    Some(NotificationItem::None) => Some(Stream::Wait),  // gave up after max_spins
    None => None,
}
```

The `wait_for_item` call uses `CondVar::wait_timeout` to block the **driver
thread** (not the executor — under `multi` the executor runs on pool threads).
After `max_spins` attempts, it yields `Stream::Wait` so the driver retries. This
is efficient for the multi-threaded case because the pool threads are still
running independently.

Under `not(multi)`, the `NotifyQueueStreamIterator` is not used — `DrivenStreamIterator`
drives the single-threaded executor directly.

## The deliberate trade-off

The stream path's inability to park is **not a missing feature** — it follows
directly from the producer/consumer relationship:

| What you want | How to get it |
|---|---|
| Park a consumer while a producer works | Wrap the stream consumer as a `TaskIterator` that uses `TaskStatus::Depends(observer.readiness())`. Let the executor interleave the producer and the parked consumer. |
| Poll a stream from outside the executor | Accept that the consumer must keep driving the executor. Use `Stream::Wait` to yield cooperatively. On JS, the browser event loop provides natural yielding. On native, the idle-manager provides exponential-backoff yielding. |
| Efficient blocking on an empty output queue | Under `multi`, `NotifyQueueStreamIterator` already uses `CondVar::wait_timeout` internally — the driver thread blocks on the OS. Under `not(multi)`, there is no other thread to hand off to; cooperative yielding is the only option. |

This is the same fundamental constraint every single-threaded event loop faces:
the event loop can't park itself waiting for work it's supposed to produce.
JavaScript's event loop, Python's asyncio, and Node.js all have the same
property — they yield to the host (browser, OS I/O) rather than parking
internally.

## Related docs

- [`executors/README.md`](../executors/README.md) — lift/schedule/distribute semantics
- [`docs/thread_looping.md`](./thread_looping.md) — sleeper lifecycle and parking patterns
- [`docs/yield_now.md`](./yield_now.md) — `yield_now` vs `CondVar` sleeping strategies
- [`docs/non_send.md`](./non_send.md) — `!Send` types and `LocalExecutor` design
