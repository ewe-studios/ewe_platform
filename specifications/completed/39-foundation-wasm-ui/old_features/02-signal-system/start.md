---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/02-signal-system/start.md"
feature_name: "02-signal-system"
created: 2026-06-05
---

# Start: Feature 02 — Signal System

## Workflow

1. Read `requirements.md` and `feature.md`
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read `backends/foundation_wasm/src/jsapi.rs` (ExternalPointer, host_invoke)
4. Read `backends/foundation_wasm/src/ops.rs` (batch operation pattern)
5. Create `backends/foundation_wasm_ui/src/shared/signal.rs`
6. Run `cargo check -p foundation_wasm_ui 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
7. Add tests per `feature.md` Testing section
8. Update `LEARNINGS.md`

_Created: 2026-06-05_