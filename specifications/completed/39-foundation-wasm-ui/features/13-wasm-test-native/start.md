---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/13-wasm-test-native/start.md"
feature_name: "13-wasm-test-native"
created: 2026-06-10
---

# Start: Feature 13 — Native `#[wasm_test]` discovery & result protocol

## Why

Owned replacement for `#[wasm_bindgen_test]` + `__wbgt_` discovery (decision 031). Our wasm
test cases run on OUR runtime (`foundation-wasm.js`) and report results over OUR ABI — no
wasm-bindgen. This is the test-execution half that Feature 12's runners drive.

## Workflow

1. Read `features.md`.
2. Read decision 031 (ownership policy) and Feature 12 (testbed runners that consume this).
3. Read `backends/foundation_macros/` (where `#[wasm_test]` lives — never a companion macro crate),
   `backends/foundation_wasm/src/host_runtime.rs` (exposed_runtime exports, web_abi imports), and the
   proven seed `integrations/nodejs/foundation-wasm/`.
4. Implement: macro → `__fwt_*` exports + manifest → result protocol → JS-side aggregation.

_Created: 2026-06-10_
