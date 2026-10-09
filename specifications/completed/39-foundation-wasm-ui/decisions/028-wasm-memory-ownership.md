# 028 — WASM boundary memory management and buffer ownership

**Date:** 2026-06-09
**Status:** Resolved

### Decision

**Each protocol owns its complete communication contract** — memory layout, handoff semantics, ACK/release, and entry points. The protocol byte doesn't just tag a format; it selects an entirely self-contained protocol handler.

**Transport is feature-detected at runtime:** `SharedArrayBuffer` + `Atomics` when server headers support it, `postMessage` with Transferable as the fallback.

---

## 1. Protocol-as-contract

The message envelope `[protocol: u8][version: u8][payload...]` is a **demux**, not a shared contract. After reading the protocol byte, each protocol handler owns everything about how that message is structured, delivered, and acknowledged.

| Protocol byte | Name | Memory layout | Handoff | ACK |
|--------------|------|--------------|---------|-----|
| 0 | Custom Binary (foundation_wasm Instructions) | `Params` binary encoding, separate ops + text arenas | raw ptr + len via `host_batch_apply` | synchronous (current bug: slots never freed — see §2) |
| 1 | Arrow | Arrow IPC columnar format | protocol-defined (see §3) | protocol-defined (see §3) |
| 2 | JSON | JSON text | protocol-defined | protocol-defined |

**No cross-protocol coupling.** Arrow is not forced through `host_batch_apply`. JSON doesn't reuse the Instructions arena system. Each protocol gets its own entry point, its own memory management, its own lifecycle.

---

## 2. Custom Binary Protocol (protocol 0) — fix the slot leak

**Current behavior** (`jsapi.rs:1032-1048`):
```rust
pub fn batch(instruction: CompletedInstructions) {
    let ops = internal_api::get_memory(instruction.ops_id);
    let text = internal_api::get_memory(instruction.text_id);
    let (ops_ptr, ops_len) = ops.as_address().unwrap();
    let (text_ptr, text_len) = text.as_address().unwrap();

    unsafe {
        host_batch_apply(ops_ptr as u64, ops_len, text_ptr as u64, text_len);
    };
    // ← ops_id and text_id are NEVER deallocated. Slot sits forever.
}
```

The `MemoryAllocations` system already has generation-based IDs (`MemoryId(index, generation)`) that prevent stale access after reuse. The fix is simple: **pass the memory IDs alongside pointers, and JS calls back to release.**

**Fixed `host_batch_apply` signature:**
```rust
pub fn host_batch_apply(
    ops_memory_id: u64,    // MemoryId::as_u64() — index + generation packed
    ops_pointer: u64,
    ops_length: u64,
    text_memory_id: u64,   // same
    text_pointer: u64,
    text_length: u64,
);
```

**JS side:** after applying the batch, call `dispose_allocation(ops_id)` and `dispose_allocation(text_id)` to return slots to the arena. The generation ID ensures JS can't accidentally free a slot that's already been reallocated.

**New JS host function:**
```rust
#[no_mangle]
pub extern "C" fn dispose_allocation(allocation_id: u64) {
    // already exists in exposed_runtime — just need JS to call it
}
```

This is a **backward-compatible fix** to the existing protocol. No redesign of the Instructions encoding needed.

---

## 3. Arrow Protocol (protocol 1) — designed from scratch

Arrow is a new protocol with its own contract. We steal what works from Custom Binary (the `MemoryId` generation system, the arena allocator) and fix what doesn't.

### Message layout

All protocols share the same 10-byte common envelope (decision 014):

```
[protocol: u8][version: u8][batch_memory_id: u64][payload...]
```

The `batch_memory_id` is the arena slot holding the entire message. JS reads it from the envelope, processes the payload, then calls `dispose_allocation(memory_id)` to free the slot.

For Arrow specifically, the payload is:

```
[arrow_ipc_length: u32][Arrow IPC buffer...]
```

- `arrow_ipc_length`: length of the Arrow IPC data within that slot
- The Arrow IPC buffer starts at `slot_ptr + 0` — no offset, no separate ops/text arenas

### Single arena slot per batch

