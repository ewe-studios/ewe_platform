# Feature 11: Request Batching

**Decisions:** New — request bundling via `/primal/messages` endpoint

Queue multiple requests during a microtask, flush as one batched call. Reduces network round-trips and frame overhead.

---

## 1. When Batching Is Enabled

| Transport | Batching | How we know |
|-----------|----------|-------------|
| HTTP server | Yes | Probe `HEAD /primal/messages` — `200 OK` means server supports it |
| Service worker | Yes | We control the SW, it intercepts HTTP |
| Web worker | Yes | Our protocol, we control both sides |
| WASM (local) | Yes | Direct memory handoff, flushed per `stabilize()` cycle |
| WebSocket | Yes (lightweight) | Persistent connection, but still buffers per microtask to reduce frame count |
| SSE | No | Server→client only, streaming protocol |

---

## 2. Probe Detection

On init, the runtime sends a `HEAD` request to `/primal/messages`:

```javascript
async function probeBatching() {
    try {
        const resp = await fetch('/primal/messages', { method: 'HEAD' });
        return resp.ok;  // 200 = server supports batching
    } catch {
        return false;
    }
}
```

If `true`, all subsequent HTTP requests go through the `RequestQueue`. If `false`, requests are sent individually.

---

## 3. RequestQueue

```javascript
class RequestQueue {
    constructor(transport) {
        this.queue = [];
        this.scheduled = false;
        this.transport = transport;
        this.enabled = false;  // set true after probe succeeds
    }

    enqueue(request) {
        if (!this.enabled) {
            this.transport.send(request.url, request.method, request.data);
            return;
        }
        this.queue.push(request);
        if (!this.scheduled) {
            this.scheduled = true;
            queueMicrotask(() => this.flush());
        }
    }

    flush() {
        this.scheduled = false;
        if (this.queue.length === 0) return;
        const batch = this.queue.splice(0);
        this.transport.send('/primal/messages', 'POST', batch);
    }
}
```

---

## 4. Batch Request Format

`POST /primal/messages` with body:
```json
[
  { "id": 1, "url": "/api/users", "method": "GET", "headers": {...} },
  { "id": 2, "url": "/api/items", "method": "POST", "headers": {...}, "body": {...} }
]
```

---

## 5. Batch Response Format

Server returns array of responses, same order:
```json
[
  { "id": 1, "status": 200, "headers": {...}, "body": [...] },
  { "id": 2, "status": 201, "headers": {...}, "body": {...} }
]
```

Each response body is parsed per its `Content-Type` (Arrow, JSON, HTML) and routed to the correct handler.

---

## 6. WebSocket Batching

WebSocket messages are buffered per microtask to reduce frame count:

```javascript
class WSBatchQueue {
    constructor(ws) {
        this.ws = ws;
        this.queue = [];
        this.scheduled = false;
    }

    send(data) {
        this.queue.push(data);
        if (!this.scheduled) {
            this.scheduled = true;
            queueMicrotask(() => this.flush());
        }
    }

    flush() {
        this.scheduled = false;
        if (this.queue.length === 1) {
            this.ws.send(JSON.stringify(this.queue.splice(0, 1)[0]));
        } else if (this.queue.length > 1) {
            this.ws.send(JSON.stringify({ batch: this.queue.splice(0) }));
        }
    }
}
```

Single message: send as-is (no wrapping). Multiple messages: wrap in `{ batch: [...] }`.

---

## 7. Worker Batching

`postMessage` calls are batched into one transfer per microtask:

```javascript
class WorkerBatchQueue {
    constructor(worker) {
        this.worker = worker;
        this.queue = [];
        this.scheduled = false;
    }

    postMessage(data) {
        this.queue.push(data);
        if (!this.scheduled) {
            this.scheduled = true;
            queueMicrotask(() => this.flush());
        }
    }

    flush() {
        this.scheduled = false;
        if (this.queue.length === 0) return;
        const batch = this.queue.splice(0);
        this.worker.postMessage({ batch }, batch.map(d => d.transferable).filter(Boolean));
    }
}
```

---

## 8. Integration Points

| Feature | Connection |
|---------|-----------|
| F06 (Web Components) | Transport classes use RequestQueue for HTTP, WSBatchQueue for WebSocket |
| F10 (Build Pipeline) | Server must expose `HEAD /primal/messages` endpoint for probe detection |
| F00 (foundation_wasm) | WASM local calls batched per `stabilize()` cycle via InstructionReceiver |

---

## 9. File Ownership

```
assets/foundation-wasm-ui.js (bundled from):
    batching.js    — RequestQueue, WSBatchQueue, WorkerBatchQueue, probeBatching()
```

---

## 10. Testing

### Probe Detection (tests 1-3)

| # | Scenario | Verify |
|---|----------|--------|
| 1 | `HEAD /primal/messages` returns 200 | `probeBatching()` returns true, RequestQueue enabled |
| 2 | `HEAD /primal/messages` returns 404 | `probeBatching()` returns false, RequestQueue disabled |
| 3 | `HEAD /primal/messages` throws | `probeBatching()` returns false, RequestQueue disabled |

### HTTP Batching (tests 4-8)

| # | Scenario | Verify |
|---|----------|--------|
| 4 | 3 requests enqueued within same microtask | Single POST to `/primal/messages` with 3 items |
| 5 | 1 request enqueued | Sent as-is (no batch wrapper) |
| 6 | RequestQueue disabled | Each request sent individually |
| 7 | Flush with empty queue | No network call |
| 8 | Batch response parsed | Each response routed to correct handler by Content-Type |

### WebSocket Batching (tests 9-11)

| # | Scenario | Verify |
|---|----------|--------|
| 9 | Single message sent | Sent as-is, no `{ batch: [...] }` wrapper |
| 10 | 3 messages buffered | Sent as `{ batch: [...] }` |
| 11 | Messages across microtask boundaries | Each microtask flushes its own batch |

### Worker Batching (tests 12-14)

| # | Scenario | Verify |
|---|----------|--------|
| 12 | 2 postMessage calls in same microtask | Single postMessage with `{ batch: [...] }` |
| 13 | Transferable buffers included | Buffers transferred, not copied |
| 14 | Worker receives batch | Parses and processes each item |
