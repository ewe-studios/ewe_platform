---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/00-crate-split/start.md"
feature_name: "00-crate-split"
created: 2026-06-09
---

# Start: Feature 00 — Crate Split & JS Runtime Rewrite

## Workflow

1. Read `feature.md`
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read decision 015 in `decisions/`
4. Read `backends/foundation_wasm/src/` — full codebase
5. Move DOM-related modules to new `backends/foundation_wasm_ui/` crate
6. Rewrite `megatron.js` → `foundation-wasm.js` + `foundation-wasm-ui.js`
7. Run `cargo check -p foundation_wasm 2>&1 | tee /tmp/cargo-check.log` (5-6 min)
8. Run `cargo check -p foundation_wasm_ui 2>&1 | tee /tmp/cargo-check.log` (5-6 min)
9. Run all existing tests
10. Update `LEARNINGS.md`

_Created: 2026-06-09_
