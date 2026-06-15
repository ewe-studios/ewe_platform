# Feature 05 — Status: COMPLETE (2026-06-12) — with an architectural decision

## The decision (deviation from the spec's framing)

The spec assumes real Apache Arrow IPC replaces the wire format INSIDE the WASM
loop (arrow crate in the module, bundled apache-arrow.js in the runtime). That
contradicts decision 031 (own the runtime, minimal deps) and the practical
budget: arrow-rs adds megabytes to a wasm module and apache-arrow.js ~100KB+ to
the runtime, for zero functional gain over the already-shipped owned columnar
layout (F01/F17) that the JS `ArrowParser` reads with the same zero-copy
`TypedArray` views the spec wants.

**So protocol byte 1 now has TWO wire forms, demuxed by the envelope VERSION
byte** (which was carried since F01 precisely for this):

| version | wire form | producer/consumer |
|---------|-----------|-------------------|
| 1 (`PROTOCOL_VERSION`) | owned compact columnar (`foundation_ui_traits::ArrowEncoder`) | the WASM loop — unchanged |
| 2 (`ARROW_IPC_VERSION`) | REAL Apache Arrow IPC stream (`foundation_arrow::ArrowIpcEncoder`) | servers (`application/primal-arrow`, SSE), analytics, ecosystem tooling |

Both forms serialize through the SAME `foundation_ui_traits::Row` mapping and
the same `ProtocolEncoder` contract — consumers choose by capability, not API.

## What shipped

`backends/foundation_arrow/src/dom_ops.rs`:

- `ArrowIpcEncoder` — `ProtocolEncoder<Vec<DomOp>>` (byte 1, version 2) over
  the decision-010 six-column schema (`op_id` u32, `node_id` u32, `operation`
  u8, `attribute`/`value`/`text_val` nullable Utf8). Real RecordBatch + IPC
  stream via the crate's existing `encode_ipc`/`decode_ipc`.
- `to_record_batch`/`from_record_batch` public — analytics pipelines can use
  the batch directly without the byte step.
- `None` row cells are REAL Arrow nulls (validity bitmaps), unlike v1's
  empty-string convention.
- foundation_arrow gained the (zero-dep, no_std) `foundation_ui_traits` dep.

## Verification

14 tests (`tests/dom_ops_tests.rs`) covering spec tests 1-12: row shapes per
the decision-010 table (CreateElement attr=tag `id:` form, value=class,
text_val NULL; CreateTextNode; secondary ids as decimal strings), all-19
round-trip with discriminant order, 1000-op batch, CJK/emoji/RTL, zero-row
stream, generic-IPC-reader consumption (proves ecosystem readability),
protocol identity, op=99 `UnknownOperation{row_index}`, 4-column + wrong-type
`SchemaMismatch`, empty (`TruncatedBuffer`) and garbage payloads. Zero clippy
warnings.

Spec test 11 (invalid UTF-8 in a string column) is documented UNREACHABLE
through this decoder: Arrow IPC validates UTF-8 before our code sees values;
corruption surfaces as a wrapped read error instead.

## Deferred (scope moved, not dropped)

- **JS `tableFromIPC` consumption** (spec §4): belongs to the fetch/SSE
  content-type handlers (F06 mounts / F11 request batching), where
  `application/primal-arrow` v2 responses arrive. The WASM loop never sees v2.
- The spec's §4 applicator/NodeRegistry JS text was already superseded by the
  F01 JS runtime upgrade (19 ops, staging registry).
