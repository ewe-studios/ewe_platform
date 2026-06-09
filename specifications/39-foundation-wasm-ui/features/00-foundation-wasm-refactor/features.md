# Feature 00: foundation_wasm Refactor & Split

## Description

Refactor `foundation_wasm` into a pure runtime/ABI layer and create `foundation_wasm_ui` for all DOM/UI concerns. Rewrite the JS runtime (`megatron.js`) into two clean files. Clarify ownership of every type, function, and capability. The callback infrastructure, FFI functions, return value parsing, and all existing capabilities are preserved.

**Refactoring approach:** New code goes into new files alongside the old ones. Old files are not edited in place — the refactored versions are written separately, tested in `{crate}/tests/` for behavior parity, and only after all tests confirm identical functionality do we switch `lib.rs` to use the new modules and delete the old files. This ensures we can always compare old vs new and roll back if needed.

**Decisions:** 014, 015, 022, 028, 030

---

## Goal

After this refactor:
- `foundation_ui_traits` = encoding layer. `ProtocolEncoder<T>` trait + concrete encoders (Arrow, JSON, CustomBinary). Pure `DomOp → Vec<u8>` — no WASM, no FFI, no allocators. Usable by HTTP servers, WebSocket servers, SSE endpoints, CLI tools — anything that needs to produce protocol-encoded bytes.
- `foundation_wasm` = pure WASM↔JS ABI. Memory management, binary encoding, function invocation, callback system, timers, WASM transport trait (`ProtocolHandler`). No DOM, no window, no UI concepts.
- `foundation_wasm_ui` = all DOM/window/animation bindings + WASM protocol implementations (compose encoder + transport) + JS runtime assets.
- Every existing capability is preserved — we're organizing, not removing.
- Three-layer protocol architecture: encoding (foundation_ui_traits) → transport (foundation_wasm) → WASM protocol impls (foundation_wasm_ui).
- JS runtime split: `foundation-wasm.js` (core ABI) + `foundation-wasm-ui.js` (DOM layer).

---

## Part A: foundation_wasm File Map — What Stays

### src/lib.rs — STAYS (minor edits)

```rust
#![no_std]
extern crate alloc;

mod base;
mod error;
// mod frames;         ← MOVES to foundation_wasm_ui
mod intervals;
mod host_runtime;      ← NEW: split from jsapi.rs (FFI imports)
mod protocol;          ← NEW: split from jsapi.rs (return value parsing)
mod mem;
mod ops;
mod registry;
mod schedule;
mod wrapped;
```

### src/base.rs — STAYS (keep everything including ReturnTypeId::DOMObject)

ALL types stay. Every enum, struct, constant, quantization function:

| Type | Used By | Moves? |
|------|---------|--------|
| `TypedSlice` | ops.rs, jsapi.rs | No |
| `ReturnTypeId` (includes DOMObject ID 29) | jsapi.rs, error.rs | No |
| `ThreeState` / `ThreeStateId` | jsapi.rs | No |
| `ReturnEncoded` | ops.rs | No |
| `ReturnIds` | jsapi.rs | No |
| `ReturnTypeHints` | jsapi.rs, ops.rs | No |
| `Returns` / `ReturnValues` | jsapi.rs, error.rs | No |
| `ParamTypeId` / `Params<'a>` | ops.rs | No |
| `StrLocation` | ops.rs | No |
| `ReturnValueMarker` / `GroupReturnHintMarker` / `ReturnHintMarker` | jsapi.rs, ops.rs | No |
| `ArgumentOperations` / `Operations` | ops.rs | No |
| `CallParams` / `CachedText` | jsapi.rs | No |
| `InternalPointer` / `ExternalPointer` | jsapi.rs, registry.rs | No |
| `TypeOptimization` + `value_quantitization` mod | ops.rs | No |
| `JSEncoding` | jsapi.rs | No |
| `MemoryLocation` | ops.rs, jsapi.rs | No |
| `MemoryId` | mem.rs, ops.rs, jsapi.rs | No |
| `CompletedInstructions` | ops.rs, jsapi.rs | No |

**ReturnTypeId::DOMObject (ID 29):** Stays. It's used by `ReturnValueParserIter` (jsapi.rs:2129-2140) to decode DOM object return values from the binary stream. The parsing code is protocol-agnostic — it parses `ReturnTypeId` values from binary, doesn't care what they represent.

### src/error.rs — STAYS (no changes)

All error types stay. They're used by base.rs types, mem.rs, ops.rs, jsapi.rs — none of which are fully removed.

### src/mem.rs — STAYS (no changes)

| Type | Purpose |
|------|---------|
| `MemoryAllocation` | Arc-wrapped Vec<u8> handle — core arena building block |
| `MemorySlot` | Pairs ops + text MemoryAllocations (Custom Binary concept) |
| `MemoryAllocations` | Arena allocator with generation-based IDs |
| `ToBinary` trait | Binary encoding for Params, ReturnTypeHints |
| `FromBinary` trait | Binary decoding for ReturnTypeHints, GroupReturnTypeHints |
| `BatchEncodable` trait | Encoder interface for ops.rs |
| `Batchable<'a>` trait | Encode Params into BatchEncodable |

### src/ops.rs — STAYS (no changes, 2827 lines)

This is the Custom Binary protocol encoding engine. It stays because the protocol struct uses it.

### src/registry.rs — STAYS (no changes)

`InternalReferenceRegistry` — monotonic callback ID registry. Pure ABI.

### src/schedule.rs — STAYS (no changes)

`ScheduleRegistry` — setTimeout-style callbacks. Pure ABI.

### src/intervals.rs — STAYS (no changes)

`IntervalRegistry` — setInterval-style callbacks. Pure ABI.

### src/wrapped.rs — STAYS (no changes)

Wrapper types for FFI. Pure ABI.

### src/frames.rs → MOVES to foundation_wasm_ui