Unlike Custom Binary (separate ops arena + text arena), Arrow is self-contained — the IPC buffer includes all columns. **One `MemoryId`, one slot, one ACK.**

### Handoff and ACK

```rust
// WASM side: produce Arrow batch
fn send_arrow_batch(ops: &[DomOp]) {
    let arrow_ipc = encode_arrow_ipc(ops);          // Vec<u8>
    let mem_id = ALLOCATIONS.allocate(arrow_ipc.len());  // arena slot
    let slot = ALLOCATIONS.get(mem_id).unwrap();
    slot.apply(|mem| mem.extend_from_slice(&arrow_ipc));

    // Pass to JS via the Arrow protocol entry point
    let (ptr, len) = slot.as_address().unwrap();
    unsafe {
        host_arrow_apply(mem_id.as_u64(), ptr as u64, len);
    }
}
```

```javascript
// JS side: Arrow protocol handler
function host_arrow_apply(ptr, len) {
    // 1. Read memory_id from common envelope (bytes 2-9)
    const memoryId = new DataView(wasmMemory.buffer, ptr + 2, 8).getBigUint64(0, true);

    // 2. Mark slot as borrowed (prevents WASM arena from reusing it)
    borrowedSlots.add(memoryId);

    try {
        // 3. Zero-copy read Arrow IPC from payload offset (skip 10-byte envelope + 4-byte length)
        const ipcOffset = 10 + 4;
        const view = new Uint8Array(wasmMemory.buffer, ptr + ipcOffset, len - ipcOffset);

        // 4. Parse Arrow IPC (TypedArray column views — zero-copy)
        const columns = ArrowParser.parse(view);

        // 5. Apply DOM ops synchronously
        ArrowDomApplicator.apply(columns);

        // 6. Release the slot back to WASM arena
        wasmExports.dispose_allocation(memoryId);
    } catch (err) {
        console.error('Arrow apply failed:', err);
        wasmExports.dispose_allocation(memoryId);
        throw err;
    } finally {
        borrowedSlots.delete(memoryId);
    }
}
```

**Synchronous apply, no async gap.** `host_arrow_apply` runs entirely in the same call stack as the WASM code that invoked it. WASM doesn't resume until JS returns. DOM ops are applied before the call returns — no `requestAnimationFrame` deferral.

**Error handling** is JS-side and uniform across all protocols: if an operation fails, JS logs to console, throws an error for developers to debug, and always calls `dispose_allocation` to prevent slot leaks.

### Why this is better than Custom Binary for Arrow

| Aspect | Custom Binary (fixed) | Arrow Protocol |
|--------|----------------------|----------------|
| Arenas | 2 separate (ops + text) | 1 self-contained |
| Memory IDs passed | 2 (`ops_id` + `text_id`) | 1 (`batch_memory_id`) |
| Encoding | row-by-row `Params` | columnar Arrow IPC |
| JS parsing | instruction decoder | TypedArray column views |
| Slot count per batch | 2 | 1 |

---

## 4. Feature detection: SharedArrayBuffer vs Transferable

On initialization, `foundation-wasm.js` probes:

```javascript
function detectTransportCapability() {
    try {
        const sab = new SharedArrayBuffer(64);
        const view = new Int32Array(sab);
        Atomics.store(view, 0, 0);
        const result = Atomics.wait(view, 0, 0, 0);
        if (result === 'timed-out' || result === 'ok') {
            return { mode: 'shared' };
        }
    } catch (e) {
        // COOP/COEP not set
    }
    return { mode: 'transfer' };
}
```

- **`mode: 'shared'`** — WASM memory instantiated with `WebAssembly.Memory({ shared: true })`. Arrow batches read directly from shared memory. Worker communication uses atomic ring buffers (`Atomics.wait`/`Atomics.notify`).
- **`mode: 'transfer'`** — WASM memory is private per instance. Arrow batches sent via `postMessage(buffer, [buffer.buffer])` — Transferable moves ownership. No shared memory, no atomics.

Components are **completely unaware** of the mode. Protocol handlers abstract it.

---

## 5. Worker communication

**Main thread runs JS only** — no WASM instance lives on the main thread. All WASM instances run in workers. The main thread handles DOM, event dispatch, and routing messages to the correct worker.

