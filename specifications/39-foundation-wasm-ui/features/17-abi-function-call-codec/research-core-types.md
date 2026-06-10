# Research: core ABI types — parity map (JS ↔ Rust)

**Why:** practically everything in the WASM↔JS ABI builds on these types. The JS classes
(megatron.js) and their Rust counterparts (`foundation_wasm`) MUST agree on discriminants, byte
layouts, and semantics. This locks them in so the port keeps both sides in lockstep. Companion to
`research.md` (the invoke flow). Sources studied 2026-06-10.

Line refs: `M:` = `sdk/jsruntime/megatron.js`; `B:` = `src/base.rs`; `O:` = `src/ops.rs`;
`P:` = `src/protocol.rs`; `H:` = `src/host_runtime.rs`.

---

## A. Reference pointers (RefPointer family)

| JS (M) | Rust | Meaning | Parity |
|---|---|---|---|
| `RefPointer { id; get value }` (M:1421) | — (base type) | wraps a `u64` id | id is the contract |
| `ExternalPointer extends RefPointer` (M:1718) | `ExternalPointer(u64)` (B:1284) | id into the **host external-object heap** (DOM nodes, JS objects). Allocated via `*_allocate_external_pointer`/`dom_allocate_external_pointer` | u64 id; reserved DOM ids 0–4 = self/this/window/document/body (now in foundation_wasm_ui::wasm::dom::constants) |
| `InternalPointer extends RefPointer` (M:1720) | `InternalPointer(u64)` (B:1247) | id into the **internal callback registry** (`InternalReferenceRegistry`, monotonic, never reused) | u64 id; stale → dropped silently |
| `CachePointer extends RefPointer` (M:1722) | `CachedText` handle (param id 29) | id into the **string cache** (`host_cache_string` → `StringCache`) | u64 handle; interned-string identity |

**Encode in flat params:** `ExternalReference`(15)/`InternalReference`(26)/`CachedText`(29) each
encode as `[id:u64 LE]` (O:144/235/132). JS surfaces them as the matching `*Pointer` wrapper.

---

## B. ThreeState

- JS `ThreeState { state_type, options }` (M:1444): `state_type` ∈ `ThreeStateId`, `options` = array
  of `ReturnTypeId`.
- Rust `ThreeState` + `ThreeStateId` (B:286): **One=70, Two=80, Three=90** — the count of allowed
  return types in a single return slot.
- Used inside return-hints to express "this return position may be one of N types".

---

## C. Return-hint hierarchy

JS classes (M:1454–1718) ↔ Rust `ReturnTypeHints` (B:382) + `ReturnIds` (B:333).

| JS class | `return_type` | Rust `ReturnTypeHints` | `ReturnIds` |
|---|---|---|---|
| `NoReturn` (M:1645) | None | `None` | **0** |
| `SingleReturn(state)` (M:1663) | One | `One(ThreeState)` | **1** |
| `MultiReturn(states)` (M:1675) | Multi | (Rust: `Multi`?) | **2** |
| `ListReturn(state)` (M:1669) | List | `List(ThreeState)` | **3** |

- `ReturnHint { return_type (ReturnIds), states (ThreeState[]) }` (M:1454) with per-`ReturnTypeId`
  **validators** (M:1488–1610) — type-checks the JS return value before encoding (Bool→boolean,
  Text8→string, ints→number, 64-bit→bigint, arrays→matching TypedArray).
- `ReturnHintValue { hint_type, states, value }` (M:1634) — a hint paired with a produced value.
- `ReplyContainer { type (ReturnTypeId), value }` (M:1431) — a typed return slot value.

**Wire format** (`ReturnTypeHints::to_binary`, O:88): `[ReturnHintMarker::Start=200][returns_u8
(ReturnIds)][ThreeState bytes…][ReturnHintMarker::Stop=201]`. ThreeState bytes =
`[ThreeStateId][ReturnTypeId × (1|2|3)]`.

---

## D. ReturnHintParser

- JS `ReturnHintParser` (M:1752): `parse_hint(start,len)` reads `[Start=200][hint_type=ReturnIds]`
  then dispatches by ReturnIds (None/One/List/Multi, M:1762–1765) to read ThreeState(s), then
  expects `[Stop=201]`. Produces `No/Single/List/MultiReturn`.
- Rust counterpart: `ReturnTypeHints::to_binary` (encode) + `FromBinary for ReturnTypeHints` /
  `GroupReturnTypeHints` (decode, now in P:). The parser must mirror the markers (200/201) + ReturnIds
  (0/1/2/3) + ThreeStateId (70/80/90) above.

---

## E. TypedArraySlice

- JS `TypedArraySlice { slice_type, slice_content, address=[start,len] }` (M:1724); `slice_type` ∈
  `TypedSlice`.
- Rust: `TypedSlice` (B:13): **Int8=1, Int16=2, Int32=3, Int64=4, Uint8=5, Uint16=6, Uint32=7,
  Uint64=8, Float32=9, Float64=10**; param variant `TypedArraySlice` (ParamTypeId **30**).
- **Flat param layout** (O:286): `[ParamTypeId=30][slice_type:u8][ptr:u64 LE][len:u64 LE]` → JS builds
  the matching TypedArray view over WASM memory at `ptr` (element count from `slice_type` + byte len).

---

## F. Reply (return-value ENCODER, JS) ↔ ReturnValueParserIter (Rust decoder)

