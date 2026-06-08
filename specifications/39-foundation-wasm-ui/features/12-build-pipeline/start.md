---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/12-build-pipeline/start.md"
feature_name: "12-build-pipeline"
created: 2026-06-09
---

# Start: Feature 12 — Build Pipeline CLI

## Workflow

1. Read `feature.md`
2. Read decision 016 in `decisions/`
3. Create `crates/foundation_codegen/`
4. Run `cargo check -p foundation_codegen 2>&1 | tee /tmp/cargo-check.log` (5-6 min)
5. Test CLI with sample project
6. Update `LEARNINGS.md`

_Created: 2026-06-09_
