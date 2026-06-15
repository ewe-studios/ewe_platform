---
workspace_name: "ewe_platform"
spec_directory: "specifications/39-foundation-wasm-ui"
this_file: "specifications/39-foundation-wasm-ui/features/17-abi-function-call-codec/start.md"
feature_name: "17-abi-function-call-codec"
created: 2026-06-10
---

# Start: Feature 17 — WASM↔JS function-call ABI codec (faithful two-way port)

## Why

Completing the F00 JS runtime split needs the `FunctionRegistry` + parameter/return codec
(WASM calling registered JS functions via `host_invoke_function`). Faithfulness is **two-way**:
the Rust encoder (`foundation_wasm`) and the JS decoder (`foundation-wasm.js`, ported from
megatron) must stay byte-for-byte in lockstep, and the Rust side must keep working with the JS
side after the port.

There are **two distinct param encodings** in this system (flat for invoke = ParameterParserV1;
marker+quantized for batch = ParameterParserV2). Conflating them silently breaks parity — hence the
careful `research.md` mapping both sides before any code is ported.

## Workflow

1. **Read `research.md`** — the authoritative map of both sides (Rust + JS) and the parity rules.
2. Cross-check against the live source while porting:
   - Rust: `foundation_wasm/src/ops.rs` (`Params::to_binary` flat; `Params::encode` Batchable
     markers), `base.rs` (ParamTypeId/ReturnTypeId/ReturnTypeHints/ArgumentOperations),
     `host_runtime.rs` (`abi::web::{register_function, invoke, host_invoke_function*}`),
     `protocol.rs` (`ReturnValueParserIter` — how Rust decodes the return).
   - JS: `sdk/jsruntime/megatron.js` (`ParameterParserV1/V2`, `ReturnHintParser`, `Reply`,
     `host_register_function`, `host_invoke_function_with_return`).
3. Port into `foundation-wasm.js` `FunctionRegistry`, validate against
   `integrations/nodejs/integrations/*` (real `.wasm` fixtures), re-pointed from megatron.js.

_Created: 2026-06-10_