### Transfer mode (postMessage) — the cost of doing business without COOP/COEP

```
Main thread (JS only)          Web Worker
┌─────────────────┐           ┌──────────────────────┐
│ DOM             │           │ WASM (private memory)│
│ ArrowDomApplic  │           │ Component logic      │
│ Event dispatch  │           └──────────┬───────────┘
│ Worker router   │                      │
└────────┬────────┘                      │
         │                               │
         │ postMessage(buf, [buf])       │
         │◀── copy out, then transfer ───│
         │  (main's ref is null)         │
         │                               │
         │ ArrowDomApplicator.apply      │
         │ (zero-copy read from buffer)  │
         │                               │
         │ postMessage(result, [r]) ────▶│  (result transferred, worker copies in)
```

**The unavoidable cost:**
- Worker → Main: WASM memory → ArrayBuffer copy → `postMessage` transfer → main reads directly
- Main → Worker: ArrayBuffer → `postMessage` transfer → worker copies into its WASM memory
- **Two copies per round-trip** — one to extract from WASM memory, one to inject back in

This is the tax for not having COOP/COEP headers. No way to avoid it with WebWorkers.

### Shared mode (SharedArrayBuffer) — the copy tax disappears

```
SHARED MEMORY (all threads see the same bytes)
┌────────────────────────────────────────────────────────────┐
│  Main thread (JS)    Web Worker 1    Web Worker 2          │
│  DOM/Events/Router   WASM (A)        WASM (B)              │
│                                                              │
│  Ring Buffer in shared memory:                              │
│  [head: AtomicU32] [tail: AtomicU32]                       │
│  [msg1_len][msg1_data...] ← written by Worker A            │
│  [msg2_len][msg2_data...] ← written by Worker B            │
│  [ACK flags...]                                             │
└────────────────────────────────────────────────────────────┘
```

**Zero copies:**
- Worker A writes at offset X → `Atomics.notify(head)` → main thread reads same bytes directly
- Main thread writes response → `Atomics.notify(tail)` → Worker A reads same bytes directly
- Worker → Worker: same shared buffer, zero copy, no routing through main

Protocol handler abstracts both modes. Application code never knows which transport is active.

---

## 6. Scenario resolutions

### WASM allocates → JS reads (Arrow batch)
1. WASM allocates arena slot → gets `MemoryId(index, generation)`
2. Encodes full envelope (protocol + version + memory_id + payload) into slot
3. Calls `host_arrow_apply(ptr, len)`
4. JS reads memory_id from envelope, parses Arrow IPC, applies DOM ops
5. JS calls `dispose_allocation(memory_id)` — slot returns to arena
6. Generation ID prevents stale reuse: if WASM reallocates the same index, the generation increments, making any old `MemoryId` invalid

### JS passes data → WASM (same loan pattern, inverted)

JS is the producer, WASM is the consumer — identical loan semantics, just flipped direction. **The protocol handler abstracts how data reaches WASM — it's transport-aware, not allocator-aware:**

```javascript
// JS side: send event payload to WASM — protocol handler abstracts the transport
function sendEventToWasm(eventData) {
    const json = JSON.stringify(eventData);
    const bytes = new TextEncoder().encode(json);
    this.protocol.sendToWasm(bytes);  // transport handled underneath
}
```

The protocol handler sends via whichever transport is active:

| Transport | How JS produces data | WASM receives |
|-----------|---------------------|---------------|
| **SharedArrayBuffer** | Writes directly into WASM's linear memory: `new Uint8Array(wasmMemory.buffer, offset, len)`. Arena slot managed by WASM `MemoryAllocations`. | Reads directly from its own memory — zero-copy at the boundary |
| **Transferable (`postMessage`)** | Creates a separate ArrayBuffer, copies data in, transfers ownership: `postMessage(buf, [buf.buffer])` | WASM copies into its own memory from the transferred buffer |
| **HTTP (ServiceWorker/Server)** | Serializes into a `fetch` request body to `/primal/messages` | Server/WASM decodes from the HTTP payload |

