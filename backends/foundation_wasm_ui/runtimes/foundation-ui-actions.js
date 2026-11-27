// Route browser actions through the negotiated frame protocol.
import * as wire from "./foundation-ui-wire.js";
import { VOCABULARY_VERSION, STRUCTURED_INPUT_VERSION } from "./foundation-ui-vocab.js";
import { base64urlDecode, base64urlEncode, readFrames, sheetSubmitField, installFrameRuntime, readViewResponse } from "./foundation-ui-frames.js";
import * as ui from "./foundation-wasm-ui.js";
import * as behaviours from "./foundation-ui-behaviours.js";

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
  const value = event.type === "click" ? sheetSubmitField(el)?.value ?? el.value : el?.value;
  const input = new wire.ActionInputT(type, json === null ? String(value ?? "") : null, !!el?.checked, event.keyCode ?? 0, mods, id(el?.parentElement), json).pack(b);
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
    if (deps.server?.handle && deps.server instanceof WebSocketActionTransport) {
      let reconnect = false;
      deps.server.onReply = bytes => this.receive(bytes).catch(error => this.onError({code: "Transport", detail: String(error)}));
      deps.server.onDisconnect = detail => {
        reconnect = true;
        this.#advertised = new Set();
        this.#structured = false;
        this.onError({code: "Transport", detail: String(detail)});
      };
      deps.server.onOpen = async () => {
        if (!reconnect) return;
        reconnect = false;
        try {
          for (const reply of await deps.server.send(this.hello())) await this.receive(reply);
          // Apply the capability reply before the next action uses it.
          deps.frames.flush?.();
          await deps.onReconnect?.();
        } catch (error) { this.onError({code: "Transport", detail: String(error)}); }
      };
    }
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
    const hello = new wire.ClientHelloT(VOCABULARY_VERSION, wire.Platform.Web, wire.MountSurface.Web, 0n, true,
      this.deps.server?.requestTag ?? 0n);
    return finishRequest(b, new wire.FrameRequestT(null, hello).pack(b), 0n, 0n);
  }

  applyCaps(caps) {
    caps = caps.unpack?.() ?? caps;
    this.#structured = caps.status === wire.CapsStatus.Ok && caps.vocabularyVersion >= STRUCTURED_INPUT_VERSION;
    this.#advertised = new Set((caps.actions ?? []).map(a => a.name));
    this.setStreamId(caps.streamId);
    if (this.deps.frames?.deps) this.deps.frames.deps.deliverySession =
      caps.subscriptionResume && caps.status === wire.CapsStatus.Ok ? this.streamId : undefined;
    const doc = this.deps.document ?? globalThis.document;
    for (const el of doc?.querySelectorAll("[primal-id]") ?? []) {
      const missing = Array.from(el.attributes).some(attr =>
        attr.name.startsWith("primal:on") && attr.value.startsWith("server:") && !this.#advertised.has(attr.value.slice(7)));
      el.toggleAttribute("data-ewe-unavailable", missing);
    }
  }

  async receive(bytes, viewMount) {
    if (await this.deps.frames.receive(bytes, viewMount ? { viewMount } : undefined) === false) throw new Error("Invalid action reply");
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

  /** Deliver a data instruction to the host action configured by the receiving view. */
  async deliverViewData(reference, el, bytes) {
    return this.deliverHostPayload(reference, el, Array.from(bytes));
  }

  /** Invoke the registered local handler for a configured host callback. */
  async deliverHostPayload(reference, el, payload) {
    if (parseReference(reference).origin !== "host") return this.onError({code:"UnavailableOrigin"});
    return this.#submit(reference, el, {type:"data"}, "data", payload, true);
  }

  async #submit(reference, el, event, type, payload, delivery = false, parent) {
    let claimed;
    let fault = "BadRequest";
    const fail = detail => {
      if (parent) throw Object.assign(new Error(detail.detail ?? detail.code), detail);
      return this.onError(detail);
    };
    try {
      const ref = parseReference(reference);
      if (ref.error) return fail({ code: ref.error });
      if (!parent && el.closest("[data-ewe-unavailable]")) return fail({ code: "UnavailableAction" });
      const registry = this.deps.registry;
      const entry = registry.lookup(ref.origin, ref.name, "web");
      const scope = this.deps.scope ?? globalThis;
      const webHost = entry.origin === "host" && entry.scope === "web";
      const globalFn = webHost && Object.hasOwn(scope, ref.name) && scope[ref.name];
      const page = entry.fn ?? (typeof globalFn === "function" ? globalFn : undefined);
      const choice = route(ref, "web", registry.lookup.bind(registry), {
        host: page ? true : !entry.page && !!this.deps.local, server: !!this.deps.server,
      }, name => this.#advertised?.has(name));
      if (choice.error) return fail({ code: choice.error });
      if (payload !== undefined && choice.route === "server" && !this.#structured) {
        return fail({ code: "UnsupportedAction", detail: "The peer did not negotiate structured input." });
      }
      const viewMount = parent ? parent.viewMount : this.deps.frames?.receivingView?.(el);
      const callAction = async (name, value, preview) => {
        if (value === undefined) throw Object.assign(new Error("Missing child input"), {code:"InvalidInput"});
        if (preview?.length) await this.receive(preview, viewMount);
        return this.#submit(name, viewMount ?? el, {}, "", value, true, {viewMount});
      };
      const root = el?.closest("[data-ewe-root]") ?? el;
      if (!delivery) {
        if (this.#busy.has(root)) return fail({ code: "Busy" });
        claimed = root;
        this.#busy.add(root);
      }
      if (choice.route === "host" && page) {
        fault = "HostThrew";
        const result = await page.call(el, { node: el, parent: el?.parentElement, event, callAction, data: payload === undefined ? ui.buildEventData(type, event, el) : payload });
        for (const bytes of (Array.isArray(result) ? result : [result])) if (bytes instanceof Uint8Array) await this.receive(bytes, viewMount);
        return;
      }
      const state = parent ? null : stateBytes(el.getAttribute("primal::state"), this.limit);
      if (this.#next > 0xffffffffffffffffn) throw new Error("Request sequence exhausted");
      let json = null;
      if (payload !== undefined) {
        fault = "InvalidInput";
        const encoded = JSON.stringify(payload);
        if (encoded === undefined) throw new Error("The payload cannot be encoded as JSON.");
        json = new TextEncoder().encode(encoded);
      }
      const bytes = encodeAction(choice.route + ":" + ref.name, parent ? null : el, event, type, state, this.streamId, this.#next, json);
      if (bytes.length > this.limit) throw new Error("Request too large");
      ++this.#next;
      fault = "Transport";
      const transport = choice.route === "host" ? this.deps.local : this.deps.server;
      let replies;
      if (choice.route === "host") replies = await transport.send(bytes, callAction);
      else if (parent && transport.sendChild) replies = await transport.sendChild(bytes);
      else replies = await transport.send(bytes);
      for (const reply of replies) {
        await this.receive(reply, viewMount);
        if (parent) checkChildReply(reply);
      }
    } catch (cause) {
      if (parent) throw Object.assign(new Error(cause.message ?? String(cause)), {code: cause.code ?? fault});
      this.onError({ code: fault, detail: String(cause) });
    } finally {
      if (claimed) this.#busy.delete(claimed);
    }
  }
}

function checkChildReply(bytes) {
  const frame = wire.Frame.getRootAsFrame(new wire.ByteBuffer(bytes));
  const body = frame.kind(new wire.FrameMessage());
  for (let i = 0; i < body.messagesLength(); i++) {
    const control = body.messages(i)?.control();
    const error = control?.error();
    const code = error ? wire.ErrorCode[error.code()] : control?.stateExpired() ? "StateExpired" :
      control?.updateRequired() ? "UpdateRequired" : null;
    if (code) throw Object.assign(new Error(error?.message() || code), {code});
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
  #children = new Map();
  constructor(socket) { this.socket = socket; }

  static connect(connections, endpoint, options = {}) {
    const sender = new WebSocketActionTransport(null);
    sender.requestTag = connections.nextTag();
    sender.timeout = options.handshakeTimeoutMs ?? 60000;
    sender.replyTimeout = options.replyTimeoutMs ?? sender.timeout;
    sender.handle = connections.acquire({ ...options, endpoint, transport: "websocket" }, {
      accept: bytes => {
        const frame = wire.Frame.getRootAsFrame(new wire.ByteBuffer(bytes));
        const body = frame.kindType() === wire.FrameKind.FrameMessage && frame.kind(new wire.FrameMessage());
        if (!body) return false;
        const caps = body.messagesLength() && body.messages(0)?.control()?.caps();
        return caps ? caps.requestTag() === sender.requestTag : body.ack() !== 0n && frame.streamId() === sender.session;
      },
      receive: bytes => {
        const frame = wire.Frame.getRootAsFrame(new wire.ByteBuffer(bytes));
        const body = frame.kind(new wire.FrameMessage());
        const caps = body.messagesLength() && body.messages(0)?.control()?.caps();
        if (caps) {
          sender.session = caps.status() === wire.CapsStatus.Ok ? caps.streamId() : null;
          sender.pending?.resolve([bytes]);
        } else {
          const child = sender.#children.get(body.ack());
          if (child) child.resolve([bytes]);
          else sender.onReply?.(bytes);
        }
      },
      onError: cause => {
        sender.session = null;
        sender.pending?.reject(cause);
        sender.#failChildren(cause);
        sender.onDisconnect?.(cause);
      },
      onOpen: () => sender.onOpen?.(),
    });
    return sender;
  }

  async send(bytes) {
    if (!this.handle) {
      if (this.socket.readyState !== 1) throw new Error("Action socket is closed");
      this.socket.send(bytes);
      return [];
    }
    const frame = wire.Frame.getRootAsFrame(new wire.ByteBuffer(bytes));
    const request = frame.kind(new wire.FrameRequest());
    if (!request?.hello()) {
      if (!this.session || frame.streamId() !== this.session) throw new Error("Action connection needs a handshake");
      await this.handle.send(bytes);
      return [];
    }
    if (this.pending) throw new Error("A capability request is already pending");
    let timer;
    try {
      return await new Promise((resolve, reject) => {
        this.pending = {resolve, reject};
        timer = setTimeout(() => reject(new Error("Capability request timed out")), this.timeout);
        this.handle.send(bytes).catch(reject);
      });
    } finally { clearTimeout(timer); this.pending = null; }
  }

  /** Keep the reply with the view that submitted the child request. */
  async sendChild(bytes) {
    if (!this.handle) return this.send(bytes);
    const frame = wire.Frame.getRootAsFrame(new wire.ByteBuffer(bytes));
    const id = frame.sequenceId();
    let timer;
    try {
      return await new Promise((resolve, reject) => {
        this.#children.set(id, {resolve, reject});
        timer = setTimeout(() => reject(new Error("Action reply timed out")), this.replyTimeout);
        this.send(bytes).catch(reject);
      });
    } finally { clearTimeout(timer); this.#children.delete(id); }
  }

  #failChildren(cause) {
    for (const child of this.#children.values()) child.reject(cause);
    this.#children.clear();
  }

  close() {
    this.#failChildren(new Error("Action socket closed"));
    this.pending?.reject(new Error("Action socket closed"));
    this.handle?.close();
  }
}

export class WasmActionTransport {
  #pending = new Map();
  #next = 1n;
  #closed = false;

  constructor(bridge) { this.bridge = bridge; }

  /** Load a browser action module and run its exported host initializer. */
  static async boot(loader, url, {registry, start, startArgs = [], imports = {}, bindings, storage, window = globalThis.window}) {
    const dom = new ui.DomHeap({window, document: window.document});
    const decoder = new TextDecoder("utf-8", {fatal: true});
    let local;
    const previous = loader.environment;
    // A factory scopes memory views and finalizers to this instance.
    const glue = bindings?.createBindings();
    loader.environment = {
      ...previous,
      ...(glue && {[bindings.importName]: glue}),
      get abi() {
        const runtime = loader.runtime;
        return {...runtime.web_abi, ...ui.domAbi(runtime, dom), ...previous.abi};
      },
      env: {
        ...previous.env, ...imports, ...WasmActionTransport.imports(() => local),
        ewe_ui_declare(origin, pointer, length, scope) {
          const name = decoder.decode(new Uint8Array(loader.runtime.bridge.memory.buffer, pointer, length));
          registry.declare([null, "host", "server"][origin], name, ["all", "web", "mobile"][scope]);
        },
      },
    };
    try {
      await loader.loadURL(url);
      const runtime = loader.runtime;
      const api = loader.module.instance.exports;
      if (glue) {
        glue.__wbg_set_wasm(api);
        api.__wbindgen_start?.();
      }
      if (storage && !glue?.configureHostStorage) throw new Error("Missing browser storage bindings");
      glue?.configureHostStorage?.(storage?.provider ?? "memory", storage?.namespace ?? "");
      if (typeof api[start] !== "function") throw new Error("Missing host initializer: " + start);
      local = new WasmActionTransport({exports: api, memory: () => runtime.bridge.memory});
      const status = api[start](...startArgs);
      if (status !== 0) throw new Error("Host initializer " + start + " returned " + status);
      await glue?.prepareBrowserHost?.();
      return local;
    } catch (error) {
      try { local?.close(); } finally { loader.runtime.dispose(); }
      throw error;
    } finally {
      loader.environment = previous;
    }
  }

  /** Install these imports before the module starts. */
  static imports(transport) {
    return {
      ewe_ui_action_done: (call, slot) => transport().complete(call, slot),
      ewe_ui_child: (...args) => transport().#startChild(...args),
    };
  }

  #startChild(parent, child, namePtr, nameLen, jsonPtr, jsonLen, previewPtr, previewLen) {
    const api = this.bridge.exports;
    const memory = this.bridge.memory().buffer;
    const read = (ptr, len) => new Uint8Array(memory, ptr, len);
    const decoder = new TextDecoder("utf-8", { fatal: true });
    const run = async () => {
      const callAction = this.#pending.get(parent)?.callAction;
      if (!callAction) throw Object.assign(new Error("The parent action closed"), {code:"UnavailableOrigin"});
      const name = decoder.decode(read(namePtr, nameLen));
      const value = JSON.parse(decoder.decode(read(jsonPtr, jsonLen)));
      await callAction(name, value, read(previewPtr, previewLen).slice());
    };
    run().then(() => {
      if (!this.#closed) api.ewe_ui_child_done(child, -1, 0n);
    }, cause => {
      if (this.#closed) return;
      let slot = -1n;
      try {
        const bytes = new TextEncoder().encode(cause?.message ?? String(cause));
        slot = api.create_allocation(BigInt(bytes.length));
        const pointer = Number(api.allocation_start_pointer(slot));
        new Uint8Array(this.bridge.memory().buffer, pointer, bytes.length).set(bytes);
      } catch {
        if (slot !== -1n) api.dispose_allocation(slot);
        slot = -1n;
      }
      api.ewe_ui_child_done(child, wire.ErrorCode[cause?.code] ?? wire.ErrorCode.ActionFailed, slot);
    });
  }

  send(bytes, callAction) {
    return new Promise((resolve, reject) => {
      if (this.#closed) throw new Error("Action bridge is closed");
      const call = this.#next++;
      const api = this.bridge.exports;
      const slot = api.create_allocation(BigInt(bytes.length));
      this.#pending.set(call, { resolve, reject, callAction });
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
  manager.dispose = () => {
    if (installations.get(runtime) !== manager.dispose) return;
    installations.delete(runtime);
    manager.detachControls?.();
    if (wasmUi.EventDispatcher.actionHook === trigger) wasmUi.EventDispatcher.actionHook = null;
    const i = extensions.indexOf(extend);
    if (i >= 0) extensions.splice(i, 1);
  };
  installations.set(runtime, manager.dispose);
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


const activeApps = new WeakSet();

/** Start the configured view once. A failed start releases its resources and permits retry. */
export function createApp(options) {
  let flight;
  const cleanup = [];
  function dispose() {
    const errors = [];
    while (cleanup.length) {
      try { cleanup.pop()(); } catch (error) { errors.push(error); }
    }
    flight = undefined;
    if (errors.length) throw new AggregateError(errors, "Browser app cleanup failed");
  }
  async function start() {
    const {mount, moduleLoader, onError = console.error} = options;
    const doc = mount.ownerDocument;
    const registry = new JsActionRegistry();
    const local = await WasmActionTransport.boot(moduleLoader, options.moduleUrl, {
      registry, start: options.start, startArgs: options.startArgs, imports: options.imports,
      window: doc.defaultView,
      bindings: options.bindings, storage: options.storage,
    });
    cleanup.push(() => moduleLoader.runtime.dispose(), () => local.close());
    const runtime = ui.Patcher.runtime;
    const connections = ui.frameConnections();
    cleanup.push(() => connections.close());
    const previous = {
      dispatcher: runtime.dispatcher, frames: runtime.frames, connections: runtime.connections,
      onMountConnect: runtime.onMountConnect,
    };
    const previousHook = ui.ProtocolHandler.frameHook;
    cleanup.push(() => {
      Object.assign(runtime, previous);
      ui.ProtocolHandler.frameHook = previousHook;
    });
    runtime.frames = null;
    runtime.dispatcher = new ui.EventDispatcher(() => {});
    const frames = installFrameRuntime(ui, {document: doc, connections, onReport: onError,
      viewMount: mount, resolveLiveUrl: options.resolveLiveUrl});
    ui.registerWebComponents();
    cleanup.push(frames.registerView(options.view, mount));
    let manager;
    const server = typeof options.transport === "function" ? await options.transport()
      : options.transport?.socket ? WebSocketActionTransport.connect(connections, options.transport.socket)
      : new FetchActionTransport({url: options.transport?.http ?? "/action", hello: () => manager.hello()});
    cleanup.push(() => server.close?.());
    manager = installActionRuntime(ui, {registry, frames, server, local, behaviours, document: doc, onError,
      onReconnect: options.onReconnect});
    cleanup.push(manager.dispose);
    let initialDeliveries = [];
    if (options.viewDataAction) {
      const receive = event => {
        const delivery = manager.deliverViewData(options.viewDataAction, event.target, event.detail.bytes);
        if (initialDeliveries) initialDeliveries.push(delivery);
        else delivery.catch(onError);
      };
      mount.addEventListener("ewe:data", receive);
      cleanup.push(() => mount.removeEventListener("ewe:data", receive));
    }
    for (const bytes of await server.send(manager.hello())) await manager.receive(bytes);
    const view = frames.registerLoader(mount, async signal => {
      const response = await fetch(options.initialRoute, {
        signal, headers: {"X-Ewe-Hello": base64urlEncode(manager.hello())},
      });
      return readViewResponse(response, frames.maxFrameBytes);
    }, {onError: error => onError({code: "ViewRecovery", detail: error.message})});
    await view.open();
    const deliveries = initialDeliveries;
    initialDeliveries = null;
    await Promise.all(deliveries);
    frames.flush();
    return manager;
  }
  return {
    boot() {
      if (!flight && activeApps.has(ui.Patcher.runtime)) {
        return Promise.reject(new Error("This browser action runtime already has an application"));
      }
      if (!flight) {
        activeApps.add(ui.Patcher.runtime);
        cleanup.push(() => activeApps.delete(ui.Patcher.runtime));
        flight = start().catch(error => { dispose(); throw error; });
      }
      return flight;
    },
    async close() {
      const pending = flight;
      if (pending) {
        try { await pending; } finally { if (flight === pending) dispose(); }
      }
    },
  };
}
