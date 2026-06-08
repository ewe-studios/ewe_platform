# Feature 00: WASM UI Core — Crate Split, Protocol Refactor & JS Runtime Rewrite

## Description

Split `foundation_wasm` into a pure runtime/ABI layer. Create `foundation_wasm_ui` as a new crate that owns ALL DOM/window/animation bindings and depends on `foundation_wasm`. Refactor the WASM boundary protocol into a trait-based system where each message carries its own protocol in the envelope. Rewrite the JS runtime (`megatron.js`, 7181 lines) into two clean files: `foundation-wasm.js` and `foundation-wasm-ui.js`.

**Decisions:** 014, 015, 022, 028, 030

---

## Message Envelope (all protocols)

Every message in either direction carries a 10-byte common envelope followed by protocol-specific payload:

```
[protocol: u8][version: u8][batch_memory_id: u64][payload...]
```

| Field | Size | Meaning |
|-------|------|---------|
| `protocol` | 1 byte | 0 = Custom Binary, 1 = Arrow, 2 = JSON |
| `version` | 1 byte | Protocol version for backward compatibility |
| `batch_memory_id` | 8 bytes | `MemoryId(index, generation)` of the arena slot holding this message. The consumer calls `dispose_allocation(memory_id)` to free after processing. This is the ACK mechanism. |
| `payload` | rest | Protocol-specific data |

**Messages can use different protocols back and forth.** There is no "session protocol" — each message's envelope declares what protocol its payload uses. The receiver reads the first byte and dispatches to the correct handler.

### Protocol payload formats

| Protocol | Payload structure |
|----------|------------------|
| Custom Binary (0) | `[ops_arena_id: u64][text_arena_id: u64][ops + text data...]` |
| Arrow (1) | `[arrow_ipc_length: u32][Arrow IPC buffer...]` |
| JSON (2) | `[json_length: u32][json text...]` |

---

## Changes to foundation_wasm

### Files removed/moved:
- `src/frames.rs` → moves to `foundation_wasm_ui/src/wasm/animation.rs`
- `src/jsapi.rs` DOM constants (`DOM_SELF`, `DOM_THIS`, `DOM_WINDOW`, `DOM_DOCUMENT`, `DOM_BODY`) → moves to `foundation_wasm_ui/src/wasm/dom/constants.rs`
- `src/jsapi.js` `allocate_dom_reference()` → moves to `foundation_wasm_ui/src/wasm/dom/element.rs`
- `src/jsapi.js` `invoke_for_dom()`, `invoke_for_object()` → moves to `foundation_wasm_ui/src/wasm/dom/element.rs`
- `src/jsapi.js` `host_cache_string` wrapper → moves to `foundation_wasm_ui/src/wasm/text_cache.rs`
- `runtime/megatron.js` → rewritten into `foundation-wasm.js` + `foundation-wasm-ui.js`

### Files unchanged (stay in foundation_wasm):
- `src/base.rs` — ReturnTypeId, ReturnTypeHints, ReturnValues (including `DOMObject` — used by Custom Binary protocol to encode DOM object messages)
- `src/error.rs` — Error types
- `src/intervals.rs` — Timer registry (generic, not DOM-specific)
- `src/mem.rs` — MemoryAllocations, MemoryId
- `src/ops.rs` — Params, Instructions, batch encoding
- `src/registry.rs` — Callback/pointer registry
- `src/schedule.rs` — Schedule registry
- `src/wrapped.rs` — Wrappers

### Cargo.toml changes:
- Update `description` to: "Low-level WASM-JS ABI for memory management, function invocation, and binary messaging. No DOM, no window, no UI concepts."
- Add feature flags (see below)

### Feature flags:
```toml
[features]
default = ["rust-api"]
rust-api = []                    # Rust ABI code only
embedded-js = ["rust-api"]       # includes embedded JS runtime access via foundation_macros::EmbeddedAs
```

The `embedded-js` feature bundles the JS runtime files (`foundation-wasm.js`, `foundation-wasm-ui.js`) into the binary at compile time. This provides programmatic access to runtime content (e.g., HTTP servers serving JS). Complementary to, not conflicting with, the build pipeline CLI's single-file bundling.

