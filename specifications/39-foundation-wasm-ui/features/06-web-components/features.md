# Feature 06: Web Components

## Description

Three custom elements that compose shared classes: `<island>` (scoped content container), `<mount-data>` (request-response), `<mount-stream>` (continuous streaming). Each is a thin glue element — logic lives in shared Transport, ProtocolHandler, Patcher, Hydrator classes.

**Decisions:** 021, 023, 024

## Module

`crates/foundation_wasm_ui/assets/foundation-wasm-ui.js`

## Shared Classes

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
}
```

## Custom Elements

### `<island>` — Pure rendering container

- Takes its children and wires them up
- Scopes styles (`primal:style`) and scripts (`primal:script`)
- Adds event bindings for `primal:on*` attributes
- No fetch, no request — just renders and hydrates

### `<mount-data>` — Request-response

- Sends POST/PUT/DELETE to configured endpoint
- Expects single response — processes via content type
- **Default**: replaces itself with response content
- **`target` attribute**: injects content at target instead
  - `target="#id"` → inject into element with matching ID
  - `target="parent"` → inject into parent (replaces siblings)
  - omitted → self-replacement

### `<mount-stream>` — Continuous streaming

- Like `<mount-data>` but expects continuous responses
- Transport: `sse` (default), `ws`, `chunked`, or omitted (fetch)
- Same response placement rules as `<mount-data>`

## Dependencies

- Feature 00 (JS runtime assets)
- Feature 07 (Arrow encoding for DOM ops)

## Testing

- Island: connected → styles scoped, scripts executed, events wired
- Island: disconnected → cleanup complete
- Mount-data: connected → request sent, response materialized
- Mount-data: with target → content placed at target
- Mount-stream: connected → SSE connected, responses applied
- Mount-stream: disconnected → SSE disconnected