```rust
// WASM side: receive event payload — same code regardless of transport
#[no_mangle]
pub extern "C" fn handle_event(memory_id: u64, ptr: u64, len: u64) {
    // 1. Read directly from WASM's own linear memory (JS wrote it there via shared memory)
    //    Zero-copy at the boundary — same memory space.
    let data = unsafe {
        core::slice::from_raw_parts(ptr as *const u8, len as usize)
    };

    // 2. Copy into owned Rust structs for safety — data must survive
    //    after JS releases the slot (which could happen immediately after this call returns).
    let owned: Vec<u8> = data.to_vec();
    process_event(&owned);

    // 3. Release the slot back to JS allocator
    js_dispose_allocation(memory_id);
}
```

**Transport-aware ACK** — the protocol handler knows which mode is active:

| Transport | JS side after `handle_event` | WASM ACK |
|-----------|----------------------------|----------|
| Transferable (`postMessage`) | `postMessage(buf, [buf.buffer])` → ownership already moved. WASM's `dispose` call is a no-op on JS side (buffer was transferred, no longer owned). | WASM reads from its own memory (JS wrote there before transfer) → frees synchronously → calls `dispose` → protocol ignores (JS already lost ownership) |
| SharedArrayBuffer | JS slot stays allocated in shared WASM memory. WASM reads directly — same memory. | WASM reads, copies into owned structs for safety → signals done → JS marks slot free for reuse |

Application code never branches on transport. The protocol handler abstracts it.

### Batch apply errors

All protocols follow the same JS-side error handling: if `apply` throws, JS logs to console, throws for developer debugging, and always calls `dispose_allocation(memoryId)` to prevent slot leaks. No protocol-specific retry asymmetry.

### Long-lived JS references to WASM allocations

Two separate safety mechanisms protect against stale references:

**Memory slots use generation IDs** (`MemoryAllocations`, `mem.rs`):
- `MemoryId = (index, generation)` — when a slot is freed and reallocated, generation increments
- Stale ID (old generation) → `get()` returns `Err(InvalidAllocationId)` → can't read wrong data
- Prevents **use-after-free on arena memory** — reading garbage from a reused slot

**Callbacks use monotonic IDs** (`InternalReferenceRegistry`, `registry.rs`):
- `InternalPointer` is a simple counter that only ever increments (`self.id += 1`)
- IDs are **never reused** — when a callback is deleted, that number is gone forever
- Stale ID → `tree.get(&id)` returns `None` → `invoke_callback` silently drops the call
- Prevents **stale callback execution** — calling the wrong callback at a reused slot

When WASM context drops (component unmount):
1. `onCleanup()` fires → effect disposal → callback `unregister(runtime)` is called
2. Registry deletes the callback ID from its `BTreeMap`
3. JS still holds the stale ID, but `invoke_callback(stale_id)` → `None` → dropped silently

### Circular references across the boundary

**Not applicable.** `foundation_wasm` does not use `wasm-bindgen` or hold any `JsValue` references. All cross-boundary communication is through integer IDs (`InternalPointer(u64)`, `ExternalPointer(u64)`) and raw pointers into WASM linear memory. Rust closures capture Rust data only. JS holds numbers. There is no cycle possible by design.

---

## 7. Protocol handler interface — three-layer design

The protocol system is split into three layers so that encoding is usable outside WASM (HTTP servers, SSE, WebSocket, etc.):

### Layer 1: Encoding (foundation_ui_traits) — no WASM dependency

Pure encoding — `DomOp → Vec<u8>`. Usable by any Rust binary.

```rust
// In foundation_ui_traits
pub trait ProtocolEncoder<T> {
    fn protocol_byte(&self) -> u8;
    fn version(&self) -> u8;
    fn encode(&self, data: T) -> Vec<u8>;
    fn decode(&self, payload: &[u8]) -> DecodeResult;
}

struct ArrowEncoder;          // DomOp → Arrow IPC bytes
struct JsonEncoder;           // DomOp → JSON bytes
struct CustomBinaryEncoder;   // DomOp → custom binary bytes
```

```rust
// HTTP server — no foundation_wasm dependency
let encoder = ArrowEncoder;
let bytes = encoder.encode(dom_ops);
response.body(bytes).content_type("application/primal-arrow")
```

### Layer 2: WASM Transport (foundation_wasm) — arena memory + FFI

