# Feature 06: Web Components

Three custom elements composing shared classes: `<island>` (scoped content container), `<mount-data>` (request-response), `<mount-stream>` (continuous streaming). Each element is thin glue — logic lives in Transport, ProtocolHandler, Patcher, Hydrator.

**Decisions:** 021, 022, 023, 024 | **Module:** `assets/foundation-wasm-ui.js`

---

## 1. Shared Classes

| Layer | Classes | Responsibility |
|-------|---------|---------------|
| Transport | `Transport`, `FetchTransport`, `SSETransport`, `WebSocketTransport`, `ChunkedTransport`, `WorkerTransport` | Send requests, receive bytes/streams |
| Protocol | `ProtocolHandler`, `ArrowHandler`, `JsonHandler`, `HtmlHandler`, `RawHandler` | Content-type detection, response parsing |
| Patcher | `Patcher` | Fragment creation, DOM insertion, delegate to Arrow/Signal subsystems |
| Hydrator | `Hydrator` | Post-insertion wiring: events, scoped styles, scoped scripts |

---

## 2. Transport Layer

```javascript
class Transport {
    static create(config) {
        switch (config.transport) {
            case 'sse':     return new SSETransport(config);
            case 'ws':      return new WebSocketTransport(config);
            case 'chunked': return new ChunkedTransport(config);
            case 'worker':  return new WorkerTransport(config);
            case 'fetch': default: return new FetchTransport(config);
        }
    }
}
```

**FetchTransport** — `constructor(config)` stores url/method/headers. `send(url, method, data)`: fetch, `ProtocolHandler.fromContentType(response)`, return `handler.process(response)`. `disconnect()`: no-op.

**SSETransport** — `constructor(config)` stores url, `_eventSource = null`. `connect(url, data, onChunk)`: create `EventSource(url)`, `onmessage` dispatches through ProtocolHandler, `onerror` triggers `_reconnect()`. `_reconnect()`: exponential backoff 1s/2s/4s capped at 30s, reopen EventSource. `disconnect()`: `_eventSource.close()`, null.

**WebSocketTransport** — `constructor(config)` stores url, `_ws = null`. `connect(url, onMessage)`: create `WebSocket(url)`, `onmessage` dispatches through ProtocolHandler, `onclose` triggers `_reconnect()`. `send(data)`: `_ws.send(JSON.stringify(data))`. `_reconnect()`: same backoff as SSE. `disconnect()`: `_ws.close()`, null.

**ChunkedTransport** — `constructor(config)` stores url, `_reader = null`, `_abortController = null`. `connect(url, data, onChunk)`: create AbortController, fetch with signal, get reader from `response.body`, loop `reader.read()` dispatching chunks through ProtocolHandler. `disconnect()`: `_abortController.abort()`, null reader.

**WorkerTransport** — `constructor(config)` stores worker reference. `send(data)`: `_worker.postMessage(data)`. `onMessage(callback)`: `_worker.onmessage`. `disconnect()`: `_worker.terminate()` (Web Worker only).

**Default resolution:** No transport attribute, no worker context -> `FetchTransport`. Inside web worker -> `WorkerTransport` via `postMessage`. Inside service worker -> `FetchTransport` with SW fetch interception.

---

## 3. Protocol Handler Layer

### Content-Type Table

| Content-Type | Handler | Stream Variant |
|-------------|---------|----------------|
| `text/html` | HtmlHandler | |
| `application/primal-html` | HtmlHandler | `text/event-stream-html` |
| `application/primal-arrow` | ArrowHandler | `text/event-stream-arrow` |
| `application/primal-json` | JsonHandler | `text/event-stream-json` |

### Factory and Handlers

```javascript
class ProtocolHandler {
    static fromContentType(response) {
        const ct = response.headers.get('content-type') || '';
        if (ct.includes('primal-arrow') || ct.includes('event-stream-arrow')) return new ArrowHandler();
        if (ct.includes('primal-json')  || ct.includes('event-stream-json'))  return new JsonHandler();
        if (ct.includes('primal-html')  || ct.includes('event-stream-html') || ct.includes('text/html'))
            return new HtmlHandler();
        return new RawHandler();
    }
}
```