All animation frame types move: `TickState`, `FrameCallback` trait, `FnFrameCallback`, `FrameCallbackList`. These are DOM-specific (`requestAnimationFrame`).

### src/jsapi.rs (3175 lines) → SPLIT into 3 files

#### NEW src/protocol.rs — STAYS in foundation_wasm

**WASM transport trait, envelope parsing, and return value parsing infrastructure stays.**

This is the WASM-specific transport layer — how encoded bytes cross the WASM↔JS boundary via arena memory. The encoding itself (DomOp → bytes) lives in `foundation_ui_traits` so it's usable outside WASM (HTTP servers, SSE, WebSocket, etc.).

| Code | Lines | Purpose |
|------|-------|---------|
| `ProtocolHandler` trait | NEW | WASM transport contract — `send_to_js(MemoryId, ptr, len)`, `handle_from_js(MemoryId, ptr, len)` |
| `Envelope` struct + parse/write | NEW | 14-byte message envelope — `[protocol][version][memory_id][length]` |
| `dispatch_message()` | NEW | Reads protocol byte, routes to correct handler |
| `ReturnValueParserIter` struct + impl | jsapi.rs:1857-2914 | Binary return value parser — reads `ReturnTypeId` values including DOMObject |
| `FromBinary for ReturnTypeHints` | jsapi.rs:2916-2950 | Decodes return type hints from binary |
| `GroupReturnTypeHints` struct + impl | jsapi.rs:2952-3174 | Group return parsing — parses batched return values |

#### NEW src/host_runtime.rs — STAYS in foundation_wasm

**ALL FFI imports and function invocation stay:**

| Code | Purpose |
|------|---------|
| FFI imports: `hook_up_animation_frames`, `schedule_timeout`, etc. | All host API imports |
| `host_invoke_function_as_*` (all variants) | WASM→JS sync function calls |
| `host_invoke_async_function` | WASM→JS async function calls |
| `host_invoke_function` | WASM→JS general invocation |
| `host_cache_string` | String caching |
| `host_register_function` / `host_unregister_function` | JS function registration |
| `function_allocate_external_pointer` / `object_allocate_external_pointer` | External pointer allocation |
| All stub implementations for non-wasm targets | Build-time safety |
| `cache_text()` | String caching helper |
| `register_function()` / `register_function_utf16()` | Function registration helpers |
| `invoke_as_*` functions | Typed invocation helpers |
| `invoke()` / `invoke_for_replies()` / `invoke_for_str()` | General invocation |
| `HostFunction` struct + methods | Function handle wrapper |
| `invoke_for_none()` / `invoke_for_bool()` / `invoke_for_*` | Typed result extraction |
| `invoke_for_object()` | Generic object invocation |
| `batch()` — MODIFIED for ACK | Sends DOM ops batch (adds memory IDs) |
| `batch_response()` — MODIFIED for ACK | Sends DOM ops batch, gets returns |
| `register_schedule()` / `unregister_schedule()` | setTimeout wrapper |
| `register_interval()` / `unregister_interval()` | setInterval wrapper |

**Key FFI import signatures** (`extern "C"` functions imported from the JS host). All imports have `#[cfg(target_arch = "wasm32")]` guards; non-WASM builds provide stub implementations that panic.

```rust
// --- Protocol batch imports ---
fn host_batch_apply(                              // Custom Binary: 2 arena slots (ops + text)
    ops_mem_id: u64, ops_ptr: u64, ops_len: u64,
    text_mem_id: u64, text_ptr: u64, text_len: u64,
);
fn host_arrow_apply(mem_id: u64, ptr: u64, len: u64);  // Arrow: 1 arena slot
fn host_json_apply(mem_id: u64, ptr: u64, len: u64);   // JSON: 1 arena slot

// --- Timers ---
fn schedule_timeout(callback_id: u64, delay_ms: u32);   // setTimeout
fn cancel_timeout(callback_id: u64);
fn schedule_interval(callback_id: u64, interval_ms: u32); // setInterval
fn cancel_interval(callback_id: u64);

// --- Animation ---
fn hook_up_animation_frames();  // Start JS rAF loop -> calls trigger_animation_callbacks

// --- String cache + function registry ---
fn host_cache_string(ptr: u64, len: u64) -> u64;                    // Cache UTF-8 string, returns handle
fn host_register_function(name_ptr: u64, name_len: u64) -> u64;     // Register named JS fn, returns handle
fn host_unregister_function(handler: u64);                           // Unregister JS fn handle
fn host_invoke_function(                                             // Call JS fn synchronously
    handler: u64, params_ptr: u64, params_len: u64, returns_ptr: u64,
) -> u64;
```

**MOVES to foundation_wasm_ui:**

| Code | Reason |
|------|--------|
| `DOM_SELF` / `DOM_THIS` / `DOM_WINDOW` / `DOM_DOCUMENT` / `DOM_BODY` | DOM constants |
| `allocate_dom_reference()` | DOM-specific pointer allocation |
| `invoke_for_dom()` | DOM-specific invocation |
| `register_animation_hook()` / `register_animation_hook_callback()` | DOM-specific (calls hook_up_animation_frames) |

#### `internal_api` module — STAYS in foundation_wasm

**ALL of it stays:**

| Code | Purpose |
|------|---------|
| `create_instructions()` | Uses ALLOCATIONS to create instruction batch |
| `get_memory()` | Fetches MemoryAllocation by ID |
| `parse_callback_replies()` | Binary return value parsing for callbacks |
| `get_total_animation_callbacks()` | Registry accessor |
| `run_animation_frames()` | Registry caller |
| `register_animation_hook()` | Registry registration |
| Schedule/interval callback registrations | All stay |
| Callback registrations | All stay |
| `run_internal_callbacks()` | Triggers registered callbacks |
| `extract_vec_from_memory()` / `extract_string_from_memory()` | Memory extraction helpers |

