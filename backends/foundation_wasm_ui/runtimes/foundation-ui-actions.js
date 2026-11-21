// Route browser actions through the negotiated frame protocol.
import * as wire from "./foundation-ui-wire.js";
import { VOCABULARY_VERSION, STRUCTURED_INPUT_VERSION } from "./foundation-ui-vocab.js";
import { base64urlDecode, base64urlEncode, readFrames, sheetSubmitField } from "./foundation-ui-frames.js";
import { buildEventData } from "./foundation-wasm-ui.js";

const validName = name => typeof name === "string" && name.length > 0 && !/[:\s\p{Cc}]/u.test(name);
const error = code => ({ error: code });

export function parseReference(text) {
  if (typeof text !== "string") return error("InvalidReference");
  const [origin, name, extra] = text.split(":");
  if (extra !== undefined || !["host", "server"].includes(origin) || !validName(name)) {
    return error("InvalidReference");
  }
  return { origin, name };
}

export class JsActionRegistry {
  #entries = new Map();

  declare(origin, name, scope = "all", local = origin === "host") {
    if (![null, "host", "server"].includes(origin) || !validName(name) || !["all", "web", "mobile"].includes(scope)) {
      throw new TypeError("Invalid action declaration");
    }
    const entries = this.#entries.get(name) ?? [];
    if (entries.some(e => (e.origin === null || origin === null || e.origin === origin)
        && (scope === "all" || e.scope === "all" || e.scope === scope))) {
      throw new Error("Duplicate action declaration");
    }
    entries.push({ origin, scope, local, page: false });
    this.#entries.set(name, entries);
  }

  declarePage(name) {
    this.declare("host", name, "web", true);
    this.lookup("host", name, "web").page = true;
  }

  registerHost(name, fn, owner) {
    if (typeof fn !== "function") throw new TypeError("Expected a page function");
    let entry = this.lookup("host", name, "web");
    if (entry.error) {
      this.declarePage(name);
      entry = this.lookup("host", name, "web");
    }
    const ownerId = owner?.getAttribute?.("primal-id");
    const sameOwner = owner && (entry.owner === owner || (ownerId != null && ownerId === entry.ownerId));
    if ((entry.fn && !sameOwner) || entry.origin !== "host" || entry.scope !== "web") {
      throw new Error("Duplicate action declaration");
    }
    Object.assign(entry, { fn, page: true, local: true, owner, ownerId });
  }

  lookup(origin, name, surface) {
    const entries = this.#entries.get(name);
    if (!entries) return error("UnknownAction");
    const matching = entries.filter(e => e.origin === null || e.origin === origin);
    if (!matching.length) return error("UnavailableOrigin");
    return matching.find(e => e.scope === "all" || e.scope === surface) ?? error("UnsupportedAction");
  }

  hostFunction(name) { return this.lookup("host", name, "web").fn; }

  refs(surface) {
    const refs = [];
    for (const [name, entries] of this.#entries) {
      for (const entry of entries) {
        if (entry.scope !== "all" && entry.scope !== surface) continue;
        for (const origin of entry.origin ? [entry.origin] : ["host", "server"]) refs.push(origin + ":" + name);
      }
    }
    return refs.sort();
  }
}

export function route(reference, surface, lookup, has, serverHas) {
  const ref = typeof reference === "string" ? parseReference(reference) : reference;
  if (ref.error) return ref;
  const entry = lookup(ref.origin, ref.name, surface);
  const available = serverHas(ref.name);
  if (entry.error && !(entry.error === "UnknownAction" && ref.origin === "server" && available)) return entry;
  let destination = ref.origin;
  if (destination === "host" && !entry.local) {
    if (entry.origin !== null || !available) return error("UnavailableOrigin");
    destination = "server";
  }
  if (!has[destination] || (destination === "server" && available === false)) return error("UnavailableOrigin");
  return { route: destination };
}

