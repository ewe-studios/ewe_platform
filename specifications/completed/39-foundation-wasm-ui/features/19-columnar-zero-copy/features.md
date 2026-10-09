# Feature 19: Columnar Zero-Copy (alignment + columnar-native accumulation)

**Crates:** `foundation_ui_traits` (layout + builder), `foundation_wasm_ui`
(framing + receiver mode), `foundation-wasm-ui.js` (parser views)
**Origin:** the Arrow-properties review (2026-06-12): batching and columnar
layout survived the owned format, but two real Arrow virtues were dropped —
aligned buffers (zero-copy `TypedArray` views) and columnar-native
accumulation (no row→column serialization pass at flush).

## 1. Problem

1. **Alignment**: the JS `ColumnarParser` copies the u32 columns element-by-
   element because it must — the 14-byte `WasmEnvelope` header plus an
   arbitrary arena address means the columns land at unaligned `byteOffset`s,
   and `new Uint32Array(buffer, offset, n)` throws unless `offset % 4 == 0`.
   Apache Arrow solves this with 8-byte buffer padding; we dropped that
   discipline without replacing it.
2. **Write-side serialization**: the receiver accumulates row-shaped
   `Vec<DomOp>` and transforms to columns at flush — and `Row::from_op`
   CLONES every string (`Cow → String`) on the way. The transform belongs at
   queue time, writing string bytes once, directly into column buffers.

## 2. Wire layout v1.1 (columnar, protocol byte 1, version 1)

Both ends are owned and nothing is deployed — the layout changes in place.

```
payload := [pad_len: u8] [pad × pad_len]            // absolute-alignment shim
           [row_count: u32] [flags: u32 = 0]        // 8-byte header
           [op_id:     u32 × N]                     // rel offset  8 — 4-aligned
           [node_id:   u32 × N]                     //              4-aligned
           [operation: u8  × N] [pad to 4]
           [attribute string column]                // each column 4-aligned
           [value     string column]
           [text_val  string column]

string column := [(N+1) offsets: u32] [data_len: u32] [utf8 bytes] [pad to 4]
```

- `pad_len` is written by the FRAMING layer (`ColumnarV1`), which knows the
  absolute slot address: it pads so the 8-byte header starts at an absolute
  address ≡ 0 (mod 8). The pure `ColumnarEncoder` always writes `pad_len = 0`
  (HTTP/SSE consumers see relative alignment only).
- The JS parser uses TRUE views when `(byteOffset + offset) % 4 == 0` and
  falls back to the copy path otherwise — zero-copy when the transport
  cooperates, correct everywhere.

## 3. Columnar-native accumulation

- **`row_view(op, f)`** in `foundation_ui_traits`: the single decision-010
  mapping refactored into a borrowing visitor —
  `f(operation, node_id, attribute: Option<&str>, value: Option<&str>, text_val: Option<&str>)`
  with NO allocation. `Row::from_op` becomes a `row_view` caller (owned cells
  for the decoders); the builder is the other caller.
- **`ColumnarBatch`** in `foundation_ui_traits`: growing column buffers
  (`Vec<u32>` ×2, `Vec<u8>`, three `{offsets: Vec<u32>, data: Vec<u8>}`
  string columns). `push(&DomOp)` appends via `row_view` (string bytes copied
  ONCE, no `String` allocations); `serialize()` emits the v1.1 body;
  `ColumnarEncoder::encode` is reimplemented over it (one layout writer).
- **Receiver columnar mode**: `RuntimeBuilder::columnar()` (or
  `SharedInstructionReceiver::columnar(...)`) backs the receiver with a
  `ColumnarBatch` instead of `Vec<DomOp>` — `queue()` pushes straight into
  columns; `flush()` frames the finished body via `ColumnarV1` without any
  intermediate `Vec<DomOp>`. Generic protocols (JSON, byte-0, mock) keep the
  row-shaped receiver — they need `DomOp`s.

**Invariant (tested):** for any ops, builder-accumulated bytes ==
`ColumnarEncoder.encode(ops)` bytes. One layout, two producers.

## 4. Future consideration (recorded, NOT in scope): cached-string columns

From review: a column TYPE marker for interned strings — a `CacheString`
column carries u32 cache ids instead of utf8 payloads; the receiving side
resolves against a pre-shipped string cache (the F00 runtime already has a
`StringCache`). True zero-deserialization for HIGHLY REPETITIVE strings (tag
names, class lists, event names) since the decode cost is paid once at cache
time — but counterproductive for unique strings. Revisit when profiling shows
string-decode cost; the v1.1 `flags` header field reserves the demux space.

## 5. Testing

| # | Scenario | Verify |
|---|----------|--------|
| 1 | Rust round-trip after re-layout | all-19 / 1000-op / unicode / empty against the v1.1 layout |
| 2 | Builder ≡ encoder bytes | `ColumnarBatch` push-per-op output == `ColumnarEncoder.encode` |
| 3 | Framing alignment | `ColumnarV1`-shipped slot: header absolute offset % 8 == 0 (assert in Rust by pointer math) |
| 4 | JS views on aligned payloads | `nodeIds.buffer === wasm memory buffer` (shared, zero-copy) in the e2e |
| 5 | JS copy fallback | misaligned synthetic payload still parses correctly |
| 6 | Columnar receiver e2e | queue→flush→handle_received round-trips; no Vec<DomOp> in the path |
| 7 | Mixed columns relative alignment | every column offset % 4 == 0 relative to header |
