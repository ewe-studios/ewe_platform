# 010 — Arrow batch: operation mapping and schema

**Date:** 2026-06-08
**Status:** Resolved

### Decision

Arrow-format DOM batching is defined in **Feature 04** (dom-batching-arrow). This decision records the schema and operation mapping for clarity.

### Arrow Schema

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

### Morph operations

`MORPH_NODE` morphs an existing element's children using Datastar's morphing algorithm (decision 027) — form state preservation, element identity tracking, move detection:

| Field | Value |
|-------|-------|
| `node_id` | Target element to morph |
| `text_val` | New HTML content as a string (parsed into fragment, morphed into target) |

`REPLACE_NODE` is a hard replacement — removes old element, inserts new one with no preservation:

| Field | Value |
|-------|-------|
| `node_id` | Element to remove |
| `value` | `new_id` for the replacement element |

### DomOp → Arrow mapping

- `DomOp::SetText(node_id, text)` → `SET_TEXT_CONTENT`
- `DomOp::SetAttr(node_id, name, val)` → `SET_ATTRIBUTE`
- `DomOp::SetClass(node_id, class)` → `ADD_CLASS`
- `DomOp::SetStyle(node_id, prop, val)` → `SET_STYLE`
- `DomOp::CreateEl(node_id, tag, class)` → `CREATE_ELEMENT`
- `DomOp::AppendChild(parent, child)` → `APPEND_CHILD`
- `DomOp::Remove(node_id)` → `REMOVE_NODE`
- `DomOp::InsertBefore(parent, child, ref)` → `INSERT_BEFORE`
- `DomOp::Replace(old, new_id)` → `REPLACE_NODE`
- `DomOp::Morph(node_id, html)` → `MORPH_NODE`

### Arrow encoding location

Arrow parsing and encoding lives in **`foundation_wasm`** (protocol layer), not `foundation_wasm_ui`:
- `ArrowParser` (JS) — TypedArray column views from ArrayBuffer → `foundation-wasm.js`
- Arrow encoder (Rust) — `DomOp[]` → Arrow buffer → `foundation_wasm` (uses `DomOp` from `foundation_ui_traits`)
- `ArrowDomApplicator` (JS) — applies parsed Arrow to real DOM → `foundation-wasm-ui.js`

This way Arrow is available to all protocols regardless of whether DOM is involved.

### JS-side: ArrowParser + ArrowDomApplicator

- `ArrowParser` (in foundation-wasm.js) parses Arrow IPC buffer via TypedArray column views (zero-copy)
- `ArrowDomApplicator` (in foundation-wasm-ui.js) `apply(buffer)` → applies ops sequentially to real DOM
- Node registry maps `primal-id` → DOM Element
