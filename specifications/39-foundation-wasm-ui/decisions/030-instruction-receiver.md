# 030 — Runtime-owned InstructionReceiver replaces global FRAME_BATCH

**Date:** 2026-06-09  
**Status:** Resolved

### Decision

**No global `FRAME_BATCH`.** The `Runtime` owns an `InstructionReceiver` instance — a batching component that receives DOM operations from effects, encodes them via the configured protocol, and handles the FFI handoff.

**Protocols are per-message, not per-session.** There is no "session protocol" that locks the system into one format. Every message carries its own protocol byte in the envelope (`[protocol: u8][version: u8][memory_id: u64][length: u32]`). The receiver reads the first byte and dispatches to the correct protocol handler. Messages can use different protocols back and forth — one message might be Arrow, the next might be JSON, the next Custom Binary. The envelope is the demux layer.

```
stabilize() completes
    ↓
InstructionReceiver.take_ops() — drains pending ops
    ↓
protocol encoder: Vec<DomOp> → Arrow / JSON / custom binary (whatever is configured)
    ↓
envelope: [protocol byte][version][memory_id][length][payload...]
    ↓
protocol FFI: host_arrow_apply / host_batch_apply / host_json_apply
    ↓
JS reads envelope byte → dispatches to correct handler → applies, ACKs, frees memory
```

### How it works

```rust
pub struct InstructionReceiver {
    ops: Vec<DomOp>,
    protocol: Box<dyn ProtocolMethods<Vec<DomOp>>>,
    memory: MemoryAllocations,
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
        self.protocol.encode_and_send(ops, &mut self.memory);  // Layer 1: encode, Layer 2: ship via FFI
    }
}
```

### Protocol encoding + FFI dispatch — three-layer composition

Each WASM protocol impl composes an encoder (from `foundation_ui_traits`) with WASM transport (from `foundation_wasm`). The encoding step is pure — it produces `Vec<u8>` with no WASM dependency, so HTTP servers, SSE endpoints, and WebSocket servers can use the same encoders directly.

```rust
// Layer 1: Encoding — in foundation_ui_traits (no WASM, no FFI)
// ArrowEncoder, JsonEncoder, CustomBinaryEncoder all impl ProtocolEncoder<Vec<DomOp>>

// Layer 2: WASM Transport — in foundation_wasm
// ProtocolHandler trait: send_to_js(MemoryId, ptr, len), handle_from_js(MemoryId, ptr, len)

// Layer 3: Composed — in foundation_wasm_ui
impl ProtocolMethods<Vec<DomOp>> for ArrowV1 {
    fn encode_and_send(&self, ops: Vec<DomOp>, memory: &mut MemoryAllocations) -> SendResult {
        let bytes = self.encoder.encode(ops);           // Layer 1: pure encoding
        let mem_id = memory.allocate(bytes.len());
        let slot = memory.get(mem_id).unwrap();
        slot.apply(|mem| mem.extend_from_slice(&bytes));

        let (ptr, len) = slot.as_address().unwrap();
        self.send_to_js(mem_id, ptr, len);              // Layer 2: WASM transport
        SendResult { memory_id: mem_id }
    }
}

impl ProtocolMethods<Vec<DomOp>> for CustomBinaryV1 {
    fn encode_and_send(&self, ops: Vec<DomOp>, memory: &mut MemoryAllocations) -> SendResult {
        let bytes = self.encoder.encode(ops);           // Layer 1: pure encoding
        // Custom binary uses 2 arena slots (ops + text) — protocol-specific allocation
        let (ops_mem, text_mem) = self.split_and_allocate(bytes, memory);
        let (ops_ptr, ops_len) = ops_mem.as_address().unwrap();
        let (text_ptr, text_len) = text_mem.as_address().unwrap();
        unsafe {
            host_batch_apply(
                ops_mem.id.as_u64(), ops_ptr as u64, ops_len as u64,
                text_mem.id.as_u64(), text_ptr as u64, text_len as u64,
            );
        }
        SendResult { memory_id: ops_mem.id }
    }
}
```

**Usage outside WASM** — the encoder is independently usable:

```rust
// HTTP server (no foundation_wasm dependency)
let encoder = ArrowEncoder;
let bytes = encoder.encode(dom_ops);
response.body(bytes).content_type("application/primal-arrow")

// SSE endpoint
let encoder = JsonEncoder;
let bytes = encoder.encode(signal_patches);
sse_stream.send_event("patch", &bytes)
```
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

Each crate owns its own Runtime:

```rust
// foundation_signals — owns the signal graph
fn init_signals() -> signals::Runtime {
    signals::Runtime::builder().build()
}

// foundation_wasm_ui — owns the instruction receiver + protocol dispatch
fn init_wasm_ui() -> wasm_ui::Runtime {
    wasm_ui::Runtime::builder()
        .protocol(ArrowProtocol::new())   // default protocol for sending DOM ops
        .memory(memory_allocations)
        .build()
}

// Application code creates an orchestrator Runtime that holds both
fn init() {
    let signals_runtime = init_signals();
    let wasm_ui_runtime = init_wasm_ui();
    let app = AppOrchestrator { signals_runtime, wasm_ui_runtime };
}
```

The `protocol` configured at init is the **default** for sending DOM operations. But incoming messages can use any protocol — the envelope byte dispatches to the correct handler. There's no session lock-in.

### Why this design

- **No global state** — `InstructionReceiver` belongs to the Runtime, not a `static mut` or `LazyCell`
- **Encoding is WASM-independent** — `ProtocolEncoder<T>` in `foundation_ui_traits` produces pure `Vec<u8>`. HTTP servers, SSE endpoints, WebSocket servers use the same encoders without any WASM dependency.
- **Three clean layers** — encoding (foundation_ui_traits) → transport (foundation_wasm) → composed impls (foundation_wasm_ui). Each layer is independently usable.
- **Protocol impls compose encoder + transport** — `ProtocolMethods<T>: ProtocolHandler` composes Layer 1 encoding with Layer 2 WASM transport. `InstructionReceiver` holds `Box<dyn ProtocolMethods<Vec<DomOp>>>` and calls `encode_and_send(ops, memory)`.
- **Per-message protocol, not per-session** — the envelope byte demuxes every message independently. One message can be Arrow, the next JSON, the next Custom Binary.
- **Same effect API** — effects call `receiver.queue(op)` unchanged. They don't know or care about protocols.
- **Single batch per stabilize** — all effects in one cycle queue into the same receiver, flushed once
- **Testable** — mock protocol implementations for unit testing (collect ops without encoding)
- **Extensible** — adding a new protocol means implementing `ProtocolEncoder<T>` + `ProtocolHandler`, no changes to effect code
- **User can intercept** — swap the default protocol at init time to add logging, filtering, or custom encoding
