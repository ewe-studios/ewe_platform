# 03. Payload tiers: typed operations, aligned columns, bulk tables

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); Document Object Model (DOM); JavaScript
(JS); TypeScript (TS); inter-process communication (IPC); identifier (id); AeroSpace and
Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. The idea, laid out

Source: `foundation_ui_rearchitecture.md` §1 ("Multi-Tier Performance Optimization"), §2
(`ColumnarPayload`, `ArrowPayload`) and §3 ("Web Browser Client Optimization").

**The size of an update decides its encoding.** The document names three tiers:

| Tier | Document's words | Shape | Best for |
|---|---|---|---|
| 1 | "lightning-fast primitive modifications via FlatBuffers" | A few typed operations | A click that changes one label |
| 2 | "direct array-buffer casting via custom 8-byte aligned Columnar blocks" | Columns of numbers (op codes, node ids, offsets) plus one string block | A list render: hundreds to thousands of operations |
| 3 | "vectorized mass data pushes powered by Apache Arrow IPC streams" | Typed columns of domain data (rows of a table, chart series) | Data grids, charts, metrics |

The trick in tier 2 is the **zero-copy cast**: the client views the received bytes as a typed
array, with no parse and no allocation per value:

```javascript
const uint32View = new Uint32Array(rawBytes.buffer, rawBytes.byteOffset, rawBytes.byteLength / 4);
```

This works only if the column starts on an aligned byte offset (4 for `Uint32Array`, 8 for
`Float64Array` and `BigUint64Array`). That is why the document insists on 8-byte alignment.

```mermaid
flowchart TD
    U[UI update from an action] --> D{How many changes?}
    D -->|"few (default up to 64)"| T1[Tier 1: OpBatch<br/>typed tables]
    D -->|many| T2[Tier 2: ColumnarBatch<br/>aligned columns]
    U2[Domain data for a grid or chart] --> T3[Tier 3: DataTable<br/>typed column vectors]
    T1 --> F[Frame body union]
    T2 --> F
    T3 --> F
```

_The diagram shows how the server picks a tier for each update. The threshold is
configuration._

## 2. Current state

Tier 2 exists by hand, on the web only (spec 39 feature 19):

| Part | Where |
|---|---|
| `ColumnarBatch`, columnar v1.1, 8-byte header alignment (`RELATIVE_ALIGN_PAD = 7`) | `backends/foundation_ui_traits/src/columnar_encoder.rs:13-32`, `:49`, `:92` |
| JS `ColumnarParser`: `Uint32Array` view, alignment check, copy fallback | `backends/foundation_wasm_ui/runtimes/foundation-wasm-ui.js:94-187` (check at `:129`, view at `:148`) |
| Arrow IPC encoder for `DomOp` | `backends/foundation_arrow/src/dom_ops.rs:54`, `:164` |
| `foundation_arrow` derive macros (`ToArrow`, `FromArrow`, `ArrowSchema`) | `backends/foundation_arrow`, `backends/foundation_macros` |

Tier 1 has no typed form (operations go as JSON or as columnar). Tier 3 for domain data has
no wire path; Arrow is used for `DomOp` and for stored `foundation_ai` data.

## 3. The new way

All three tiers are **types in the one FlatBuffers schema** ([02](./02-unified-wire-contract.md)).
The hand-written formats become generated code.

### 3.1 Tier 1: `OpBatch`

```flatbuffers
enum SelectorKind : ubyte { NodeId = 0, Id = 1, Class = 2, Query = 3 }
enum MorphAction : ubyte { ReplaceChildren = 0, ReplaceElement = 1, InsertBefore = 2,
                           InsertAfter = 3, AppendSibling = 4 }
table TargetSelector { kind: SelectorKind; node_id: uint; text: string; }

table HtmlMorph {
  target: TargetSelector;   // node id, CSS id, class or query
  action: MorphAction;      // which part of the target to reconcile
  content: string;          // HTML fragment
}

union Op { CreateNode: CreateNode, SetText: SetText, SetProp: SetProp,
           AppendChild: AppendChild, Remove: Remove, ReplaceSubtree: ReplaceSubtree,
           HtmlMorph: HtmlMorph }        // web target only (D8)
table OpRow  { op: Op; }
table OpBatch { ops: [OpRow]; }
```