**ArrowHandler.process(response)** — `arrayBuffer()` -> `ArrowParser.parse(buffer)` -> return `{ type: 'arrow', columns }`. Caller passes to `ArrowDomApplicator.apply`.

**JsonHandler.process(response)** — `response.json()` -> return `{ type: 'json', patches }`. Caller passes to `SignalBridge.applyPatches`.

**HtmlHandler.process(response)** — `response.text()` -> return `{ type: 'html', html }`. Caller passes to `Patcher.materialize`.

**RawHandler.process(response)** — `response.text()` -> return `{ type: 'raw', text }`. Fallback: inserted as `textContent`.

---

## 4. Patcher

```javascript
class Patcher {
    static materialize(html, target) {
        // 1. createRange().createContextualFragment(html) — parse without inserting
        // 2. target.appendChild(fragment) — triggers connectedCallback on custom elements
        // 3. Hydrator.hydrate(target) — wire events, scope styles, execute scripts
    }
    static applyDomOps(columns)        { ArrowDomApplicator.apply(columns); }
    static applySignalPatches(patches) { SignalBridge.applyPatches(patches); }
}
```

---

## 5. Hydrator

```javascript
class Hydrator {
    static hydrate(root) {
        // 1. Wire events: querySelectorAll('[primal\\:on]') and primal:on* attributes
        //    For each, extract event name (strip "on" prefix), resolve handler, addEventListener
        // 2. Process styles: querySelectorAll('style[primal\\:style]')
        //    For each, new CSSStyleSheet(), scope rules with parent id/class prefix,
        //    document.adoptedStyleSheets push, remove inline tag
        // 3. Process scripts: querySelectorAll('script[primal\\:script]')
        //    For each, new Function('scope', body), call with _createScope(parentElement), remove tag
    }

    static _createScope(targetElement) {
        const eventCleanups = [];
        return {
            targets()             { return targetElement.querySelectorAll('[primal-id]'); },
            parent()              { return targetElement; },
            querySelector(sel)    { return targetElement.querySelector(sel); },
            querySelectorAll(sel) { return targetElement.querySelectorAll(sel); },
            addEvent(sel, event, handler) {
                const el = targetElement.querySelector(sel);
                if (el) { el.addEventListener(event, handler); eventCleanups.push({ el, event, handler }); }
            },
            cleanup() {
                eventCleanups.forEach(({ el, event, handler }) =>
                    el.removeEventListener(event, handler));
                eventCleanups.length = 0;
            }
        };
    }
}
```

**CSS scoping:** Every rule wrapped with parent selector. `.title { color: red }` inside `#island-1` becomes `#island-1 .title { color: red }`. Compound selectors each prefixed: `.a, .b {}` becomes `#island-1 .a, #island-1 .b {}`.

---

## 6. IslandComponent

No network requests. Wires scoped styles, scripts, and events on existing children.

```javascript
class IslandComponent extends HTMLElement {
    connectedCallback() {
        // Step 1: _processStyles()
        //   this._sheets = []
        //   querySelectorAll('style[primal\\:style]') -> for each:
        //     new CSSStyleSheet(), scope with this.id or this.className prefix
        //     document.adoptedStyleSheets push, track in _sheets, remove tag
        //
        // Step 2: _processScripts()
        //   this._scope = Hydrator._createScope(this)
        //   querySelectorAll('script[primal\\:script]') -> for each:
        //     new Function('scope', textContent), call with _scope, remove tag
        //
        // Step 3: _wireEvents()
        //   querySelectorAll('[primal\\:on]') and primal:on* attrs -> for each:
        //     extract event name, register via _scope.addEvent
    }

    disconnectedCallback() {
        // 1. Remove adopted stylesheets: filter out this._sheets from document.adoptedStyleSheets
        // 2. this._scope.cleanup() — remove all event listeners
        // 3. this._sheets = []; this._scope = null
    }
}
```

