---
workspace_name: "ewe_platform"
spec_directory: "specifications/50-valtron-async-readiness"
this_file: "specifications/50-valtron-async-readiness/features/01-waker-queue-bridge/start.md"
feature_name: "01-waker-queue-bridge"
created: 2026-06-17
---

# Start: 01-waker-queue-bridge

## Workflow

1. Read `feature.md`
2. Read `specifications/50-valtron-async-readiness/requirements.md`
3. Read `backends/foundation_core/src/valtron/executors/future_task.rs` and `task.rs`
4. Read `.agents/skills/rust-valtron-usage/skill.md` (test with #[valtron_test], no #[serial])
5. Implement, using #[valtron_test] for pool tests
6. Run `cargo test -p foundation_core --features multi` and single-executor tests

_Created: 2026-06-17_
