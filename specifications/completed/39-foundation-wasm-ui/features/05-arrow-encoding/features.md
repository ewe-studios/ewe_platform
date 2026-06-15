# Feature 05: Arrow Encoding

Arrow IPC encoding for DOM operations. 17 operation types in a columnar RecordBatch.
JS parses via TypedArray views (zero-copy). MORPH_NODE (op 16) delegates to Datastar's
morphing algorithm (decision 027).

**Decisions:** 010 (Arrow batch schema), 009 (no backpressure), 028 (WASM memory ownership)

---

## 1. Types and Structs

### Arrow Schema (6-column RecordBatch)

| # | Column | Arrow Type | Nullable | Rust Builder |
|---|--------|-----------|----------|--------------|
| 0 | `op_id` | UInt32 | No | `Vec<u32>` |
| 1 | `node_id` | UInt32 | No | `Vec<u32>` |
| 2 | `operation` | UInt8 | No | `Vec<u8>` |
| 3 | `attribute` | Utf8 | Yes | `Vec<Option<String>>` |
| 4 | `value` | Utf8 | Yes | `Vec<Option<String>>` |
| 5 | `text_val` | Utf8 | Yes | `Vec<Option<String>>` |

### 19 Operations (u8 discriminant)

| ID | Operation | node_id | attribute | value | text_val |
|----|-----------|---------|-----------|-------|----------|
| 0 | CREATE_ELEMENT | new_id | tag_name | class_name | |
| 1 | CREATE_TEXT_NODE | new_id | | | content |
| 2 | SET_TEXT_CONTENT | target | | | new_text |
| 3 | SET_ATTRIBUTE | target | name | val | |
| 4 | REMOVE_ATTRIBUTE | target | name | | |
| 5 | SET_PROPERTY | target | name | serialized_val | |
| 6 | ADD_EVENT_LISTENER | target | | event_name | |
| 7 | REMOVE_EVENT_LISTENER | target | | event_name | |
| 8 | APPEND_CHILD | parent | child_id* | | |
| 9 | REMOVE_CHILD | parent | child_id* | | |
| 10 | REMOVE_NODE | target | | | |
| 11 | INSERT_BEFORE | parent | child_id* | | ref_child_id* |
| 12 | REPLACE_NODE | old_id | new_id* | | |
| 13 | SET_STYLE | target | prop | val | |
| 14 | ADD_CLASS | target | | class_name | |
| 15 | REMOVE_CLASS | target | | class_name | |
| 16 | MORPH_NODE | target | | | new_html |
| 17 | REGISTER_NODE | node_id | | | |
| 18 | UNREGISTER_NODE | node_id | | | |

*Secondary u32 IDs stored as decimal strings in string columns. JS `parseInt()` recovers.

**Registry ops (17-18):** Explicit NodeRegistry management. `CREATE_ELEMENT`/`CREATE_TEXT_NODE` do NOT auto-register — caller must emit `RegisterNode` explicitly. `RemoveNode` (op 10) implicitly unregisters. `ReplaceNode` (op 12) implicitly unregisters old_id and registers new_id.

---

## 2. Algorithms

### Rust Encoding: Vec<DomOp> -> Arrow IPC bytes

1. Allocate six builders pre-sized to `ops.len()`.
2. For each `(i, op)` in `ops.iter().enumerate()`:
   - Push `i as u32` into `op_ids`.
   - Extract primary ID (node_id/parent_id/old_id) into `node_ids`.
   - Push discriminant 0-16 into `operations`.
   - Map fields into nullable string columns per table above. Unused = `None`.
     Secondary u32 IDs formatted via `to_string()`.
3. Build `RecordBatch` with `UInt32Array`, `UInt8Array`, 3x nullable `StringArray`.
4. Write via `arrow::ipc::writer::StreamWriter` to `Vec<u8>`. Finish (appends EOS).
5. Return bytes.

