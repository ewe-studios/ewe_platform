# Feature 07: JS Runtime UI (foundation-wasm-ui.js)

## Description

Create `foundation-wasm-ui.js` — the DOM-layer JS runtime. Handles DOM application (Arrow + morph), node registry, signal bridge, component registry, event dispatcher, SSE client, and animation frames. Depends on `foundation-wasm.js`.

**Decisions:** 015, 022, 024

## Module

`crates/foundation_wasm_ui/assets/foundation-wasm-ui.js`

## Classes

### ArrowDomApplicator

```javascript
class ArrowDomApplicator {
    static apply(buffer) {
        const cols = ArrowParser.parse(buffer);
        for (let i = 0; i < cols.operations.length; i++) {
            const op = cols.operations[i];
            const nodeId = cols.nodeIds[i];
            const node = NodeRegistry.get(nodeId);
            switch (op) {
                case 2: node.textContent = cols.textVals[i]; break;
                case 3: node.setAttribute(cols.attributes[i], cols.values[i]); break;
                case 14: node.classList.add(cols.values[i]); break;
                case 16: MorphDom.morph(node, cols.textVals[i]); break;
                // ... all 17 operations
            }
        }
    }
}
```

### NodeRegistry

```javascript
class NodeRegistry {
    static register(nodeId, element)
    static get(nodeId) → Element
    static unregister(nodeId)
    static unregisterPrefix(prefix)  // cleanup all with "prefix:*"
}
```

Maps `primal-id` → DOM Element.

### SignalBridge

```javascript
class SignalBridge {
    static applyPatches(json)  // server signal patches
    static bindInput(nodeId, signalId)
    static bindChange(nodeId, signalId)
}
```

### ComponentRegistry

```javascript
class ComponentRegistry {
    static register(tagName, classDef)
    static get(tagName) → classDef
}
```

### EventDispatcher

```javascript
class EventDispatcher {
    static dispatch(nodeId, eventName, data)  // user action → WASM
    static bindDirect(element, event, handler)
    static bindDelegate(parent, event, selector, handler)
}
```

### SSEClient

```javascript
class SSEClient {
    constructor(url)
    connect()
    onMessage(handler)
    disconnect()
}
```

Content-Type aware: dispatches to Arrow/JSON/HTML handlers based on response type.

### `primal` namespace

```javascript
window.primal = {
    // Mount/unmount
    mountData(api, data, targetNode, options) { ... },
    mountStream(api, data, targetNode, options) { ... },
    unmount(targetNode) { ... },

    // Event binding
    on(element, eventType, handler, options) { ... },
    onclick(element, handler, options) { ... },
    onchange(element, handler, options) { ... },
    off(element, eventType) { ... },

    // Scope factory
    scope(parentElement) { ... },

    // Core systems
    Transport,
    ProtocolHandler,
    Patcher,
    Hydrator,
};
```

## Dependencies

- `foundation-wasm.js` (ArrowParser, MemoryAllocations, protocol dispatch)

## Testing

- ArrowDomApplicator: apply batch → DOM updated correctly
- NodeRegistry: register → get → unregister → stale get returns null
- SignalBridge: apply patches → effects re-run
- EventDispatcher: direct bind → fires on click
- EventDispatcher: delegate bind → fires on child click
- SSEClient: connects, receives, dispatches by content type
