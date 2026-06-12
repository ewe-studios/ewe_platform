---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/08-headless-ui-components/start.md"
feature_name: "08-headless-ui-components"
created: 2026-06-05
---

# Start: Feature 08 — Headless UI Components

## Workflow

1. Read `requirements.md` and `feature.md`
2. Read `.agents/skills/rust-clean-code/skill.md`
3. Read Feature 03 output (html! macro, template system)
4. Read Feature 05 output (Component trait, web component registration)
5. Create `backends/foundation_ui_components/` crate
6. Start with simplest components: Button, Switch, Checkbox
7. Progress to complex: Dialog (focus trap), Combobox (filtering), Menu (roving tabindex)
8. Run `cargo check -p foundation_ui_components 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)
9. Update `LEARNINGS.md`

**Dependencies:** Requires Features 01-06 (foundation_wasm_ui core) to be completed first.

_Created: 2026-06-05_