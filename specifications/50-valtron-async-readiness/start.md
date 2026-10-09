---
workspace_name: "ewe_platform"
spec_directory: "specifications/50-valtron-async-readiness"
this_file: "specifications/50-valtron-async-readiness/start.md"
feature_name: "overview"
created: 2026-06-17
updated: 2026-06-17
---

# Start: Valtron Async Readiness

## Workflow

1. Read `requirements.md`
2. Read `backends/foundation_core/src/valtron/executors/future_task.rs` (the noop-waker busy-poll)
3. Read `backends/foundation_core/src/valtron/task.rs` (`EventReadiness`, `QueueReadiness`, `TaskStatus::Depends`)
4. Read `backends/foundation_core/src/valtron/executors/local.rs` (`Sleepable::Readiness` park/wake)
5. Read `backends/foundation_core/src/valtron/docs/debugging_multi_pool_test_hangs.md` (test discipline: `#[valtron_test]`, no `#[serial]`)
6. Read `.agents/skills/rust-valtron-usage/skill.md` and `.agents/skills/rust-clean-code/`
7. Implement F01 (waker→queue bridge), then F03 (async macros), then F02 (reactor seam)
8. Use `#[valtron_test]` for all pool tests — never `#[test]`/`#[serial]`
9. Run `cargo test -p foundation_core --features multi` and the single-executor (wasm) tests

**Note:** F02 only adds the trait seam + an in-tree test reactor — it does NOT
ship a native reactor (that's a future spec in a platform crate).

_Created: 2026-06-17_