The client walks the vector and reads each table through generated accessors. Cost per
operation is a few offset reads. For a handful of operations this is as fast as tier 2.

`HtmlMorph` carries the existing `DomOp::MorphNode` (op 16, decision 027) as a schema type.
The web applicator passes `content` to `MorphDom`, unchanged. This is the only operation whose
payload is markup, and it is valid only for the web target. The server never sends it to a
native client. Section [11](./11-app-store-and-command-bus.md) explains why that keeps the app
store rule.

### 3.2 Tier 2: `ColumnarBatch` as a schema type

```flatbuffers
table ColumnarBatch {
  op_codes:   [ubyte];                       // one per operation
  node_ids:   [uint]   (force_align: 8);     // one per operation
  arg_a:      [uint]   (force_align: 8);     // parent id, prop id, ... by op code
  str_offset: [uint]   (force_align: 8);     // start of each string in str_bytes
  str_len:    [uint]   (force_align: 8);
  str_bytes:  [ubyte];                       // all strings, UTF-8, written once
}
```

- A FlatBuffers vector of scalars is a contiguous little-endian array. The generated TS
  reader gives a typed-array view over the received buffer for such a vector. So the zero-copy
  cast of the document still happens, but the code that does it is generated, not
  hand-written.
- Swift and Kotlin get the same columns through their generated readers (`UnsafeBufferPointer`
  on Swift, a `ByteBuffer` view on Kotlin).
- Strings stay one block. JS decodes a string only when an operation needs it
  (`TextDecoder` on a `subarray`). The document's "zero object allocation" claim is true for
  the numeric columns only. The design must say this.
- Tier 1 and tier 2 use **one consolidated representation**: the schema types and the
  generated readers. The web reads both through the same path, not through two parsers
  (D27, owner answer 2026-11-11).

### 3.3 Tier 3: `DataTable`

```flatbuffers
table Column {
  name_id: uint;                 // interned column name (04)
  data: ColumnData;              // union of typed vectors
  validity: [ubyte];             // null bitmap, optional
}
union ColumnData { I32: I32s, I64: I64s, F64: F64s, Bool: Bools, Utf8: Strings }
table I64s { values: [long] (force_align: 8); }
table F64s { values: [double] (force_align: 8); }
table DataTable { rows: ulong; columns: [Column]; }
```

This is Arrow's columnar idea, expressed in our schema, with generated readers on every
platform. A chart on iOS reads `F64s.values` straight into its drawing code.

### 3.4 The Arrow IPC escape hatch

Arrow IPC is its own format (FlatBuffers headers plus raw buffers). It is not a table in our
schema. Its value is interoperability with Arrow tools (DuckDB, Polars, DataFusion). The new
way keeps one union member, `ArrowIpc { bytes: [ubyte] (force_align: 8) }`, only for data that
goes to such a tool. UI frames do not use it.

### 3.5 Improvements over the document

- Tiers are chosen by the framework, not by the action author, from a configured threshold.
- Tier 1 and tier 2 share one consolidated representation, so the web has one read path for
  both (D27).
- Tier 2 and tier 3 are generated types. No hand-written parser on any platform.
- Arrow IPC stays as a union member for data that goes to Arrow tools (D28).
- One more tier boundary is made explicit: streaming. A large table goes as a stream of
  `DataTable` frames with a row cap per frame (configuration), never as one large frame.

## 4. What this changes for us

- `ColumnarBatch` (`columnar_encoder.rs`) and the JS `ColumnarParser` retire when the
  consolidated form delivers the same functionality on the web (D27). One representation
  serves tier 1 and tier 2.
- The Arrow `DomOp` encoder (`foundation_arrow/src/dom_ops.rs`) is not needed for UI frames.
  Arrow IPC stays for data that goes to Arrow tools (D28).
- The columnar op code 16 (`MORPH_NODE`) and the JS branch stay. They read the same
  `HtmlMorph` fields from the generated reader.

## 5. Open questions

Q6 in [`questions.md`](./questions.md): the tier threshold default. Q7 is answered: retire the
hand-written form when the consolidated form delivers the same web functionality (D27). Q8 is
answered: keep Arrow IPC and the other data formats (D28).
