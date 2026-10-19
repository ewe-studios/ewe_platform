---
feature: "Parking bridges"
description: "Rewrite the four parking stream_future.rs bridges to require SI: WakeableStream, register a wake callback on the non-target waiting variants, and park on Successful (falling back to self-wake on NotSupported). Close the lost-wakeup race with register-then-recheck (Pipe's stash-then-recheck). Audit the cross-crate callers of the bridge constructors."
status: "completed"
priority: "high"
depends_on: ["02-wakeable-stream"]
estimated_effort: "medium"
created: 2026-10-18
last_updated: 2026-10-19
author: "Main Agent"
tasks:
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100
---

# Parking bridges

## Overview

The five bridge types (`StreamCollectFuture`, `StreamReadyFuture`, `StreamPendingFuture`,
`StreamAsFutureStream`, `StreamNextStream`) currently self-wake on every non-target
state. Four of them actually self-wake; `StreamAsFutureStream` is a **raw lens**
(`poll_next` always returns `Poll::Ready(iter.next())`, never `Pending`) so it never
parks and is left unchanged. This feature rewires the four parking bridges so that, on
the "waiting" variants, they register
`wake(|| cx.waker().wake_by_ref())` and — when the iterator returns `Successful` —
**park** (return `Poll::Pending` without self-waking). The producer's `push` fires
the callback → `queue_waker` pushes a `WakeToken` → `FutureTask`'s `Depends`
readiness flips ready → the task is requeued (`future_task.rs:297-306`). This is the
actual busy-loop elimination.

`NotSupported` keeps today's self-wake as the fallback, so a plain
`vec![Stream::Next(1)].into_iter().into_stream_iter()` or a buffering combinator
still resolves.

## Why the loop stays (Ignore skip)

`Stream::Ignore` is "consumed but not meaningful" — a `FilterDone`-dropped `Next`
(`non_sendable.rs:1414`) or a `TaskStatus::Spawn/Ignore/Depends` mapping
(`task.rs:402/406/417`). The bridge must keep pulling through `Ignore` and cannot
park on it: the item that produced it is gone, so no wake will come. The `loop`
therefore stays for `Ignore`; only the `Pending`/`Wait`/`Init`/`Delayed` arm changes.

## Register-then-recheck (lost-wakeup)

A producer may `push` between the bridge's `next()` (empty → `Wait`) and its
`wake(…)` registration; that push already fired the still-empty registry and will
not re-fire. Close it exactly as `Pipe` does (`pipe.rs:247-279` stash-then-recheck):
register the callback, **then re-check `next()` once**, and only park if the
re-check is still non-target.

Reference shape for the `StreamCollectFuture` arm (per-bridge targets differ):

```rust
Some(Stream::Pending(_) | Stream::Delayed(_) | Stream::Init | Stream::Wait) => {
    let waker = cx.waker().clone();
    match iter.wake(Box::new(move || waker.wake_by_ref())) {
        WakeResult::Successful => {
            // re-check once: a push between our last next() and wake() is caught here
            match iter.next() {
                Some(Stream::Next(v)) => this.collected.push(v), // loop continues
                Some(Stream::Ignore) => {}                       // loop continues
                Some(_) => return Poll::Pending,                 // park — cb is registered
                None => return Poll::Ready(std::mem::take(&mut this.collected)),
            }
        }
        WakeResult::NotSupported => {
            cx.waker().wake_by_ref(); // legacy self-wake
            return Poll::Pending;
        }
    }
}
```

The exact shape per bridge follows the same discipline; the "target" variant is the
only difference (`Next`/`Spread(Done)` for the Done-collectors, `Pending`/
`Spread(Pending)` for `StreamPendingFuture`).

## Tasks