---

## 7. MountDataComponent

Request-response. Sends one request on connect, processes one response.

```javascript
class MountDataComponent extends HTMLElement {
    connectedCallback() {
        // 1. Read attributes:
        //    api = getAttribute('api')                    // required
        //    method = getAttribute('method') || 'POST'
        //    data = JSON.parse(getAttribute('data') || '{}')
        //    target = getAttribute('target')              // optional: "#id" or "parent"
        //    transport = getAttribute('transport')        // optional override
        //
        // 2. this._transport = Transport.create({ transport, url: api, method })
        //
        // 3. const result = await this._transport.send(api, method, data)
        //
        // 4. const targetEl = this._resolveTarget(target)
        //
        // 5. Route: html -> Patcher.materialize | arrow -> Patcher.applyDomOps
        //           json -> Patcher.applySignalPatches | raw -> targetEl.textContent
    }

    disconnectedCallback() { this._transport.disconnect(); }
    _resolveTarget(target) { /* See section 9 */ }
}
```

**Programmatic:** `primal.mountData(api, data, targetNode, { method })`

---

## 8. MountStreamComponent

Continuous streaming. Connects on insert, processes responses until disconnect.

```javascript
class MountStreamComponent extends HTMLElement {
    connectedCallback() {
        // 1. Read attributes:
        //    api = getAttribute('api')
        //    transport = getAttribute('transport') || 'sse'   // default SSE
        //    data = JSON.parse(getAttribute('data') || '{}')
        //    target = getAttribute('target')
        //
        // 2. this._transport = Transport.create({ transport, url: api })
        //
        // 3. const targetEl = this._resolveTarget(target)
        //
        // 4. this._transport.connect(api, data, (result) => {
        //        Route same as MountData: html/arrow/json/raw
        //    })
    }

    disconnectedCallback() { this._transport.disconnect(); this._transport = null; }
    _resolveTarget(target) { /* See section 9 */ }
}
```

**Programmatic:** `primal.mountStream(api, data, targetNode, { transport })`

---

## 9. Response Placement

Shared by `<mount-data>` and `<mount-stream>` via `_resolveTarget(target)`:

| `target` value | Behavior |
|---------------|----------|
| omitted / `null` | Self-replacement: create container div, `parentNode.replaceChild(container, this)` |
| `"parent"` | Clear parent children, return parent element |
| `"#some-id"` | `document.getElementById`, clear, return. Throw if missing. |
| CSS selector | `document.querySelector`, clear, return. Throw if missing. |

---

## 10. `window.primal` Namespace

```javascript
window.primal = {
    mountData(api, data, targetNode, opts = {}),   // programmatic mount-data
    mountStream(api, data, targetNode, opts = {}), // programmatic mount-stream
    unmount(element),                               // trigger disconnectedCallback, remove

    on(selector, event, handler),    // addEventListener on querySelector result
    onclick(selector, handler),      // shorthand: on(sel, 'click', handler)
    onchange(selector, handler),     // shorthand: on(sel, 'change', handler)
    off(selector, event, handler),   // removeEventListener

    scope(element),                  // returns Hydrator._createScope(element)

    Transport, ProtocolHandler, Patcher, Hydrator   // advanced direct access
};
```

---

## 11. Custom Element Registration

```javascript
customElements.define('primal-island', IslandComponent);
customElements.define('mount-data', MountDataComponent);
customElements.define('mount-stream', MountStreamComponent);
```

Runs once on `foundation-wasm-ui.js` load. Elements inert until connected to DOM.

---

## 12. Error Cases

