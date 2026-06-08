# Feature 04: InstructionReceiver

## Description

Replace the global `FRAME_BATCH` with a runtime-owned `InstructionReceiver` that receives DOM operations from effects, batches them per stabilize cycle, and dispatches to the configured protocol for encoding and FFI handoff.

**Decisions:** 030, 022

## Module

`crates/foundation_wasm_ui/src/instruction_receiver.rs`

## Types

```rust
pub struct InstructionReceiver {
    ops: Vec<DomOp>,
    protocol: Box<dyn Protocol>,
    memory: Arc<MemoryAllocations>,
}

impl InstructionReceiver {
    /// Called by effects during/after stabilize() — batches ops without encoding
    pub fn queue(&mut self, op: DomOp);

    /// Called once per stabilize cycle — encodes and ships
    pub fn flush(&mut self);
}
```

### Protocol trait

```rust
pub trait Protocol {
    fn protocol_byte(&self) -> u8;
    /// Takes DomOps, encodes, allocates arena slot, calls FFI
    fn send(&self, ops: Vec<DomOp>, memory: &MemoryAllocations);
}
```

Implementations: `ArrowProtocol`, `CustomBinaryProtocol`, `JsonProtocol`.

### Flow

```
stabilize() completes
    ↓
receiver.take_ops() — drains pending ops
    ↓
protocol.send(ops, memory) — encodes + allocates + FFI call
    ↓
JS applies, ACKs, frees memory
```

### Runtime construction

```rust
fn init() {
    let runtime = Runtime::builder()
        .protocol(ArrowProtocol::new())  // user-configured
        .memory(memory_allocations)
        .build();
    // InstructionReceiver created internally with the protocol
}
```

## Dependencies

- `foundation_ui_traits` (DomOp)
- `foundation_wasm` (MemoryAllocations, FFI)

## Testing

- queue + flush → protocol.send called with correct ops
- Multiple effects in one cycle → single batch
- Empty queue → no protocol call
- Protocol swap → different encoding output
- Mock protocol → collects ops for unit testing