#### `exposed_runtime` module — STAYS in foundation_wasm

**ALL of it stays — these are the WASM exports that JS calls.**

Exact `#[no_mangle] pub extern "C"` signatures for all 11 WASM exports:

```rust
// --- Memory management (JS allocates/frees arena slots) ---
fn create_allocation(size: u32) -> u64;              // Allocate arena slot, returns packed MemoryId
fn allocation_start_pointer(allocation_id: u64) -> u32; // Raw pointer into WASM linear memory
fn allocation_length(allocation_id: u64) -> u32;     // Current byte length of slot data
fn dispose_allocation(allocation_id: u64);            // ACK — free slot, generation increments
fn clear_allocation(allocation_id: u64);              // Reset slot contents without freeing

// --- Timer callbacks (JS fires when timer triggers) ---
fn run_scheduled_callback(callback_id: u64);          // One-shot (setTimeout) — runs + removes
fn run_interval_callback(callback_id: u64);           // Repeating (setInterval) — runs, stays registered

// --- Animation (JS calls from requestAnimationFrame loop) ---
fn trigger_animation_callbacks(timestamp: f64);       // Run all rAF callbacks; TickState::Done removes
fn get_total_animation_callbacks() -> u32;            // 0 = JS stops rAF loop

// --- Callback management ---
fn unregister_callback(callback_id: u64);             // Permanently dead — monotonic IDs never reused
fn invoke_callback(internal_pointer: u64, allocation_id: u64); // JS async response -> WASM callback
```

**Key semantics:** `dispose_allocation` is the critical ACK call -- JS MUST call it in a `finally` block after processing any protocol message. `invoke_callback` sends an async JS response back to WASM: `internal_pointer` is the callback ID, `allocation_id` is the arena slot containing the response payload parsed via `parse_callback_replies`.

---

## Part B: New foundation_wasm_ui Crate

### Module Structure:
```
src/
├── lib.rs                  # Feature-gated re-exports + embedded module
├── embedded.rs             # (feature: embedded-js) includes JS runtime assets
├── protocol/
│   ├── mod.rs              # WASM protocol impls (compose encoder from foundation_ui_traits + transport from foundation_wasm)
│   ├── custom_binary.rs    # CustomBinaryV1 — impl ProtocolHandler, uses CustomBinaryEncoder
│   ├── arrow.rs            # ArrowV1 — impl ProtocolHandler, uses ArrowEncoder
│   └── json.rs             # JsonV1 — impl ProtocolHandler, uses JsonEncoder
├── instruction/
│   ├── mod.rs
│   ├── receiver.rs         # InstructionReceiver (decision 030) — holds Box<dyn ProtocolMethods<Vec<DomOp>>>
│   └── bridge.rs           # JS→WASM flow: sendToWasm, loan pattern
├── wasm/
│   ├── mod.rs
│   ├── dom/
│   │   ├── constants.rs    # DOM_SELF, DOM_WINDOW, etc. (moved from jsapi.rs)
│   │   └── element.rs      # allocate_dom_reference, invoke_for_dom (moved)
│   └── animation.rs        # FrameCallback, FrameCallbackList, register_animation_hook (moved)
└── jsapi.rs                # Re-exports of moved items + UI-specific bridges
```

### Cargo.toml:
```toml
[package]
name = "foundation_wasm_ui"
version = "0.0.1"
edition.workspace = true

[dependencies]
foundation_wasm = { workspace = true }
foundation_ui_traits = { workspace = true }
foundation_macros = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }

[features]
default = []
embedded-js = []                 # embeds JS runtime files into binary
```

---

## Part C: Protocol Architecture

### Three-Layer Design

Encoding, transport, and WASM protocol implementations are separated into three layers so that each is independently usable:

```
Layer 1: Encoding (foundation_ui_traits) — no WASM, no FFI, no allocators
  ProtocolEncoder<T> trait      — "Vec<DomOp> → Vec<u8>"
  ArrowEncoder                  — DomOp → Arrow IPC bytes
  JsonEncoder                   — DomOp → JSON bytes
  CustomBinaryEncoder           — DomOp → custom binary bytes
  Envelope                      — 14-byte header write/parse (pure bytes, no MemoryId)
  Used by: HTTP servers, WebSocket servers, SSE endpoints, CLI, tests, WASM

Layer 2: WASM Transport (foundation_wasm) — arena memory + FFI
  ProtocolHandler trait         — "ship bytes across WASM↔JS boundary via arena"
  Used by: WASM protocol impls only

Layer 3: WASM Protocol Impls (foundation_wasm_ui) — compose Layer 1 + Layer 2
  ArrowV1, CustomBinaryV1, JsonV1
  ProtocolMethods<T> trait      — "encode T + allocate + ship via FFI"
  Used by: InstructionReceiver
```

### Layer 1: Encoding (foundation_ui_traits)

Pure encoding — turns structured data into bytes. No WASM concepts, no `MemoryAllocations`, no FFI. Any Rust binary can use these encoders.

```rust
// In foundation_ui_traits — usable everywhere
pub trait ProtocolEncoder<T> {
    fn protocol_byte(&self) -> u8;
    fn version(&self) -> u8;
    fn encode(&self, data: T) -> Vec<u8>;
    fn decode(&self, payload: &[u8]) -> DecodeResult;
}

pub struct ArrowEncoder;
pub struct JsonEncoder;
pub struct CustomBinaryEncoder;
```

**Usage outside WASM:**

```rust
// HTTP server — no foundation_wasm dependency
let encoder = ArrowEncoder;
let bytes = encoder.encode(dom_ops);
response.body(bytes).content_type("application/primal-arrow")

// SSE endpoint
let encoder = JsonEncoder;
let bytes = encoder.encode(signal_patches);
sse_stream.send_event("patch", &bytes)

// WebSocket server
let encoder = ArrowEncoder;
let bytes = encoder.encode(dom_ops);
ws.send_binary(bytes)
```

