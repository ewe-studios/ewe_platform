# 030 — Runtime-owned InstructionReceiver replaces global FRAME_BATCH

**Date:** 2026-06-09  
**Status:** Resolved

### Decision

**No global `FRAME_BATCH`.** The `Runtime` owns an `InstructionReceiver` instance — a batching component that receives DOM operations, knows how to encode them into the chosen protocol, and handles the FFI handoff.

```
stabilize() completes
    ↓
InstructionReceiver.take_ops() — drains pending ops
    ↓
protocol encoder: Vec<DomOp> → Arrow / JSON / custom binary
    ↓
protocol FFI: host_arrow_apply / host_batch_apply / host_json_apply
    ↓
JS applies, ACKs, frees memory
```

### How it works

```rust
pub struct InstructionReceiver {
    ops: Vec<DomOp>,
    protocol: Protocol,
    memory: Arc<MemoryAllocations>,
}

impl InstructionReceiver {
    /// Called by effects during/after stabilize() — batches ops without encoding
    pub fn queue(&mut self, op: DomOp) {
        self.ops.push(op);
    }

    /// Called once per stabilize cycle — encodes and ships
    pub fn flush(&mut self) {
        if self.ops.is_empty() {
            return;
        }
        let ops = std::mem::take(&mut self.ops);
        self.protocol.send(ops, &self.memory);
    }
}
```

### Protocol encoding + FFI dispatch

Each protocol implementation owns its complete contract — encoding, memory allocation, FFI call:

```rust
// Arrow protocol
impl Protocol for ArrowProtocol {
    fn send(&self, ops: Vec<DomOp>, memory: &MemoryAllocations) {
        let ipc = encode_arrow_ipc(&ops);        // Vec<u8>
        let mem_id = memory.allocate(ipc.len());
        let slot = memory.get(mem_id).unwrap();
        slot.apply(|mem| mem.extend_from_slice(&ipc));

        let (ptr, len) = slot.as_address().unwrap();
        unsafe {
            host_arrow_apply(mem_id.as_u64(), ptr as u64, len as u64);
        }
    }
}

// Custom binary protocol
impl Protocol for CustomBinaryProtocol {
    fn send(&self, ops: Vec<DomOp>, memory: &MemoryAllocations) {
        let (ops_mem, text_mem) = encode_instructions(&ops, memory);
        let (ops_ptr, ops_len) = ops_mem.as_address().unwrap();
        let (text_ptr, text_len) = text_mem.as_address().unwrap();
        unsafe {
            host_batch_apply(
                ops_mem.id.as_u64(), ops_ptr as u64, ops_len as u64,
                text_mem.id.as_u64(), text_ptr as u64, text_len as u64,
            );
        }
    }
}
```

### Effect queueing — same pattern, simpler

Effects don't know about protocols. They just queue:

```rust
ctx.effect(move || {
    let value = signal.get();
    receiver.queue(DomOp::SetText(node_id, value.to_string()));
});
```

After `stabilize()` completes, the Runtime calls `receiver.flush()` — one encoding call, one FFI call, one batch shipped.

### Runtime construction

```rust
fn init() {
    let runtime = Runtime::builder()
        .protocol(ArrowProtocol::new())   // or JsonProtocol, CustomBinaryProtocol
        .memory(memory_allocations)
        .build();
    // InstructionReceiver is created internally with the protocol
}
```

### Why this design

- **No global state** — `InstructionReceiver` belongs to the Runtime, not a `static mut` or `LazyCell`
- **Protocol owns encoding** — the `Protocol` trait takes `Vec<DomOp>` and does everything: encode, allocate, FFI call. No handoff ambiguity.
- **Same effect API** — effects call `receiver.queue(op)` unchanged. They don't know or care about protocols.
- **Single batch per stabilize** — all effects in one cycle queue into the same receiver, flushed once
- **Testable** — mock protocol implementations for unit testing (collect ops without encoding)
- **Extensible** — adding a new protocol means implementing `Protocol::send`, no changes to effect code
- **User can intercept** — swap the protocol at init time to add logging, filtering, or custom encoding
