# Feature 04: InstructionReceiver

**Crate path:** `crates/foundation_wasm_ui/src/instruction/receiver.rs`
**Decisions:** 030 (replaces global FRAME_BATCH), 028 (WASM memory ownership), 009 (no backpressure)
**Constraint:** Runtime-owned, not global. Effects queue ops without knowledge of protocols or FFI.

Runtime-owned bridge between the reactive signal graph and the WASM-JS boundary. Effects call `receiver.queue(DomOp)` during stabilize; after stabilize completes, the runtime calls `receiver.flush()` which drains the batch, delegates to the configured protocol for encoding, allocates arena memory, and ships bytes to JS via FFI. Per-message protocol demux (envelope byte), not per-session. No backpressure — synchronous processing prevents buffer buildup.

---

## 1. Types & Structs

```rust
pub struct InstructionReceiver {
    ops: Vec<DomOp>,                                  // queued ops, drained on flush, append-only between flushes
    protocol: Box<dyn ProtocolMethods<Vec<DomOp>>>,   // protocol impl composing encoder + WASM transport
    memory: MemoryAllocations,                         // shared arena allocator for WASM linear memory slots
    flush_count: u64,                                  // running count since construction, for diagnostics
}

pub struct SendResult {
    pub memory_id: MemoryId,    // primary arena slot (Arrow/JSON: single slot; CustomBinary: ops slot)
    pub op_count: usize,        // number of DomOps encoded
    pub encoded_bytes: usize,   // payload byte size excluding envelope
}

pub enum HandleResult {
    Ok { op_count: usize },                // successfully decoded JS-to-Rust message
    DecodeError(DecodeError),              // Layer 1 decode failure
    StaleMemory { memory_id: MemoryId },   // generation mismatch on memory lookup
}

// From foundation_wasm — shown for completeness
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MemoryId {
    pub index: u32,       // slot index in the arena
    pub generation: u32,  // incremented on recycle, prevents stale access
}
```

## 2. Trait Definitions (Three-Layer Protocol Architecture)

```rust
// Layer 1 (foundation_ui_traits) — pure encoding, no WASM dependency
pub trait ProtocolEncoder<T> {
    fn protocol_byte(&self) -> u8;   // 0=CustomBinary, 1=Arrow, 2=JSON
    fn version(&self) -> u8;        // starts at 1
    fn encode(&self, data: T) -> Vec<u8>;
    fn decode(&self, payload: &[u8]) -> DecodeResult<T>;
}
// Layer 2 (foundation_wasm) — WASM transport: arena + FFI, no encoding logic
pub trait ProtocolHandler {
    fn protocol_byte(&self) -> u8;
    fn version(&self) -> u8;
    fn send_to_js(&self, memory_id: MemoryId, ptr: *const u8, len: usize);
    fn handle_from_js(&self, memory_id: MemoryId, ptr: *const u8, len: usize);
}
// Layer 3 (foundation_wasm_ui) — composes encoder + transport
pub trait ProtocolMethods<T>: ProtocolHandler {
    fn encode_and_send(&self, data: T, memory: &mut MemoryAllocations) -> SendResult;
    fn handle_received(&self, payload: &[u8]) -> HandleResult;
    fn ack(&self, memory_ids: &[MemoryId], memory: &mut MemoryAllocations);
}
```

## 3. Method Signatures & Behavior

### InstructionReceiver::new
```rust
pub fn new(protocol: Box<dyn ProtocolMethods<Vec<DomOp>>>, memory: MemoryAllocations) -> Self
```
Initializes with empty `ops` (pre-allocated capacity 64), `flush_count` 0.

### InstructionReceiver::queue
```rust
pub fn queue(&mut self, op: DomOp)
```
1. Push `op` onto `self.ops`. 2. No deduplication — DomOps are order-dependent (SetText then SetAttribute on the same node is valid). Callers are responsible for not producing redundant ops. 3. No capacity check or backpressure. Vec grows as needed. Typical batch: 10-200 ops.

### InstructionReceiver::flush — step-by-step algorithm
```rust
pub fn flush(&mut self)
```
```
1. if self.ops.is_empty() → return            // no-op, no protocol call, no allocation
2. let batch = mem::take(&mut self.ops)        // self.ops becomes Vec::new(); batch owns ops
3. self.protocol.encode_and_send(batch, &mut self.memory)
   // protocol encodes → allocates arena slot(s) → calls FFI
   // JS applies synchronously, ACKs, calls dispose_allocation
4. self.flush_count += 1
```
Post-flush: `self.ops` is empty with zero capacity (from `mem::take`). Next `queue()` re-allocates. Intentional — avoids holding memory between cycles.

