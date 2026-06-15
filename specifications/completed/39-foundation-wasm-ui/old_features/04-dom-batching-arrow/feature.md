# Feature 04: DOM Batching with Arrow Format

## Description

Implement Arrow-format batching for DOM operations, enabling zero-copy transfer of DOM mutations from WASM to the JS host. Instead of serializing individual operations, batch them into Arrow record batches that the JS side parses and applies to the real DOM in one shot.

This eliminates serialization costs entirely — Arrow's columnar format maps directly to typed arrays.

**TODO**: This is why we need to abstract the communication protocol as we indicated in feature 01, so that different communication backend protocols can be clearly articulated to both side and used for communication.

## Module

`backends/foundation_wasm_ui/src/shared/arrow/`

## Arrow Schema

```
DOM Operations RecordBatch:
┌──────────┬──────────┬──────────────┬───────────────┬─────────────┬──────────┐
│ op_id    │ node_id  │ operation    │ attribute     │ value       │ text_val │
│ (u32)    │ (u32)    │ (u8 enum)    │ (string)      │ (string)    │ (string) │
├──────────┼──────────┼──────────────┼───────────────┼─────────────┼──────────┤
│ 1        │ 101      │ SET_TEXT     │               │             │ "Hello"  │
│ 2        │ 102      │ SET_ATTR     │ "class"       │ "active"    │          │
│ 3        │ 103      │ SET_PROP     │ "value"       │             │ "abc"    │
│ 4        │ 104      │ ADD_LISTENER │               │ "click"     │          │
│ 5        │ -1       │ CREATE_EL    │ "div"         │ "container" │          │
│ 6        │ 105      │ REMOVE       │               │             │          │
└──────────┴──────────┴──────────────┴───────────────┴─────────────┴──────────┘
```

### Operation enum (u8)
```
0 = CREATE_ELEMENT        // node_id=new_id, attribute=tag_name, value=class_name
1 = CREATE_TEXT_NODE      // node_id=new_id, text_val=content
2 = SET_TEXT_CONTENT      // node_id=target, text_val=new_text
3 = SET_ATTRIBUTE         // node_id=target, attribute=name, value=val
4 = REMOVE_ATTRIBUTE      // node_id=target, attribute=name
5 = SET_PROPERTY          // node_id=target, attribute=name, value=serialized_val
6 = ADD_EVENT_LISTENER    // node_id=target, value=event_name
7 = REMOVE_EVENT_LISTENER // node_id=target, value=event_name
8 = APPEND_CHILD          // node_id=parent, attribute=child_id
9 = REMOVE_CHILD          // node_id=parent, attribute=child_id
10 = REMOVE_NODE          // node_id=target
11 = INSERT_BEFORE        // node_id=parent, attribute=child_id, text_val=ref_child_id
12 = REPLACE_NODE         // node_id=old, attribute=new_id
13 = SET_STYLE            // node_id=target, attribute=prop, value=val
14 = ADD_CLASS            // node_id=target, value=class_name
15 = REMOVE_CLASS         // node_id=target, value=class_name
```

## API Surface

```rust
/// DOM operation batch builder using Arrow format.
pub struct ArrowBatch {
    op_ids: Vec<u32>,
    node_ids: Vec<u32>,
    operations: Vec<u8>,
    attributes: Vec<String>,
    values: Vec<String>,
    text_vals: Vec<String>,
}

impl ArrowBatch {
    /// Create a new empty batch.
    pub fn new() -> Self;

    /// Queue a DOM operation.
    pub fn create_element(&mut self, node_id: u32, tag: &str, class: &str);
    pub fn set_text(&mut self, node_id: u32, text: &str);
    pub fn set_attribute(&mut self, node_id: u32, name: &str, value: &str);
    pub fn set_property(&mut self, node_id: u32, name: &str, value: &str);
    pub fn add_event(&mut self, node_id: u32, event: &str);
    pub fn append_child(&mut self, parent_id: u32, child_id: u32);
    pub fn remove_node(&mut self, node_id: u32);
    pub fn insert_before(&mut self, parent_id: u32, child_id: u32, ref_id: u32);
    // ... all 15 operation types

    /// Encode to Arrow IPC format, return bytes for transfer.
    pub fn encode(&self) -> Vec<u8>;

    /// Send to host via batch API.
    pub fn apply(&self) {
        let bytes = self.encode();
        // Use foundation_wasm's host_batch_apply
        let ops_id = self.ops_id();
        let text_id = self.text_id();
        // ... send via existing batch mechanism
    }

    /// Flush all queued operations and reset.
    pub fn flush(&mut self);

    /// Number of queued operations.
    pub fn len(&self) -> usize;
}

/// Frame-batched dispatcher — coalesces all DOM updates within one animation frame.
pub struct FrameBatch {
    batch: ArrowBatch,
    frame_scheduled: AtomicBool,
}

impl FrameBatch {
    /// Queue an operation. If this is the first since last flush, schedule a frame.
    pub fn queue<F: FnOnce(&mut ArrowBatch)>(&self, f: F);
}
```

## Implementation Details

### Arrow encoding
- Uses `foundation_arrow` (planned) or manual Arrow IPC encoding
- Columnar format: each column is a contiguous typed array
- JS side: `new Uint32Array(buffer, offset, length)` — zero-copy view
- No JSON parsing, no string escaping

### Frame coalescing
- `FrameBatch` schedules via `requestAnimationFrame` (through foundation_wasm's animation hook)
- All signal changes within one frame → accumulated in single batch
- Single `host_batch_apply` per frame
- Eliminates redundant operations (e.g., set text twice → last write wins)

### JS runtime (Arrow parser)
```javascript
// Pseudo-code for JS side Arrow parser
function applyArrowBatch(buffer) {
    const view = new DataView(buffer);
    const opCount = view.getUint32(0);

    const opIds = new Uint32Array(buffer, 4, opCount);
    const nodeIds = new Uint32Array(buffer, 4 + opCount*4, opCount);
    const ops = new Uint8Array(buffer, 4 + opCount*8, opCount);
    // ... columns follow

    for (let i = 0; i < opCount; i++) {
        const op = ops[i];
        const nodeId = nodeIds[i];
        const node = nodeRegistry.get(nodeId);

        switch (op) {
            case 0: // CREATE_ELEMENT
                createNode(nodeId, ...);
                break;
            case 2: // SET_TEXT_CONTENT
                node.textContent = textVals[i];
                break;
            // ... all operations
        }
    }
}
```

### Node ID management
- WASM assigns sequential node IDs starting from 100
- JS side maintains a registry: `nodeId → DOM Element`
- When a component is removed, its nodes are unregistered
- Node IDs are scoped per component to avoid conflicts

## Dependencies

- `foundation_wasm` (for ExternalPointer, batch apply)
- Arrow IPC encoding — either `arrow` crate or custom minimal encoder

## Testing

- ArrowBatch: encode → decode → verify column values match
- FrameBatch: queue multiple ops → single apply call
- Frame coalescing: set text twice → only last text in batch
- Large batch (1000 ops): encode time < 1ms
- Zero-copy: JS receives buffer, creates TypedArray views, no allocation
