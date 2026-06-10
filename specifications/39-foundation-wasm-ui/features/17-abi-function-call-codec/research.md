# Research: WASM↔JS function-call ABI codec

**Goal:** map BOTH sides of `host_invoke_function` / `host_register_function` (Rust encode ↔ JS
decode/encode) precisely, so the JS port into `foundation-wasm.js` is faithful AND the Rust side
keeps working unchanged. Sources studied 2026-06-10: `foundation_wasm/src/{ops.rs, base.rs,
host_runtime.rs, protocol.rs}` and `sdk/jsruntime/megatron.js`.

---

## 0. The TWO param encodings (do NOT conflate)

| Encoding | Rust producer | JS consumer | Used by | Shape |
|---|---|---|---|---|
| **Flat** | `Params::to_binary` (ops.rs ~121) via `ToBinary for &[Params]` (concat, no list wrapper) | **`ParameterParserV1.parse_array`** (megatron ~1940) | **`host_invoke_function`** (function calls) | `[ParamTypeId:u8][value]` per param, concatenated; read until buffer end |
| **Marker + quantized** | `Params::encode` (Batchable, ops.rs ~300) wrapped by `ARGUMENT_STARTER/ENDER` | **`ParameterParserV2.parseParams`** (megatron ~2452) | batch / `create_instructions` / DOM-op path | `[Begin][ParamTypeId][TypeOptimization?][value(quantized)][End]…[Stop]` |

**This feature is the FLAT/invoke path (V1).** The marker/V2 path is the instructions/batch codec
(separate; pairs with the host_apply/DOM-op work). Verified: `host_invoke_function_with_return`
calls `this.parameter_v1.parse_array(...)` (megatron ~6487), and Rust `abi::web::invoke` calls
`params.to_binary()` (host_runtime.rs ~1642) — both flat. They match today.

---

## 1. Shared type tables (both sides must agree)

### ParamTypeId (base.rs ~566) — the `[type:u8]` in flat params; V1 dispatch keys 0–31
```
Null=0 Undefined=1 Bool=2 Text8=3 Text16=4 Int8=5 Int16=6 Int32=7 Int64=8
Uint8=9 Uint16=10 Uint32=11 Uint64=12 Float32=13 Float64=14 ExternalReference=15
Uint8ArrayBuffer=16 Uint16ArrayBuffer=17 Uint32ArrayBuffer=18 Uint64ArrayBuffer=19
Int8ArrayBuffer=20 Int16ArrayBuffer=21 Int32ArrayBuffer=22 Int64ArrayBuffer=23
Float32ArrayBuffer=24 Float64ArrayBuffer=25 InternalReference=26 Int128=27 Uint128=28
CachedText=29 TypedArraySlice=30 ErrorCode=31
```
(megatron V1 `this.parsers` map 0→parseNull … 31→parseErrorCode matches 1:1.)

### ReturnTypeId (base.rs ~62) — the `[type:u8]` in the RETURN ReturnValues binary
```
Bool=1 Text8=2 Int8=3 Int16=4 Int32=5 Int64=6 Uint8=7 Uint16=8 Uint32=9 Uint64=10
Float32=11 Float64=12 Int128=13 Uint128=14 MemorySlice=15 ExternalReference=16
InternalReference=17 Uint8ArrayBuffer=18 … Float64ArrayBuffer=27 Object=28 DOMObject=29
None=30 ErrorCode=31 TypedArraySlice=32
```
NOTE: ParamTypeId and ReturnTypeId are **different enumerations** — don't reuse one for the other.

---

## 2. Flat param value layout (`Params::to_binary`, ops.rs 121–294) — the V1 contract

Each param = `[ParamTypeId:u8]` then the value below (all multi-byte little-endian):

