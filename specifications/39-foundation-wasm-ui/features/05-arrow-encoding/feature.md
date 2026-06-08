# Feature 05: Arrow Encoding

## Description

Implement Arrow IPC encoding for DOM operations. 17 operation types in a columnar RecordBatch format. JS parses via TypedArray views (zero-copy). Includes MORPH_NODE (ID 16) for DOM morphing.

**Decisions:** 010, 009

## Module

`crates/foundation_wasm_ui/src/arrow/`

## Arrow Schema

```
DOM Operations RecordBatch:
┌──────────┬──────────┬──────────────┬───────────────┬─────────────┬──────────┐
│ op_id    │ node_id  │ operation    │ attribute     │ value       │ text_val │
│ (u32)    │ (u32)    │ (u8 enum)    │ (string)      │ (string)    │ (string) │
└──────────┴──────────┴──────────────┴───────────────┴─────────────┴──────────┘
```

### Operation Enum (u8)

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
| 8 | APPEND_CHILD | parent | child_id | | |
| 9 | REMOVE_CHILD | parent | child_id | | |
| 10 | REMOVE_NODE | target | | | |
| 11 | INSERT_BEFORE | parent | child_id | | ref_child_id |
| 12 | REPLACE_NODE | old | new_id | | |
| 13 | SET_STYLE | target | prop | val | |
| 14 | ADD_CLASS | target | | class_name | |
| 15 | REMOVE_CLASS | target | | class_name | |
| 16 | MORPH_NODE | target | | | new_html |

### MORPH_NODE (ID 16)

Morphs an existing element's children using Datastar's morphing algorithm (form preservation, element identity tracking, move detection). `text_val` contains the new HTML as a string.

### API

```rust
pub struct ArrowBatch {
    op_ids: Vec<u32>,
    node_ids: Vec<u32>,
    operations: Vec<u8>,
    attributes: Vec<String>,
    values: Vec<String>,
    text_vals: Vec<String>,
}

impl ArrowBatch {
    pub fn new() -> Self;
    pub fn queue(&mut self, op: DomOp);
    pub fn encode(&self) -> Vec<u8>;  // Arrow IPC format
}
```

### Message layout

```
[protocol: u8=1][version: u8][batch_memory_id: u64][arrow_ipc_length: u32][Arrow IPC...]
```

`batch_memory_id` is the arena slot — JS reads it from the envelope, processes, then calls `dispose_allocation` to free.

### JS side (covered in Feature 07)

`ArrowParser` in `foundation-wasm.js` — TypedArray column views from ArrayBuffer (zero-copy).
`ArrowDomApplicator` in `foundation-wasm-ui.js` — applies ops sequentially to real DOM.

## Dependencies

- `foundation_ui_traits` (DomOp)

## Testing

- Each DomOp variant → correct column values after encode
- 1000 ops → encode time < 1ms
- MORPH_NODE → html string in text_val column
- Round-trip: encode → JS parse → apply → correct DOM state