| Scenario | Behavior |
|----------|----------|
| Fetch failure (network, 4xx, 5xx) | `FetchTransport.send` rejects. Mount catches, logs error, element unchanged. |
| SSE connection lost | `_reconnect`: exponential backoff 1s/2s/4s capped 30s, retries until disconnect or success. |
| WebSocket close | Same reconnect strategy as SSE. |
| Missing `api` attribute | `connectedCallback` throws `Error('mount-data: missing api attribute')`. Element inert. |
| Missing target element | `_resolveTarget` throws `Error('mount target not found: #id')`. Response discarded. |
| Unknown content-type | `RawHandler` fallback. Response inserted as `textContent`. |
| Invalid JSON in `data` attr | `JSON.parse` throws. `connectedCallback` catches, logs. |
| Script execution error | Caught per-script. Other scripts still execute. Error logged. |
| DOM move (reconnect) | `disconnectedCallback` cleans up, new `connectedCallback` re-wires. Idempotent. |

---

## 13. Integration Points

| Feature | Relationship |
|---------|-------------|
| F00 (JS Runtime) | Asset loading, `window.primal` namespace initialization |
| F02 (Signals) | `JsonHandler` patches -> `SignalBridge.applyPatches` -> signal updates |
| F05 (Arrow) | `ArrowHandler` -> `ArrowParser.parse` + `ArrowDomApplicator.apply` |
| F07 (DOM Morphing) | `Patcher.materialize` may trigger `MorphDom.morph` for incremental updates |
| F08 (Event Runtime) | `Hydrator._wireEvents` feeds into `EventDispatcher` |
| F09 (Scoped Styles) | `StyleProcessor` shares CSS scoping logic with theme system |

**mount-data flow:** `<mount-data api="/items">` -> `connectedCallback` -> `FetchTransport.send` -> server `application/primal-html` -> `HtmlHandler.process` -> `Patcher.materialize` -> `Hydrator.hydrate` -> events/styles/scripts wired.

**mount-stream flow:** `<mount-stream api="/feed" transport="sse">` -> `SSETransport.connect` -> each SSE message -> ProtocolHandler -> Patcher -> DOM updated incrementally.

---

## 14. File Ownership

```
assets/foundation-wasm-ui.js (bundled from):
    transport.js    — Transport factory + 5 transport classes
    protocol.js     — ProtocolHandler factory + 4 handler classes
    patcher.js      — Patcher (materialize, applyDomOps, applySignalPatches)
    hydrator.js     — Hydrator, EventBinder, StyleProcessor, ScriptExecutor
    island.js       — IslandComponent
    mount-data.js   — MountDataComponent
    mount-stream.js — MountStreamComponent
    primal-api.js   — window.primal namespace
```

---

## 15. Refactoring Strategy

1. **Transport:** `Transport.create` factory + `FetchTransport`. Test with mock fetch. Add SSE/WS/Chunked/Worker.
2. **Protocol Handlers:** `ProtocolHandler.fromContentType` + 4 handlers. Test content-type routing.
3. **Patcher + Hydrator:** `materialize` with fragment creation. `hydrate` with EventBinder, StyleProcessor, ScriptExecutor. Test with jsdom.
4. **IslandComponent:** Register, implement connected/disconnected. Test scoping, scripts, events, cleanup.
5. **MountDataComponent:** Attribute reading, transport creation, response routing, target resolution.
6. **MountStreamComponent:** Streaming lifecycle with SSE/WS/chunked. Test connect/disconnect/reconnect.
7. **window.primal:** Wire namespace. Test programmatic mountData/mountStream/unmount/event helpers.

---

## 16. Testing

### Transport (tests 1-7)

| # | Scenario | Verify |
|---|----------|--------|
| 1 | `Transport.create({ transport: 'fetch' })` | Returns `FetchTransport` instance |
| 2 | `Transport.create({})` (no transport) | Returns `FetchTransport` (default) |
| 3 | `Transport.create({ transport: 'sse' })` | Returns `SSETransport` instance |
| 4 | `FetchTransport.send('/api', 'POST', { id: 1 })` | fetch called with correct URL, method, body |
| 5 | `SSETransport.connect` then `.disconnect` | EventSource created then closed |
| 6 | `WebSocketTransport.connect` then `.disconnect` | WebSocket opened then closed |
| 7 | SSE connection lost | `_reconnect` called, backoff 1s/2s/4s |

### Protocol Handlers (tests 8-14)

