# 015 — foundation_wasm crate split: what moves, what stays, what gets refactored

**Date:** 2026-06-08
**Status:** Resolved

### Decision

Split `foundation_wasm` into a pure runtime/ABI layer and move all DOM/window/animation concepts to `foundation_wasm_ui`. The JS runtime (`megatron.js`) is rewritten into two files.

---

## What stays in foundation_wasm (pure ABI, no_std)

| File | Purpose | Notes |
|------|---------|-------|
| `lib.rs` | Re-exports | Update description |
| `mem.rs` | MemoryAllocations, MemoryId | Pure memory management |
| `ops.rs` | Params, Instructions, batch encoding | Pure binary encoding — no DOM |
| `registry.rs` | Callback/pointer registry | Pure function registry |
| `intervals.rs` | Timer registry (schedule/interval) | Pure timers |
| `schedule.rs` | Schedule registry | Pure scheduling |
| `base.rs` | ReturnTypeId, ReturnTypeHints, ReturnValues | Pure types |
| `error.rs` | Error types | Pure errors |
| `wrapped.rs` | Wrapper types | Pure wrappers |

### Cargo.toml
```toml
description = "Low-level WASM-JS ABI for memory management, function invocation, and binary messaging. No DOM, no window, no UI concepts."
```

---

## What moves to foundation_wasm_ui

| Currently in foundation_wasm | Moves to |
|------------------------------|----------|
| `frames.rs` | `foundation_wasm_ui/src/wasm/animation.rs` |
| `jsapi.rs` DOM constants (`DOM_SELF`, `DOM_THIS`, `DOM_WINDOW`, etc.) | `foundation_wasm_ui/src/wasm/dom/constants.rs` |
| `jsapi.rs` `allocate_dom_reference()` | `foundation_wasm_ui/src/wasm/dom/element.rs` |
| `jsapi.rs` `invoke_for_dom()`, `invoke_for_object()` | `foundation_wasm_ui/src/wasm/dom/element.rs` |
| `jsapi.rs` `ReturnTypeId::DOMObject` | `foundation_wasm_ui/src/wasm/dom/types.rs` |
| `jsapi.rs` `host_cache_string` wrapper | `foundation_wasm_ui/src/wasm/text_cache.rs` |
| JS runtime: DOM element creation, attribute setting | `foundation-wasm-ui.js` |
| JS runtime: event listener management | `foundation-wasm-ui.js` |
| JS runtime: animation frame handling | `foundation-wasm-ui.js` |

We should also refactor to make the code cleaner and nicer and more clear especially the many nested modules we have in there that might just be over nested.

---

## JS Runtime Split

### foundation-wasm.js (rewritten from megatron.js)

**What it keeps:**
- `MemoryAllocations` — create_allocation, dispose_allocation, get, clear
- Instructions encoder/decoder — parse_ops, parse_text
- **ArrowParser** — TypedArray column views from ArrayBuffer
- **ArrowParser.encode()** — Rust-side Arrow encoding (columnar, zero-copy)
- FunctionRegistry — register_function, invoke_as_*, invoke_async
- CallbackRegistry — register_callback, invoke_callback, unregister_callback
- TimerRegistry — schedule_timeout, schedule_interval
- Batch API — `host_batch_apply`, `host_batch_returning_apply`
- **Protocol dispatcher** — reads protocol byte, routes to handler (0=custom binary, 1=arrow, 2=json)

**What changes:**
- Rewritten for clarity — megatron.js is 7181 lines of tangled code
- Clean class-based API with clear separation of concerns
- Protocol-aware: all messages start with `[protocol: u8][version: u8][payload...]`

### foundation-wasm-ui.js (new)

**What it adds:**
- ArrowDomApplicator — apply(buffer) → real DOM, 15 operation types (uses ArrowParser from foundation-wasm.js)
- NodeRegistry — nodeId → DOM Element
- SignalBridge — bindInput(), bindChange()
- ComponentRegistry — register(tagName, ...), WasmElement class
- EventDispatcher — user actions → WASM
- SSEClient — server → DOM patches
- DOM event handling — event delegation, listener management
- Animation frame handling — requestAnimationFrame wrapper

---

## Protocol-Aware Communication

The current `megatron.js` has a single hardcoded protocol (custom binary Instructions). The rewritten JS runtime must support multiple protocols:

```
Message envelope: [protocol: u8][version: u8][payload_length: u32][payload...]
```

| Protocol byte | Meaning |
|--------------|---------|
| 0 | Custom binary (foundation_wasm Instructions) |
| 1 | Arrow format |
| 2 | JSON format |

The JS runtime reads the protocol byte and dispatches to the correct handler:
```javascript
function handleMessage(buffer) {
  const protocol = new Uint8Array(buffer)[0];
  switch (protocol) {
    case 0: handleCustomBinary(buffer); break;
    case 1: handleArrow(buffer); break;
    case 2: handleJson(buffer); break;
  }
}
```

The `#[wasm_bin]`, `#[wasm_worker]`, `#[wasm_service]` proc macros handle delivery (decision 014) — the protocol byte is in the message, not the transport.
