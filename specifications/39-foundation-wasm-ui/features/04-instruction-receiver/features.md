# Feature 04: InstructionReceiver

## Description

`InstructionReceiver` is runtime-owned (not global). It receives DOM operations from effects, batches them per stabilize cycle, and dispatches to the configured protocol for encoding and FFI handoff.

**Decisions:** 030, 022

## Module

`crates/foundation_wasm_ui/src/instruction/receiver.rs`

## Types

```rust
pub struct InstructionReceiver {
    ops: Vec<DomOp>,
    protocol: Box<dyn ProtocolMethods>,
    memory: MemoryAllocations,
}

impl InstructionReceiver {
    pub fn queue(&mut self, op: DomOp);
    pub fn flush(&mut self);  // encodes + ships via protocol
}
```

### ProtocolMethods trait

```rust
pub trait ProtocolMethods {
    fn protocol_byte(&self) -> u8;
    fn version(&self) -> u8;
    fn encode(&self, memory: &mut MemoryAllocations, data: ProtocolData<'_>) -> EncodeResult;
    fn handle_received(&self, payload: &[u8]) -> HandleResult;
    fn ack(&self, memory_ids: &[MemoryId], memory: &mut MemoryAllocations);
}
```

### Flow

```
stabilize() completes
    ↓
receiver.flush() — drains ops, protocol encodes + allocates + FFI call
    ↓
JS applies, ACKs, frees memory
```

## Dependencies

- `foundation_ui_traits` (DomOp)
- `foundation_wasm` (MemoryAllocations, FFI)

## Testing

- queue + flush → protocol.send called with correct ops
- Multiple effects in one cycle → single batch
- Empty queue → no protocol call
- Mock protocol → collects ops for unit testing