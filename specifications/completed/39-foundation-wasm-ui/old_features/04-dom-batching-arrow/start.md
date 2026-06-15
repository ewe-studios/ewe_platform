---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/04-dom-batching-arrow/start.md"
feature_name: "04-dom-batching-arrow"
created: 2026-06-05
---

# Start: Feature 04 — DOM Batching with Arrow Format

## Workflow

1. Read `requirements.md` and `feature.md`
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read `backends/foundation_wasm/src/ops.rs` (existing batch encoding pattern)
4. Read `backends/foundation_wasm/src/jsapi.rs` (host_batch_apply, host_batch_returning_apply)
5. Create `backends/foundation_wasm_ui/src/shared/arrow/mod.rs`
6. Create `backends/foundation_wasm_ui/src/shared/arrow/schema.rs`
7. Create `backends/foundation_wasm_ui/src/shared/arrow/encode.rs`
8. Run `cargo check -p foundation_wasm_ui 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
9. Add tests per `feature.md` Testing section
10. Update `LEARNINGS.md`

_Created: 2026-06-05_