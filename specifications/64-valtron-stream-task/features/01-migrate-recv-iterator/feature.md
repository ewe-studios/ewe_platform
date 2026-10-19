---
feature: "Migrate RecvIterator to NotifyQueue"
description: "Replace ThreadedIterFuture's worker-channel RecvIterator (mpp park-timeout spin) with NotifyQueue/NotifyRecvIterator (condvar wake) — same bounded-channel + backpressure + close semantics, no 100 ms park latency."
status: "complete"
priority: "medium"
depends_on: ["00-cleanup"]
estimated_effort: "small"
created: 2026-10-18
last_updated: 2026-10-19
author: "Main Agent"
tasks:
  completed: 7
  uncompleted: 0
  total: 7
  completion_percentage: 100
---

# Migrate RecvIterator to NotifyQueue

## Overview

`ThreadedIterFuture::execute()` (`multi`) bridges a `!Send` future's iterator
across a thread boundary: the worker thread drains `I: Iterator<Item = Result<T, E>>`
and pushes items into a bounded channel; the caller pulls them through a blocking
`Iterator<Item = ThreadedValue<T, E>>`. Today that channel is `mpp::bounded` +
`RecvIterator` (`synca/mpp.rs`), whose receive path is a `park_timeout`/`yield_now`
spin with a 100 ms granularity (`into_recv_iter` = `from_hundred_millis`).

`NotifyQueue` + `NotifyRecvIterator` (`executors/local.rs`) deliver the same
bounded-channel + backpressure + close semantics but replace the spin with a
condvar: `push` fires `notify_one`, the consumer blocks in `condvar.wait_timeout`.
The migration is contained to the `multi`-only `ThreadedIterFuture` path.

## Why not Pipe

`Pipe<T>` (`pipe.rs`) is the wrong layer here. It is a *waker-hooked* seam for
cooperating valtron tasks / async handlers: its only wait surfaces are
`try_send`/`try_recv` (non-blocking), `send`/`receive` (async futures that park by
stashing a `Waker`), and `readiness`/`vacancy` (valtron `Depends`). It has **no
blocking receive and no `Iterator` impl**, so it cannot park a raw `std::thread`.
`ThreadedIterFuture`'s consumer is a synchronous thread pulling a plain `Iterator`
(no executor, no poll context), and the producer is a raw worker thread that cannot
`await` a future. Driving `try_recv` in a `park_timeout` loop re-implements
`RecvIterator` while the producer's `wake_consumer()` (a `Waker`, not a condvar)
finds no stashed waker — a pure spin with no cross-thread wakeup. `NotifyQueue`'s
condvar is exactly the *thread-blocking* notification this sync bridge needs.

## Migration scope (multi only)

1. `FutureIterator<T, E>` (`future_task.rs:676`) — swap `mpp::RecvIterator<ThreadedValue<T,E>>`
   for `NotifyRecvIterator<ThreadedValue<T,E>>`.
2. `impl Iterator for FutureIterator` (`:681`) — map `NotificationItem::Ready(v) → Some(v)`,
   `NotificationItem::None → Some(ThreadedValue::Waiting)`, closed `None` → `None`.
3. `ThreadedIterFuture::execute()` (`:738`) — `mpp::bounded` → `Arc::new(NotifyQueue::bounded(...))`;
   worker `sender.send` loop → `queue.push` mapping `PushError::Full`/`Closed`;
   `sender.close()` → `queue.close()`; `receiver.into_recv_iter()` →
   `NotifyRecvIterator::from_notify_queue(queue.clone(), wait)`.
4. Imports: drop `use crate::synca::mpp::{self, SenderError};` (`:34`); add
   `use super::local::{NotifyQueue, NotifyRecvIterator, NotificationItem};` and
   `concurrent_queue::PushError`.

## Semantic note — `Waiting` now yields on the multi path

The current `RecvIterator` blocks until an item or close, so the `multi` path never
actually emits `ThreadedValue::Waiting` (only the single-threaded path does, on
`Poll::Pending`). `NotifyRecvIterator` returns `NotificationItem::None` after
`max_spins` condvar timeouts, so the `multi` path will now yield
`ThreadedValue::Waiting` between values — matching the single-threaded contract
("single unified API", `sendables.rs:432`). DB consumers already handle
`Waiting` (`Stream::Pending` / `continue`), so this unifies rather than breaks.

Granularity: on native `DEFAULT_NOTIFY_QUEUE_MAX_SPINS = 100` and a 10 ms wait
timeout, `wait_for_item` yields `NotificationItem::None` only after ~1 s of a
continuously-empty queue, so `Waiting` is a ~1 s heartbeat (not per-item).
`close()` does not `notify_one`, so a parked consumer observes close on the next
`wait_timeout` expiry (≤ 10 ms), not after the full `max_spins` budget — close
stays fast.

## Tasks

