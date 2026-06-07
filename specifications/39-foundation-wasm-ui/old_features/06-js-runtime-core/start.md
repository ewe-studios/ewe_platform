---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/06-js-runtime-core/start.md"
feature_name: "06-js-runtime-core"
created: 2026-06-05
---

# Start: Feature 06 — JS Runtime Core

## Workflow

1. Read `requirements.md` and `feature.md`
2. Read `backends/foundation_wasm/runtime/` (existing JS runtime)
3. Read Feature 04 output (Arrow format spec)
4. Create `backends/foundation_wasm_ui/runtime/foundation-wasm-ui.js`
5. Create `backends/foundation_wasm_ui/runtime/foundation-wasm-ui.mjs`
6. Implement ArrowParser, ArrowDomApplicator, SignalBridge, ComponentRegistry
7. Run browser test (manual or automated) to verify Arrow batch application
8. Update `LEARNINGS.md`

_Created: 2026-06-05_