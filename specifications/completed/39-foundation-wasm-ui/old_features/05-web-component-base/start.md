---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/05-web-component-base/start.md"
feature_name: "05-web-component-base"
created: 2026-06-05
---

# Start: Feature 05 — Web Component Base

## Workflow

1. Read `requirements.md` and `feature.md`
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read `backends/foundation_wasm/src/jsapi.rs` (host_invoke, ExternalPointer)
4. Read Feature 01 output (Component trait, ComponentHost)
5. Create `backends/foundation_wasm_ui/src/wasm/custom_elements.rs`
6. Update JS runtime (Feature 06 output) with registerElement function
7. Run `cargo check -p foundation_wasm_ui 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
8. Add tests per `feature.md` Testing section
9. Update `LEARNINGS.md`

_Created: 2026-06-05_