function stateBytes(text, limit) {
  if (!text) return null;
  if (text.length > Math.ceil(limit * 4 / 3) || !/^[A-Za-z0-9_-]+$/.test(text)) throw new Error("Invalid state attachment");
  return base64urlDecode(text);
}

function finishRequest(builder, request, stream, sequence) {
  const frame = wire.Frame.createFrame(builder, 1, stream, sequence, wire.FrameKind.FrameRequest, request);
  wire.Frame.finishFrameBuffer(builder, frame);
  return builder.asUint8Array();
}

function encodeAction(name, el, event, type, state, stream, sequence, json) {
  const id = element => Number(element?.getAttribute("primal-id") ?? 0);
  const b = new wire.Builder();
  const mods = Number(!!event.altKey) | Number(!!event.ctrlKey) << 1 | Number(!!event.shiftKey) << 2 | Number(!!event.metaKey) << 3;
  const value = event.type === "click" ? sheetSubmitField(el)?.value ?? el.value : el.value;
  const input = new wire.ActionInputT(type, json === null ? String(value ?? "") : null, !!el.checked, event.keyCode ?? 0, mods, id(el.parentElement), json).pack(b);
  const ref = b.createString(name);
  const attachment = state && wire.ActionRequest.createStateVector(b, state);
  wire.ActionRequest.startActionRequest(b);
  wire.ActionRequest.addAction(b, ref);
  wire.ActionRequest.addNodeId(b, id(el));
  wire.ActionRequest.addInput(b, input);
  if (attachment) wire.ActionRequest.addState(b, attachment);
  const action = wire.ActionRequest.endActionRequest(b);
  wire.FrameRequest.startFrameRequest(b);
  wire.FrameRequest.addAction(b, action);
  return finishRequest(b, wire.FrameRequest.endFrameRequest(b), stream, sequence);
}

export class WebActionManager {
  #busy = new Set();
  #advertised;
  #structured = false;
  #next = 1n;
  #reloaded = false;

  constructor(deps) {
    this.deps = deps;
    this.streamId = deps.streamId ?? 0n;
    this.onError = deps.onError ?? console.error;
    this.limit = deps.maxRequestBytes ?? 1024 * 1024;
    const hooks = deps.frames?.deps;
    if (hooks) {
      const previousCaps = hooks.onCaps;
      const previousError = hooks.onError;
      const caps = value => { previousCaps?.(value); this.applyCaps(value); };
      const failure = detail => {
        if (previousError !== this.onError) previousError?.(detail);
        this.reportControl(detail);
      };
      hooks.onCaps = caps;
      hooks.onError = failure;
      this.detachControls = () => {
        if (hooks.onCaps === caps) hooks.onCaps = previousCaps;
        if (hooks.onError === failure) hooks.onError = previousError;
      };
    }
  }

  setStreamId(id) { this.streamId = BigInt(id); }

  hello() {
    const b = new wire.Builder();
    const hello = new wire.ClientHelloT(VOCABULARY_VERSION, wire.Platform.Web, wire.MountSurface.Web, 0n);
    return finishRequest(b, new wire.FrameRequestT(null, hello).pack(b), 0n, 0n);
  }