| # | Scenario | Verify |
|---|----------|--------|
| 8 | `content-type: application/primal-html` | HtmlHandler selected |
| 9 | `content-type: application/primal-arrow` | ArrowHandler selected |
| 10 | `content-type: application/primal-json` | JsonHandler selected |
| 11 | `content-type: text/event-stream-html` | HtmlHandler selected |
| 12 | `content-type: text/html` | HtmlHandler selected |
| 13 | No content-type header | RawHandler selected (fallback) |
| 14 | `HtmlHandler.process(response)` | Returns `{ type: 'html', html }` |

### Patcher (tests 15-18)

| # | Scenario | Verify |
|---|----------|--------|
| 15 | `materialize('<div>hello</div>', target)` | Fragment created, appended, hydrate called |
| 16 | `materialize` with nested custom elements | connectedCallback fires on inner elements |
| 17 | `applyDomOps(columns)` | ArrowDomApplicator.apply called |
| 18 | `applySignalPatches([{ signalId: 1, value: 'x' }])` | SignalBridge.applyPatches called |

### Hydrator (tests 19-24)

| # | Scenario | Verify |
|---|----------|--------|
| 19 | hydrate root with `primal:onclick` elements | Event listeners registered |
| 20 | hydrate root with `<style primal:style>` | CSSStyleSheet created, adopted, tag removed |
| 21 | hydrate root with `<script primal:script>` | Function executed with scope, tag removed |
| 22 | `_createScope(el).targets()` | Returns all `[primal-id]` descendants |
| 23 | `_createScope(el).addEvent('.btn', 'click', fn)` | Listener added, tracked for cleanup |
| 24 | `scope.cleanup()` | All event listeners removed |

### IslandComponent (tests 25-30)

| # | Scenario | Verify |
|---|----------|--------|
| 25 | Island connected with `<style primal:style>` | Scoped with island id prefix, adopted |
| 26 | Island connected with `<script primal:script>` | Script executed, scope object available |
| 27 | Island connected with `primal:onclick` children | Events wired |
| 28 | Island disconnected | Stylesheets removed, events unbound |
| 29 | CSS scoping: `.title {}` in `#island-1` | Becomes `#island-1 .title {}` |
| 30 | Island moved in DOM | Disconnect cleans, reconnect re-wires |

### MountDataComponent (tests 31-37)

| # | Scenario | Verify |
|---|----------|--------|
| 31 | `<mount-data api="/items">` connected | POST to `/items`, response materialized |
| 32 | `api="/items" method="PUT" data='{"id":1}'` | PUT with JSON body |
| 33 | `api="/items" target="#list"` | Response injected into `#list` |
| 34 | `api="/items" target="parent"` | Response injected into parent |
| 35 | No target attribute | Element replaces itself with response |
| 36 | Server returns 500 | Error caught, element unchanged, error logged |
| 37 | `target="#missing"` | Error thrown: target not found |

### MountStreamComponent (tests 38-44)

| # | Scenario | Verify |
|---|----------|--------|
| 38 | `<mount-stream api="/feed">` connected | SSE connection opened (default) |
| 39 | `transport="ws"` | WebSocket connection opened |
| 40 | Stream receives HTML message | Patcher.materialize called |
| 41 | Stream receives Arrow message | Patcher.applyDomOps called |
| 42 | Element disconnected | Transport.disconnect called, connection closed |
| 43 | SSE connection lost | Reconnect with exponential backoff |
| 44 | `target="#output"` | All chunks directed to `#output` |

### window.primal API (tests 45-49)

| # | Scenario | Verify |
|---|----------|--------|
| 45 | `primal.mountData('/api', {}, node)` | MountDataComponent created, connected |
| 46 | `primal.mountStream('/feed', {}, node, { transport: 'ws' })` | MountStreamComponent with ws |
| 47 | `primal.unmount(element)` | Element removed, disconnectedCallback fired |
| 48 | `primal.on('.btn', 'click', fn)` then `primal.off(...)` | Listener added then removed |
| 49 | `primal.scope(element)` | Returns scope with targets, parent, addEvent |
