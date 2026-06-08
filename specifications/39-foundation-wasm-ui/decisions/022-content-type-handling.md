# 022 — Content-Type-based response handling

**Date:** 2026-06-08
**Status:** Resolved

### Content types

| Content-Type | Meaning |
|--------------|---------|
| `text/html` | Plain HTML — browser renders, no framework handling |
| `application/primal-html` | HTML with primal semantics (scoped scripts/styles, event bindings) |
| `application/primal-arrow` | Arrow binary format — DOM ops, signal patches |
| `application/primal-json` | JSON signal/DOM updates |
| `text/event-stream-html` | SSE with HTML patches |
| `text/event-stream-arrow` | SSE with Arrow binary patches |
| `text/event-stream-json` | SSE with JSON signal/DOM updates |

### First render vs. runtime handling

**First render**: The initial page is `text/html`. Browser renders it normally. No framework intervention — the JS doesn't know how to handle it yet because the runtime hasn't loaded.

**After runtime initializes**:
1. Scan document for `primal:*` attributes → wire up event bindings
2. Find all `<island>` tags → create `IslandComponent` instances → process scoped scripts/styles
3. Mount components (`<mount-ui>`, `<mount-data>`, `<mount-api>`, `<mount-stream>`) → register with runtime

**Content types matter for dynamic responses** — when components fetch/stream data, the response `Content-Type` tells the runtime what to do:

```js
// Inside mount-stream / mount-data / mount-api components
async function handleResponse(response) {
    const contentType = response.headers.get('content-type') || '';
    
    if (contentType.includes('application/primal-html')) {
        const html = await response.text();
        this.materialize(html);  // wrap in <island>, process scoped content
    }
    else if (contentType.includes('application/primal-arrow')) {
        const buffer = await response.arrayBuffer();
        this.applyArrow(buffer);  // ArrowDomApplicator
    }
    else if (contentType.includes('application/primal-json')) {
        const json = await response.json();
        this.applyJson(json);  // signal patches + DOM updates
    }
    else if (contentType.includes('text/event-stream')) {
        this.streamSSE(response);  // SSE parser dispatches to above handlers
    }
    else {
        // text/html or unknown — let browser handle
        this.renderRaw(response);
    }
}
```

### Protocol-aware communication (Rust → JS)

All messages carry a protocol header: `[protocol: u8][version: u8][payload_length: u32][payload...]`

| Protocol byte | Meaning |
|---------------|---------|
| 0 | Custom binary (foundation_wasm Instructions) |
| 1 | Arrow format |
| 2 | JSON format |

The `InstructionReceiver` (decision 030) doesn't produce Arrow directly. It collects `Vec<DomOp>`. When it flushes after `stabilize()`, the **protocol implementation** encodes and ships them:

```rust
// InstructionReceiver.flush() — owned by the Runtime
let ops = self.receiver.take_ops();
self.protocol.send(ops, &self.memory);  // encodes + calls FFI
```

The protocol is **set at init time** — the client doesn't negotiate, it reads the protocol byte from the incoming message and uses the matching handler.

### Bidirectional synergy

The JS runtime provides client-side capabilities, but the **true power** is when both sides understand each other:

```
Server (Rust)                         Client (JS Runtime)
─────────────                         ─────────────────────
html! { ... }                    →    <island> hydrates primal:*
                                      scopes scripts/styles
                                      wires events

Signal::set(value)               ←→  signal.get() updates DOM
                                      receiver.queue(DomOp) → flush → FFI → host_apply

WebServe routes()                →    ServiceWorker intercepts matching fetches

SSE stream (primal-json)         ←→  signal patches applied, DOM morphed
```

Both sides share:
- **Same content types** — `application/primal-html`, `primal-arrow`, `primal-json`
- **Same protocol envelope** — `[protocol: u8][version: u8][payload...]`
- **Same HTML semantics** — `primal:*` attributes, `<island>` components, `primal:id` identifiers
- **Same signal model** — `foundation_signals` (Rust) ↔ JS signal counterpart

### Full-stack synergy

When using both the Rust framework (`html!` macro) and JS runtime together:

1. **Compile-time optimization** — Rust processes scoped styles/scripts, generates `primal:id` attributes, transforms CSS
2. **Runtime handling** — JS runtime hydrates `primal:*` attributes, creates `<island>` components, processes dynamic responses
3. **Best of both worlds** — pre-processed output + reactive runtime = fast initial load + full interactivity

Without the Rust framework, plain HTML works but loses compile-time optimizations. Without the JS runtime, the browser handles content types it doesn't understand as plain text.

### Ubiquitous execution — the framework's power key

The same Rust framework code runs in **four execution contexts**:

| Context | How | Use Case |
|---------|-----|----------|
| **Main thread** | WASM instance in the page | Direct, fast, DOM access |
| **Web Worker** | WASM in worker, postMessage bridge | Offloaded computation, no blocking |
| **Service Worker** | WASM in service worker, fetch interception | Offline, routing, caching |
| **HTTP Server** | Native Rust binary | Server-rendered HTML, API endpoints |

**Content types are the universal protocol** that ties all four together:
- Server sends `application/primal-html` → client renders via `<island>`
- Web Worker sends `application/primal-arrow` → main thread applies DOM ops
- Service Worker sends `application/primal-json` → browser handles signal patches
- Client fetches → gets any content type → runtime handles accordingly

**Same code, same protocol, different runtime.** The `html!` macro compiles the same way. The `#[wasm_bin]` / `#[wasm_worker]` / `#[wasm_service]` proc macros (decisions 013, 014, 016, 017) handle the delivery. The content type system ensures every context understands what the others are sending.