The `Envelope` struct also lives here — it's pure byte layout, no WASM types:

```rust
pub struct Envelope {
    pub protocol: u8,
    pub version: u8,
    pub length: u32,
}

impl Envelope {
    pub fn write(protocol: u8, version: u8, payload: &[u8]) -> Vec<u8>;
    pub fn parse(bytes: &[u8]) -> (Envelope, &[u8] /* payload */);
}
```

Note: the envelope in `foundation_ui_traits` does NOT include `memory_id` — that's a WASM-specific field. The WASM transport layer (Layer 2) extends the envelope with the arena memory ID when shipping across the WASM↔JS boundary.

### Layer 2: WASM Transport (foundation_wasm)

WASM-specific transport — how encoded bytes cross the WASM↔JS boundary using arena memory and FFI. Defined in `foundation_wasm::protocol`.

#### ProtocolHandler trait — full definition

```rust
// In foundation_wasm::protocol — WASM-only
pub trait ProtocolHandler {
    /// Protocol discriminant: 0=CustomBinary, 1=Arrow, 2=JSON.
    fn protocol_byte(&self) -> u8;
    /// Protocol version for backward compatibility.
    fn version(&self) -> u8;
    /// WASM->JS: ship payload in arena slot `memory_id` at (ptr, len).
    /// JS reads the data then calls dispose_allocation(memory_id).
    fn send_to_js(&self, memory_id: MemoryId, ptr: *const u8, len: usize);
    /// JS->WASM: receive payload from arena slot `memory_id` at (ptr, len).
    /// Handler calls memory.deallocate(memory_id) after processing.
    fn handle_from_js(&self, memory_id: MemoryId, ptr: *const u8, len: usize);
}
```

#### WasmEnvelope — 14-byte WASM-specific message header

The base `Envelope` in `foundation_ui_traits` is 6 bytes: `[protocol:1][version:1][length:4]`. For WASM transport, we extend it with the arena memory ID to form a 14-byte `WasmEnvelope`:

```rust
/// Byte layout (all multi-byte fields little-endian):
///   [0]       protocol   (u8)  — 0=CustomBinary, 1=Arrow, 2=JSON
///   [1]       version    (u8)  — protocol version
///   [2..10]   memory_id  (u64) — MemoryId packed as (index << 32 | generation)
///   [10..14]  length     (u32) — payload byte count following the header
/// Total: 14 bytes header + `length` bytes payload.
pub struct WasmEnvelope {
    pub protocol: u8,
    pub version: u8,
    pub memory_id: u64,
    pub length: u32,
}

impl WasmEnvelope {
    pub fn write(protocol: u8, version: u8, memory_id: u64, payload: &[u8]) -> Vec<u8> {
        // [protocol:1][version:1][memory_id:8 LE][length:4 LE][payload...]
        let mut buf = Vec::with_capacity(14 + payload.len());
        buf.push(protocol);
        buf.push(version);
        buf.extend_from_slice(&memory_id.to_le_bytes());
        buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        buf.extend_from_slice(payload);
        buf
    }

    pub fn parse(bytes: &[u8]) -> (WasmEnvelope, &[u8] /* payload */) {
        let protocol = bytes[0];
        let version = bytes[1];
        let memory_id = u64::from_le_bytes(bytes[2..10].try_into().unwrap());
        let length = u32::from_le_bytes(bytes[10..14].try_into().unwrap());
        (WasmEnvelope { protocol, version, memory_id, length }, &bytes[14..14 + length as usize])
    }
}
```

**Relationship to the base Envelope:** The 6-byte base `Envelope` is used by HTTP/SSE/WebSocket transports where there is no arena memory ID. The 14-byte `WasmEnvelope` is used only for WASM<->JS communication where the JS side needs the `memory_id` to call `dispose_allocation()` after processing.

#### dispatch_message() — protocol routing algorithm

```rust
/// Routes an incoming message to the correct protocol handler based on the
/// protocol byte in the envelope header.
///
/// Algorithm (step by step):
///   1. Read bytes[0] as `protocol` (u8)
///   2. Read bytes[1] as `version` (u8)
///   3. Read bytes[2..10] as `memory_id` (u64, little-endian)
///   4. Read bytes[10..14] as `length` (u32, little-endian)
///   5. Slice `payload = &bytes[14 .. 14 + length]`
///   6. Match protocol byte to handler:
///        0 => custom_binary_handler
///        1 => arrow_handler
///        2 => json_handler
///        _ => panic!("unknown protocol: {protocol}")
///   7. Call `handler.handle_from_js(MemoryId::from_u64(memory_id), payload.as_ptr(), payload.len())`
///
pub fn dispatch_message(bytes: &[u8], handlers: &ProtocolHandlerRegistry) {
    let protocol = bytes[0];
    let _version = bytes[1];
    let memory_id_raw = u64::from_le_bytes(bytes[2..10].try_into().unwrap());
    let length = u32::from_le_bytes(bytes[10..14].try_into().unwrap()) as usize;
    let payload = &bytes[14..14 + length];
    let memory_id = MemoryId::from_u64(memory_id_raw);

    match protocol {
        0 => handlers.custom_binary.handle_from_js(memory_id, payload.as_ptr(), payload.len()),
        1 => handlers.arrow.handle_from_js(memory_id, payload.as_ptr(), payload.len()),
        2 => handlers.json.handle_from_js(memory_id, payload.as_ptr(), payload.len()),
        _ => panic!("unknown protocol: {protocol}"),
    }
}
```

Also owns the WASM-specific envelope extension (adds `memory_id: u64` to the base envelope) and `dispatch_message()` which reads the protocol byte and routes to the correct handler.

### Layer 3: WASM Protocol Impls (foundation_wasm_ui)