Bidirectional transport contract — both WASM→JS and JS→WASM use the same loan pattern (producer allocates, consumer ACKs):

```rust
// In foundation_wasm
pub trait ProtocolHandler {
    fn protocol_byte(&self) -> u8;
    fn version(&self) -> u8;

    // WASM → JS: WASM produces, JS consumes and ACKs
    fn send_to_js(&self, memory_id: MemoryId, ptr: *const u8, len: usize);

    // JS → WASM: JS produces, WASM consumes and ACKs
    fn handle_from_js(&self, memory_id: MemoryId, ptr: *const u8, len: usize);
}
```

### Layer 3: WASM Protocol Impls (foundation_wasm_ui) — compose encoding + transport

```rust
// In foundation_wasm_ui — composes Layer 1 + Layer 2
pub trait ProtocolMethods<T>: ProtocolHandler {
    fn encode_and_send(&self, data: T, memory: &mut MemoryAllocations) -> SendResult;
    fn handle_received(&self, payload: &[u8]) -> HandleResult;
    fn ack(&self, memory_ids: &[MemoryId], memory: &mut MemoryAllocations);
}

struct ArrowV1;          // impl ProtocolHandler + ProtocolMethods<Vec<DomOp>>, uses ArrowEncoder
struct CustomBinaryV1;   // impl ProtocolHandler + ProtocolMethods<Vec<DomOp>>, uses CustomBinaryEncoder
struct JsonV1;           // impl ProtocolHandler + ProtocolMethods<Vec<DomOp>>, uses JsonEncoder
```

JS side mirrors the transport layer:

```javascript
class ProtocolHandler {
    // JS → WASM: JS produces, WASM consumes and ACKs
    sendToWasm(data) {
        // Protocol-specific encoding — transport handles the rest:
        //   SharedArrayBuffer: write to WASM memory, pass (memId, ptr, len)
        //   Transferable: copy to ArrayBuffer, postMessage with transfer
        //   HTTP: fetch('/primal/messages', { body: data })
        this.transport.send(data);
    }

    // WASM → JS: WASM produces, JS consumes and ACKs
    handleFromWasm(memId, ptr, len) {
        this.borrowedSlots.add(memId);
        try {
            this.apply(ptr, len);  // protocol-specific apply
            wasmExports.dispose_allocation(memId);  // ACK: release back to WASM arena
        } catch (err) {
            console.error('Batch apply failed:', err);  // log for developer
            wasmExports.dispose_allocation(memId);  // always free, even on error
            throw err;  // let developer fix it
        } finally {
            this.borrowedSlots.delete(memId);
        }
    }
}
```

Both sides use `SharedTransport` or `TransferTransport` — picked at init by `detectTransportCapability()`. The protocol handler doesn't care which transport is underneath; it only cares that the loan pattern is respected.

---

## 8. Why this design

- **Protocol isolation** — each protocol owns its contract; no cross-protocol leakage
- **Encoding is WASM-independent** — `ProtocolEncoder<T>` in `foundation_ui_traits` produces pure `Vec<u8>` with no FFI, no `MemoryAllocations`, no WASM dependency. HTTP servers, SSE endpoints, WebSocket servers, CLI tools all use the same encoders.
- **Three clean layers** — encoding (foundation_ui_traits) → transport (foundation_wasm) → composed impls (foundation_wasm_ui). Each layer is independently usable.
- **Arrow gets its own design** — not shoehorned into Custom Binary's ops+text arenas
- **Custom Binary gets a simple fix** — pass memory IDs, JS calls `dispose_allocation`
- **Generation IDs prevent memory use-after-free** — arena slot reuse increments generation, stale IDs fail validation
- **Monotonic callback IDs prevent stale execution** — callback IDs never reused, stale lookups return `None` and drop silently
- **Synchronous apply eliminates async gaps** — no `requestAnimationFrame` deferral in batch path
- **Graceful transport degradation** — SharedArrayBuffer when headers allow, Transferable fallback
- **Components unaware** — same `ctx.renderer()`, protocol handler abstracts delivery
- **One copy per direction at the boundary** — producer copies into WASM linear memory, consumer reads zero-copy (same address space), then copies into owned structs for lifetime safety