---

## New foundation_wasm_ui Crate

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

Note: `foundation_signals` is a **separate crate** (Feature 02), not a feature of `foundation_wasm_ui`.

### Module Structure:
```
src/
├── lib.rs                  # Feature-gated re-exports + embedded module
├── embedded.rs             # (feature: embedded-js) includes JS runtime assets
├── instruction/
│   ├── mod.rs
│   ├── receiver.rs         # InstructionReceiver (runtime-owned batching)
│   └── protocol.rs         # Protocol trait + ArrowProtocol, CustomBinaryProtocol, JsonProtocol
├── wasm/
│   ├── mod.rs
│   ├── dom/
│   │   ├── constants.rs    # DOM_SELF, DOM_WINDOW, etc. (moved)
│   │   ├── element.rs      # DOM operations (moved)
│   │   └── event.rs        # Event listeners (moved)
│   ├── animation.rs        # Animation frames (moved from frames.rs)
│   └── text_cache.rs       # String interning (moved)
└── jsapi.rs                # Re-exports + UI-specific bridges
```

Flat structure — no `shared/` nesting. Decision 015 says reduce over-nesting.

---

## Protocol Refactor (Rust side)

### Protocol trait

```rust
/// Each protocol owns its complete communication contract:
/// memory layout, encoding, FFI entry point, and ACK semantics.
pub trait Protocol {
    fn protocol_byte(&self) -> u8;

    /// Encode ops, allocate arena slot, write envelope + payload, call FFI.
    fn send(&self, ops: Vec<DomOp>, memory: &mut MemoryAllocations);
}
```

### CustomBinaryProtocol (protocol 0)

- Uses two arena slots: ops arena + text arena
- Fixed `host_batch_apply` signature now includes memory IDs for ACK:
```rust
pub fn host_batch_apply(
    ops_memory_id: u64, ops_pointer: u64, ops_length: u64,
    text_memory_id: u64, text_pointer: u64, text_length: u64,
);
```
- JS calls `dispose_allocation(ops_memory_id)` and `dispose_allocation(text_memory_id)` after applying

### ArrowProtocol (protocol 1)

- Single arena slot — self-contained Arrow IPC buffer
- New FFI entry point:
```rust
pub fn host_arrow_apply(ptr: u64, len: u64);
```
- JS reads `memory_id` from envelope (bytes 2-9), processes payload, calls `dispose_allocation(memory_id)`

### JsonProtocol (protocol 2)

- Single arena slot — JSON text
- FFI entry point: `host_json_apply(ptr: u64, len: u64)`
- Same ACK pattern as Arrow

### dispatch_message (Rust side)

```rust
// In foundation_wasm — shared across all execution modes
fn dispatch_message(message: &[u8]) {
    let protocol = message[0];
    let version = message[1];
    let memory_id = u64::from_le_bytes(message[2..10].try_into().unwrap());
    let payload = &message[10..];

    match protocol {
        0 => handle_custom_binary(payload, version),
        1 => handle_arrow(payload, version),
        2 => handle_json(payload, version),
        _ => panic!("unknown protocol"),
    }
}
```

### dispose_allocation export

```rust
#[no_mangle]
pub extern "C" fn dispose_allocation(allocation_id: u64) {
    // frees arena slot — called by JS after processing any protocol message
}
```

---

## JS SDK Split

### foundation-wasm.js (rewritten from megatron.js)

**Memory management:**
- `MemoryAllocations` — create_allocation, dispose_allocation, get, clear

**Registries:**
- `FunctionRegistry` — register_function, invoke_as_*, invoke_async
- `CallbackRegistry` — register_callback, invoke_callback, unregister_callback
- `TimerRegistry` — schedule_timeout, schedule_interval

**Protocol dispatch:**
- `dispatchMessage(buffer)` — reads protocol byte, routes to correct handler
- `CustomBinaryHandler` — parses Instructions format, calls `dispose_allocation(ops_id)` + `dispose_allocation(text_id)` after apply
- `ArrowHandler` — uses ArrowParser, calls `dispose_allocation(memoryId)` after apply
- `JsonHandler` — parses JSON, calls `dispose_allocation(memoryId)` after apply

