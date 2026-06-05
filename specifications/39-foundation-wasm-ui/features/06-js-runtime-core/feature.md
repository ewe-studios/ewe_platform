# Feature 06: JS Runtime Core — Split SDK

## Description

Rewrite the existing foundation_wasm JS runtime into two files with clean separation:

1. **foundation-wasm.js** — Standard communication layer: memory, binary messaging, function calling, callbacks, timers. Rewritten for clarity.
2. **foundation-wasm-ui.js** — DOM-specific layer: Arrow parsing, DOM application, signal bridging, web component registration, event dispatch, SSE client.

## File 1: foundation-wasm.js (Rewritten)

**Location:** `backends/foundation_wasm/runtime/foundation-wasm.js`

### Architecture

```
class FoundationWasm {
    constructor() {
        this.memory = new MemoryAllocations();
        this.functions = new FunctionRegistry();
        this.callbacks = new CallbackRegistry();
        this.timers = new TimerRegistry();
    }
}
```

### MemoryAllocations

```javascript
class MemoryAllocations {
    constructor() {
        this._allocations = new Map();  // id -> Uint8Array
        this._nextId = 1;
    }

    createAllocation(size) {
        const id = this._nextId++;
        this._allocations.set(id, new Uint8Array(size));
        return id;
    }

    getPointer(id) {
        const mem = this._allocations.get(id);
        return mem ? mem.buffer.byteOffset : 0;
    }

    getLength(id) {
        const mem = this._allocations.get(id);
        return mem ? mem.length : 0;
    }

    getBuffer(id) {
        return this._allocations.get(id);
    }

    dispose(id) {
        this._allocations.delete(id);
    }

    clear(id) {
        const mem = this._allocations.get(id);
        if (mem) mem.fill(0);
    }
}
```

### Instructions Encoder/Decoder

```javascript
class Instructions {
    // Encodes instructions (operations + text) into memory slots
    // Decodes return values from memory slots
    // Same binary format as foundation_wasm ops.rs expects
}
```

### FunctionRegistry

```javascript
class FunctionRegistry {
    constructor() {
        this._functions = new Map();  // handle -> function
        this._nextHandle = 1;
    }

    register(code) {
        // Compiles and registers JS code as a callable function
        const handle = this._nextHandle++;
        const fn = new Function('params', code);
        this._functions.set(handle, fn);
        return handle;
    }

    unregister(handle) {
        this._functions.delete(handle);
    }

    invoke(handle, paramsBuffer, returnsBuffer) {
        // Invokes function with params from WASM memory
        // Writes result to returnsBuffer
        const fn = this._functions.get(handle);
        const params = decodeParams(paramsBuffer);
        const result = fn(params);
        encodeResult(returnsBuffer, result);
    }

    invokeAsync(handle, callbackHandle, paramsBuffer, returnsBuffer) {
        // Invokes async function, result sent via callback
        const fn = this._functions.get(handle);
        const params = decodeParams(paramsBuffer);
        const result = fn(params);
        if (result instanceof Promise) {
            result.then(value => {
                this._invokeCallback(callbackHandle, value, returnsBuffer);
            });
        }
    }

    _invokeCallback(callbackHandle, value, returnsBuffer) {
        // Called by WASM to deliver async result
        encodeResult(returnsBuffer, value);
        wasmExports.invoke_callback(callbackHandle, returnsBuffer);
    }
}
```

### CallbackRegistry

```javascript
class CallbackRegistry {
    constructor() {
        this._callbacks = new Map();  // id -> callback function
    }

    register(id, callback) {
        this._callbacks.set(id, callback);
    }

    unregister(id) {
        this._callbacks.delete(id);
    }

    invoke(id, resultBuffer) {
        const callback = this._callbacks.get(id);
        if (callback) {
            const result = decodeResult(resultBuffer);
            callback(result);
        }
    }
}
```

### TimerRegistry

```javascript
class TimerRegistry {
    constructor() {
        this._timeouts = new Map();
        this._intervals = new Map();
        this._animationRequested = false;
    }

    scheduleTimeout(ms, callbackId) {
        const timerId = setTimeout(() => {
            wasmExports.run_scheduled_callback(callbackId);
        }, ms);
        this._timeouts.set(callbackId, timerId);
    }

    unscheduleTimeout(callbackId) {
        const timerId = this._timeouts.get(callbackId);
        if (timerId) {
            clearTimeout(timerId);
            this._timeouts.delete(callbackId);
        }
    }

    scheduleInterval(ms, callbackId) {
        const timerId = setInterval(() => {
            const shouldContinue = wasmExports.run_interval_callback(callbackId);
            if (!shouldContinue) {
                this.unscheduleInterval(callbackId);
            }
        }, ms);
        this._intervals.set(callbackId, timerId);
    }

    unscheduleInterval(callbackId) {
        const timerId = this._intervals.get(callbackId);
        if (timerId) {
            clearInterval(timerId);
            this._intervals.delete(callbackId);
        }
    }

    hookUpAnimationFrames() {
        if (this._animationRequested) return;
        this._animationRequested = true;

        const tick = (time) => {
            wasmExports.trigger_animation_frames(time);
            if (wasmExports.get_total_animation_callbacks() > 0) {
                requestAnimationFrame(tick);
            } else {
                this._animationRequested = false;
            }
        };

        requestAnimationFrame(tick);
    }
}
```

### Batch API

```javascript
// host_batch_apply: apply operations, no return
function host_batch_apply(opsPtr, opsLen, textPtr, textLen) {
    const opsBuffer = memorySlice(opsPtr, opsLen);
    const textBuffer = memorySlice(textPtr, textLen);
    applyInstructions(opsBuffer, textBuffer);
}

// host_batch_returning_apply: apply operations, return results
function host_batch_returning_apply(opsPtr, opsLen, textPtr, textLen) {
    const opsBuffer = memorySlice(opsPtr, opsLen);
    const textBuffer = memorySlice(textPtr, textLen);
    const results = applyInstructionsWithReturns(opsBuffer, textBuffer);
    return encodeResults(results);
}
```

