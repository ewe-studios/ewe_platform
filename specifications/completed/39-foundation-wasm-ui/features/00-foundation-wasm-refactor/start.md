---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/00-foundation-wasm-refactor/start.md"
feature_name: "00-foundation-wasm-refactor"
created: 2026-06-09
---

# Start: Feature 00 — foundation_wasm Refactor & Split

## Workflow

1. Read `feature.md` — full file map with what stays, what moves, what splits
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read decisions: 014, 015, 022, 028, 030 in `decisions/`
4. Read `backends/foundation_wasm/src/` — every file reviewed
5. Read `backends/foundation_wasm/sdk/jsruntime/megatron.js` — current JS runtime
6. Move `frames.rs` → new `foundation_wasm_ui/src/wasm/animation.rs`
7. Split `jsapi.rs` → `host_runtime.rs`, `protocol.rs` in foundation_wasm
8. Create `foundation_wasm_ui` crate with initial structure
9. Rewrite `megatron.js` → `foundation-wasm.js` + `foundation-wasm-ui.js`
10. Run `cargo check -p foundation_wasm 2>&1 | tee /tmp/cargo-check.log` (5-6 min)
11. Run `cargo check -p foundation_wasm_ui 2>&1 | tee /tmp/cargo-check.log` (5-6 min)
12. Run all existing tests
13. Update `LEARNINGS.md`

_Created: 2026-06-09_