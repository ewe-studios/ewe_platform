---
workspace_name: "ewe_platform"
spec_directory: "specifications/42-ui-component"
this_file: "specifications/42-ui-component/features/05-headless-components/start.md"
feature_name: "05-headless-components"
created: 2026-06-13
---

# Start: Feature 05 — Headless Components (the catalog)

## Workflow

1. Read `features.md` (the catalog overview), `machinery.md` (algorithms)
2. Read all `families/F1-primitives.md` through `F8-indicators-surfaces.md`
3. Read `styling/README.md` and the vendored CSS files
4. Read `.agents/skills/rust-clean-code/skill.md`
5. Read `specifications/42-ui-component/requirements.md`
6. Read feature 00 (slot composition), feature 03 (App), feature 04 (mount protocol)
7. Create `backends/foundation_ui_components/` crate
8. Build machinery first: M6 (field state) → M5 (roving/typeahead) → M3
   (dismiss/hover) → M1 (positioning) → M2 (layering) → M7 (transitions)
9. Then families in order: F1+F2 → F3 → F4 → F7 → F5 → F8 → F6
10. Each family: per-component acceptance tests (ARIA, keyboard, data-attr,
    morph, styled example)
11. Run `cargo check -p foundation_ui_components 2>&1 | tee /tmp/cargo-check.log`
12. Update `LEARNINGS.md`

**Dependencies:** Features 00, 03, 04 (done). Feature 01 (`<For>`) needed
before F6 (pickers). Feature 06 (auth UI) depends on all of this.

_Created: 2026-06-13 (supersedes initial-headlessui-start.md from spec-39)_