Composes encoding (Layer 1) and transport (Layer 2) into a single call. Each impl uses an encoder from `foundation_ui_traits` to produce bytes, then uses `ProtocolHandler` to ship them through the arena.

```rust
// In foundation_wasm_ui
pub trait ProtocolMethods<T>: ProtocolHandler {
    fn encode_and_send(&self, data: T, memory: &mut MemoryAllocations) -> SendResult;
    fn handle_received(&self, payload: &[u8]) -> HandleResult;
    fn ack(&self, memory_ids: &[MemoryId], memory: &mut MemoryAllocations);
}

// ArrowV1 composes ArrowEncoder + ProtocolHandler
impl ProtocolMethods<Vec<DomOp>> for ArrowV1 {
    fn encode_and_send(&self, ops: Vec<DomOp>, memory: &mut MemoryAllocations) -> SendResult {
        let bytes = self.encoder.encode(ops);           // Layer 1: encode
        let mem_id = memory.allocate(bytes.len());      // allocate arena slot
        let slot = memory.get(mem_id).unwrap();
        slot.apply(|mem| mem.extend_from_slice(&bytes));
        let (ptr, len) = slot.as_address().unwrap();
        self.send_to_js(mem_id, ptr, len);              // Layer 2: transport
        SendResult { memory_id: mem_id }
    }
}
```

`InstructionReceiver` holds `Box<dyn ProtocolMethods<Vec<DomOp>>>` — it calls `encode_and_send(ops, memory)` and the impl owns the full pipeline.

### Protocol-specific Differences

| Aspect | CustomBinaryV1 | ArrowV1 | JsonV1 |
|--------|---------------|---------|--------|
| FFI import | `host_batch_apply(6 params)` | `host_arrow_apply(ptr, len)` | `host_json_apply(ptr, len)` |
| Memory per message | 2 arena slots (ops + text) | 1 arena slot | 1 arena slot |
| ACK calls | 2x `dispose_allocation` | 1x `dispose_allocation` | 1x `dispose_allocation` |
| Envelope payload | `[ops_arena_id][text_arena_id][ops + text data...]` | `[arrow_ipc_length][Arrow IPC...]` | `[json_length][JSON text...]` |
| Return value parsing | `ReturnValueParserIter` on ops slot | N/A (DOM ops) | JSON parse |

---

## Part D: JS Runtime Split

### foundation-wasm.js (rewritten from sdk/jsruntime/megatron.js, 7181 lines)

**What it keeps:**
- `MemoryAllocations` — create_allocation, dispose_allocation, get, clear
- Instructions encoder/decoder — parse_ops, parse_text
- FunctionRegistry — register_function, invoke_as_*, invoke_async
- CallbackRegistry — register_callback, invoke_callback, unregister_callback
- TimerRegistry — schedule_timeout, schedule_interval
- Batch API — host_batch_apply, host_batch_returning_apply
- Protocol dispatcher — reads protocol byte, routes to handler
- Transport detection — SharedArrayBuffer vs Transferable

**What changes:**
- Rewritten for clarity — clean class-based API
- Protocol-aware: all messages start with `[protocol: u8][version: u8][memory_id: u64][length: u32]`
- JS calls `dispose_allocation()` after processing each protocol message

**Class signatures for the rewritten foundation-wasm.js:**

```javascript
class MemoryAllocations {
    create(size)       // calls wasmExports.create_allocation(size) -> memoryId (BigInt)
    get(id)            // returns { ptr, len } from allocation_start_pointer + allocation_length
    dispose(id)        // calls wasmExports.dispose_allocation(id) — MUST be called in finally
    clear(id)          // calls wasmExports.clear_allocation(id) — resets without freeing
}

class FunctionRegistry {
    register(name, fn) // calls host_register_function, stores name->handle mapping
    unregister(handle) // calls host_unregister_function, removes from registry
    invoke(handle, params) // calls host_invoke_function, returns decoded result
}

class CallbackRegistry {
    register(fn)       // allocates InternalPointer, stores fn in callback map
    invoke(id, data)   // calls wasmExports.invoke_callback(id, allocationId)
    unregister(id)     // calls wasmExports.unregister_callback(id), removes from map
}

class TimerRegistry {
    schedule(fn, delay)     // calls schedule_timeout, returns callbackId
    cancel(id)              // calls cancel_timeout
    interval(fn, ms)        // calls schedule_interval, returns callbackId
    cancelInterval(id)      // calls cancel_interval
}

class ProtocolDispatcher {
    // Reads protocol byte from incoming buffer, routes to the correct handler.
    // Uses the 14-byte WasmEnvelope format.
    dispatch(buffer) {
        const view = new DataView(buffer);
        const protocol = view.getUint8(0);
        const version = view.getUint8(1);
        const memoryId = view.getBigUint64(2, true);
        const length = view.getUint32(10, true);
        const payload = new Uint8Array(buffer, 14, length);
        switch (protocol) {
            case 0: this.customBinaryHandler.apply(memoryId, payload); break;
            case 1: this.arrowHandler.apply(memoryId, payload); break;
            case 2: this.jsonHandler.apply(memoryId, payload); break;
            default: throw new Error(`unknown protocol: ${protocol}`);
        }
    }
}
```

### foundation-wasm-ui.js (new)

**What it adds:**
- ArrowParser — TypedArray column views from ArrayBuffer (zero-copy)
- ArrowDomApplicator — apply(buffer) → real DOM, 17 operation types
- MorphDom — DOM morphing with form preservation
- NodeRegistry — primal-id → DOM Element
- SignalBridge — bindInput(), bindChange()
- ComponentRegistry — register(tagName, ...), IslandComponent, MountDataComponent, MountStreamComponent
- EventDispatcher — user actions → WASM, event delegation, listener management
- SSEClient — server → DOM patches
- Transport layer — FetchTransport, SSETransport, WebSocketTransport, ChunkedTransport, WorkerTransport
- ProtocolHandler — ArrowHandler, JsonHandler, HtmlHandler (based on Content-Type)
- Patcher — materialize(html), applyDomOps(buffer), applySignalPatches(patches)
- Hydrator — wireEvents, processStyles, processScripts
- MutationObserver — non-island event binding/cleanup
- `window.primal` namespace — single entry point