### Accessors
```rust
pub fn flush_count(&self) -> u64       // successful flushes since construction
pub fn pending_count(&self) -> usize   // self.ops.len()
```

## 4. Concrete Protocol Implementations

### ArrowV1 (protocol_byte=1, 1 MemoryId per batch)
```rust
pub struct ArrowV1 { encoder: ArrowEncoder }

impl ProtocolMethods<Vec<DomOp>> for ArrowV1 {
    fn encode_and_send(&self, ops: Vec<DomOp>, memory: &mut MemoryAllocations) -> SendResult {
        let op_count = ops.len();
        let bytes = self.encoder.encode(ops);              // Layer 1: DomOps → Arrow IPC bytes
        let mem_id = memory.allocate(bytes.len());          // single arena slot
        let slot = memory.get_mut(mem_id).unwrap();
        slot.apply(|mem| mem.extend_from_slice(&bytes));    // copy into arena
        let (ptr, len) = slot.as_address().unwrap();
        self.send_to_js(mem_id, ptr, len);                  // FFI: host_batch_apply(idx,gen,ptr,len)
        SendResult { memory_id: mem_id, op_count, encoded_bytes: bytes.len() }
    }
    fn ack(&self, ids: &[MemoryId], memory: &mut MemoryAllocations) {
        for &id in ids { memory.dispose(id); }              // frees slot, increments generation
    }
}
```

### CustomBinaryV1 (protocol_byte=0, 2 MemoryIds per batch)
```rust
pub struct CustomBinaryV1 { encoder: CustomBinaryEncoder }

impl ProtocolMethods<Vec<DomOp>> for CustomBinaryV1 {
    fn encode_and_send(&self, ops: Vec<DomOp>, memory: &mut MemoryAllocations) -> SendResult {
        let op_count = ops.len();
        let bytes = self.encoder.encode(ops);
        let (ops_bytes, text_bytes) = self.split_regions(&bytes);
        let ops_id = memory.allocate(ops_bytes.len());       // slot 1: op structs
        let txt_id = memory.allocate(text_bytes.len());       // slot 2: text pool
        memory.get_mut(ops_id).unwrap().apply(|m| m.extend_from_slice(&ops_bytes));
        memory.get_mut(txt_id).unwrap().apply(|m| m.extend_from_slice(&text_bytes));
        let (op, ol) = memory.get(ops_id).unwrap().as_address().unwrap();
        let (tp, tl) = memory.get(txt_id).unwrap().as_address().unwrap();
        self.send_binary_to_js(ops_id, op, ol, txt_id, tp, tl);  // 8-param FFI
        SendResult { memory_id: ops_id, op_count, encoded_bytes: bytes.len() }
    }
}
```
Two slots so JS can access the text pool independently via TextDecoder without offset arithmetic.

### JsonV1 (protocol_byte=2, 1 MemoryId per batch)
```rust
pub struct JsonV1 { encoder: JsonEncoder }
```
Same single-slot pattern as ArrowV1. Intended for debugging and SSR. Not recommended for production WASM due to payload size and parse overhead.

## 5. Runtime Ownership & Construction

InstructionReceiver is owned by `wasm_ui::Runtime`, not by contexts or components.

```rust
pub struct Runtime { receiver: InstructionReceiver }

pub struct RuntimeBuilder {
    protocol: Option<Box<dyn ProtocolMethods<Vec<DomOp>>>>,
    memory: Option<MemoryAllocations>,
}
impl RuntimeBuilder {
    pub fn protocol(mut self, p: impl ProtocolMethods<Vec<DomOp>> + 'static) -> Self {
        self.protocol = Some(Box::new(p)); self
    }
    pub fn memory(mut self, m: MemoryAllocations) -> Self { self.memory = Some(m); self }
    pub fn build(self) -> Runtime {
        let protocol = self.protocol.expect("protocol is required");
        let memory = self.memory.expect("memory is required");
        Runtime { receiver: InstructionReceiver::new(protocol, memory) }
    }
}
```
**Application-level wiring:**
```rust
let wasm_ui = wasm_ui::Runtime::builder()
    .protocol(ArrowV1::new())
    .memory(MemoryAllocations::new(/* initial_slots: 16 */))
    .build();
let app = AppOrchestrator { signals_runtime, wasm_ui_runtime: wasm_ui };
```