### Rust Decoding: Arrow IPC bytes -> Vec<DomOp>

1. Open `StreamReader` over byte slice. Read single RecordBatch (zero rows = `Success(vec![])`).
2. Validate: 6 columns, correct types. Else `SchemaMismatch`.
3. Downcast to typed arrays: 2x `UInt32Array`, 1x `UInt8Array`, 3x `StringArray`.
4. For each row: read `operation` u8, `node_id` u32, nullable strings.
   Match 0-16 to reconstruct DomOp. Parse secondary IDs via `str::parse::<u32>()`.
   Unknown op -> `UnknownOperation`. Invalid UTF-8 -> `InvalidUtf8`.

---

## 3. Memory and Byte Layouts

### Arrow IPC Stream Format

```
Schema message
 ├─ continuation: 0xFFFFFFFF (4 bytes)
 ├─ metadata_length: i32 LE
 ├─ metadata flatbuffer (Schema: 6 fields)
 └─ padding to 8-byte boundary
RecordBatch message
 ├─ continuation: 0xFFFFFFFF (4 bytes)
 ├─ metadata_length: i32 LE
 ├─ metadata flatbuffer (field nodes + buffer descriptors)
 ├─ padding to 8-byte boundary
 └─ body (column buffers, contiguous, 8-byte aligned):
    ├─ col0 op_id:    [u32 LE × N]                          — N×4 bytes
    ├─ col1 node_id:  [u32 LE × N]                          — N×4 bytes
    ├─ col2 operation: [u8 × N]                              — N bytes
    ├─ col3 attribute: [validity bitmap | offsets | utf8 data]
    ├─ col4 value:     [validity bitmap | offsets | utf8 data]
    └─ col5 text_val:  [validity bitmap | offsets | utf8 data]
EOS marker
 ├─ continuation: 0xFFFFFFFF (4 bytes)
 └─ metadata_length: 0x00000000 (4 bytes)
```

### Column Buffer Internals

**UInt32 (non-nullable):** `N * 4` bytes, each value 4-byte LE. No validity bitmap.
JS: `new Uint32Array(buffer, offset, N)` — true zero-copy view.

**UInt8 (non-nullable):** `N` bytes, one per row. JS: `new Uint8Array(buffer, offset, N)`.

**Utf8 nullable:** Three sub-buffers:
- **Validity bitmap:** `ceil(N/8)` bytes. Bit `i` = 1 if non-null. LSB-first ordering.
- **Offset array:** `(N+1) × 4` bytes of i32 LE. String `i` spans `data[offsets[i]..offsets[i+1]]`.
  Null rows: `offsets[i] == offsets[i+1]`.
- **Data buffer:** Concatenated UTF-8 bytes. No terminators.

---

## 4. JS Runtime

**G3/G25 resolved:** Uses Apache Arrow JS library, bundled into our runtime. We copy the
`apache-arrow` JS source file from the npm package into `assets/apache-arrow.js` — no npm
dependency for the end user.

### ArrowParser (foundation-wasm-ui.js)

```javascript
import { tableFromIPC } from './apache-arrow.js';

class ArrowParser {
    static parse(buffer) {
        const table = tableFromIPC(buffer);
        const numRows = table.numRows;
        return {
            opIds:      table.getChild('op_id').toArray(),
            nodeIds:    table.getChild('node_id').toArray(),
            operations: table.getChild('operation').toArray(),
            attributes: table.getChild('attribute').toArray(),
            values:     table.getChild('value').toArray(),
            textVals:   table.getChild('text_val').toArray(),
            numRows,
        };
    }
}
```

**Rust side** uses the `arrow` crate (`features = ["ipc"]`).

### ArrowDomApplicator (foundation-wasm-ui.js)

