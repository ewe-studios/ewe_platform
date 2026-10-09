# 023 — Mount components: `<mount-data>`, `<mount-stream>`

**Date:** 2026-06-08
**Status:** Resolved

### Decision

`<island>`, `<mount-data>`, and `<mount-stream>` are custom elements that inherit from a shared core. Each serves a distinct purpose:

### `<island>` — Pure rendering container

- Takes its children and wires them up to work correctly
- Scopes styles (`primal:style`) and scripts (`primal:script`)
- Adds event bindings for elements with `primal:*` attributes
- No fetch, no request — just renders and hydrates content

### `<mount-data>` — Request-response action trigger

- Sends a request (POST/PUT/DELETE, default POST) to the configured endpoint
- Carries state data defined in its HTML attributes
- Expects a single response — processes it via content type
- Like an RPC op — one request, one response

```html
<mount-data api="/v2/users" method="POST" data='{"id": 42}' />
```

### `<mount-stream>` — Streaming connection

- Like `<mount-data>` but expects a continuous stream of responses
- Can optionally include initial data in the request
- Transport specified via `transport` attribute:

```html
<!-- SSE (default for streaming) -->
<mount-stream api="/v2/notifications" transport="sse" />

<!-- WebSocket -->
<mount-stream api="ws://localhost/v2/chat" transport="ws" />

<!-- Chunked HTTP -->
<mount-stream api="/v2/export" transport="chunked" />

<!-- Omit transport — default to fetch (HTTP server) -->
<mount-stream api="/v2/feed" />
```

### Default behavior

- **No transport specified** → defaults to fetch (HTTP server)
- **Runtime initialized via build pipeline** (decision 016) → JS bundle knows the communication mechanism (webworker, wasm main thread, or service worker HTTP)
- **If running with a web worker** → defaults to postMessage to worker
- **If running with a service worker** → defaults to fetch interception
- **Otherwise** → plain HTTP to server

### `<mount-ui>` — Removed

`<mount-ui>` is redundant — its "materialize content into this spot" behavior is exactly what `<mount-data>` or `<mount-stream>` already does. No separate component needed.

### Response placement

By default, `<mount-data>` and `<mount-stream>` **replace themselves** with the response content in their DOM location. The element acts as a placeholder:

```html
<!-- Before request -->
<mount-data api="/v2/users" method="POST" data='{"filter": "active"}' />

<!-- After response -->
<div class="user-list">
  <!-- response content materialized here -->
</div>
```

**Optional `target` attribute** — if specified, content is placed at the target instead of self-replacement:

```html
<mount-data api="/v2/users" target="#user-container" />
<!-- Response content injected into #user-container, mount-data element stays -->
```

Values:
- `target="#id"` — inject into element with matching ID
- `target="parent"` — inject into parent element (replaces all siblings)
- omitted — self-replacement (default)

### Server-rendered mount components

When the server renders `<mount-data>` or `<mount-stream>`, it populates the `data` attribute with a JSON payload (single-quoted to escape double quotes in HTML):

```html
<mount-data api="/v2/users" method="POST" data='{"filter": "active"}' />
```

This tells the client: "when this component hydrates, send this payload to the endpoint and materialize the response."

### Programmatic JS API

The runtime provides functions that the custom elements use internally, and that developers can call directly:

```js
// Mount data — single request/response
primal.mountData(api, data, targetNode, { transport: 'fetch', method: 'POST' });

// Mount stream — continuous stream
primal.mountStream(api, data, targetNode, { transport: 'sse' });

// Disconnect
primal.unmount(targetNode);
```

The `<mount-data>` and `<mount-stream>` custom elements call these functions in their `connectedCallback`:

```js
class MountDataComponent extends HTMLElement {
    connectedCallback() {
        const api = this.getAttribute('api');
        const data = JSON.parse(this.getAttribute('data') || '{}');
        const method = this.getAttribute('method') || 'POST';
        primal.mountData(api, data, this, { method });
    }
}
```
