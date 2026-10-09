# Feature 19 — Status: COMPLETE (2026-06-12)

## What shipped

**Wire layout v1.1** (compact columnar, protocol byte 1, wire version 1):
`[pad_len:u8][shim][row_count:u32][flags:u32][u32 columns][u8 column][string
columns]`, every column 4-aligned relative to the 8-byte header.

**The alignment contract (review-driven):** EVERY producer pads so the header
lands on an 8-byte boundary — aligned data is the rule, not a lucky case:
- the PURE `ColumnarEncoder` always emits `pad_len = 7` (header at relative
  offset 8), so any payload held as its own buffer is aligned at offset 0;
- the wasm FRAMING layer (`ColumnarV1::write_batch`) computes the shim from
  the ABSOLUTE arena slot address (readable because `allocate` zero-fills to
  capacity, fixing the buffer before the address is read), so the header is
  8-aligned in linear memory.
The JS parser verifies and falls back to copying — correctness is decoupled
from the optimization (apache-arrow.js has the same fallback).

**Columnar-native accumulation:**
- `row_view(op, f)` — THE decision-010 mapping as a borrowing visitor; string
  cells are borrowed wherever the op owns the text (only wire-id/numeric/
  morph-packing cells format into temporaries). `Row::from_op` is now a
  `row_view` caller — one mapping, every format.
- `ColumnarBatch` — `push(&DomOp)` appends straight into column buffers (each
  string byte copied exactly once, no `String`s, no `Vec<DomOp>`);
  `serialize`/`serialize_with_pad`; `clear()` keeps capacity (G22).
- `ColumnarEncoder::encode` reimplemented over the builder (one layout writer).
- `ColumnarReceiver` + `RuntimeBuilder::columnar()` +
  `SharedInstructionReceiver` dual mode — the columnar path has no row vector
  anywhere; generic protocols (JSON/byte-0/mock) keep the row receiver.

**JS `ColumnarParser` v1.1**: pad-shim aware; u32 columns become TRUE
`TypedArray` views (sharing the payload's buffer) when aligned, copy-fallback
otherwise; exposes `opIds` and a `zeroCopy` flag; `flags` header reserved for
the future cached-string demux (recorded in features.md §4).

## Verification

- Rust: builder≡encoder byte identity (shim included); framed header absolute
  8-alignment by pointer math on a live slot; columnar receiver round-trip +
  buffer reuse across cycles; `RuntimeBuilder.columnar()` signals-loop e2e;
  all re-layout round-trips (all-19/1000-op/unicode/empty) re-green across
  ui_traits (23), wasm_ui (24 receiver + 8 protocol + 26 html), arrow (14+4).
- JS: 61 + 20 tests green, including the new e2e: a real wasm module flushes
  through `ColumnarReceiver` → parser reports `zeroCopy: true` AND
  `nodeIds.buffer === payload.buffer` (views over live linear memory),
  snapshotted inside the apply handler (slots are ACK-disposed after).
- Zero clippy warnings across all three crates (`--all-targets`, uat).

## Notes

- Spec test 11's invalid-UTF-8 corruption test now locates the text bytes
  dynamically (the shim moved all fixed offsets).
- `slot_contains_exact_encoded_bytes` became `..._body`: shims legitimately
  differ by placement; the BODY is the byte-identical part.
- Cached-string columns (dictionary encoding) deliberately deferred — the
  `id:N` wire form already dictionary-encodes tags/attrs; measure before
  building (features.md §4).
