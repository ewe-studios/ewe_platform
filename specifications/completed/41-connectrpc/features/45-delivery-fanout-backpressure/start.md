---
workspace_name: "ewe_platform"
spec_directory: "specifications/41-connectrpc"
this_file: "specifications/41-connectrpc/features/45-delivery-fanout-backpressure/start.md"
feature_name: "45-delivery-fanout-backpressure"
created: 2026-07-07
updated: 2026-07-07
---

# Start: Delivery & Fan-out Backpressure (Decision 00 §L1b, in valtron's own queues)

## Workflow

1. Read `feature.md` (this feature) in full — note the Resolutions section was
   rewritten to match the session's final architecture (no Pipe in splits, `Wait`
   on observers, `readiness()` accessor, `into_next_stream()` adapter).
2. Read `decisions/00-valtron-async-readiness.md` §"Level 1b" + §00-F4 (the rule
   this feature enforces beyond the seam pipe).
3. Read the evidence sites before touching them:
   - `backends/foundation_core/src/valtron/extensions/tasks/{sendable,non_sendable}.rs` —
     task split family: C1 park is done (✅); observers need `Wait` fix (Resolution 2)
     + `readiness()` accessor (Resolution 3).
   - `backends/foundation_core/src/valtron/extensions/streams/{sendable,non_sendable}.rs` —
     stream split family: still `force_push` (❌) — needs stash+`Wait` (Resolution 4);
     observers need `Wait` fix + `readiness()` (Resolutions 2, 3).
   - `backends/foundation_core/src/valtron/stream_future.rs` — existing
     `into_ready_future()`/`into_pending_future()` bridges (read-only); add
     `into_next_stream()` (Resolution 5).
   - `backends/foundation_connectrpc/src/transport/base.rs` — `TransportError`
     needs `Clone` (Resolution 7: Arc-wrap `Io`/`Connect` variants).
   - `backends/foundation_netio/src/simple_http/client/shared/request_task.rs` —
     `HttpExchange::Failed` → `Arc<dyn Error + Send + Sync>` (Resolution 7).
   - `backends/foundation_connectrpc/src/transport/h1.rs` — Part D: head/body via
     `split_collect_until_map` + `split_collector_map` with `Result` payloads.
4. Implement in order:
   - **Resolution 2** — observer `Ignore → Wait` on empty-open (10 structs, 1 line each).
   - **Resolution 3** — `readiness()` accessor on all observers (mechanical).
   - **Resolution 4** — stream splits: `force_push` → stash + `Stream::Wait`
     (4 continuation types × 2 twins, 8 call sites).
   - **Resolution 5** — `into_next_stream()` adapter (new file or addition to
     `stream_future.rs` / `StreamIteratorExt`).
   - **Resolution 7** — `TransportError` Clone + `HttpExchange::Failed` Arc.
   - **Part D** — Transport head/body via `*_map` splits; `send_body` stays the
     one Pipe.
5. Prove the Decision 00 metric: **turn-count flat while a bounded queue is
   full** (park, not spin). Unbounded queues must be behaviourally unchanged.
6. Use `#[valtron_test]` for all pool tests — never `#[test]`/`#[serial]`
   (see `valtron/docs/debugging_multi_pool_test_hangs.md`).
7. Run `cargo test -p foundation_core --features multi` and the single-executor
   (wasm) tests; then `cargo test -p foundation_connectrpc` for Part D.

## What's already done (do not redo)

- **Part A:** delivery vacancy-park in all `*ConsumingIter`s.
- **Part B:** `sequenced` bounded queue + `DEFAULT_DELIVERY_CAPACITY`.
- **Part C1:** task-split `force_push` → `Depends(QueueVacancyReadiness)` +
  observer `Drop` impls.
- **F46 Parts A & B:** Mapper deleted, verb builders split into
  `builders/{mod,sendable,non_sendable}.rs`.

## Key architectural decisions (from session review)

- **No Pipe embedded in splits.** Task-path parking works via the executor's
  sleeper re-checking `EventReadiness::is_ready()` (Decision 00 §L1b). Async-path
  consumption works via the existing `into_ready_future()` bridge — it just needs
  observers to yield `Wait` instead of `Ignore` so the bridge yields to the
  executor instead of spinning inside `poll()`.
- **Channel element type stays `Stream<D, P>`.** `Pending` is in-band data; the
  `*_map` variants project what crosses, but the primitive never discards.
- **`send_body` stays the only actual `Pipe` in the transport.** The seam's
  `FramePipe` is also unchanged. No new pipe instances are created.

## Guardrails

- **No per-function `#[cfg]`.** The `#[cfg]` split is at module scope on the
  sendable/non_sendable twins (F46 already enforced this for builders; same
  pattern applies here).
- **Behaviour is frozen for already-landed parts.** Parts A, B, C1 (task park),
  and F46 A/B are done and tested. Only add, don't reshape.
- **Read the `rust-clean-code` skill before writing Rust** (repo standard); tests
  go in `tests/`, WHY/WHAT/HOW doc comments, imports at file top.

_Created: 2026-07-07_