| Type (id) | Value bytes |
|---|---|
| Null(0), Undefined(1) | (none) |
| Bool(2) | 1 byte: `0` / `1` |
| Int8(5), Uint8(9) | 1 |
| Int16(6), Uint16(10), ErrorCode(31) | 2 |
| Int32(7), Uint32(11), Float32(13) | 4 |
| Int64(8), Uint64(12), Float64(14), ExternalReference(15), InternalReference(26), CachedText(29) | 8 |
| Int128(27), Uint128(28) | 16 = `[msb i64/u64 LE][lsb i64/u64 LE]` |
| Text8(3), Text16(4) | 16 = `[ptr:u64 LE][len:u64 LE]` → bytes live in WASM memory at `ptr` (UTF-8 / UTF-16) |
| *ArrayBuffer(16–25) | 16 = `[ptr:u64 LE][len:u64 LE]` → typed-array view over WASM memory |
| TypedArraySlice(30) | `[slice_type:u8][ptr:u64 LE][len:u64 LE]` |

**Decoder loop (V1):** `index=0; while index<len { type=u8@index; index+=1; (index, value)=parse[type](index); push value }`.
No Begin/End, no length-prefix, no count — consume to buffer end.

**CRITICAL:** Text8/Text16/arrays carry **pointers into WASM linear memory**, not inline data. The
JS decoder reads them from `wasmMemory.buffer` at `ptr`. This is safe because the call is
synchronous (WASM is paused inside `host_invoke_function`; memory is stable).

---

## 3. Return-hints encoding (`ReturnTypeHints::to_binary`, ops.rs 88) — what JS parses to know the return type

```
[ReturnHintMarker::Start][returns_u8][value bytes…][ReturnHintMarker::Stop]
```
- `returns_u8` = `ReturnIds` (None / One / List / Multi).
- For One/List the value is a `ThreeState`: `[ThreeStateId][ReturnTypeId … (1, 2, or 3 ids)]`.
- megatron `ReturnHintParser.parse_hint` (~1768) reads exactly this: Start → hint_type(ReturnIds)
  → per-id ThreeState parse → Stop.
- TO VERIFY during port: exact discriminant values of `ReturnHintMarker::{Start,Stop}`, `ReturnIds`,
  and `ThreeStateId` (there are several Begin/End-style enums in base.rs — pin the right ones:
  base.rs has ArgumentOperations at ~1024 (Begin=2,End=3,Stop=4) AND other Begin/End enums at
  ~899/~1086/~1148; confirm which `ReturnHintMarker`/`ArgumentOperations` each parser uses).

---

## 4. Return VALUE encoding (what JS writes back; Rust decodes via `ReturnValueParserIter`)

`host_invoke_function` returns `u64` = a `MemoryId` for an arena slot holding the **ReturnValues
binary**, which Rust decodes with `ReturnValueParserIter` (now in `protocol.rs`). Format per value:
`[ReturnTypeId:u8][value]` (e.g. None → `[30]`; Bool → `[1][0/1]`; Int32 → `[5][4 LE]`;
Uint8ArrayBuffer/MemorySlice → reference an arena slot — see ReturnValueParserIter array-buffer
arms which `ALLOCATIONS.get(mem_id).take()` then `deallocate`).
- megatron's `Reply.immediate(return_hint, value, always_encoded)` (~3602) does this encode +
  allocation and returns the handle. TO MAP during port: its per-ReturnTypeId encode + how it
  allocates the slot + the `always_encoded` flag.

---

## 5. Rust side — entry points (host_runtime.rs `abi::web`)

- `register_function(code: &str) -> HostFunction` (~1359) / `register_function_utf16(&[u16])`
  (~1377) → FFI `host_register_function(start:u64, len:u64, utf_indicator:i32) -> handle:u64`.
- `HostFunction::invoke(&self, params: &[Params], returns: ReturnTypeHints) -> u64` (~1638):
  `param_bytes = params.to_binary()`; `return_hints = returns.to_binary()`; calls
  `host_invoke_function(handler, p_ptr, p_len, r_ptr, r_len) -> u64` (MemoryId of result).
  Sig confirmed from .wat: `host_invoke_function:(i64,i32,i64,i32,i64)->i64`,
  `host_register_function:(i64,i64,i32)->i64`.
- Typed fast-paths `invoke_as_{bool,i8..u64,f32,f64,str}` → `host_invoke_function_as_*`. These
  expect a specific return type; TO VERIFY whether they return the value directly or a slot id.