1. Rewrite the four parking bridges in `stream_future.rs`
   (`StreamCollectFuture`, `StreamReadyFuture`, `StreamPendingFuture`,
   `StreamNextStream`) to bound `SI: WakeableStream` (the `StreamIterator` bound is
   implied by the super-trait) and apply the register-then-recheck + park/fallback
   pattern above. Keep the `Ignore` skip and the existing `Spread` draining logic.
   Leave `StreamAsFutureStream` (raw lens) on its `StreamIterator + Unpin` bound.
2. Update the four `StreamIteratorExt` constructors in
   `extensions/streams/sendable.rs` (`:614-641`) to add `Self: WakeableStream` to
   their `where` clauses, so `into_collect_future`/`into_ready_future`/
   `into_pending_future`/`into_next_stream` only compile on a `WakeableStream`
   iterator. Plain iterators reach them via `into_stream_iter()`. Leave
   `into_future_stream` (raw lens) unchanged.
3. Audit the cross-crate callers of the bridges and confirm each passes a
   `WakeableStream` iterator:
   - `collect_one` / `collect_result` (used by foundation_db, foundation_deployment_huggingface,
     foundation_nativeapis, foundation_connectrpc) — these are plain sync
     `Iterator::filter_map`/`find_map` helpers in `executors/sync.rs`; they do **not**
     build on the bridges, so no `where` change is needed.
   - connectrpc `head_obs.into_next_stream()` / `body_obs.into_next_stream()`
     (`ws.rs:161-162`, `h1.rs:147-148`) — `SplitUntilObserverMap`/
     `SplitCollectorMapObserver` carry empty `WakeableStream` impls (default
     `NotSupported`), so these fall back to self-wake (behavior-preserving).
   - d1_wasm `StreamReadyFuture::new(stream)` (`d1_wasm.rs:491,510`) — `stream` is a
     type-erased `Box<dyn Iterator<Item = Stream<…>>>` (not `WakeableStream`), so it
     was wrapped with `into_stream_iter()` (default `NotSupported` self-wake). **This
     is the one site that needed a rewrite.**
   - cf-valtron-counter `driven_iter.into_future_stream()` (`lib.rs:135`) — raw lens,
     unaffected.
   Any caller that turns out to pass a plain iterator must wrap with
   `into_stream_iter()` (self-wake) — record each such site (only d1_wasm needed it).
4. Update the stale self-wake docs in `doc.rs` (`:1196-1205`, `:1408-1410`) to
   describe register-then-park, and the `StreamIteratorExt` doc examples that call
   the constructors on plain iterators.
5. Add/extend tests (landed in `tests/valtron/bridge_park.rs`, 6 tests): (a) a
   producer `push` wakes a parked bridge (no self-wake spin); (b) `close` wakes a
   parked bridge to `None`; (c) a `NotSupported` iterator still resolves via
   self-wake; (d) a spurious wake (an item for another variant, e.g.
   `StreamPendingFuture` sees a `Next`) re-parks and re-registers correctly.
6. Build `foundation_core` and its cross-crate consumers (DB, connectrpc) under
   `multi` and `!multi`; run the full valtron suite.

## Out of scope

- The producer-side waker (`PipeSender::send` parking on a full pipe).
- Removing the `StreamIterator` blanket impl.
- Migrating the combinator layer off `ConcurrentQueue` (features 00–01).

## Success criteria

- The bridges do not self-wake on the `Pending`/`Wait`/`Init`/`Delayed` path when
  `wake()` returns `Successful`; they park and are woken by the producer.
- A `NotSupported` iterator and a plain `Vec<Stream>` (via `into_stream_iter()`)
  still resolve via self-wake.
- `execute(..).map_done(..).into_future_stream()` parks when supported.
- No cross-crate bridge caller regresses; `foundation_core` + DB + connectrpc
  compile and pass under `multi` and `!multi`.

## Skills

`.agents/skills/rust-valtron-usage/skill.md`, `.agents/skills/rust-valtron-iterator/skill.md`

---

_Created: 2026-10-18_
