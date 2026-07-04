---
feature: "#[valtron]/#[valtron_test] accept async fn (00-F3)"
description: "Async entrypoint/test macro support via block_on_future; sync path untouched"
status: "complete"
priority: "high"
phase: 1
depends_on: ["01-waker-queue-bridge"]
estimated_effort: "small"
created: 2026-07-03
---
# Feature 03-async-valtron-macros: #[valtron]/#[valtron_test] accept async fn (00-F3)

## Description

#[valtron] / #[valtron_test] accept async fn bodies by wrapping them in block_on_future (from_future + run-to-completion). Enables plain-async handlers and conformance tests. Supersedes spec-50 feature 03-async-valtron-macros.

## Normative sources (single source of truth — read before writing code)

- decisions/00-valtron-async-readiness.md — Level 3 (normative expansion sketch)
- backends/foundation_macros/src/valtron_entry.rs

## Scope

- block_on_future<F> in foundation_core (single/wasm cfg variant without Send)
- Macro branches on sig.asyncness; ?/return semantics preserved; sync expansion byte-for-byte unchanged

## Acceptance criteria

- #[valtron_test] async fn compiles and runs on single and multi; sync forms unchanged

## Verification (complete)

- `block_on_future<F>` added in both executor surfaces:
  - `sendables.rs` (multi): `sync_collect_one(from_future(future))` — schedules the future
    on the running pool and blocks the caller on the result channel (future parks per F01).
  - `non_sendables.rs` (single/wasm): drives `drive_future(future)` to its `Ready` via the
    single-threaded `DrivenTaskIterator` (`run_until_next_state` between polls).
- `foundation_macros/src/valtron_entry.rs`: removed the `async fn` rejection; the wrapper fn
  is now always sync (asyncness stripped from the re-emitted signature). Sync bodies keep the
  inner-fn expansion byte-for-byte; async bodies expand to
  `block_on_future(async move #block)`, so `.await` parks on the engine and `?`/`return` exit
  the future (whose output is the fn's return value). Works for both the timeout and
  non-timeout paths and for `#[valtron]` + `#[valtron_test]`.
- Tests (`tests/valtron/valtron_macro_tests.rs`): `#[valtron_test] async fn` awaiting a ready
  future and a self-waking `YieldOnce` (parks then completes — exercises F01 under the macro);
  async `?`/`return` with a `Result` body; async `#[valtron]` entry point returning an awaited
  value. All existing sync macro tests still pass (expansion unchanged).

Result: full valtron suite `248 passed; 0 failed`. Single-threaded `block_on_future` and the
async expansion are compile-verified on default (non-`multi`) and `wasm32-unknown-unknown`;
runtime execution runs on the `multi` path because this workspace unconditionally unifies
`foundation_core/multi` on for the test build (the single/wasm impls are `cfg`-excluded there).
