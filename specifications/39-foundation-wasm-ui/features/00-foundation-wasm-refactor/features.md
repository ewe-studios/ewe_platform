# Feature 00: foundation_wasm Refactor & Split

## Description

Refactor `foundation_wasm` into a pure runtime/ABI layer and create `foundation_wasm_ui` for all DOM/UI concerns. Rewrite the JS runtime (`megatron.js`) into two clean files. Clarify ownership of every type, function, and capability. **Nothing is deleted** — everything moves or stays. The callback infrastructure, FFI functions, return value parsing, and all existing capabilities are preserved.

**Decisions:** 014, 015, 022, 028, 030

---

## Goal

After this refactor:
- `foundation_wasm` = pure WASM↔JS ABI. Memory management, binary encoding, function invocation, callback system, timers. No DOM, no window, no UI concepts.
- `foundation_wasm_ui` = all DOM/window/animation bindings + protocol layer + JS runtime assets.
- Every existing capability is preserved — we're organizing, not removing.
- New protocol architecture: structs (CustomBinaryV1, ArrowV1, JsonV1) with a common method subset.
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

**ALL return value parsing infrastructure stays:**

| Code | Lines | Purpose |
|------|-------|---------|
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

**ALL of it stays — these are the WASM exports that JS calls:**

| Function | Purpose |
|----------|---------|
| `create_allocation()` | JS → WASM: allocate memory slot |
| `allocation_start_pointer()` | JS → WASM: get pointer for slot |
| `allocation_length()` | JS → WASM: get slot length |
| `dispose_allocation()` | JS → WASM: ACK — free arena slot (CRITICAL) |
| `clear_allocation()` | JS → WASM: clear slot contents |
| `run_scheduled_callback()` | JS → WASM: trigger scheduled callback |
| `run_interval_callback()` | JS → WASM: trigger interval callback |
| `trigger_animation_callbacks()` | JS → WASM: trigger animation frames |
| `get_total_animation_callbacks()` | JS → WASM: count animation callbacks |
| `unregister_callback()` | JS → WASM: unregister callback |
| `invoke_callback()` | JS → WASM: send async response to WASM callback |

---

## Part B: New foundation_wasm_ui Crate

### Module Structure:
```
src/
├── lib.rs                  # Feature-gated re-exports + embedded module
├── embedded.rs             # (feature: embedded-js) includes JS runtime assets
├── protocol/
│   ├── mod.rs              # ProtocolMethods trait, PROTOCOL_MAP
│   ├── custom_binary.rs    # CustomBinaryV1 struct
│   ├── arrow.rs            # ArrowV1 struct
│   └── json.rs             # JsonV1 struct
├── instruction/
│   ├── mod.rs
│   ├── receiver.rs         # InstructionReceiver (decision 030)
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

### The Envelope Layer (NOT a protocol, universal)

Every message in either direction carries a 14-byte envelope:

```
[protocol: u8][version: u8][memory_id: u64][length: u32]
```

| Field | Size | Purpose |
|-------|------|---------|
| `protocol` | 1 byte | Which protocol struct handles this (0=CustomBinary, 1=Arrow, 2=JSON) |
| `version` | 1 byte | Protocol version for backward compatibility |
| `memory_id` | 8 bytes | `MemoryId(index, generation)` — the arena slot holding this message |
| `length` | 4 bytes | Payload length in bytes (not including the 14-byte envelope) |

### Protocol Structs with Common Method Subset

```rust
pub trait ProtocolMethods {
    fn protocol_byte(&self) -> u8;
    fn version(&self) -> u8;
    fn encode(&self, memory: &mut MemoryAllocations, data: ProtocolData<'_>) -> EncodeResult;
    fn handle_received(&self, payload: &[u8]) -> HandleResult;
    fn ack(&self, memory_ids: &[MemoryId], memory: &mut MemoryAllocations);
}
```

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

## Part G: Event Listener Flow

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

## Part H: Cargo.toml Changes

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

- No new dependencies for foundation_wasm (removes some)
- foundation_wasm_ui adds: foundation_wasm, foundation_ui_traits, foundation_macros, serde, serde_json

## Testing

- foundation_wasm compiles without DOM types
- ReturnTypeId::DOMObject present — ReturnValueParserIter parses it correctly
- All existing callback infrastructure works: invoke_callback, host_invoke_async_function, etc.
- Protocol structs: each encode/decode correctly, ACK disposes correct number of slots
- Message envelope: 14-byte header correctly packed/unpacked
- JS dispatchMessage: routes to correct protocol handler
- foundation-wasm.js: MemoryAllocations, registries, protocol dispatch all work
- foundation-wasm-ui.js: ArrowDomApplicator applies 17 ops, MorphDom preserves form state
- Error handling: all protocols try/catch/dispose consistently
- Event flow: primal:on* → JS listener → invoke_callback → WASM callback → signal change