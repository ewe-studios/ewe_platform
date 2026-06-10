---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/14-wasm-bindgen-interop-boundary/start.md"
feature_name: "14-wasm-bindgen-interop-boundary"
created: 2026-06-10
---

# Start: Feature 14 — wasm-bindgen interop boundary (minimal, opt-in)

## Why

Decision 031 makes owned infrastructure the default, but some third parties genuinely mandate
wasm-bindgen — notably **Cloudflare Workers via `worker-rs`** (heavily wasm-bindgen) and
**`foundation_db`** / SDKs that ship wasm-bindgen integrations. This feature defines the ONLY
sanctioned wasm-bindgen usage: small, isolated, explicitly opt-in, never in the shared
runtime/build/test backbone.

## Workflow

1. Read `features.md`.
2. Read decision 031 (the policy this enforces) and Feature 12 (the testbed's bindgen-* modes
   that become opt-in here).
3. Read `backends/foundation_wasm_testbed/src/{wasm,wasm_test}.rs` (existing wasm-bindgen CLI +
   `__wbgt_` discovery) and how worker-rs/foundation_db use wasm-bindgen today.
4. Implement: gate the bindgen path behind explicit opt-in; isolate it; document the seams.

_Created: 2026-06-10_
