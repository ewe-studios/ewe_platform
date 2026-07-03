---
feature: "Waker → QueueReadiness bridge (00-F1)"
description: "FutureTask parks via Depends(QueueReadiness) instead of busy-polling with a no-op waker"
status: "pending"
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