## File 2: foundation-wasm-ui.js (New)

**Location:** `backends/foundation_wasm_ui/runtime/foundation-wasm-ui.js`

### ArrowParser + ArrowDomApplicator

```javascript
class ArrowDomApplicator {
    constructor(nodeRegistry) {
        this.registry = nodeRegistry;
    }

    apply(buffer) {
        const header = new DataView(buffer, 0, 4);
        const opCount = header.getUint32(0, true);

        // Parse columnar data
        let offset = 4;
        const nodeIds = new Uint32Array(buffer, offset, opCount);
        offset += opCount * 4;
        const ops = new Uint8Array(buffer, offset, opCount);
        offset += opCount;
        // String columns: length-prefixed
        const attrs = this._parseStringColumn(buffer, offset, opCount);
        // ... parse remaining columns

        for (let i = 0; i < opCount; i++) {
            this._applyOp(ops[i], nodeIds[i], attrs[i], values[i], texts[i]);
        }
    }

    _applyOp(op, nodeId, attr, value, text) {
        const node = this.registry.get(nodeId);
        switch (op) {
            case 0: this._createElement(nodeId, attr, value); break;
            case 2: if (node) node.textContent = text; break;
            case 3: if (node) node.setAttribute(attr, value); break;
            // ... all 15 operations
        }
    }
}
```

### NodeRegistry

```javascript
class NodeRegistry {
    constructor() {
        this._nodes = new Map();
        this._nextId = 100;
    }

    allocate(componentId) {
        const id = componentId * 10000 + this._nextId++;
        return id;
    }

    register(id, element) {
        this._nodes.set(id, element);
    }

    get(id) {
        return this._nodes.get(id);
    }

    unregister(id) {
        this._nodes.delete(id);
    }

    unregisterComponent(componentId) {
        // Remove all nodes belonging to a component
        const prefix = componentId * 10000;
        for (const [id] of this._nodes) {
            if (Math.floor(id / 10000) === componentId) {
                this._nodes.delete(id);
            }
        }
    }
}
```

### SignalBridge

```javascript
class SignalBridge {
    constructor(registry) {
        this.registry = registry;
    }

    bindInput(nodeId, callbackId) {
        const node = this.registry.get(nodeId);
        if (!node) return;
        node.addEventListener('input', (e) => {
            this._sendToWasm(callbackId, e.target.value);
        });
    }

    bindChange(nodeId, callbackId) {
        const node = this.registry.get(nodeId);
        if (!node) return;
        node.addEventListener('change', (e) => {
            this._sendToWasm(callbackId, e.target.checked);
        });
    }

    _sendToWasm(callbackId, value) {
        const encoded = encodeValue(value);
        wasmExports.invoke_callback(callbackId, encoded);
    }
}
```

### ComponentRegistry

```javascript
class ComponentRegistry {
    constructor(nodeRegistry, signalBridge) {
        this.registry = nodeRegistry;
        this.signalBridge = signalBridge;
    }

    register(tagName, observedAttributes, useShadow) {
        class WasmElement extends HTMLElement {
            constructor() {
                super();
                this._componentId = nextComponentId++;
                if (useShadow) {
                    this._shadowRoot = this.attachShadow({ mode: 'open' });
                }
            }

            connectedCallback() {
                const root = this._shadowRoot || this;
                const rootNodeId = this.registry.allocate(this._componentId);
                this.registry.register(rootNodeId, root);
                wasmExports.on_connected(this._componentId, rootNodeId);
            }

            disconnectedCallback() {
                wasmExports.on_disconnected(this._componentId);
                this.registry.unregisterComponent(this._componentId);
            }

            attributeChangedCallback(name, oldVal, newVal) {
                wasmExports.on_attribute_changed(this._componentId, name, oldVal, newVal);
            }
        }

        WasmElement.observedAttributes = observedAttributes;
        customElements.define(tagName, WasmElement);
    }
}
```

### SSEClient

```javascript
class SSEClient {
    constructor(applicator) {
        this.applicator = applicator;
    }

    connect(url) {
        const source = new EventSource(url);
        source.addEventListener('dom-patch', (event) => {
            const patch = JSON.parse(event.data);
            this._applyPatch(patch);
        });
    }

    _applyPatch(patch) {
        switch (patch.type) {
            case 'morph':
                morph(patch.target, patch.html);
                break;
            case 'signal':
                wasmExports.update_signal(patch.signalId, patch.value);
                break;
        }
    }
}
```

## Delivery

Both files are delivered as:
1. Standalone `.js` files (script tag)
2. ES modules `.mjs` (import)
3. `foundation-wasm-ui.js` depends on `foundation-wasm.js` being loaded first

## Dependencies

- foundation-wasm-ui.js requires foundation-wasm.js loaded first
- Both use `wasmExports` — the WASM module's exported functions

## Testing

- foundation-wasm.js: memory allocate/get/dispose works
- foundation-wasm.js: function register/invoke works
- foundation-wasm.js: callback register/invoke works
- foundation-wasm.js: timers fire correctly
- foundation-wasm.js: batch apply works (no DOM ops, just memory)
- foundation-wasm-ui.js: Arrow parse + DOM apply works
- foundation-wasm-ui.js: NodeRegistry allocate/register/get works
- foundation-wasm-ui.js: SignalBridge input event → WASM callback
- foundation-wasm-ui.js: ComponentRegistry → customElements defined
- Both files: no circular dependencies
