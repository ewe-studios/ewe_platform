---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/11-scoped-styles-theme/start.md"
feature_name: "11-scoped-styles-theme"
created: 2026-06-09
---

# Start: Feature 11 — Scoped Styles & Theme

## Workflow

1. Read `feature.md`
2. Read decisions: 019, 020 in `decisions/`
3. Implement compile-time CSS transform + theme derive macro
4. Implement runtime fallback in `foundation-wasm-ui.js`
5. Run `cargo check -p foundation_wasm_ui 2>&1 | tee /tmp/cargo-check.log` (5-6 min)
6. Test in browser
7. Update `LEARNINGS.md`

_Created: 2026-06-09_