1. Swap `FutureIterator` (multi) to wrap `NotifyRecvIterator<ThreadedValue<T,E>>`
   and map `NotificationItem` → `ThreadedValue`.
2. Rewrite `ThreadedIterFuture::execute()` (multi) to build an
   `Arc<NotifyQueue<ThreadedValue<T,E>>>`, push from the worker, and hand out a
   `NotifyRecvIterator`; keep the existing backpressure-sleep loop on `Full`.
3. Fix imports in `future_task.rs` (drop `mpp`/`SenderError`, add
   `local::{NotifyQueue, NotifyRecvIterator, NotificationItem}` + `PushError`).
4. Choose the condvar `wait` timeout (the `from_notify_queue` `dur`) — a backstop
   only; the real wake is `notify_one`. Reuse the existing `backpressure_sleep`
   default (10 ms) or a short constant; do not keep 100 ms.
5. Update the `multi` tests (`tests/valtron/threaded_future.rs`): three tests map
   `ThreadedValue::Waiting => unreachable!("sync iteration should not yield Waiting")`
   — `test_threaded_future_future_error` (`:39`), `test_threaded_future_empty_iterator`
   (`:56`), and `test_threaded_future_backpressure` (`:110`). Once `Waiting` is a
   legitimate item, convert all three to the tolerant `filter_map` pattern already
   used by `test_threaded_future_basic` (`:18-21`) and
   `test_threaded_future_custom_queue_size` (`:76-79`). Add one deterministic
   `Waiting` case: an inner iterator that sleeps ~1.2 s between items, so the queue
   drains empty past the native `max_spins = 100 × 10 ms` budget and the consumer
   provably yields `Waiting` before the next value (pins the new contract rather
   than leaving it timing-dependent).
6. Grep to confirm `RecvIterator`/`into_recv_iter` have zero remaining non-test
   callers in `foundation_core`; leave `synca/mpp.rs` (Broadcaster/Receiver/
   recv_timeout still used cross-crate) and `RecvIterator`'s own tests intact.
7. Build `foundation_core` under `multi` and `!multi`; run `threaded_future`,
   `threaded_future_std`, and the db-backend tests that exercise `run_future_iter`.

## Out of scope

- Removing `RecvIterator`/`mpp` entirely (module stays; only this caller migrates).
- Migrating the combinator layer or any `Stream<D,P>` path to `Pipe`/`NotifyQueue`.
- Changing the public `run_future_iter` / `ThreadedValue` contract.

## Success criteria

- `ThreadedIterFuture` (multi) uses `NotifyQueue`/`NotifyRecvIterator`; no
  `mpp::bounded`/`into_recv_iter` in the execute path.
- Item handoff is condvar-driven: no 100 ms `park_timeout` floor.
- `foundation_core` + DB backends compile and pass under `multi` and `!multi`.

## Landed implementation

- `FutureIterator<T, E>` (`future_task.rs`) now wraps
  `NotifyRecvIterator<ThreadedValue<T, E>>`; `next()` maps
  `NotificationItem::Ready(v) → Some(v)`, `NotificationItem::None →
  Some(ThreadedValue::Waiting)`, closed `None → None`.
- `ThreadedIterFuture::execute()` (`multi`) builds
  `Arc::new(NotifyQueue::bounded(queue_size))`, pushes from the worker via
  `worker_queue.push(...)` (mapping `PushError::Full` → backpressure sleep/retry,
  `PushError::Closed` → stop), closes with `worker_queue.close()`, and returns
  `NotifyRecvIterator::from_notify_queue(queue, DEFAULT_NOTIFY_QUEUE_WAIT_TIMEOUT)`.
- Imports: dropped `crate::synca::mpp::{self, SenderError}`; added
  `crate::valtron::executors::local::{NotificationItem, NotifyQueue,
  NotifyRecvIterator}` + `DEFAULT_NOTIFY_QUEUE_WAIT_TIMEOUT`, and
  `concurrent_queue::PushError`.
- Tests (`tests/valtron/threaded_future.rs`): the three `Waiting ⇒ unreachable!`
  tests converted to tolerant `filter_map`; added
  `test_threaded_future_yields_waiting_on_empty_queue` (a ~1.2 s sleep between
  items drains the queue past the native `max_spins` budget and pins the new
  `Waiting` heartbeat). **Also removed the stacked `#[traced_test]`** on every
  test — `#[valtron_test]` already installs a subscriber, and `traced_test`'s
  `set_global_default().expect(..)` panics with `SetGlobalDefaultError` once any
  earlier test in the binary has set the global default (see LEARNINGS.md).
- Verified: `cargo check` under `multi` and `!multi`; `threaded_future` 6/6 pass;
  `foundation_db --features libsql` compiles and `libsql_generic_instantiation_compiles`
  passes (the other `libsql_storage_tests` fns are dead code — no `#[test]` —
  a pre-existing gap, not this feature).

