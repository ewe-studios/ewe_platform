// One manager belongs to one authenticated application context.
export class ConnectionManager {
  #adapters = new Map();
  #connections = new Set();
  #closed = false;
  #tag = 0n;
  nextTag() {
    if (this.#tag === 0xffffffffffffffffn) throw new Error("Connection identifiers exhausted");
    return ++this.#tag;
  }


  supports(transport) { return this.#adapters.has(transport); }

  register(transport, open) {
    if (this.#closed || this.#adapters.has(transport)) throw new Error("Connection adapter is unavailable");
    this.#adapters.set(transport, open);
    return this;
  }

  acquire(options, listener = {}) {
    if (this.#closed) throw new Error("Connection manager is closed");
    const { transport, endpoint, shared = transport === "websocket", keepAlive = false,
      settings = null, authority = null, protocol = "frame" } = options;
    const url = new URL(endpoint, options.base ?? globalThis.location?.href);
    if (url.username || url.password) throw new Error("Use the connection authentication context");
    url.hash = "";
    if (transport === "websocket") {
      if (url.protocol === "https:") url.protocol = "wss:";
      if (url.protocol === "http:") url.protocol = "ws:";
    }
    let entry = shared && Array.from(this.#connections).find(c =>
      c.shared && c.transport === transport && c.address === url.href && c.settings === settings &&
      c.authority === authority && c.protocol === protocol);
    const handle = { ...listener };
    if (!entry) {
      const open = this.#adapters.get(transport);
      if (!open) throw new Error("Unknown connection transport: " + transport);
      entry = { shared, address: url.href, settings, authority, transport, protocol, retainers: new Set(), handles: new Set([handle]) };
      this.#connections.add(entry);
      try {
        entry.connection = open({ ...options, endpoint: url.href }, {
          listening: () => [...entry.handles].some(h => h.receive),
          deliver: bytes => {
            if (!this.#connections.has(entry)) return;
            if (options.isContext?.(bytes)) {
              entry.context = new Uint8Array(bytes).slice();
              for (const h of [...entry.handles]) h.onContext?.(entry.context.slice());
              return;
            }
            const delivered = new Set();
            for (const h of [...entry.handles]) {
              if (!entry.handles.has(h) || !h.receive || (h.owner && delivered.has(h.owner))) continue;
              try {
                const copy = new Uint8Array(bytes).slice();
                if (h.accept && !h.accept(copy)) continue;
                h.receive(copy);
                if (h.owner) delivered.add(h.owner);
              } catch (cause) { h.onError?.(cause); }
            }
          },
          opened: () => { entry.opened = true; for (const h of [...entry.handles]) h.onOpen?.(); },
          disconnected: cause => { entry.opened = false; entry.context = null; for (const h of [...entry.handles]) h.onError?.(cause); },
        });
      } catch (cause) { this.#connections.delete(entry); throw cause; }
    } else {
      entry.handles.add(handle);
      if (entry.opened || entry.context) queueMicrotask(() => {
        if (!entry.handles.has(handle)) return;
        if (entry.opened) handle.onOpen?.();
        if (entry.context) handle.onContext?.(entry.context.slice());
      });
    }
    const retention = {};
    if (keepAlive) entry.retainers.add(retention);
    const releaseUnused = () => {
      if (!entry.handles.size && !entry.retainers.size) this.#close(entry);
    };
    return {
      send: async bytes => {
        if (!entry.handles.has(handle) || !entry.connection?.send) throw new Error("Connection cannot send");
        await entry.connection.send(bytes);
      },
      close: () => { entry.handles.delete(handle); releaseUnused(); },
      releaseRetention: () => { entry.retainers.delete(retention); releaseUnused(); },
    };
  }

  #close(entry) {
    if (!this.#connections.delete(entry)) return;
    entry.handles.clear();
    entry.connection?.close();
  }

  close() {
    this.#closed = true;
    for (const entry of [...this.#connections]) this.#close(entry);
  }
}