## 6. Stabilize-Flush Integration

```
1. Effects run during stabilize(), each calls receiver.queue(DomOp::*)
2. stabilize() completes all dirty nodes
3. NotificationManager::on_stabilize_complete() fires → calls receiver.flush()
4. flush() drains ops → protocol.encode_and_send(ops, memory)
5. JS applies batch synchronously (no async gap)
6. JS calls dispose_allocation(mem_idx, gen) for each MemoryId
7. Rust frees arena slots (generation incremented on recycle)
```
**Critical invariant:** No async gap between step 4 and step 6. JS processes in the same synchronous call chain. JS always calls `dispose_allocation` even on error — prevents arena leaks.

## 7. Mock Protocol for Testing

```rust
pub struct MockProtocol {
    pub sent_batches: RefCell<Vec<Vec<DomOp>>>,  // all batches in order
    pub acked_ids: RefCell<Vec<MemoryId>>,        // all ACKed MemoryIds
    next_id: Cell<u32>,                            // monotonic, starts at 0
}
impl ProtocolHandler for MockProtocol {
    fn protocol_byte(&self) -> u8 { 255 }  // reserved for testing
    fn version(&self) -> u8 { 0 }
    fn send_to_js(&self, _: MemoryId, _: *const u8, _: usize) {}  // no-op
    fn handle_from_js(&self, _: MemoryId, _: *const u8, _: usize) {}
}
impl ProtocolMethods<Vec<DomOp>> for MockProtocol {
    fn encode_and_send(&self, ops: Vec<DomOp>, _: &mut MemoryAllocations) -> SendResult {
        let n = ops.len(); self.sent_batches.borrow_mut().push(ops);
        let id = self.next_id.get(); self.next_id.set(id + 1);
        SendResult { memory_id: MemoryId { index: id, generation: 0 }, op_count: n, encoded_bytes: 0 }
    }
    fn ack(&self, ids: &[MemoryId], _: &mut MemoryAllocations) {
        self.acked_ids.borrow_mut().extend_from_slice(ids); }
}
```

## 8. Error Cases & Edge Cases

| Scenario | Behavior |
|----------|----------|
| Empty batch (no ops queued) | `flush()` returns immediately. No protocol call, no allocation, flush_count unchanged. |
| Allocation failure (arena full) | `memory.allocate()` panics. Arena sized for worst case. Future: `try_allocate` returning `Option<MemoryId>`. |
| Stale MemoryId | `memory.get(id)` returns `None` on generation mismatch. Fresh IDs use `.unwrap()` — stale access is a bug. |
| JS ACK not called | Arena slot leaks. Detectable via `memory.active_count()`. Debug builds log warning past threshold. |
| JS processing error | JS still calls `dispose_allocation` (contract). Error reported via JS console, not Rust ACK path. |
| Double flush (no ops between) | No-op (ops empty). Safe and idempotent. |
| Queue after flush, before next stabilize | Ops accumulate normally for the next flush cycle. |
| Very large batch (10,000+ ops) | Works but slow. No artificial limit imposed. |
| Protocol encode failure | Infallible for well-formed DomOps (enum is exhaustive, decision from F01). |

## 9. Integration Points

| Feature | Connection |
|---------|-----------|
| F01 (foundation_ui_traits) | Imports `DomOp`, `ProtocolEncoder<T>`, `DecodeResult`, `DecodeError` |
| F02 (Signal System) | Effects call `receiver.queue()`. `NotificationManager` triggers flush after stabilize. |
| F00 (foundation_wasm) | Imports `MemoryAllocations`, `MemoryId`, `ProtocolHandler`, FFI externs |
| F05 (Arrow Encoding) | `ArrowEncoder` composed by `ArrowV1` |
| F06 (Web Components) | Component mount/unmount generates DomOps queued into receiver |
| F07 (DOM Morphing) | `DomOp::MorphNode` queued for HTML diff-and-patch |
| F08 (Event Runtime) | JS events → signal updates → effects → queued ops (indirect upstream) |

## 10. File Ownership