```javascript
class ArrowDomApplicator {
    constructor(nodeRegistry) { this.nodes = nodeRegistry; } // Map<number, Element>

    apply(columns) {
        const { nodeIds, operations, attributes, values, textVals, numRows } = columns;
        for (let i = 0; i < numRows; i++) {
            const nid = nodeIds[i], attr = attributes[i], val = values[i], text = textVals[i];
            switch (operations[i]) {
                case 0: { // CREATE_ELEMENT
                    const el = document.createElement(attr);
                    if (val) el.className = val;
                    this.nodes.set(nid, el); break; }
                case 1: { // CREATE_TEXT_NODE
                    this.nodes.set(nid, document.createTextNode(text || '')); break; }
                case 2:  this.nodes.get(nid).textContent = text; break;
                case 3:  this.nodes.get(nid).setAttribute(attr, val); break;
                case 4:  this.nodes.get(nid).removeAttribute(attr); break;
                case 5:  this.nodes.get(nid)[attr] = JSON.parse(val); break;
                case 6:  this.nodes.get(nid).addEventListener(val, this._handler(nid,val)); break;
                case 7:  this.nodes.get(nid).removeEventListener(val, this._handler(nid,val)); break;
                case 8:  this.nodes.get(nid).appendChild(this.nodes.get(parseInt(attr))); break;
                case 9:  this.nodes.get(nid).removeChild(this.nodes.get(parseInt(attr))); break;
                case 10: { const n=this.nodes.get(nid); n.parentNode?.removeChild(n); break; }
                case 11: this.nodes.get(nid).insertBefore(
                             this.nodes.get(parseInt(attr)), this.nodes.get(parseInt(text))); break;
                case 12: { const o=this.nodes.get(nid);
                           o.parentNode.replaceChild(this.nodes.get(parseInt(attr)), o); break; }
                case 13: this.nodes.get(nid).style[attr] = val; break;
                case 14: this.nodes.get(nid).classList.add(val); break;
                case 15: this.nodes.get(nid).classList.remove(val); break;
                case 16: { const t=this.nodes.get(nid);
                           MorphDom.morph(t, document.createRange().createContextualFragment(text));
                           break; }
                default: throw new Error(`unknown operation: ${operations[i]} at row ${i}`);
            }
        }
    }
}
```

### NodeRegistry (foundation-wasm-ui.js)

**G45 resolved — primal-id string↔number conversion:** Compile-time primal-ids are `u32` (0, 1, 2...).
At runtime, they're prefixed: `"42:0"` (string). Arrow ops use `node_id: u32` (the raw number).
The NodeRegistry stores entries by `u32` key. The mapping is:
- **Arrow ops → Registry:** `node_id` is already `u32` — direct key lookup.
- **DOM scan → Registry:** JS parses `primal-id="42:0"` by splitting on `:` and extracting the
  template-local ID. For registry lookups, JS doesn't need to convert — Arrow ops always use the
  prefixed number directly. The `primal-id` attribute on DOM elements is for developer debugging
  and JS-side scanning; the Arrow pipeline uses numeric IDs exclusively.

Central registry mapping `node_id: u32` → `DOM Element`. All Arrow ops that reference a `node_id` look it up here.

```javascript
class NodeRegistry {
    constructor() {
        this._map = new Map();  // Map<number, Element>
    }

    /** Register a DOM element under a numeric node_id. Idempotent — no-op if already registered. */
    register(nodeId, element) {
        if (!this._map.has(nodeId)) {
            this._map.set(nodeId, element);
        }
    }

    /** Unregister a node_id. Does NOT remove the element from DOM. No-op if not registered. */
    unregister(nodeId) {
        this._map.delete(nodeId);
    }

    /** Get element by node_id. Returns undefined if not registered — caller throws. */
    get(nodeId) {
        return this._map.get(nodeId);
    }

    /** Clear all entries. Used during teardown. */
    clear() {
        this._map.clear();
    }

    /** Debug: count of registered nodes. */
    get size() { return this._map.size; }
}
```

