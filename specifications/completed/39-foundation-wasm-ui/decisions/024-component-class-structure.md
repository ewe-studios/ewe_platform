# 024 — Component architecture: shared classes for transport, patching, and hydration

**Date:** 2026-06-08
**Status:** Resolved

### Decision

`<island>`, `<mount-data>`, and `<mount-stream>` are thin custom elements that compose shared classes. Each class encapsulates a specific responsibility.

### Class structure

```js
// ─── Transport layer ───────────────────────────────────────────────

class Transport {
    // Factory — creates the right transport based on config
    static create(config) {
        switch (config.transport) {
            case 'fetch':     return new FetchTransport(config);
            case 'sse':       return new SSETransport(config);
            case 'ws':        return new WebSocketTransport(config);
            case 'chunked':   return new ChunkedTransport(config);
            case 'worker':    return new WorkerTransport(config);
            default:          return new FetchTransport(config);
        }
    }
}

class FetchTransport {
    async send(url, method, data) {
        const response = await fetch(url, { method, body: data });
        const handler = ProtocolHandler.fromContentType(response);
        return handler.process(response);
    }
}

class SSETransport {
    connect(url, data) {
        // Fetch with Accept: text/event-stream
        return streamResponse(fetch(url, { body: data }));
    }
}

class WebSocketTransport {
    connect(url) {
        this.ws = new WebSocket(url);
        this.ws.onmessage = (e) => this._dispatch(e.data);
    }
}

// ─── Protocol handler layer ────────────────────────────────────────

class ProtocolHandler {
    static fromContentType(responseOrHeaders) {
        const ct = responseOrHeaders.headers?.get('content-type') || '';
        if (ct.includes('primal-arrow')) return new ArrowHandler();
        if (ct.includes('primal-json')) return new JsonHandler();
        if (ct.includes('primal-html')) return new HtmlHandler();
        return new RawHandler();
    }
}

class ArrowHandler {
    async process(response) {
        const buffer = await response.arrayBuffer();
        ArrowDomApplicator.apply(buffer);  // zero-copy, columnar
    }
}

class JsonHandler {
    async process(response) {
        const json = await response.json();
        SignalBridge.applyPatches(json);
    }
}

class HtmlHandler {
    async process(response) {
        const html = await response.text();
        Patcher.materialize(html);  // wrap in <island>, hydrate
    }
}

// ─── Patcher layer ─────────────────────────────────────────────────

class Patcher {
    static materialize(html, target) {
        // Parse HTML, wrap scoped content in <island>, inject
        const fragment = document.createRange().createContextualFragment(html);
        target.appendChild(fragment);
        Hydrator.hydrate(fragment);
    }

    static applyDomOps(ops) {
        // Apply Arrow-encoded DOM operations
        ArrowDomApplicator.apply(ops);
    }

    static applySignalPatches(patches) {
        // Update signals, trigger effects
        SignalBridge.applyPatches(patches);
    }
}

// ─── Hydrator layer ────────────────────────────────────────────────

class Hydrator {
    static hydrate(root) {
        this._wireEvents(root);
        this._processStyles(root);
        this._processScripts(root);
    }

    static _wireEvents(root) {
        const elements = root.querySelectorAll('[primal\\:on]');
        for (const el of elements) {
            EventBinder.wire(el);
        }
    }

    static _processStyles(root) {
        const styles = root.querySelectorAll('style[primal\\:style]');
        for (const style of styles) {
            StyleProcessor.process(style);
        }
    }

    static _processScripts(root) {
        const scripts = root.querySelectorAll('script[primal\\:script]');
        for (const script of scripts) {
            ScriptExecutor.execute(script);
        }
    }
}

// ─── Specialized processors ────────────────────────────────────────

class EventBinder {
    static wire(element) {
        // Parse primal:onclick, primal:onchange, etc.
        // Wire direct listener or delegation based on modifier
    }
}

class StyleProcessor {
    static process(styleElement) {
        // Extract CSS, prefix selectors, inject into <head>
    }
}

class ScriptExecutor {
    static execute(scriptElement) {
        // Extract function(scope){...}, create scope object, execute
    }
}

// ─── The `primal` namespace — single entry point for all runtime logic ───

window.primal = {
    // Mount/unmount components
    mountData(api, data, targetNode, options) { ... },
    mountStream(api, data, targetNode, options) { ... },
    unmount(targetNode) { ... },

    // Event binding
    on(element, eventType, handlerRef, options) { ... },
    onclick(element, handlerRef, options) { ... },
    onchange(element, handlerRef, options) { ... },
    off(element, eventType) { ... },

    // Scope factory (for scoped scripts)
    scope(parentElement) { ... },

    // Access to core systems (for advanced users)
    Transport,
    ProtocolHandler,
    Patcher,
    Hydrator,
};
```

### Why this design

- **Single namespace** — `primal.*` is the only global the runtime creates
- **Developers use it directly** — `primal.mountData(...)` for programmatic mounting
- **Custom elements use it internally** — `<mount-data>` calls `primal.mountData()` in `connectedCallback`
- **Clean API surface** — one entry point, well-documented, easy to discover

class IslandComponent extends HTMLElement {
    connectedCallback() {
        Hydrator.hydrate(this);
    }

    disconnectedCallback() {
        // Cleanup: styles, scripts, events
    }
}

class MountDataComponent extends HTMLElement {
    constructor() {
        super();
        this.transport = Transport.create({ transport: 'fetch' });
    }

    connectedCallback() {
        // Read api, method, data attributes
        // Execute transport, process response
    }
}

class MountStreamComponent extends HTMLElement {
    constructor() {
        super();
        this.transport = Transport.create({
            transport: this.getAttribute('transport') || 'sse',
            api: this.getAttribute('api'),
        });
    }

    connectedCallback() {
        this.transport.connect(this.getAttribute('api'), this._getData());
    }

    disconnectedCallback() {
        this.transport.disconnect();
    }
}

customElements.define('island', IslandComponent);
customElements.define('mount-data', MountDataComponent);
customElements.define('mount-stream', MountStreamComponent);
```

### Why this design

- **Separation of concerns** — transport, protocol, patching, hydration are independent
- **Composable** — `<island>` uses `Hydrator`, `<mount-data>` uses `Transport` + `ProtocolHandler` + `Patcher`
- **Extensible** — add new transports or protocol handlers without touching components
- **Testable** — each class can be tested in isolation
- **Thin custom elements** — web components are just glue, no logic in the element itself