```
crates/foundation_wasm_ui/src/
├── instruction/{mod.rs, receiver.rs}        // InstructionReceiver, queue(), flush()
├── protocol/{mod.rs, arrow_v1.rs, custom_binary_v1.rs, json_v1.rs, mock.rs}
│                                             // ProtocolMethods<T>, SendResult, HandleResult, impls
├── runtime.rs                                // wasm_ui::Runtime, RuntimeBuilder
└── lib.rs                                    // crate root, re-exports
```

## 11. Refactoring Strategy

**Phase 1 — Trait + Mock:** Define `ProtocolMethods<T>`, `SendResult`, `HandleResult`. Implement `MockProtocol`. Write `InstructionReceiver`. All unit tests pass against mock.
**Phase 2 — ArrowV1:** Compose `ArrowEncoder` (F01) + `ProtocolHandler` FFI. Integration test: verify arena slot contains valid Arrow IPC.
**Phase 3 — CustomBinaryV1:** Two-slot allocation. Verify byte-identical output against existing `ops.rs` in `foundation_wasm`.
**Phase 4 — JsonV1:** Single-slot JSON. Verify output parseable by `serde_json::from_slice`.
**Phase 5 — Runtime wiring:** Wire into `wasm_ui::Runtime` via builder. Register `NotificationManager` for flush. End-to-end: signal change → stabilize → flush → mock captures correct DomOps.

## 12. Testing

### Queue & Flush (tests 1-9)

| # | Scenario | Verify |
|---|----------|--------|
| 1 | Queue 3 ops, flush | MockProtocol receives single batch of 3 ops in queue order |
| 2 | Queue 0 ops, flush | No batches sent (batch_count == 0) |
| 3 | Queue 5, flush, queue 3, flush | Two batches: first 5 ops, second 3 ops |
| 4 | Flush twice, no queue between | Second flush is no-op, batch_count unchanged |
| 5 | Queue 1000 ops, flush | Single batch of 1000, correct order |
| 6 | flush_count after 3 non-empty flushes | flush_count == 3; empty flushes do not increment |
| 7 | Queue SetText(1), SetAttribute(1), SetText(2) | Batch preserves exact insertion order |
| 8 | Two effects in same stabilize cycle | All ops in single batch, effect-1 before effect-2 (height order) |
| 9 | Same node_id, different op types | No dedup — both ops present |

### Protocol Integration (tests 10-14)

| # | Scenario | Verify |
|---|----------|--------|
| 10 | ArrowV1 encode_and_send, 5 mixed DomOps | Valid MemoryId, op_count==5, encoded_bytes>0 |
| 11 | CustomBinaryV1 encode_and_send | Exactly 2 arena slots allocated (memory.active_count()) |
| 12 | JsonV1 encode_and_send | Arena slot contains valid JSON (serde_json parseable) |
| 13 | ArrowV1 ack frees memory | After ack, memory.get(id) returns None |
| 14 | Protocol byte values | Arrow==1, CustomBinary==0, JSON==2 |

### Memory Lifecycle (tests 15-18)

| # | Scenario | Verify |
|---|----------|--------|
| 15 | Allocate, write, read back | Arena slot contains exact encoded bytes |
| 16 | Dispose then access | memory.get() returns None |
| 17 | Generation prevents stale access | Allocate A, dispose, allocate B (same index). Old MemoryId returns None. |
| 18 | 10 flush+ack cycles | memory.active_count() == 0 after all ACKs (no leaks) |

### Runtime Builder & Error Paths (tests 19-26)

| # | Scenario | Verify |
|---|----------|--------|
| 19 | Builder with ArrowV1 + memory | build() succeeds, pending_count() == 0 |
| 20 | Builder missing protocol | Panics "protocol is required" |
| 21 | Builder missing memory | Panics "memory is required" |
| 22 | End-to-end: build, queue, flush | MockProtocol receives ops through builder-constructed receiver |
| 23 | Empty flush | No panic, no allocation, flush_count unchanged |
| 24 | Double flush back-to-back | Second is no-op, batch_count unchanged |
| 25 | Stale MemoryId after dispose | memory.get(stale_id) returns None, no panic |
| 26 | Large batch (5000 ops) | Completes without error, op_count == 5000 |

### Mock Protocol (tests 27-29)

| # | Scenario | Verify |
|---|----------|--------|
| 27 | 3 flush cycles | sent_batches.len() == 3, each batch correct |
| 28 | ack with 2 MemoryIds | acked_ids contains both |
| 29 | protocol_byte | Returns 255 (reserved test value) |
