---
feature: "Waker → QueueReadiness bridge (00-F1)"
description: "FutureTask parks via Depends(QueueReadiness) instead of busy-polling with a no-op waker"
status: "complete"
priority: "high"
phase: 1
depends_on: []
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 01-waker-queue-bridge: Waker → QueueReadiness bridge (00-F1)

## Description

Replace FutureTask's no-op-waker busy-poll with the queue waker: wake() pushes a WakeToken onto the task's wake queue and Pending returns TaskStatus::Depends(QueueReadiness) so the executor parks. Apply to BOTH future_task.rs impls (single/wasm and multi/native); drain stale tokens before each poll (wake-before-park race). No timed fallback for non-waking futures (tokio/smol parity). Supersedes spec-50 feature 01-waker-queue-bridge.

## Normative sources (single source of truth — read before writing code)

- decisions/00-valtron-async-readiness.md — Level 1 (normative code sketch, race notes)
- backends/foundation_core/src/valtron/executors/future_task.rs (current busy-poll)
- backends/foundation_core/src/valtron/task.rs (EventReadiness/QueueReadiness/Depends)

## Scope

- WakeToken {id} + queue_waker() via RawWaker vtable (std/alloc only, no new deps)
- FutureTask gains wake_queue; next_status drains stale tokens then polls; Pending → Depends
- Both executors (single + multi) at parity, per Decision 00 resolution record

## Out of scope

- Reactor wiring (feature 10)
- Pipe mechanics (feature 02)

## Acceptance criteria

- A Pending future parks — turn count flat while blocked (Decision 00 success criteria)
- Wake-before-park stress test shows no lost wakeups
- Works on single and multi executors; wasm build unchanged

## Verification (complete)

Implemented in `backends/foundation_core/src/valtron/executors/future_task.rs`:
- `WakeToken { id }` (Copy) + `pub fn queue_waker(queue, id)` via a `RawWaker`/`RawWakerVTable`
  over `Arc<WakerState>` (std/alloc only, no new dep). `wake`/`wake_by_ref` push a token.
- `FutureTask` and `StreamTask` gained an unbounded `wake_queue`; `next_status` drains stale
  tokens, polls with `queue_waker`, and returns `Depends(QueueReadiness)` on `Pending`
  (both single/wasm and multi impls at parity).
- `CancellableFutureTask` composes its cancel flag into the park signal
  (`CancelOrReadiness` = inner ready OR cancelled, Decision 00 L1b) so a set cancel unparks a
  future that never wakes — otherwise the Pending→Depends change would park it forever.
- `get_noop_waker` re-gated to non-`multi` (only the inline single-threaded `FutureIterator`
  still uses it); `create_noop_waker` retained for the background-job pollers.

Tests (`backends/foundation_core/tests/valtron/waker_queue_bridge.rs`, plus updated
`futures_in_valtron.rs` / `cancellable_future.rs`):
- Park-not-spin: parked signal stays not-ready across 1000 checks, poll count flat at 1;
  pool end-to-end polls exactly twice (park + post-wake).
- Wake-before-park: 2000-iteration self-wake stress — the returned `Depends` signal is
  already ready, no lost wakeup.
- Single + multi: contract tests compile/run under both feature sets; pool test drives the
  real multi-threaded executor. wasm32 lib check passes.

Result: full valtron suite `237 passed; 0 failed` (was hanging before the cancel-compose
fix); lib clean under default + `multi`; `cargo check` clean on `wasm32-unknown-unknown`.