---

## Part E: Runtime Architecture

Each crate owns its own Runtime:

### foundation_signals Runtime (decision 002)
Owns the signal graph. No knowledge of protocols, DOM ops, or memory allocations.

### foundation_wasm_ui Runtime
Owns the instruction receiver, protocol dispatch, and memory allocations. No knowledge of signal internals.

### Application Orchestrator
```rust
pub struct AppOrchestrator {
    signals: foundation_signals::Runtime,
    wasm_ui: foundation_wasm_ui::Runtime,
}

impl AppOrchestrator {
    pub fn stabilize(&mut self) {
        self.signals.stabilize();
        self.wasm_ui.flush();
    }
}
```

---

## Part F: JS → WASM Flow

**This infrastructure already exists in foundation_wasm.** We preserve all of it.

### Existing Callback Infrastructure (ALL STAYS)

| Function | Direction | Purpose |
|----------|-----------|---------|
| `invoke_callback(internalPointer, allocationId)` | JS → WASM | JS sends async response back to WASM callback |
| `host_invoke_async_function(handler, callback, params, returns)` | WASM → JS | WASM calls async JS function, JS responds via invoke_callback |
| `host_invoke_function(handler, params, returns)` | WASM → JS | WASM calls sync JS function, gets return value |
| `host_invoke_function_as_*` variants | WASM → JS | WASM calls JS, expects specific return type |
| `host_batch_apply` / `host_batch_returning_apply` | WASM → JS | WASM sends DOM ops batch |
| `host_cache_string` | WASM → JS | WASM caches string in JS runtime |
| `host_register_function` / `host_unregister_function` | WASM → JS | WASM registers JS functions |

### JS → WASM: Event Data

| Transport | How JS produces data | WASM receives |
|-----------|---------------------|---------------|
| SharedArrayBuffer | Writes directly into WASM linear memory | Reads directly — zero-copy |
| Transferable (postMessage) | Creates ArrayBuffer, copies, transfers | WASM copies into own memory |
| HTTP (ServiceWorker/Server) | Serializes into fetch body | Server/WASM decodes |

### JS → WASM: Async Callback Responses

```javascript
function onAsyncResult(callbackId, resultData) {
    const memId = wasmExports.create_allocation(resultData.length);
    const ptr = wasmExports.allocation_start_pointer(memId);
    const buffer = new Uint8Array(wasmMemory.buffer, ptr, resultData.length);
    buffer.set(resultData);
    wasmExports.invoke_callback(callbackId, memId);
}
```

This is how WASM knows when JS fails or succeeds — the callback mechanism is already bidirectional.

---

## Part G: MemoryAllocations Arena Lifecycle

The `MemoryAllocations` arena (defined in `mem.rs`) is the core memory management primitive for all WASM<->JS communication. Every protocol message passes through it. The arena uses generation-based IDs to prevent use-after-free.

### Arena data structure

```rust
pub struct MemoryAllocations {
    allocs: Vec<(u32, MemoryAllocation)>,  // (generation, slot)
    free: Vec<usize>,                       // indices of deallocated slots
}
```

- `allocs` is a growable array of `(generation_counter, MemoryAllocation)` pairs.
- `free` is a stack of indices into `allocs` that have been released and can be reused.
- `MemoryId(index: u32, generation: u32)` — packed as `u64` for FFI: `(index as u64) << 32 | generation as u64`.

### Full lifecycle — allocate, use, dispose

**Step 1: WASM allocates a slot**
```rust
let mem_id = memory.allocate(size)?;
// If free list is empty:  push new entry at allocs[next_index] with generation=0
//                         return MemoryId(next_index, 0)
// If free list has index: pop index, increment generation, reset_to(size)
//                         return MemoryId(index, new_generation)
```

**Step 2: WASM writes data into the slot**
```rust
let slot = memory.get(mem_id)?;
slot.apply(|mem| {
    mem.clear();
    mem.extend_from_slice(&encoded_payload);
});
```

**Step 3: WASM ships data to JS via protocol FFI**
```rust
let (ptr, len) = slot.as_address()?;
unsafe { host_arrow_apply(mem_id.as_u64(), ptr as u64, len); }
// JS now reads from WASM linear memory at (ptr, len)
```

**Step 4: JS processes the data**
```javascript
// JS reads the payload from WASM memory (zero-copy view)
const view = new Uint8Array(wasmMemory.buffer, ptr, len);
ArrowDomApplicator.apply(ArrowParser.parse(view));
```

**Step 5: JS ACKs — calls dispose_allocation**
```javascript
// CRITICAL: always in a finally block
try {
    processPayload(ptr, len);
} finally {
    wasmExports.dispose_allocation(memoryId);
}
```

**Step 6: WASM arena reclaims the slot**
```rust
// Inside dispose_allocation(allocation_id):
let mem_id = MemoryId::from_u64(allocation_id);
memory.deallocate(mem_id)?;
// Pushes mem_id.index onto the free list.
// The slot's generation stays at its current value.
// Next allocate() that reuses this index will increment the generation.
```

**Step 7: Stale MemoryId detection**
```rust
// Any code holding an old MemoryId with the same index but previous generation:
let stale_id = MemoryId(index, old_generation);
memory.get(stale_id)  // => Err(InvalidAllocationId)
// The generation in allocs[index] is now old_generation+1, so the check fails.
```

### Generation ID encoding

`MemoryId(index: u32, generation: u32)` is packed into a `u64` for FFI transport. The `index` identifies the slot position in the `allocs` vector. The `generation` is a monotonically increasing counter per slot — it increments each time the slot is recycled through `deallocate` + `allocate`. This makes stale IDs detectable without any bookkeeping on the JS side.

