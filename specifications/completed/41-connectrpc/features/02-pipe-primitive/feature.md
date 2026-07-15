---
feature: "Pipe<T>/FramePipe + vacancy readiness + AnyReadiness (00-F4)"
description: "Two-sided waker-hooked bounded pipe; producer parking on full pipes; composite readiness"
status: "complete"
priority: "high"
phase: 1
depends_on: ["01-waker-queue-bridge"]
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 02-pipe-primitive: Pipe<T>/FramePipe + vacancy readiness + AnyReadiness (00-F4)

## Description

The generic bounded pipe primitive Pipe<T> (FramePipe = Pipe<Frame>): bounded ConcurrentQueue + two waker stashes (push wakes stashed consumer, pop wakes stashed producer) + QueueReadiness/QueueVacancyReadiness accessors + the AnyReadiness combinator. Async ends await; valtron-task ends return Depends. Bare Pending is never the answer to pipe-full/pipe-empty; awaits/parks compose the call's CancelSignal.

## Normative sources (single source of truth — read before writing code)

- decisions/00-valtron-async-readiness.md — Level 1b (normative)
- decisions/11-transport-seam.md — §Core primitive (Pipe<T>, layering)

## Scope

- QueueVacancyReadiness (ready = has capacity); AnyReadiness (one Depends, composite logic inside)
- Pipe<T> with PipeSender<T>/PipeReceiver<T> halves; waker stashes fired by the opposite end's push/pop
- Depth default 4, configurable; awaits/parks compose an external signal (cancellation)
- Replaces ConcurrentQueueStreamIterator's polling handoff wherever the seam uses a pipe

## Out of scope

- The Frame enum itself (feature 17 — a connectrpc crate type)

## Acceptance criteria

- Producer awaiting a full pipe parks and is woken by the consumer's pop (turn count flat while full)
- Consumer/empty case symmetric; AnyReadiness fires when any child is ready
- No max_turns/park_duration spinning in any pipe path

## Verification (complete)

- `backends/foundation_core/src/valtron/task.rs`: `QueueVacancyReadiness<T>` (ready = has
  capacity, closed-aware) and `AnyReadiness` (ORs `Arc<dyn EventReadiness>` children, one
  `Depends`); `QueueReadiness` made closed-aware too (symmetric — a consumer parked on an
  empty pipe unparks when the producer closes; no external users, FutureTask's wake queue is
  never closed so feature 01 is unaffected).
- `backends/foundation_core/src/valtron/pipe.rs`: `Pipe<T>` (`DEFAULT_DEPTH = 4`,
  `with_depth`) handing out owned `PipeSender<T>`/`PipeReceiver<T>` halves over a bounded
  `ConcurrentQueue<T>` + two single-slot waker stashes. `try_send`/`try_recv` (task path,
  fire the opposite waker) and async `send`/`receive` (stash-then-re-check to close the
  wake-before-park race). `vacancy()`/`readiness()` expose the task-path signals; drop or
  `close()` closes the pipe and wakes the parked opposite end. Error types
  `TrySendError`/`TryRecvError`/`SendError` (Debug/Display don't require `T: Debug/Display`).
- `Frame` enum is intentionally NOT here (feature 17, connectrpc crate); `FramePipe =
  Pipe<Frame>` and `Pipe<Bytes>` are instantiated there. Async-path CancelSignal wake wiring
  lands with feature 16/17; the task path composes cancellation today via `AnyReadiness`.

Tests (`backends/foundation_core/tests/valtron/pipe_primitive.rs`): producer-parks-on-full /
consumer-parks-on-empty verified with an observing `queue_waker` (fired token proves the
opposite end woke it; empty observation queue proves flat turn count); FIFO delivery;
sender-drop EOS + parked-consumer wake-on-close; vacancy/close readiness; AnyReadiness fires
on any child; 2000-iter send re-check race; and a multi-pool producer/consumer backpressure
roundtrip over a depth-3 pipe. Full valtron suite `245 passed; 0 failed`; wasm32 lib clean.
