---
workspace_name: "ewe_platform"
spec_directory: "specifications/41-connectrpc"
this_file: "specifications/41-connectrpc/features/46-executor-verb-builders/start.md"
feature_name: "46-executor-verb-builders"
created: 2026-07-07
updated: 2026-07-07
---

# Start: Executor Verb-Builders (decompose the task builder; drop Mapper)

## Status

- **Parts A & B1-B3:** ✅ done. Mapper deleted, verb builders split into
  `builders/{mod,sendable,non_sendable}.rs`, both cfgs compile clean, call sites
  migrated.
- **Part B4 (fold multi/mod.rs):** pending. `ThreadPoolTaskBuilder`/`spawn2`
  still re-implement their own dispatch in `multi/mod.rs` (1689 lines, 11 refs
  to `ThreadPoolTaskBuilder`). Needs routing through the shared `builders/` verb
  builders.

## Workflow

1. Read `feature.md` (this feature) in full.
2. Read the twin-module pattern that Part B1-B3 already implemented:
   - `backends/foundation_core/src/valtron/executors/builders/mod.rs` — config
     stage + ungated spawn() + `as_*` transitions.
   - `backends/foundation_core/src/valtron/executors/builders/sendable.rs` —
     `#![cfg(feature = "multi")]` delivery + broadcast impls.
   - `backends/foundation_core/src/valtron/executors/builders/non_sendable.rs` —
     `#![cfg(not(feature = "multi"))]` delivery + broadcast-fallback impls.
3. Read `multi/mod.rs` — the `ThreadPoolTaskBuilder` and its dispatch methods
   (`schedule_iter`, `schedule`, `spawn`) that need to be routed through the
   shared verb builders.
4. Implement: Part B4 — fold `multi/mod.rs`'s builder onto the shared path.
   The shared verb builders already exist; `ThreadPoolTaskBuilder` should
   delegate to them rather than re-implementing dispatch.
5. Iterate with fast checks (both cfgs):
   - `CARGO_TERM_COLOR=never cargo check -p foundation_core 2>&1 | tee /tmp/single.log`
   - `CARGO_TERM_COLOR=never cargo check -p foundation_core --features multi 2>&1 | tee /tmp/multi.log`
   - Grep exit status explicitly — a `| tee | tail` pipeline masks cargo's exit code.
6. Run valtron tests on **both** cfgs; `#[valtron_test]`, never `#[test]`/`#[serial]`.

## Guardrails

- **No per-function `#[cfg]` in the builder modules.** The only `#[cfg]` is the
  module-scope `#![cfg]` on the twins and `#[cfg]` `pub use` in `mod.rs`.
- **Behaviour is frozen.** Dispatch semantics, Feature 45 Part B queue bounding,
  and the vacancy-park are unchanged.
- **Read the `rust-clean-code` skill before writing Rust** (repo standard);
  tests in `tests/`, WHY/WHAT/HOW docs, imports at file top.

_Created: 2026-07-07_
