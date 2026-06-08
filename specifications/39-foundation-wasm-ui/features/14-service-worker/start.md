---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/14-service-worker/start.md"
feature_name: "14-service-worker"
created: 2026-06-09
---

# Start: Feature 14 — Service Worker Routing

## Workflow

1. Read `feature.md`
2. Read decision 017 in `decisions/`
3. Implement proc macro in `crates/foundation_wasm_ui/src/wasm_service/`
4. Run `cargo check -p foundation_wasm_ui 2>&1 | tee /tmp/cargo-check.log` (5-6 min)
5. Test with service worker
6. Update `LEARNINGS.md`

_Created: 2026-06-09_
