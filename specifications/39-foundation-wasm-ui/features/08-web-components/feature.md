# Feature 08: Web Components (island, mount-data, mount-stream)

## Description

Three custom elements that compose shared classes: `<island>` (scoped content container), `<mount-data>` (request-response), `<mount-stream>` (continuous streaming). Each is a thin glue element — logic lives in shared Transport, ProtocolHandler, Patcher, Hydrator classes.

**Decisions:** 021, 023, 024

## Module

`crates/foundation_wasm_ui/assets/foundation-wasm-ui.js` (web component classes)

## Shared Classes (Feature 07)

### Transport layer

```javascript
class Transport {
    static create(config) {
        switch (config.transport) {
            case 'fetch':   return new FetchTransport(config);
            case 'sse':     return new SSETransport(config);
            case 'ws':      return new WebSocketTransport(config);
            case 'chunked': return new ChunkedTransport(config);
            case 'worker':  return new WorkerTransport(config);
            default:        return new FetchTransport(config);
        }
    }
}
```

### Protocol handler layer

```javascript
class ProtocolHandler {
    static fromContentType(response) {
        const ct = response.headers.get('content-type') || '';
        if (ct.includes('primal-arrow')) return new ArrowHandler();
        if (ct.includes('primal-json')) return new JsonHandler();
        if (ct.includes('primal-html')) return new HtmlHandler();
        return new RawHandler();
    }
}
```

### Patcher layer

```javascript
class Patcher {
    static materialize(html, target) {
        const fragment = document.createRange().createContextualFragment(html);
        target.appendChild(fragment);
        Hydrator.hydrate(fragment);
    }
    static applyDomOps(buffer) { ArrowDomApplicator.apply(buffer); }
    static applySignalPatches(patches) { SignalBridge.applyPatches(patches); }
}
```

### Hydrator layer

```javascript
class Hydrator {
    static hydrate(root) {
        this._wireEvents(root);
        this._processStyles(root);
        this._processScripts(root);
    }
}
```

## Custom Elements

### `<island>` — Pure rendering container

```javascript
class IslandComponent extends HTMLElement {
    connectedCallback() { Hydrator.hydrate(this); }
    disconnectedCallback() { /* cleanup: styles, scripts, events */ }
}
```

- Takes its children and wires them up
- Scopes styles (`primal:style`) and scripts (`primal:script`)
- Adds event bindings for `primal:on*` attributes
- No fetch, no request — just renders and hydrates

### `<mount-data>` — Request-response

```javascript
class MountDataComponent extends HTMLElement {
    connectedCallback() {
        const api = this.getAttribute('api');
        const data = JSON.parse(this.getAttribute('data') || '{}');
        const method = this.getAttribute('method') || 'POST';
        primal.mountData(api, data, this, { method });
    }
}
```

- Sends POST/PUT/DELETE to configured endpoint
- Expects single response — processes via content type
- **Default**: replaces itself with response content
- **`target` attribute**: injects content at target instead
  - `target="#id"` → inject into element with matching ID
  - `target="parent"` → inject into parent (replaces siblings)
  - omitted → self-replacement

### `<mount-stream>` — Continuous streaming

```javascript
class MountStreamComponent extends HTMLElement {
    connectedCallback() {
        const api = this.getAttribute('api');
        const transport = this.getAttribute('transport') || 'sse';
        const data = JSON.parse(this.getAttribute('data') || '{}');
        primal.mountStream(api, data, this, { transport });
    }
}
```

- Like `<mount-data>` but expects continuous responses
- Transport: `sse` (default), `ws`, `chunked`, or omitted (fetch)
- Same response placement rules as `<mount-data>`

### Server-rendered mount components

Server populates `data` attribute with initial JSON payload:
```html
<mount-data api="/v2/users" method="POST" data='{"filter": "active"}' />
```

### Programmatic JS API

```javascript
primal.mountData(api, data, targetNode, { transport: 'fetch', method: 'POST' });
primal.mountStream(api, data, targetNode, { transport: 'sse' });
primal.unmount(targetNode);
```

## Dependencies

- Feature 06 (foundation-wasm.js)
- Feature 07 (foundation-wasm-ui.js core classes)

## Testing

- Island: connected → styles scoped, scripts executed, events wired
- Island: disconnected → cleanup complete
- Mount-data: connected → request sent, response materialized
- Mount-data: with target → content placed at target
- Mount-stream: connected → SSE connected, responses applied
- Mount-stream: disconnected → SSE disconnected
- Server-rendered mount-data: data attribute parsed correctly
