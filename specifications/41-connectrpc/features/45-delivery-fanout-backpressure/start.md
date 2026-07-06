---
workspace_name: "ewe_platform"
spec_directory: "specifications/41-connectrpc"
this_file: "specifications/41-connectrpc/features/45-delivery-fanout-backpressure/start.md"
feature_name: "45-delivery-fanout-backpressure"
created: 2026-07-07
---

# Start: Delivery & Fan-out Backpressure (Decision 00 §L1b, in valtron's own queues)

## Workflow

1. Read `feature.md` (this feature) in full.
2. Read `decisions/00-valtron-async-readiness.md` §"Level 1b" + §00-F4 (the rule
   this feature enforces beyond the seam pipe).
3. Read the evidence sites before touching them:
   - `backends/foundation_core/src/valtron/executors/task_iters.rs` — `*ConsumingIter`
     `PushError::Full` arms (bare `Pending(None)` today).
   - `backends/foundation_core/src/valtron/executors/local.rs` — `NotifyQueue`
     (consumer-only condvar; add `queue()` Arc accessor).
   - `backends/foundation_core/src/valtron/extensions/tasks/sendable.rs` —
     `split_*` family + `force_push` (drop-oldest); `CollectorStreamIterator`.
   - `backends/foundation_core/src/valtron/task.rs` — `QueueVacancyReadiness`,
     `AnyReadiness` (reuse verbatim — already implemented).
4. Implement in order: **Part A** (delivery vacancy-park) → **Part C1**
   (split park, no `force_push`) → **Part C2** (`split_n`) → **Part D**
   (transport Pipe-shrink + `Result` payloads).
5. Prove the Decision 00 metric: **turn-count flat while a bounded queue is
   full** (park, not spin). Unbounded queues must be behaviourally unchanged.
6. Use `#[valtron_test]` for all pool tests — never `#[test]`/`#[serial]`
   (see `valtron/docs/debugging_multi_pool_test_hangs.md`).
7. Run `cargo test -p foundation_core --features multi` and the single-executor
   (wasm) tests; then `cargo test -p foundation_connectrpc` for Part D.

**Note:** Part B keeps the global `unbounded` default — do NOT flip it. Real
backpressure is opt-in via `channel_capacity`; the vacancy-park is inert on
unbounded queues, so Part A ships safely everywhere.

_Created: 2026-07-07_
