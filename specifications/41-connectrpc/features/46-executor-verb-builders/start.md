---
workspace_name: "ewe_platform"
spec_directory: "specifications/41-connectrpc"
this_file: "specifications/41-connectrpc/features/46-executor-verb-builders/start.md"
feature_name: "46-executor-verb-builders"
created: 2026-07-07
---

# Start: Executor Verb-Builders (decompose the task builder; drop Mapper)

## Workflow

1. Read `feature.md` (this feature) in full.
2. Read the twin-module pattern you are copying, before touching anything:
   - `backends/foundation_core/src/valtron/executors/mod.rs` — how
     `sendables`/`non_sendables` are `#[cfg]`-gated and re-exported.
   - `backends/foundation_core/src/valtron/executors/sendables.rs` (head) — the
     `#![cfg(feature = "multi")]` module-scope gate.
   - `backends/foundation_core/src/valtron/extensions/tasks/sendable.rs` —
     `TaskIteratorExt` (`map_ready`/`map_pending`/`filter_ready`/…), the upstream
     transforms that replace Mapper.
3. Read the surfaces you are reshaping:
   - `executors/builders.rs` — `ExecutionTaskIteratorBuilder`, `spawn_builder`,
     `spawn_broadcaster`, `on_next*`.
   - `executors/multi/mod.rs` — `ThreadPoolTaskBuilder` / `spawn2`.
   - `executors/task_iters.rs` — `*ConsumingIter` (`mappers` field + loop).
   - `task.rs` — `ExecutionEngine` verbs, `GlobalTask`, `EventReadinessPtr`,
     `TaskStatusMapper` & friends.
4. Implement in order:
   - **Part A first (Mapper deletion).** Removing the type parameter shrinks
     every builder signature you are about to move, so do it before the split.
     Audit `OnNext`/pool paths for residual `map()` use; then delete
     `TaskStatusMapper`, `FnMapper`, `FnOptionMapper`, `ZeroMapping`, the
     `IntoBoxed*`/`Boxed*` aliases; drop `mappers` from the three `*ConsumingIter`s
     and their `new(...)`.
   - **Part B (verb split).** `builders.rs` → `builders/mod.rs` (`TaskSpawnConfig`
     + `as_*` transitions + `on_next*`); add `builders/sendable.rs`
     (`#![cfg(multi)]`) and `builders/non_sendable.rs` (`#![cfg(not multi)]`) with
     the four verb builders × uniform `spawn/recv/stream/stream_with_config`.
   - **Part B4.** Fold `multi/mod.rs`'s `ThreadPoolTaskBuilder`/`spawn2` onto the
     shared verb builders.
   - **Migration.** Update `actions.rs`, the `sendables`/`non_sendables` wrappers,
     and the test call sites to the `as_<verb>().{spawn,recv,stream}` surface.
5. Iterate with fast checks (both cfgs), per the repo check workflow:
   - `CARGO_TERM_COLOR=never cargo check -p foundation_core 2>&1 | tee /tmp/single.log`
   - `CARGO_TERM_COLOR=never cargo check -p foundation_core --features multi 2>&1 | tee /tmp/multi.log`
   - Grep exit status explicitly — a `| tee | tail` pipeline masks cargo's exit
     code (learned the hard way on Feature 45).
6. Run the valtron tests on **both** cfgs; use `#[valtron_test]`, never
   `#[test]`/`#[serial]` (see `valtron/docs/debugging_multi_pool_test_hangs.md`).

## Guardrails

- **No per-function `#[cfg]` in the builder modules.** The only `#[cfg]` allowed
  is the module-scope `#![cfg]` on the twins and the `#[cfg]` `pub use` in
  `mod.rs`. If you find yourself gating a method, the method is in the wrong file.
- **Behaviour is frozen.** Dispatch semantics, Feature 45 Part B queue bounding
  (`sequenced` bounded / `lift`+`schedule` unbounded), and the vacancy-park are
  unchanged. This is a surface reshape.
- **Read the `rust-clean-code` skill before writing Rust** (repo standard); tests
  go in `tests/`, WHY/WHAT/HOW doc comments, imports at file top.

_Created: 2026-07-07_