**Registry op handling in ArrowDomApplicator:**
```javascript
case 17: { // REGISTER_NODE
    const el = this.nodes.get(nid);
    if (el) this.registry.register(nid, el);
    break;
}
case 18: { // UNREGISTER_NODE
    this.registry.unregister(nid);
    break;
}
case 10: { // REMOVE_NODE — implicit unregister
    const el = this.nodes.get(nid);
    el?.parentNode?.removeChild(el);
    this.registry.unregister(nid);
    break;
}
case 12: { // REPLACE_NODE — implicit unregister old, register new
    const oldEl = this.nodes.get(nid);
    const newEl = this.nodes.get(parseInt(attr));
    oldEl?.parentNode?.replaceChild(newEl, oldEl);
    this.registry.unregister(nid);
    this.registry.register(parseInt(attr), newEl);
    break;
}
```

**Typical op sequence for creating a registered element:**
```
CreateElement  { node_id: 42, tag: "div", class: "main" }
RegisterNode   { node_id: 42 }                     // now addressable by subsequent ops
SetAttribute   { node_id: 42, name: "id", value: "hero" }
SetText        { node_id: 42, text: "Hello" }
AppendChild    { parent_id: 1, child_id: 42 }
```

**Reserved node_ids** (allocated at init, never collide with macro-assigned primal-ids):
- `0` = `<head>` element — registered by first-batch theme injection
- `1` = `<body>` element — registered by init batch
- `2` = `<html>` element — registered by init batch
- Macro-assigned primal-ids start at `1000` to avoid collision. The Runtime's atomic counter for component prefixes begins at `1000`.

---

## 5. WASM Envelope

14-byte WasmEnvelope (decision 028). All protocols use uniform 3-param FFI via `host_apply`.

```
Offset  Size  Field             Encoding
0       1     protocol          1 (Arrow)
1       1     version           1
2       8     batch_memory_id   u64 LE — MemoryId packed (index << 32 | generation)
10      4     arrow_ipc_length  u32 LE
14      N     Arrow IPC bytes   Schema + RecordBatch + EOS

 0   1   2        10  14             14+N
 ┌───┬───┬────────┬───┬──────────────┐
 │ 1 │ 1 │ mem_id │len│ Arrow IPC... │
 └───┴───┴────────┴───┴──────────────┘
```

JS processing:
```javascript
const view = new DataView(buffer);
const memoryId = view.getBigUint64(2, true);
const length   = view.getUint32(10, true);
const payload  = new Uint8Array(buffer, 14, length);
try {
    arrowDomApplicator.apply(ArrowParser.parse(payload.buffer, payload.byteOffset, length));
} finally {
    wasmExports.dispose_allocation(memoryId); // ACK: free arena slot
}
```

Synchronous apply — no requestAnimationFrame deferral (decision 009).

---

## 6. Error Cases and Edge Cases

| Scenario | Behavior |
|----------|----------|
| Zero-row batch | Valid. Decode returns `Success(vec![])`. JS loop runs 0 iterations. |
| Unknown operation byte | Rust: `UnknownOperation { op_id, row_index }`. JS: throws Error. |
| Null in non-nullable column | Rejected at RecordBatch construction. Cannot occur. |
| Invalid UTF-8 in string column | `InvalidUtf8 { column_name, row_index }`. |
| Schema mismatch | `SchemaMismatch { detail }`. |
| Secondary ID not parseable / node_id missing | `nodes.get()` returns undefined, DOM call throws TypeError. |
| Unregistered node_id used in op | `nodes.get()` returns undefined, DOM call throws TypeError. Caller must emit `RegisterNode` before referencing. |
| Duplicate register (same node_id twice) | No-op — second register ignored. Element stays the same. |
| Unregister non-existent node_id | No-op — `Map.delete` on missing key returns false. |
| MORPH_NODE with empty html | Target children removed. |
| JS fails to dispose_allocation | Arena slot leaks. Mandatory try/finally mitigates. |
| Concurrent batches | Impossible. Synchronous processing serializes batches. |
| MORPH_NODE ordering with other ops | MORPH_NODE is self-contained — it takes a target element (via node_id in the registry) and an HTML string, parses it, and morphs the target's children. It doesn't depend on other ops in the batch. The target must already be registered in the NodeRegistry before MORPH_NODE fires. |
| Arrow IPC size limit | Max batch: 10MB Arrow IPC buffer. Beyond this, the encoder splits into multiple batches. Arena slot must accommodate the full buffer — slots are sized dynamically at allocate time. JS TypedArray views can handle buffers up to ~2GB (ArrayBuffer max). |

