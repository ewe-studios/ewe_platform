# Feature 06: JS Runtime Core (foundation-wasm.js)

## Description

Rewrite `megatron.js` (7181 lines) into a clean `foundation-wasm.js` — the core JS runtime with no DOM concepts. Handles memory management, function/callback registries, protocol dispatch, and Arrow parsing. Feature-detects SharedArrayBuffer vs Transferable at init.

**Decisions:** 014, 015, 028

## Module

`crates/foundation_wasm_ui/assets/foundation-wasm.js`

## Classes

### MemoryAllocations

```javascript
class MemoryAllocations {
    createAllocation(size) → { memoryId, ptr, len }
    disposeAllocation(memoryId)
    get(memoryId) → ArrayBufferView
    clear()
}
```

Generation-based IDs: `MemoryId = (index, generation)`. Stale ID → `InvalidAllocationId`.

### FunctionRegistry

```javascript
class FunctionRegistry {
    registerFunction(fn) → functionId
    invokeAsSync(functionId, args) → result
    invokeAsync(functionId, args) → Promise<result>
    unregisterFunction(functionId)
}
```

### CallbackRegistry

```javascript
class CallbackRegistry {
    registerCallback(fn) → callbackId  // monotonic, never reused
    invokeCallback(callbackId, data)
    unregisterCallback(callbackId)
}
```

Stale ID → silently dropped (no panic, no wrong callback).

### TimerRegistry

```javascript
class TimerRegistry {
    scheduleTimeout(fn, ms) → timerId
    scheduleInterval(fn, ms) → timerId
    cancelTimer(timerId)
}
```

### Protocol Dispatcher

```javascript
function dispatchMessage(buffer) {
    const protocol = new Uint8Array(buffer)[0];
    switch (protocol) {
        case 0: handleCustomBinary(buffer); break;
        case 1: handleArrow(buffer); break;
        case 2: handleJson(buffer); break;
    }
}
```

### ArrowParser

```javascript
class ArrowParser {
    static parse(buffer) → {
        opIds: Uint32Array,      // zero-copy view
        nodeIds: Uint32Array,
        operations: Uint8Array,
        attributes: string[],
        values: string[],
        textVals: string[],
    }
    static encode(ops) → ArrayBuffer  // Rust-side encoding
}
```

### Transport Detection

```javascript
function detectTransportCapability() {
    // Probes SharedArrayBuffer + Atomics
    // Returns { mode: 'shared' } or { mode: 'transfer' }
}
```

- `shared` — WASM memory is `SharedArrayBuffer`, direct reads
- `transfer` — WASM memory is private, use `postMessage` with Transferable

### Batch API

```javascript
function host_batch_apply(ptr, len) { ... }
function host_arrow_apply(ptr, len) { ... }
```

Common envelope: `[protocol: u8][version: u8][batch_memory_id: u64][payload...]`.
All protocols include `batch_memory_id` in the 10-byte envelope so JS can ACK.

## Dependencies

- None (pure JS)

## Testing

- MemoryAllocations: create → get → dispose → stale get fails
- FunctionRegistry: register → invoke → result correct
- CallbackRegistry: monotonic IDs, stale invoke silently dropped
- ArrowParser: encode → parse → columns match
- Protocol dispatcher: routes to correct handler
- Transport detection: correct mode based on COOP/COEP headers
