# Feature 17 — Status: COMPLETE (2026-06-11)

The WASM↔JS function-call ABI codec is fully ported into the new runtimes with
faithful two-way parity, validated by three independent oracles.

## Delivered

| Piece | State |
|---|---|
| V1 (flat) param codec | `ParameterParser` in foundation-wasm.js — `[ParamTypeId][value]`, all 32 param types incl. 128-bit, CachedText, TypedArraySlice, *ArrayBuffer; 64/128-bit surface as BigInt (megatron `parseBigInt64` parity). |
| Return hints + replies | `ReturnHintParser` (`[200][ReturnIds][ThreeStates…][201]`, offset-based variant for batch streams) + `ReplyEncoder` (Begin=100..End=101 framed `ReturnValues`; naked typed fast-paths transform-before-naked; One/List/Multi; union ThreeStates resolved by runtime-type checks in declared order). |
| Typed fast-paths | `host_invoke_function_as_{bool,ints,floats,u64,i64}` + restored `as_object` / `as_dom` (naked heap handles) + `as_str` (raw UTF-8 slot). |
| Async path | `host_invoke_async_function` → `callbackSuccess`/`callbackFailure` (ErrorCode unwrap) → `invoke_callback`; `AsyncTaskCollector` tracking. |
| V2 (quantized batch) codec | `BatchParameterParser` + `BatchInstructions` — full `Operations`/`ArgumentOperations`/TypeOptimization 0–27 table, TEXTS pool, MakeFunction/Invoke/InvokeAsync, group returns `[111](…)[222]`, `registerOperation` extension point. |
| Context API | `as*` helper family, `ReplyContainer`, `FakeNode`, `ReplyError` — the megatron middleware surface registered functions call on `this`. |
| Protocol byte 0 | Re-based on this codec per decision 022: `BatchMessage` (Rust) / `registerOperation` (JS) selective opt-in, `BatchInstructionsV1` + `batchProtocolHandler`, DomOps as opcode `BATCH_OP_APPLY_DOM` (see LEARNINGS "Custom protocol (byte 0) re-based"). |

## Verification (the parity oracles)

1. **Purpose-built e2e fixtures** (uat-profile wasm32 modules in
   `foundation_wasm/integration` + `foundation_wasm_ui/integration`): every codec
   path round-trips against real compiled modules — 61 + 16 node tests green.
2. **All 21 megatron-era fixtures** (`integration/test/megatron-parity.test.js`):
   the old compiled modules run unmodified on the new runtime and SELF-ASSERT
   decoded values in Rust — drop-in replacement proven; megatron.js retired.
3. **Native Rust round-trips** (`foundation_wasm_ui/tests/protocol_impl_tests.rs`):
   byte-0 batch payloads decode back to `DomOp`s without a JS host.

## Key parity findings (full detail in LEARNINGS.md)

- Two param encodings stay separate: flat (invoke ↔ V1) vs marker+quantized (batch ↔ V2).
- Reply binaries REQUIRE the Begin=100/End=101 frame (`FromBinary for ReturnTypeHints`).
- Union ThreeStates pick by runtime-type check in declared order (megatron `check_for_type`).
- Only a None HINT yields `-1`; an undefined result under `One(None)` still encodes a framed reply.
- Never ship via `host_apply` while holding the global arena lock (JS ACK re-enters WASM).
