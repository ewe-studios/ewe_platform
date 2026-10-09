---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/12-testbed-native-harness/start.md"
feature_name: "12-testbed-native-harness"
created: 2026-06-10
---

# Start: Feature 12 — Testbed-Owned Native WASM Test Harness

## Why this feature exists

Spec-31 (`specifications/completed/31-wasm-testbed`) built `foundation_wasm_testbed` to
"replace the complexity of wasm-pack/wasm-bindgen-test-runner" — but it still **depends on
wasm-bindgen**: its `bindgen-*` modes shell out to the `wasm-bindgen` CLI to generate JS glue,
discover tests via the `__wbgt_` wasm-bindgen convention (walrus), and feature-02 wrote
`#[wasm_bindgen_test]` tests run wasm-pack-style. That contradicts spec-39's core principle:
**we own the WASM↔JS interop** (`foundation_wasm` ABI + `foundation-wasm.js` / `foundation-wasm-ui.js`).
A test harness for our framework must run on OUR runtime, not wasm-bindgen's.

This session proved the owned path end-to-end (no wasm-bindgen, no wasm-pack):
`foundation_wasm_e2e` cdylib → `{ abi: rt.web_abi }` (foundation-wasm.js) → `node:test`. This
feature generalises that into testbed-owned infrastructure.

## Workflow

1. Read `features.md` — gap analysis of spec-31 + the owned-harness design.
2. Read `.agents/skills/rust-clean-code/skill.md`.
3. Read decisions: 014 (execution modes), 016 (build pipeline) — and feature 10 (build-pipeline),
   which shares the wasm build step.
4. Read the proven seed: `integrations/nodejs/foundation-wasm/` (runtime + module + node tests) and
   `backends/foundation_wasm_testbed/src/{cli,main,build,deno,wasm,wasm_test}.rs`.
5. Implement incrementally, evolving `foundation_wasm_testbed` to own each piece as the JS runtime
   split (feature 00 / Task 5) grows — build the fixture, stage it, run the suite, report.

_Created: 2026-06-10_