  applyCaps(caps) {
    caps = caps.unpack?.() ?? caps;
    this.#structured = caps.status === wire.CapsStatus.Ok && caps.vocabularyVersion >= STRUCTURED_INPUT_VERSION;
    this.#advertised = new Set((caps.actions ?? []).map(a => a.name));
    this.setStreamId(caps.streamId);
    const doc = this.deps.document ?? globalThis.document;
    for (const el of doc?.querySelectorAll("[primal-id]") ?? []) {
      const missing = Array.from(el.attributes).some(attr =>
        attr.name.startsWith("primal:on") && attr.value.startsWith("server:") && !this.#advertised.has(attr.value.slice(7)));
      el.toggleAttribute("data-ewe-unavailable", missing);
    }
  }

  async receive(bytes) {
    if (await this.deps.frames.receive(bytes) === false) throw new Error("Invalid action reply");
  }

  reportControl(detail) {
    this.onError(detail);
    if (!this.#reloaded && ["StateExpired", "UpdateRequired"].includes(detail.code)) {
      this.#reloaded = true;
      (this.deps.reload ?? (() => globalThis.location.reload()))();
    }
  }

  async trigger(reference, el, event = {}, type = event.type) {
    return this.#submit(reference, el, event, type, undefined);
  }

  async triggerStructured(reference, el, payload, event = {}, type = event.type) {
    if (payload === undefined) return this.onError({ code: "InvalidInput" });
    return this.#submit(reference, el, event, type, payload);
  }

  async #submit(reference, el, event, type, payload) {
    let claimed;
    let fault = "BadRequest";
    try {
      const ref = parseReference(reference);
      if (ref.error) return this.onError({ code: ref.error });
      if (el.closest("[data-ewe-unavailable]")) return this.onError({ code: "UnavailableAction" });
      const registry = this.deps.registry;
      const entry = registry.lookup(ref.origin, ref.name, "web");
      const scope = this.deps.scope ?? globalThis;
      const webHost = entry.origin === "host" && entry.scope === "web";
      const globalFn = webHost && Object.hasOwn(scope, ref.name) && scope[ref.name];
      const page = entry.fn ?? (typeof globalFn === "function" ? globalFn : undefined);
      const choice = route(ref, "web", registry.lookup.bind(registry), {
        host: page ? true : !entry.page && !!this.deps.local, server: !!this.deps.server,
      }, name => this.#advertised?.has(name));
      if (choice.error) return this.onError({ code: choice.error });
      if (payload !== undefined && choice.route === "server" && !this.#structured) {
        return this.onError({ code: "UnsupportedAction", detail: "The peer did not negotiate structured input." });
      }
      const root = el.closest("[data-ewe-root]") ?? el;
      if (this.#busy.has(root)) return this.onError({ code: "Busy" });
      claimed = root;
      this.#busy.add(root);
      if (choice.route === "host" && page) {
        fault = "HostThrew";
        const result = await page.call(el, { node: el, parent: el.parentElement, event, data: payload === undefined ? buildEventData(type, event, el) : payload });
        for (const bytes of (Array.isArray(result) ? result : [result])) if (bytes instanceof Uint8Array) await this.receive(bytes);
        return;
      }
      const state = stateBytes(el.getAttribute("primal::state"), this.limit);
      if (this.#next > 0xffffffffffffffffn) throw new Error("Request sequence exhausted");
      let json = null;
      if (payload !== undefined) {
        fault = "InvalidInput";
        const encoded = JSON.stringify(payload);
        if (encoded === undefined) throw new Error("The payload cannot be encoded as JSON.");
        json = new TextEncoder().encode(encoded);
      }
      const bytes = encodeAction(choice.route + ":" + ref.name, el, event, type, state, this.streamId, this.#next, json);
      if (bytes.length > this.limit) throw new Error("Request too large");
      ++this.#next;
      fault = "Transport";
      const transport = choice.route === "host" ? this.deps.local : this.deps.server;
      for (const reply of await transport.send(bytes)) await this.receive(reply);
    } catch (cause) {
      this.onError({ code: fault, detail: String(cause) });
    } finally {
      if (claimed) this.#busy.delete(claimed);
    }
  }
}

export class FetchActionTransport {
  constructor(options) { this.options = options; }

  async send(bytes) {
    const o = this.options;
    const headers = { "content-type": "application/primal-frame", accept: "application/primal-frame", "x-ewe-action": "1" };
    if (o.hello) headers["x-ewe-hello"] = base64urlEncode(await o.hello());
    if (o.csrfToken) headers["x-csrf-token"] = o.csrfToken;
    for (let attempt = 1; ; attempt++) {
      try {
        const response = await (o.fetch ?? globalThis.fetch)(o.url, {
          method: "POST", credentials: "same-origin", headers, body: bytes, signal: o.requestSignal?.(),
        });
        if (response.status !== 200) {
          await response.body?.cancel();
          throw new Error("http " + response.status);
        }
        const limit = o.maxResponseBytes ?? 4 * 1024 * 1024;
        const frames = [];
        let total = 0;
        for await (const frame of readFrames(response.body, limit)) {
          total += frame.byteLength + 4;
          if (total > limit) throw new Error("Action response too large");
          frames.push(frame);
        }
        if (!frames.length) throw new Error("Empty action response");
        return frames;
      } catch (cause) {
        if (!o.retry || !await o.retry(cause, attempt)) throw cause;
      }
    }
  }
}

export class WebSocketActionTransport {
  constructor(socket) { this.socket = socket; }
  async send(bytes) {
    if (this.socket.readyState !== 1) throw new Error("Action socket is closed");
    this.socket.send(bytes);
    return [];
  }
}

export class WasmActionTransport {
  #pending = new Map();
  #next = 1n;
  #closed = false;

  constructor(bridge) { this.bridge = bridge; }

  send(bytes) {
    return new Promise((resolve, reject) => {
      if (this.#closed) throw new Error("Action bridge is closed");
      const call = this.#next++;
      const api = this.bridge.exports;
      const slot = api.create_allocation(BigInt(bytes.length));
      this.#pending.set(call, { resolve, reject });
      let handedOff = false;
      try {
        const ptr = Number(api.allocation_start_pointer(slot));
        new Uint8Array(this.bridge.memory().buffer, ptr, bytes.length).set(bytes);
        const status = api.ewe_ui_action(call, slot);
        handedOff = true;
        if (status !== 0) throw new Error("Action start failed: " + status);
      } catch (cause) {
        this.#pending.delete(call);
        if (!handedOff) api.dispose_allocation(slot);
        reject(cause);
      }
    });
  }

  complete(call, slot) {
    const pending = this.#pending.get(call);
    this.#pending.delete(call);
    if (BigInt.asUintN(64, slot) === 0xffffffffffffffffn) { pending?.reject(new Error("Action task failed")); return; }
    const api = this.bridge.exports;
    try {
      if (pending) {
        const bytes = new Uint8Array(this.bridge.memory().buffer, Number(api.allocation_start_pointer(slot)), Number(api.allocation_length(slot))).slice();
        pending.resolve([bytes]);
      }
    } catch (cause) { pending?.reject(cause); }
    finally { api.dispose_allocation(slot); }
  }

  close() {
    this.#closed = true;
    for (const pending of this.#pending.values()) pending.reject(new Error("Action bridge closed"));
    this.#pending.clear();
    this.bridge.exports.ewe_ui_actions_close?.();
  }
}

const installations = new WeakMap();

export function installActionRuntime(wasmUi, deps) {
  const runtime = wasmUi.Patcher.runtime;
  installations.get(runtime)?.();
  const manager = new WebActionManager(deps);
  const trigger = manager.trigger.bind(manager);
  wasmUi.EventDispatcher.actionHook = trigger;
  const extensions = wasmUi.Hydrator.scopeExtensions;
  const extend = (scope, owner) => {
    scope.action = (name, fn) => deps.registry.registerHost(name, fn, owner);
    scope.behaviour = (name, fn) => deps.behaviours.registerWebBehaviour(name, fn);
  };
  extensions.push(extend);
  installations.set(runtime, () => {
    manager.detachControls?.();
    if (wasmUi.EventDispatcher.actionHook === trigger) wasmUi.EventDispatcher.actionHook = null;
    const i = extensions.indexOf(extend);
    if (i >= 0) extensions.splice(i, 1);
  });
  const doc = deps.document ?? globalThis.document;
  if (doc && !runtime.dispatcher) {
    runtime.dispatcher = new wasmUi.EventDispatcher(id => manager.onError({code: "UnavailableOrigin", detail: "No local callback " + id}),
      {scope: deps.scope});
    wasmUi.initEventRuntime(runtime.dispatcher, doc);
  } else if (doc?.body) {
    runtime.dispatcher.scanAndWire(doc.body);
  }
  return manager;
}