---

## 7. Performance Characteristics

**Columnar advantages:** Cache locality (one column = contiguous memory). Zero-copy
TypedArray views for UInt32/UInt8 columns. Batch amortization (envelope + schema overhead
paid once per batch, not per op).

**Encoding:** O(N) time and memory. Builders allocate once per column. String columns use
contiguous UTF-8 + offset array. Target: 1000 ops in < 1ms.

**JS parsing:** Fixed-width columns = O(1) TypedArray construction. String columns =
O(N * avg_str_len) via TextDecoder. DOM application dominates total cost.

**Memory:** 1 arena slot per batch, freed synchronously. At most one batch in-flight.

---

## 8. Integration Points

| Feature | Relationship |
|---------|-------------|
| F01 (foundation_ui_traits) | Owns `ArrowEncoder` (Layer 1). F05 specifies Arrow internals. |
| F04 (InstructionReceiver) | Calls `ArrowV1.encode_and_send` (composes encoder + transport). |
| F00 (foundation_wasm) | `ProtocolHandler`, WasmEnvelope, `host_apply` FFI, arena. |
| F06 (Web Components) | ArrowHandler for `primal-arrow` content-type in mount elements. |
| F07 (DOM Morphing) | Op 16 delegates to `MorphDom.morph`. Html via text_val column. |
| F08 (Event Runtime) | Ops 6-7 wire listeners. EventDispatcher manages handler registry. |

**Flow:** `DomOp -> ArrowEncoder.encode (L1) -> ArrowV1.encode_and_send (L3) ->
memory.allocate + host_apply (L2) -> JS ArrowParser.parse ->
ArrowDomApplicator.apply -> dispose_allocation (ACK)`

---

## 9. File Ownership

- `crates/foundation_ui_traits/src/arrow_encoder.rs` — ArrowEncoder encode/decode
- `crates/foundation_ui_traits/src/dom_op.rs` — DomOp enum (17 variants)
- `crates/foundation_wasm_ui/src/protocol/arrow.rs` — ArrowV1 (encoder + transport)
- `crates/foundation_wasm_ui/assets/apache-arrow.js` — Apache Arrow JS (bundled from npm package source)
- `assets/foundation-wasm-ui.js` — ArrowDomApplicator class

---

## 10. Refactoring Strategy

1. Implement `ArrowEncoder` in `foundation_ui_traits`. Pass Rust round-trip tests.
2. Implement `ArrowV1` in `foundation_wasm_ui`. Test with mock FFI.
3. Implement `ArrowParser` in `foundation-wasm.js`. Test against Rust hex fixtures.
4. Implement `ArrowDomApplicator` in `foundation-wasm-ui.js`. Test 19 ops with jsdom.
5. Wire `ProtocolDispatcher` (protocol byte 1 -> Arrow path).
6. End-to-end: Rust encodes, WASM ships, JS applies, DOM correct, slot freed.

---

## 11. Testing

### Rust Encoding (tests 1-8)

