# Feature 13: Fetch Wrapper & Request Bundling

## Description

Bundle multiple component requests into a single message via microtask coalescing. Direct communication (shared memory / postMessage) is inherently batched. HTTP transport uses `POST /primal/messages` with Arrow/JSON content type. Responses streamed back with `request_id` and `order_id` for routing.

**Decision:** 026

## Module

`crates/foundation_wasm_ui/assets/foundation-wasm-ui.js` (fetch wrapper)

## Communication modes

| Mode | Bundling | How |
|------|----------|-----|
| WASM instance (direct) | Always bundled | Shared memory batch |
| WebWorker | Always bundled | postMessage batch |
| HTTP Server / ServiceWorker | Bundled | `POST /primal/messages` |
| External HTTP (no primal) | No bundling | Individual fetches |

## Message structure

| Field | Purpose |
|-------|---------|
| `request_id` | Identifies which component made the request |
| `order_id` | scru128 for ordering — out-of-order applied in sequence |
| `dataprotocol` | Protocol payload uses: `arrow`, `json`, etc. |
| `version` | Protocol version |

## Request bundling lifecycle

```
1. Component A calls primal.mountData("/api/update", data) → queued
2. Component B calls primal.mountData("/api/settings", data) → queued
3. Browser fires queueMicrotask → all bundled into one message
4. Message sent (shared memory / postMessage / POST /primal/messages)
5. Server/WASM processes bundle, streams responses with request_id, order_id
6. Client routes each response to the correct component
```

## HTTP bundled endpoint

```
POST /primal/messages
Content-Type: application/primal-arrow   (or application/primal-json)
```

Server reads Content-Type to decode bundle, processes each request, streams responses back via SSE.

## Fetch wrapper

```javascript
class FetchTransport {
    async send(url, method, data) {
        const response = await fetch(url, { method, body: data });
        const handler = ProtocolHandler.fromContentType(response);
        return handler.process(response);
    }
}

class SSETransport {
    connect(url, data) {
        return streamResponse(fetch(url, { body: data }));
    }
}
```

## Dependencies

- Feature 07 (ProtocolHandler, Transport)
- Feature 08 (mount-data, mount-stream)

## Testing

- Bundling: two requests in one microtask → single POST
- Request routing: response with request_id → correct component
- Order: responses arrive out of order → applied in order_id sequence
- External HTTP: individual fetches when no primal server