- Result convention: megatron returns `BigInt(-1)` when the JS fn returns undefined/null
  (host_invoke_function ~6625), else `BigInt(MemoryId)`.

---

## 6. JS side — megatron flow to port (sdk/jsruntime/megatron.js)

- `host_register_function(start, len, utf_indicator)` (~6397): `ALLOWED_UTF8_INDICATOR` ∈ {8,16};
  read source via `texts.readUTF8/16FromMemory`; `Function('"use strict"; return(' + body + ')')()`;
  `function_heap.create(fn) -> handle`.
- `host_invoke_function(...)` (~6598) → `host_invoke_function_with_return` (~6472):
  1. `args = parameter_v1.parse_array(p_start, p_len)` (FLAT V1).
  2. `[_, hints] = return_hints.parse_hint(r_start, r_len)`.
  3. `fn = function_heap.get(handle)`.
  4. `response = fn.call(this, ...args)` — `this` = the runtime (so registered fns can use
     `this.mock`, `this.asMemorySlice`, etc.); args **spread** positionally.
  5. `return reply_parser.immediate(hints, response, true)` → MemoryId/value.
  Then host_invoke_function wraps: undefined/null → `-1n`; number → `BigInt`; else the bigint.
- Deps to REPLACE in the port: `texts` (TextCodec) → our string read helpers; `function_heap` →
  a `Map<handle, fn>`; `operator/MemoryOperator` → our `MemoryAllocations`; `text_cache`
  (SimpleStringCache) → our `StringCache`. Keep `parameter_v1`, `return_hints`, `reply` logic.

---

## 7. Two-way parity rules (the whole point)

1. **Flat invoke encoding is a contract between Rust `Params::to_binary` and JS V1.** Any change to
   a param's byte layout or a `ParamTypeId` value MUST change both sides together.
2. **Marker/batch encoding (Rust `Batchable::encode` ↔ JS V2) is a SEPARATE contract.** Don't let
   the invoke port touch it; don't reuse V2 for invoke or V1 for batch.
3. **`ReturnTypeId` and the ReturnValues binary** are the contract between JS `Reply` and Rust
   `ReturnValueParserIter`. Keep them in lockstep.
4. **Pointers cross the boundary for Text/arrays** — JS reads WASM memory at `ptr`; valid only for
   the synchronous duration of the call.
5. After porting, **run `integrations/nodejs/integrations/*` against `foundation-wasm.js`** (not
   megatron) — the real `.wasm` fixtures are the parity oracle. If any diverge, a side drifted.

---

## 8. Open items to confirm while porting (don't lose these)

- [ ] Exact V1 per-type readers (`parseText8`/`parseInt32`/`parseBigInt64`/`parseUint8Array`/…) —
      confirm endianness, signedness, and how arrays are surfaced to JS (TypedArray view vs copy).
- [ ] `Reply.immediate` internals: per-ReturnTypeId encode, slot allocation, `always_encoded`,
      and the None / undefined→-1 path.
- [ ] `ReturnHintMarker` / `ReturnIds` / `ThreeStateId` discriminant values (base.rs) — pin exact.
- [ ] Typed `invoke_as_*` return semantics (direct value vs MemoryId).
- [ ] Whether current Rust `invoke` (flat `to_binary`) matches the committed test `.wasm` fixtures
      (rebuild one fixture from current Rust and diff against megatron-decoded expectations).
- [ ] V2/markers + `TypeOptimization` quantization (`value_quantitization::qf64/qi16/qu16`) — only
      when the batch/instructions codec is tackled (separate feature/effort).

---

## 9. Validation plan

Port `FunctionRegistry` (register + invoke + V1 param decode + hint parse + Reply encode) into
`foundation-wasm.js`. Add to the owned harness (`foundation_wasm/integration`) a real module that
`register_function` + `invoke`s with each param type and each return type, asserting round-trips.
Then re-point the existing `integrations/nodejs/integrations/*` suites at `foundation-wasm.js` to
confirm full parity with the historical fixtures (this is the F12/F13 migration).