| # | Scenario | Verify |
|---|----------|--------|
| 1 | Single CreateElement | 1 row: `op=0`, `attr="div"`, `val="main"`, `text_val=null`. |
| 2 | Single CreateTextNode | `op=1`, `text_val="hello"`, `attr=null`. |
| 3 | AppendChild (secondary ID) | `attr` is child_id decimal string. |
| 4 | InsertBefore (two secondary IDs) | `attr`=child_id string, `text_val`=ref_id string. |
| 5 | All 19 variants in one batch | 19 rows, each op matches index, all fields round-trip. |
| 6 | 1000 SetText ops | Encode < 1ms. All texts byte-exact after round-trip. |
| 7 | Unicode (CJK, emoji, RTL) | Byte-identical after round-trip. |
| 8 | Empty Vec | 0-row Arrow IPC. Decode returns `Success(vec![])`. |

### Rust Decoding Errors (tests 9-12)

| # | Scenario | Verify |
|---|----------|--------|
| 9 | Hand-crafted op=99 | `UnknownOperation { op_id: 99, row_index: 0 }`. |
| 10 | 4-column Arrow buffer | `SchemaMismatch`. |
| 11 | Invalid UTF-8 in text_val | `InvalidUtf8 { column_name: "text_val" }`. |
| 12 | Empty byte slice | `TruncatedBuffer` or `SchemaMismatch`. |

### JS ArrowParser (tests 13-16)

| # | Scenario | Verify |
|---|----------|--------|
| 13 | Parse Rust fixture (5 ops) | `numRows==5`, discriminants match. |
| 14 | Fixed-width = TypedArray views | `opIds instanceof Uint32Array`, shares buffer. |
| 15 | Null string columns | `attributes[i] === null` where expected. |
| 16 | 1000-row batch | Parses correctly, all strings match. |

### JS ArrowDomApplicator (tests 17-30)

| # | Scenario | Verify |
|---|----------|--------|
| 17 | CREATE_ELEMENT | createElement with tag, className set. Element NOT in registry (auto-register removed). |
| 18 | CREATE_TEXT_NODE | createTextNode. Element NOT in registry. |
| 19 | SET_TEXT_CONTENT | textContent updated. |
| 20 | SET/REMOVE_ATTRIBUTE | setAttribute/removeAttribute with correct args. |
| 21 | SET_PROPERTY | Bracket property set, value JSON-parsed. |
| 22 | ADD/REMOVE_EVENT_LISTENER | addEventListener/removeEventListener called. |
| 23 | APPEND/REMOVE_CHILD | Parent-child created/broken. Secondary ID parsed. |
| 24 | INSERT_BEFORE | insertBefore with child and ref, both IDs parsed. |
| 25 | REPLACE_NODE | replaceChild. Old unregistered, new registered. |
| 26 | SET_STYLE | `style[prop] = val`. |
| 27 | ADD/REMOVE_CLASS | classList.add/remove called. |
| 28 | MORPH_NODE | MorphDom.morph called with fragment from html string. |
| 29 | REGISTER_NODE | Element added to registry under node_id. |
| 30 | UNREGISTER_NODE | Element removed from registry, DOM element unchanged. |

### NodeRegistry (tests 31-35)

| # | Scenario | Verify |
|---|----------|--------|
| 31 | register(42, el), get(42) | Returns el. |
| 32 | register(42, el1), register(42, el2), get(42) | Returns el1 (idempotent, first wins). |
| 33 | unregister(42), get(42) | Returns undefined. |
| 34 | unregister(999) (never registered) | No error, no-op. |
| 35 | clear() | size == 0, all get() return undefined. |

### End-to-End (tests 36-39)

| # | Scenario | Verify |
|---|----------|--------|
| 36 | Full pipeline: encode -> ship -> parse -> apply | DOM matches after 10-op batch. |
| 37 | dispose_allocation after apply | Slot freed, generation incremented. |
| 38 | dispose_allocation on error | Handler throws; finally still frees slot. |
| 39 | MORPH_NODE end-to-end | MorphNode op applied, form state preserved. |