### Custom Binary — two slots per message

The Custom Binary protocol allocates two arena slots per batch: one for ops, one for text. Both `MemoryId` values are passed to `host_batch_apply`. JS must call `dispose_allocation` twice — once for each slot. If JS fails to ACK either slot, that slot leaks until the WASM instance is torn down.

### Arrow / JSON — one slot per message

Arrow and JSON protocols use a single arena slot per message. One `MemoryId`, one `dispose_allocation` call.

---

## Part H: Event Listener Flow

```
1. WASM renders via Arrow batch
   <button primal-id="42:3" primal:onclick="callback-7">Click</button>

2. JS ArrowDomApplicator applies the DOM ops

3. JS Hydrator scans DOM for primal:on* attributes
   → registers: element.addEventListener('click', () => invoke_callback(7, data))

4. User clicks → JS fires → invoke_callback(7, allocation_id) → WASM callback executes

5. WASM callback → setter executes → signal changes → stabilize() → new DOM ops
```

**User JS way (manual):**
```javascript
runtime.on(element, 'click', 'controller.delete');
primal.onclick(element, 'handler.update');
element.addEventListener('click', () => wasmExports.invoke_callback(callbackId, allocationId));
```

---

## Part I: Cargo.toml Changes

### foundation_wasm:
- Update `description`: "Low-level WASM-JS ABI for memory management, function invocation, and binary messaging. No DOM, no window, no UI concepts."
- Remove `frames.rs` from compilation
- Split `jsapi.rs` → `host_runtime.rs`, `protocol.rs`
- Add feature flags:
```toml
[features]
default = ["rust-api"]
rust-api = []
embedded-js = ["rust-api"]
```

### foundation_wasm_ui:
As shown in Part B.

---

## Dependencies

- foundation_ui_traits: no new dependencies (already has serde if needed for JSON encoder)
- foundation_wasm: no new dependencies (removes some)
- foundation_wasm_ui adds: foundation_wasm, foundation_ui_traits, foundation_macros, serde, serde_json

## Refactoring Strategy

**New file, not in-place edits.** For each module being refactored (e.g., `jsapi.rs` → `protocol.rs` + `host_runtime.rs`):

1. Create the new file alongside the old one — write the refactored code in the new file
2. Add tests in `{crate}/tests/` for the new module — verify behavior parity with the old code
3. Once all tests pass and behavior is confirmed identical, update `lib.rs` to use the new module
4. Delete the old file only after the refactor is complete and all tests pass

This applies to every split/move: `jsapi.rs` split, `frames.rs` move, encoder extraction to `foundation_ui_traits`, etc. Nothing is deleted until the replacement is proven correct.

## Error Cases

Exhaustive list of error conditions and their expected behavior:

### Stale MemoryId (generation mismatch)

A `MemoryId` becomes stale when the slot it references has been deallocated and reallocated to a new consumer. The generation counter in `allocs[index]` no longer matches the generation in the `MemoryId`.

```rust
let id = memory.allocate(128)?;      // MemoryId(0, 0)
memory.deallocate(id)?;              // slot 0 pushed to free list
let id2 = memory.allocate(64)?;      // MemoryId(0, 1) — same index, generation incremented

memory.get(id)   // => Err(InvalidAllocationId) — generation 0 != 1
memory.get(id2)  // => Ok(slot) — generation matches
```

**Impact:** Any WASM code or JS code holding an old `MemoryId` after `dispose_allocation` will fail safely. No garbage data is returned — the error is detected at the generation check.

### Unknown protocol byte

If `dispatch_message` receives a protocol byte that is not 0, 1, or 2:

```rust
match protocol {
    0 => { /* custom binary */ },
    1 => { /* arrow */ },
    2 => { /* json */ },
    _ => panic!("unknown protocol: {protocol}"),
}
```

This is a hard panic — an unknown protocol byte indicates a binary-level corruption or version mismatch. There is no recovery path. The WASM instance traps and JS catches a `RuntimeError`.

### Arena full — allocation failure

When `allocate()` is called and the free list is empty, a new slot is appended to `allocs`. If `allocs.len()` exceeds `u32::MAX`, allocation returns `Err(NoMoreAllocationSlots)`. In practice this means ~4 billion slots — if this is reached, the application has a fundamental leak.

```rust
if u32::try_from(next_index).is_err() {
    return Err(MemoryAllocationError::NoMoreAllocationSlots);
}
```

### Slot leak — JS fails to ACK

If JS does not call `dispose_allocation` after processing a message, the arena slot is permanently occupied. The slot will never return to the free list. Over time, this exhausts available slots and forces new allocations (growing the `allocs` vector) instead of reusing freed ones.

**Mitigation:** All JS protocol handlers MUST wrap processing in `try/finally` and call `dispose_allocation` in the `finally` block:

```javascript
try {
    applyPayload(ptr, len);
} finally {
    wasmExports.dispose_allocation(memoryId);
}
```

### Stale callback — invoke after unregister

When a callback is unregistered (removed from `InternalReferenceRegistry`), its monotonic `InternalPointer(u64)` ID is permanently dead — monotonic IDs are never reused. If JS later calls `invoke_callback(stale_id, allocation_id)`:

```rust
// Inside invoke_callback:
let callback = registry.get(&id);  // => None — ID not in BTreeMap
// Silently dropped. No panic, no error. The allocation_id slot is still
// freed (dispose_allocation is called by the invoke_callback implementation).
```

This is the correct behavior — race conditions between component unmount and async JS responses should not crash the application.

### WasmEnvelope parse with insufficient bytes

If `WasmEnvelope::parse` is called with `bytes.len() < 14`, the slice indexing panics. This is by design — a sub-14-byte message is always invalid and indicates transport corruption.

### Invalid allocation_id in exposed_runtime

