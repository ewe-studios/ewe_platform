---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/03-html-templates/start.md"
feature_name: "03-html-templates"
created: 2026-06-05
---

# Start: Feature 03 — HTML Templates

## Workflow

1. Read `requirements.md` and `feature.md`
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read `backends/foundation_macros/` (existing proc macro patterns)
4. Read `backends/foundation_wasm/src/ops.rs` (DOM operation encoding)
5. Create `backends/foundation_wasm_ui/src/shared/template.rs`
6. Create `backends/foundation_wasm_ui/src/shared/template_macro.rs` (in foundation_macros or new crate)
7. Run `cargo check -p foundation_wasm_ui 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
8. Add tests per `feature.md` Testing section
9. Update `LEARNINGS.md`

_Created: 2026-06-05_