**Arrow support:**
- `ArrowParser` — TypedArray column views from ArrayBuffer (zero-copy)
- `ArrowParser.encode()` — Rust-side Arrow encoding

**FFI imports (from WASM):**
- `host_batch_apply(ops_memory_id, ops_ptr, ops_len, text_memory_id, text_ptr, text_len)`
- `host_arrow_apply(ptr, len)`
- `host_json_apply(ptr, len)`
- `dispose_allocation(memory_id)` — exported by WASM for JS to ACK

**Transport detection:**
```javascript
function detectTransportCapability() {
    // Probes SharedArrayBuffer + Atomics
    // Returns { mode: 'shared' } or { mode: 'transfer' }
}
```

### foundation-wasm-ui.js (new)

**DOM application:**
- `ArrowDomApplicator` — apply(buffer) → real DOM, **17 operation types** (IDs 0-16 including MORPH_NODE)
- `MorphDom` — DOM morphing with form preservation, move detection, anti-churn
- `NodeRegistry` — primal-id → DOM Element, with prefix-based cleanup

**Transport layer:**
- `Transport.create(config)` — factory for Fetch/SSE/WebSocket/Chunked/Worker
- Each transport handles JS→WASM flow: produce data → write to arena/shared memory → signal WASM

**Protocol handler layer (JS→WASM):**
```javascript
class ProtocolHandler {
    // JS produces → WASM consumes → ACKs
    sendToWasm(data) {
        const memId = this.jsAlloc.allocate(data.length);
        // ... write to arena, call WASM entry point
        // Slot borrowed until WASM signals done via dispose call
    }

    // WASM produces → JS consumes → ACKs
    handleFromWasm(memId, ptr, len) {
        this.borrowedSlots.add(memId);
        try {
            this.apply(ptr, len);  // protocol-specific apply
            wasmExports.dispose_allocation(memId);  // ACK
        } catch (err) {
            console.error('Batch apply failed:', err);
            wasmExports.dispose_allocation(memId);  // always free, even on error
            throw err;
        } finally {
            this.borrowedSlots.delete(memId);
        }
    }
}
```

**Content-Type routing (HTTP responses):**
- `application/primal-html` → HtmlHandler → materialize
- `application/primal-arrow` → ArrowHandler → applyArrow
- `application/primal-json` → JsonHandler → applyJson
- `text/event-stream-*` → SSEClient → dispatches to above handlers

**Web components:**
- `IslandComponent` — connectedCallback hydrates, disconnectedCallback cleans up
- `MountDataComponent` — request-response
- `MountStreamComponent` — continuous streaming

**Patcher/Hydrator:**
- `Patcher.materialize(html, target)` — parse HTML, wrap in `<island>`, hydrate
- `Hydrator.hydrate(root)` — wireEvents, processStyles, processScripts

**Event runtime:**
- `EventDispatcher` — direct binding, opt-in delegation, MutationObserver cleanup (non-island only)
- `primal.*` namespace — single entry point

**Error handling (uniform across all protocols):**
- JS side: try/catch → log to console → throw for developer → always `dispose_allocation`
- No protocol-specific retry asymmetry

---

## Dependencies

- No new dependencies for foundation_wasm (removes some)
- foundation_wasm_ui adds: foundation_wasm, foundation_ui_traits, foundation_macros, serde, serde_json

## Testing

- foundation_wasm compiles without DOM types
- ReturnTypeId::DOMObject present — no references broken
- Protocol trait: each protocol encodes correctly, FFI calls correct entry point
- Message envelope: 10-byte header correctly packed/unpacked
- ACK flow: JS calls dispose_allocation, arena slot freed, generation prevents stale reuse
- foundation-wasm.js dispatches by protocol byte correctly
- foundation-wasm-ui.js loads, depends on foundation-wasm.js
- Error handling: all protocols follow same try/catch/dispose pattern