If JS calls `allocation_start_pointer(invalid_id)` or `allocation_length(invalid_id)` with an ID that does not correspond to a valid, non-freed slot, the `get()` call returns `Err(InvalidAllocationId)`. The exposed_runtime function panics (via `.expect()`). JS catches this as a `RuntimeError`.

---

## Testing

### Layer 1: Encoding (foundation_ui_traits)

Specific test scenarios:

- **Arrow round-trip:** Create `Vec<DomOp>` with 5 mixed operations (SetText, SetAttribute, AppendChild, RemoveChild, InsertBefore). Encode via `ArrowEncoder.encode(ops)`. Decode via `ArrowEncoder.decode(bytes)`. Assert decoded ops are identical to input.
- **JSON round-trip:** Same 5 ops. `JsonEncoder.encode(ops)` produces valid UTF-8 JSON. `JsonEncoder.decode(bytes)` returns identical ops.
- **CustomBinary round-trip:** Same 5 ops. `CustomBinaryEncoder.encode(ops)` produces binary. `CustomBinaryEncoder.decode(bytes)` returns identical ops.
- **Base Envelope write/parse:** `Envelope::write(1, 0, &payload)` produces 6 + payload.len() bytes. `Envelope::parse(bytes)` returns protocol=1, version=0, length=payload.len(), and the exact payload slice.
- **Content-Type mapping:** `ArrowEncoder.protocol_byte()` == 1. `JsonEncoder.protocol_byte()` == 2. `CustomBinaryEncoder.protocol_byte()` == 0.
- **no_std compatibility:** All encoders compile and pass tests under `#![no_std]` + `extern crate alloc`.

### Layer 2: WASM Transport (foundation_wasm)

Specific test scenarios:

- **WasmEnvelope write/parse:** `WasmEnvelope::write(1, 0, 0x0000_0001_0000_0003, &payload)` produces exactly 14 + payload.len() bytes. Parse returns protocol=1, version=0, memory_id=0x0000_0001_0000_0003, length=payload.len().
- **dispatch_message routing:** Construct 14-byte envelope with protocol=1. Call `dispatch_message`. Assert arrow handler's `handle_from_js` was called. Repeat with protocol=0 (custom binary) and protocol=2 (json).
- **dispatch_message unknown protocol:** Construct envelope with protocol=255. Call `dispatch_message`. Assert panic with message "unknown protocol: 255".
- **MemoryAllocations lifecycle:** Allocate slot, write 100 bytes, ship as Arrow, JS ACKs via `dispose_allocation` — slot freed, generation incremented. Attempt access with old MemoryId returns `Err(InvalidAllocationId)`.
- **Stale MemoryId safety:** Allocate MemoryId(0,0). Deallocate. Allocate again — get MemoryId(0,1). Assert `get(MemoryId(0,0))` fails. Assert `get(MemoryId(0,1))` succeeds.
- **Callback lifecycle:** Register callback, invoke it — closure runs. Unregister, invoke again — silently dropped (no panic).
- **foundation_wasm compiles without DOM types:** `cargo check` with no DOM feature — no compile errors.
- **ReturnTypeId::DOMObject present:** `ReturnValueParserIter` parses a binary stream containing DOMObject return values without error.

### Layer 3: WASM Protocol Impls (foundation_wasm_ui)

Specific test scenarios:

- **ArrowV1 encode_and_send:** Create 3 DomOps. Call `encode_and_send`. Assert exactly 1 arena slot allocated. Assert `host_arrow_apply` called with correct (mem_id, ptr, len). Call `ack` — slot freed.
- **CustomBinaryV1 encode_and_send:** Create 3 DomOps. Call `encode_and_send`. Assert exactly 2 arena slots allocated (ops + text). Assert `host_batch_apply` called with 6 parameters (2x mem_id, ptr, len). Call `ack` with both IDs — both slots freed.
- **JsonV1 encode_and_send:** Create 3 DomOps. Call `encode_and_send`. Assert 1 arena slot. Assert `host_json_apply` called.
- **host_batch_apply with 2 arena slots:** Allocate ops and text slots. Ship via `host_batch_apply`. JS processes, calls `dispose_allocation` twice. Assert both slots freed and available for reuse.
- **InstructionReceiver flush:** Queue 10 DomOps via `receiver.queue()`. Call `flush()`. Assert protocol's `encode_and_send` called once with all 10 ops. Assert `receiver.ops` is empty after flush.
- **InstructionReceiver empty flush:** Call `flush()` with no queued ops. Assert no encoding or FFI calls made.

### JS Runtime

Specific test scenarios:

- **ProtocolDispatcher routing:** Construct ArrayBuffer with protocol=1 (Arrow) in 14-byte header. Call `dispatcher.dispatch(buffer)`. Assert `arrowHandler.apply` called with correct memoryId and payload.
- **ProtocolDispatcher unknown protocol:** Protocol byte = 255. Assert `dispatch` throws `Error("unknown protocol: 255")`.
- **MemoryAllocations create/dispose cycle:** `mem.create(128)` returns valid ID. `mem.get(id)` returns ptr and len. `mem.dispose(id)` succeeds. Subsequent `mem.get(id)` with old ID fails.
- **CallbackRegistry lifecycle:** `reg.register(fn)` returns ID. `reg.invoke(id, data)` calls the function. `reg.unregister(id)`. `reg.invoke(id, data)` — no-op (does not throw).
- **foundation-wasm-ui.js ArrowDomApplicator:** Parse a mock Arrow IPC buffer with 17 operation types. Assert all DOM mutations applied correctly.
- **Error handling — dispose on throw:** Protocol handler throws during `apply`. Assert `dispose_allocation` still called (via finally block). Assert slot not leaked.
- **Event flow end-to-end:** `primal:onclick` attribute on element. JS Hydrator wires listener. Simulate click. Assert `invoke_callback` called with correct callback ID and allocation containing event data.