- JS `Reply` (M:3480): `reply_types[ReturnTypeId] → encode*` dispatch (M:3538–3577) covering None,
  Bool, Int/Uint 8/16/32/64/128, Float32/64, Text8, Object, TypedSlice, DOMObject,
  External/Internal reference, MemorySlice, and all *ArrayBuffer types. `immediate(return_hint,
  value, always_encoded)` (M:3602): validate via the hint, encode the value into the ReturnValues
  binary, allocate a slot, return the `MemoryId`.
- Rust decoder: `ReturnValueParserIter` (P:) reads `[ReturnTypeId:u8][value]` per the hints; for
  array-buffer/MemorySlice it `ALLOCATIONS.get(mem_id).take()` then `deallocate` (so the return-value
  encoder on the JS side must hand back data the way these arms expect — inline scalars vs slot ids).
- **`ReturnTypeId` table is the shared contract** (B:62): Bool=1 … None=30, MemorySlice=15, Object=28,
  DOMObject=29, ErrorCode=31, TypedArraySlice=32. (Distinct from `ParamTypeId`!)
- OPEN: map each `encode*` body to the exact `[ReturnTypeId][value]` bytes `ReturnValueParserIter`
  expects (esp. MemorySlice/*ArrayBuffer slot semantics, 128-bit msb/lsb order, Text8 as slot).

---

## G. BatchOperation + Operations (the batch opcodes)

- JS `BatchOperation { operation_id, handler, perform(...) }` (M:3459) — a registered handler for one
  `Operations` opcode. Instances: `MAKE_FUNCTION` (M:5263), `INVOKE`, `INVOKE_ASYNC`.
- Rust `Operations` (B:1078): **Begin=0, MakeFunction=1, Invoke=…, InvokeAsync=…, Stop=…** (Begin=0 +
  MakeFunction=1 confirmed; OPEN: read exact Invoke/InvokeAsync/Stop discriminants + each op's byte
  layout — `Operations` doc-comments at B:1078+ specify layouts, e.g. MakeFunction = 23 bytes incl.
  Begin/Stop).
- `ArgumentOperations` (B:1024): **Start=1, Begin=2, End=3, Stop=4** — the param-list markers used by
  the **V2** parser (M:2488/2495/2510/2517) and the Batchable param encode (O:300+). (This is the
  marker set; NOT the same as Operations or ReturnHintMarker.)

---

## H. BatchInstructions (the batch builder/decoder)

- JS `BatchInstructions` (M:5569): holds `reply_parser` (Reply), `return_hints` (ReturnHintParser),
  `parameter_v2` (marker parser), `texts`, `async_tasks`, `operator`; an `operations` registry seeded
  with `[MAKE_FUNCTION, INVOKE, INVOKE_ASYNC]` (M:5610). `parse_one_batch(read_index, operations,
  texts)` (M:5620) reads an `Operations` opcode and dispatches to the matching `BatchOperation`.
- Rust counterpart: `Instructions` (O:2170) + `CompletedInstructions` (B:2048) +
  `internal_api::create_instructions` (H:68). The batch uses the **V2/marker + Operations** encoding
  (the `host_apply`/instructions path) — DISTINCT from the flat invoke path (research.md §0).
- This is the bridge to the DOM-op/host_apply work; the function-call codec (this feature) only needs
  MAKE_FUNCTION/INVOKE/INVOKE_ASYNC semantics, but they share Reply + ReturnHintParser + the pointer
  types, hence locking them in here.

---

## I. Parity rules (consolidated)

1. **Discriminant tables are law** — both sides hardcode the same values; changing one requires the
   other: `ParamTypeId` (0–31, B:566), `ReturnTypeId` (1–32, B:62), `ReturnIds` (0/1/2/3, B:333),
   `ThreeStateId` (70/80/90, B:286), `ReturnHintMarker` (200/201, B:960), `ArgumentOperations`
   (1/2/3/4, B:1024), `Operations` (B:1078), `TypedSlice` (1–10, B:13).
2. **`ParamTypeId` ≠ `ReturnTypeId`** — never reuse one enum for the other direction.
3. **Pointers carry only u64 ids**; the heaps live host-side (external object heap / internal callback
   registry / string cache). No JsValue crosses the boundary.
4. **Two param encodings stay separate**: flat (invoke ↔ V1) vs marker+quantized (batch ↔ V2).
5. **Reply (JS encode) ↔ ReturnValueParserIter (Rust decode)** must agree on `[ReturnTypeId][value]`
   incl. slot semantics for MemorySlice/array buffers.
6. Validate against `integrations/nodejs/integrations/*` real `.wasm` fixtures (parity oracle).

## J. Open items to confirm during port (carry-forward; don't lose)

- [ ] `Operations` full discriminants + per-op byte layouts (B:1078+ doc-comments).
- [ ] `Reply.encode*` exact bytes per ReturnTypeId (esp. MemorySlice / *ArrayBuffer slot handling,
      128-bit msb/lsb, Text8/Object as slot ids) vs `ReturnValueParserIter` arms.
- [ ] V1 per-type readers (parseText8/parseBigInt64/parseUint8Array/parseTypedArraySlice) — endian/sign,
      TypedArray surface (view vs copy), `should_break` semantics (InternalReference returns break?).
- [ ] `MultiReturn` Rust counterpart (is there a `ReturnTypeHints::Multi`, or only None/One/List? B:382).
- [ ] TypedArraySlice element-count derivation from `slice_type` + byte length.
- [ ] `Reply.immediate` `always_encoded` flag + the undefined/null → `-1n` convention (M:6625).
