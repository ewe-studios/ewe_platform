---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/01-wasm-ui-core/start.md"
feature_name: "01-wasm-ui-core"
created: 2026-06-05
---

# Start: Feature 01 — WASM UI Core (Crate Split)

## Workflow

1. Read `requirements.md` and `feature.md`
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read `backends/foundation_wasm/src/` — identify all DOM-related code to move
4. Read `backends/foundation_wasm/runtime/` — identify JS code to split
5. Create `backends/foundation_wasm_ui/` crate with Cargo.toml and initial structure
6. Move DOM constants, animation, text cache from foundation_wasm to foundation_wasm_ui
7. Remove DOM-related code from foundation_wasm (frames.rs, DOM constants, invoke_for_dom)
8. Rewrite foundation-wasm.js — clean standard communication only
9. Create foundation-wasm-ui.js stub (DOM-specific code moved in Feature 06)
10. Update foundation_wasm Cargo.toml description
11. Run `cargo check -p foundation_wasm -p foundation_wasm_ui 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
12. Update `LEARNINGS.md`

_Created: 2026-06-05_