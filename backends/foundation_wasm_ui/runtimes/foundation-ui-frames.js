// Apply EWF1 message frames to the browser node registry.
import { ViewRecovery } from "./foundation-ui-recovery.js";
import { bindLiveMounts } from "./foundation-ui-live.js";
import * as wire from "./foundation-ui-wire.js";
import * as vocab from "./foundation-ui-vocab.js";
import * as registryBehaviours from "./foundation-ui-behaviours.js";
import { NodeRegistry, Patcher } from "./foundation-wasm-ui.js";

const asBytes = input => ArrayBuffer.isView(input)
  ? new Uint8Array(input.buffer, input.byteOffset, input.byteLength)
  : new Uint8Array(input);
const utf8 = new TextDecoder("utf-8", { fatal: true });
const defaultFrameBytes = 4 * 1024 * 1024;

export function isFrame(input) {
  const b = asBytes(input);
  return b.length >= 8 && b[4] === 69 && b[5] === 87 && b[6] === 70 && b[7] === 49;
}

// Read length-prefixed frames without retaining consumed transport chunks.
class FrameSplitter {
  constructor(limit) {
    this.limit = limit;
    this.prefix = new Uint8Array(4);
    this.used = 0;
    this.frame = null;
  }
  *push(input) {
    const bytes = asBytes(input);
    let cursor = 0;
    while (cursor < bytes.length) {
      const dest = this.frame ?? this.prefix;
      const count = Math.min(dest.length - this.used, bytes.length - cursor);
      dest.set(bytes.subarray(cursor, cursor + count), this.used);
      this.used += count;
      cursor += count;
      if (this.used !== dest.length) continue;
      this.used = 0;
      if (this.frame) {
        this.frame = null;
        if (!isFrame(dest)) throw new Error("Invalid EWF1 identifier");
        yield dest;
      } else {
        const size = new DataView(dest.buffer).getUint32(0, true);
        if (size < 8 || size > this.limit) throw new RangeError("Invalid frame size");
        this.frame = new Uint8Array(size);
      }
    }
  }
  finish() {
    if (this.frame || this.used) throw new Error("Truncated frame");
  }
}

export function splitFrames(input, limit = defaultFrameBytes) {
  const splitter = new FrameSplitter(limit);
  const frames = Array.from(splitter.push(input));
  splitter.finish();
  return frames;
}

export async function* readFrames(body, limit = defaultFrameBytes) {
  const splitter = new FrameSplitter(limit);
  if (!body) throw new Error("Missing frame response body");
  const reader = body.getReader();
  let done = false;
  try {
    while (true) {
      const chunk = await reader.read();
      if (chunk.done) { splitter.finish(); done = true; break; }
      yield* splitter.push(chunk.value);
    }
  } finally {
    try { if (!done) await reader.cancel(); } finally { reader.releaseLock(); }
  }
}

// Raw initial responses use the HTTP message boundary. Action streams carry a length prefix.
export async function readViewResponse(response, limit = defaultFrameBytes) {
  if (!response.ok) throw new Error("View request failed with HTTP " + response.status);
  if (!response.body) throw new Error("Missing view response body");
  const reader = response.body.getReader();
  const chunks = [];
  let size = 0, complete = false;
  try {
    while (true) {
      const {done, value} = await reader.read();
      if (done) { complete = true; break; }
      size += value.byteLength;
      if (size > limit) throw new RangeError("View exceeds frame limit");
      chunks.push(value);
    }
  } finally {
    try { if (!complete) await reader.cancel(); } finally { reader.releaseLock(); }
  }
  const frame = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) { frame.set(chunk, offset); offset += chunk.byteLength; }
  if (!isFrame(frame)) throw new Error("Invalid view frame");
  return frame;
}

export function base64urlEncode(bytes) {
  let binary = "";
  for (const value of bytes) binary += String.fromCharCode(value);
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
}

export function base64urlDecode(text) {
  return Uint8Array.from(atob(text.replaceAll("-", "+").replaceAll("_", "/")), c => c.charCodeAt(0));
}

// Generated readers normally trust their buffer. Bound vector lengths before walking them.
class CheckedBuffer extends wire.ByteBuffer {
  check(pos, length) {
    if (pos < 0 || length < 0 || pos + length > this.bytes().length) throw new Error("Truncated frame field");
  }
  readInt8(pos) { this.check(pos, 1); return super.readInt8(pos); }
  readUint8(pos) { this.check(pos, 1); return super.readUint8(pos); }
  readInt16(pos) { this.check(pos, 2); return super.readInt16(pos); }
  readUint16(pos) { this.check(pos, 2); return super.readUint16(pos); }
  readInt32(pos) { this.check(pos, 4); return super.readInt32(pos); }
  readUint32(pos) { this.check(pos, 4); return super.readUint32(pos); }
  __vector_len(pos) {
    const len = super.__vector_len(pos);
    this.check(super.__vector(pos), len);
    return len;
  }
  __string(pos, encoding) {
    const start = pos + this.readInt32(pos);
    this.check(start + 4, this.readInt32(start));
    return super.__string(pos, encoding);
  }
}

function vector(table, field, vtableOffset, Type, stats) {
  const count = table[field + "Length"]();
  const bb = table.bb;
  const offset = bb.__offset(table.bb_pos, vtableOffset);
  if (!offset) return new Type();
  const start = bb.__vector(table.bb_pos + offset);
  bb.check(start, count * Type.BYTES_PER_ELEMENT);
  if ((bb.bytes().byteOffset + start) % Type.BYTES_PER_ELEMENT === 0) {
    if (stats) stats.zeroCopyColumns++;
    return table[field + "Array"]?.() ?? new Type(bb.bytes().buffer, bb.bytes().byteOffset + start, count);
  }
  if (stats) stats.copiedColumns++;
  return Type.from({ length: count }, (_, i) => table[field](i));
}

function problem(code, fields = {}) {
  return Object.assign(new Error(code), { report: { code, ...fields } });
}

const utilityClasses = new WeakMap();
function applyClasses(node, classes) {
  const previous = utilityClasses.get(node) ?? [];
  for (const cls of previous) node.classList.remove(cls);
  const words = (classes ?? "").split(/\s+/).filter(Boolean);
  const shared = words.filter(word => !/^(ios|android|web)-/.test(word));
  const overrides = words.filter(word => word.startsWith("web-")).map(word => word.slice(4));
  const ordered = [...shared, ...overrides];
  const applied = [];
  for (const word of ordered) {
    if (!node.classList.contains(word)) {
      node.classList.add(word);
      applied.push(word);
    }
  }
  node.setAttribute("data-ewe-themed", "");
  for (const [prefix, categories, field] of [
    ["text-", ["color"], "color"], ["text-", ["font-size"], "font-size"],
    ["bg-", ["color"], "background"], ["p-", ["padding", "spacing"], "padding"],
  ]) {
    let value = "";
    for (const word of ordered) {
      const name = word.slice(prefix.length);
      if (!word.startsWith(prefix) || !/^[a-zA-Z0-9_-]+$/.test(name)) continue;
      for (const category of categories) value = "var(--" + category + "-" + name + (value ? "," + value : "") + ")";
    }
    const property = "--ewe-utility-" + field;
    if (value) node.style.setProperty?.(property, value);
    else node.style.removeProperty?.(property);
  }
  utilityClasses.set(node, applied);
}

const propStyles = { PADDING: "padding", SPACING: "gap", WIDTH: "width", HEIGHT: "height" };
function applyProp(node, id, type, value) {
  if (!value || wire.PropValue[type] !== vocab.PROP_KIND[id]) throw problem("BadFrame");
  const prop = wire.PropId[id];
  const v = value.v;
  if (Object.hasOwn(propStyles, prop)) {
    node.style[propStyles[prop]] = v.unit === wire.LengthUnit.Auto ? "auto" :
      String(v.value) + (v.unit === wire.LengthUnit.Percent ? "%" : "px");
    return;
  }
  switch (prop) {
    case "ID": node.setAttribute("id", v); break;
    case "ALIGN": node.style.alignItems = ["flex-start", "center", "flex-end", "stretch"][v]; break;
    case "AXIS": node.style.flexDirection = v === wire.Axis.Horizontal ? "row" : "column"; break;
    case "FONT": node.style.font = "var(--ewe-font-" + value.id + ")"; break;
    case "COLOR": case "BACKGROUND":
      node.style[prop === "COLOR" ? "color" : "backgroundColor"] = "var(--ewe-color-" + value.id + ")"; break;
    case "ENABLED": case "HIDDEN": {
      const attr = prop === "HIDDEN" ? "hidden" : "disabled";
      if (prop === "HIDDEN" ? v : !v) node.setAttribute(attr, "");
      else node.removeAttribute(attr);
      break;
    }
    case "UTILITY_CLASSES": applyClasses(node, v); break;
    case "A11Y_LABEL": node.setAttribute("aria-label", v ?? ""); break;
    case "SOURCE": node.setAttribute("src", v ?? ""); break;
    case "PLACEHOLDER": node.setAttribute("placeholder", v ?? ""); break;
    case "VALUE": node.value = v ?? ""; break;
    case "CHECKED":
      node.checked = v;
      if (node.tagName === "BUTTON") node.setAttribute("aria-pressed", String(v));
      break;
    default: throw problem("UnknownOp");
  }
}

function dispatchEvent(target, name, detail) {
  const EventType = target.ownerDocument?.defaultView?.CustomEvent ?? globalThis.CustomEvent;
  target.dispatchEvent?.(EventType ? new EventType(name, { bubbles: true, detail }) : { type: name, detail, bubbles: true });
}

// A submit button can use the value of one enabled sibling field.
export function sheetSubmitField(button) {
  if (!button.classList?.contains("ewe-submit-button") || !button.closest?.(".ewe-bottom-sheet")) return null;
  const fields = Array.from(button.parentElement.children).filter(node =>
    node.tagName === "INPUT" && !node.disabled && !node.hidden && node.hasAttribute("primal:onsubmit"));
  return fields.length === 1 ? fields[0] : null;
}

// A sheet forwards a submit gesture to the input that owns the action.
function wireBottomSheet(sheet, options) {
  function submit(input) {
    if (input && !input.disabled) {
      const EventType = sheet.ownerDocument.defaultView.Event;
      input.dispatchEvent(new EventType("submit", { cancelable: true }));
    }
  }
  sheet.addEventListener("keydown", event => {
    const input = event.target;
    if (event.key !== "Enter" || event.isComposing || input.closest(".ewe-bottom-sheet") !== sheet) return;
    if (input.tagName !== "INPUT" || !input.hasAttribute("primal:onsubmit")) return;
    event.preventDefault();
    if (!event.repeat) submit(input);
  }, options);
  sheet.addEventListener("click", event => {
    const button = event.target.closest?.('button.ewe-submit-button');
    if (!button || button.closest(".ewe-bottom-sheet") !== sheet || button.hasAttribute("primal:onclick")) return;
    submit(sheetSubmitField(button));
  }, options);
}

export class FrameApplicator {
  constructor(deps) {
    this.deps = deps;
    this.document = deps.document;
    this.registry = deps.registry;
    this.elementListeners = new WeakMap();
    this.bindings = [];
    this.layers = new Set();
    this.containerEvents = new WeakMap();
    this.maxFrameBytes = deps.maxFrameBytes ?? defaultFrameBytes;
    this.gapTimeoutMs = deps.gapTimeoutMs ?? 1000;
    this.maxPendingFrames = deps.maxPendingFrames ?? 128;
    this.timer = deps.timer ?? { set: setTimeout, clear: clearTimeout };
    this.schedule = deps.schedule ?? (run => {
      if (this.document.visibilityState === "hidden" || !globalThis.requestAnimationFrame) queueMicrotask(run);
      else globalThis.requestAnimationFrame(run);
    });
    this.stats = { frames: 0, rows: 0, reports: 0, copiedColumns: 0, zeroCopyColumns: 0 };
    this.defaultView = { registry: deps.registry, cursors: new Map(), gaps: new Map() };
    this.viewStates = new Map();
    this.loaders = new Map();
    this.queue = [];
    this.scheduled = false;
    this.cssInstalled = false;
    this.nodeIds = new WeakMap();
    this.origins = new WeakMap();
    this.views = new Map();
    this.sharedKinds = new WeakMap();
    this.drawingTargets = [];
    this.drawingJobs = new Map();
  }
  bindDrawing(canvas, nodeId) {
    const graphics = this.deps.graphics;
    if (!graphics || !canvas.isConnected) return;
    const name = canvas.id || String(nodeId);
    const host = this.document.defaultView;
    let surface, removed = false, observer;
    const enqueue = work => {
      const previous = this.drawingJobs.get(name) ?? Promise.resolve();
      const job = previous.then(work).catch(error => this.report("Graphics", {node:nodeId, detail:error.message}));
      this.drawingJobs.set(name, job);
      job.then(() => { if (this.drawingJobs.get(name) === job) this.drawingJobs.delete(name); });
    };
    const resize = () => {
      if (removed) return;
      const rect = canvas.getBoundingClientRect();
      const scale = host.devicePixelRatio || 1;
      const width = Math.round(rect.width * scale), height = Math.round(rect.height * scale);
      if (canvas.width === width && canvas.height === height && surface) return;
      if (canvas.width !== width) canvas.width = width;
      if (canvas.height !== height) canvas.height = height;
      enqueue(async () => {
        if (removed) return;
        if (surface) await surface.resize(width, height);
        else surface = await graphics.attach(name, canvas);
      });
    };
    resize();
    if (host.ResizeObserver) {
      observer = new host.ResizeObserver(resize);
      observer.observe(canvas);
    }
    this.registerUnmount(canvas, () => {
      removed = true;
      observer?.disconnect();
      enqueue(async () => { if (surface) await surface.remove(); });
    });
  }
  viewOwner(mount) {
    for (let node = mount; node; node = node.parentNode) {
      if (this.viewStates.has(node)) return node;
    }
    return this.deps.viewMount ?? this;
  }
  viewState(mount) {
    return this.viewStates.get(this.viewOwner(mount)) ?? this.defaultView;
  }
  cursor(stream, mount) { return this.viewState(mount).cursors.get(stream); }
  setCursor(stream, sequence, mount) {
    const {gaps, cursors} = this.viewState(mount);
    const gap = gaps.get(stream);
    if (gap) this.timer.clear(gap.timer);
    gaps.delete(stream);
    cursors.set(stream, sequence);
  }
  registerUnmount(node, hook) { Patcher.registerUnmount(node, hook); }
  report(code, fields = {}) {
    this.stats.reports++;
    const report = { code, ...fields };
    if (this.deps.onReport) this.deps.onReport(report);
    else console.error("Frame operation failed", report);
  }
  receive(input, origin) {
    try {
      const bytes = asBytes(input);
      if (bytes.length > this.maxFrameBytes) { this.report("TooLarge"); return false; }
      if (!isFrame(bytes)) throw problem("BadFrame");
      const frame = wire.Frame.getRootAsFrame(new CheckedBuffer(bytes));
      if (frame.kindType() !== wire.FrameKind.FrameMessage) throw problem("BadFrame");
      const msg = frame.kind(new wire.FrameMessage());
      if (!msg) throw problem("BadFrame");
      const count = msg.messagesLength();
      if (count) {
        const bb = msg.bb;
        bb.check(bb.__vector(msg.bb_pos + bb.__offset(msg.bb_pos, 6)), count * 4);
      }
      const seq = frame.sequenceId();
      const stream = frame.streamId();
      if (origin) {
        this.origins.set(msg, origin);
        origin.observeStream?.(stream);
      }
      if (seq === 0n || msg.ack() !== 0n) this.enqueue(msg);
      else this.order(stream, seq, msg);
      return true;
    } catch (error) { this.report("BadFrame", { detail: error.message }); return false; }
  }
  order(stream, seq, msg) {
    const owner = this.viewOwner(this.origins.get(msg)?.viewMount);
    const {cursors, gaps} = this.viewState(owner);
    const cursor = cursors.get(stream);
    if (cursor !== undefined && seq <= cursor) return;
    let gap = gaps.get(stream);
    if (cursor !== undefined && seq > cursor + 1n) {
      if (!gap) {
        gap = { held: new Map(), timer: this.timer.set(() => {
          gaps.delete(stream);
          this.deps.onGap?.({ streamId: stream, ...(this.viewStates.has(owner) ? {view: owner} : {}), resumeFrom: cursors.get(stream) });
        }, this.gapTimeoutMs) };
        gaps.set(stream, gap);
      }
      if (!gap.held.has(seq) && gap.held.size >= this.maxPendingFrames) {
        this.timer.clear(gap.timer);
        gaps.delete(stream);
        this.deps.onGap?.({ streamId: stream, ...(this.viewStates.has(owner) ? {view: owner} : {}), resumeFrom: cursor });
      } else gap.held.set(seq, msg);
      return;
    }
    this.enqueue(msg);
    while (gap?.held.has(seq + 1n)) {
      seq++;
      this.enqueue(gap.held.get(seq));
      gap.held.delete(seq);
    }
    cursors.set(stream, seq);
    if (gap && !gap.held.size) { this.timer.clear(gap.timer); gaps.delete(stream); }
  }
  enqueue(msg) {
    this.queue.push(msg);
    if (!this.scheduled) {
      this.scheduled = true;
      this.schedule(() => this.flush());
    }
  }
  flush() {
    this.scheduled = false;
    for (const msg of this.queue.splice(0)) {
      const previousOrigin = this.origin;
      this.origin = this.origins.get(msg);
      if (this.origin?.active && !this.origin.active()) { this.origin = previousOrigin; continue; }
      const previousRegistry = this.registry;
      this.registry = this.viewState(this.origin?.viewMount).registry;
      this.stats.frames++;
      for (let i = 0; i < msg.messagesLength(); i++) {
        try {
          const m = msg.messages(i);
          if (m.operation()) this.applyUnit(m.operation(), i);
          if (m.data()) this.applyData(m.data());
          if (m.control()) this.applyControl(m.control());
        } catch (e) { this.report(e.report?.code ?? "BadFrame", { unitIndex: i, ...(e.report ?? {}), detail: e.message }); }
      }
      for (const {node, id, index, name, args, shared, registry: nodeRegistry} of this.bindings.splice(0)) {
        if (nodeRegistry.get(id) !== node) continue;
        const registry = this.deps.behaviours ?? registryBehaviours;
        try {
          const applied = shared ? registry.applyBehaviour(node, name, args) : registry.applyWebBehaviour(node, name, args);
          if (!applied) this.report("UnknownBehaviour", {unitIndex:index,node:id,name:String(name)});
        } catch (error) { this.report("BadFrame",{unitIndex:index,node:id,detail:error.message}); }
      }
      for (const layer of this.layers) {
        if (!layer.isConnected) continue;
        const hidden = layer.closest("[hidden]") !== null;
        if (layer.tagName === "DIALOG") {
          if (hidden && layer.open) layer.close();
          else if (!hidden && !layer.open) layer.showModal();
        } else {
          const open = layer.matches(":popover-open");
          if (hidden && open) layer.hidePopover();
          else if (!hidden && !open) layer.showPopover();
        }
      }
      for (const [canvas, nodeId] of this.drawingTargets.splice(0)) this.bindDrawing(canvas, nodeId);
      this.registry = previousRegistry;
      this.origin = previousOrigin;
      this.deps.onApplied?.();
    }
  }
  node(id, staged = false) {
    const n = this.registry.get(id);
    if (n) return n;
    if (staged && this.registry.pending.has(id)) return this.registry.expectOrStaged(id);
    throw problem("UnknownNode", { node: id });
  }
  applyUnit(op, index) {
    const read = (key, enumType, handler) => {
      const id = op[key + "Type"]();
      if (!id) return false;
      const name = enumType[id];
      if (!wire[name]) throw problem("UnknownOp", { field: key });
      const reader = op[key](new wire[name]());
      if (!reader) throw problem("BadFrame");
      handler.call(this, name, reader, index);
      this.stats.rows++;
      return true;
    };
    const ui = read("ui", wire.UiOperation, this.applyUiReader);
    const web = read("web", wire.WebOperation, this.applyWebReader);
    if (op.columnar()) this.applyColumnar(op.columnar(), index);
    if (op.mobileType()) this.report("UnsupportedPlatformOp", { unitIndex: index, field: "mobile" });
    else if (!ui && !web && !op.columnar()) this.report("UnknownOp", { unitIndex: index });
  }
  applyUiReader(name, reader, index) {
    if (name === "ReplaceSubtree") {
      const root = this.node(reader.nodeId());
      for (const child of Array.from(root.childNodes)) { this.discard(child); root.removeChild(child); }
      this.applyNested(reader, index);
    } else if (name === "Place") this.place(reader, index);
    else this.applyUi(name, reader.unpack(), index);
  }
  applyNested(reader, index) {
    for (let i = 0; i < reader.operationsLength(); i++) this.applyUnit(reader.operations(i), index);
  }
  discard(node) {
    if (this.layers.delete(node)) {
      if (node.tagName === "DIALOG" && node.open) node.close();
      else if (node.hasAttribute("popover") && node.matches(":popover-open")) node.hidePopover();
    }
    this.containerEvents.get(node)?.abort();
    this.containerEvents.delete(node);
    for (const child of Array.from(node.childNodes ?? [])) this.discard(child);
    Patcher.releaseScope(node);
    registryBehaviours.disposeBehaviours(node);
    this.deps.onUnmount?.(node);
    (this.deps.dispatcher ?? this.deps.runtime?.dispatcher)?.off?.(node);
    const id = this.nodeIds.get(node) ?? node.getAttribute?.("primal-id");
    if (id != null && this.registry.get(Number(id)) === node) {
      this.registry.unregister(Number(id));
    }
    for (const [name, handler] of this.elementListeners.get(node) ?? []) {
      node.removeEventListener(name, handler);
    }
    this.elementListeners.delete(node);
    this.nodeIds.delete(node);
  }
  selectorQuery(selector) {
    const text = selector.text;
    const escape = this.document.defaultView?.CSS?.escape ?? globalThis.CSS?.escape;
    if (selector.kind === wire.SelectorKind.Id) return "#" + escape(text);
    if (selector.kind === wire.SelectorKind.Class) return "." + escape(text);
    if (selector.kind === wire.SelectorKind.Query) return text;
    throw problem("UnknownOp");
  }
  resolve(selector) {
    if (!selector) return null;
    if (selector.kind === wire.SelectorKind.NodeId) return this.registry.get(selector.nodeId);
    if (selector.kind === wire.SelectorKind.Id) return this.document.getElementById(selector.text);
    return this.document.querySelector(this.selectorQuery(selector));
  }
  registerView(name, root) {
    if (!/^[A-Za-z_][A-Za-z0-9_-]*$/.test(name) || !root || this.views.has(name) || [...this.views.values()].includes(root))
      throw problem("DuplicateView", {detail: name});
    const primary = root === this.deps.viewMount || (!this.deps.viewMount && this.views.size === 0);
    const state = primary ? this.defaultView : {registry: new NodeRegistry(), cursors: new Map(), gaps: new Map()};
    this.viewStates.set(root, state);
    this.views.set(name, root);
    return () => {
      if (this.views.get(name) !== root) return;
      const previous = this.registry;
      this.registry = state.registry;
      try {
        for (const child of Array.from(root.childNodes)) { this.discard(child); child.remove(); }
      } finally {
        this.registry = previous;
        for (const gap of state.gaps.values()) this.timer.clear(gap.timer);
        state.gaps.clear();
        state.cursors.clear();
        this.loaders.get(root)?.close();
        this.loaders.delete(root);
        this.views.delete(name);
        this.viewStates.delete(root);
      }
    };
  }
  registerLoader(root, load, options = {}) {
    if (this.loaders.get(root)?.active) throw new Error("View already has a loader");
    const recovery = new ViewRecovery(load, (bytes, expired) => {
      if (!isFrame(bytes) || bytes.byteLength > this.maxFrameBytes) throw new Error("Invalid view snapshot");
      const frame = wire.Frame.getRootAsFrame(new CheckedBuffer(asBytes(bytes)));
      const body = frame.kindType() === wire.FrameKind.FrameMessage && frame.kind(new wire.FrameMessage());
      if (!body || body.messagesLength() !== 1) throw new Error("Expected one full view");
      const operation = body.messages(0).operation();
      const place = operation?.uiType() === wire.UiOperation.Place && operation.ui(new wire.Place());
      if (!place || place.kind() !== wire.PlacementKind.FullView) throw new Error("Expected a full view");
      const doc = this.deps.snapshotDocument?.() ?? this.document.implementation.createHTMLDocument("");
      const detached = doc.createElement("main");
      doc.body.appendChild(detached);
      const issues = [];
      const probe = new FrameApplicator({document:doc, registry:new NodeRegistry().seedDocument(doc),
        viewMount:detached, schedule:run => run(), maxFrameBytes:this.maxFrameBytes, onReport:error => issues.push(error)});
      if (!probe.receive(bytes) || issues.length) throw new Error("Invalid snapshot content");
      const cursors = new Map();
      for (let i = 0; i < place.operationsLength(); i++) {
        const op = place.operations(i);
        if (op.uiType() !== wire.UiOperation.LiveMount) continue;
        const mount = op.ui(new wire.LiveMount());
        if (mount.kind() !== wire.LiveKind.Stream || mount.transport() === wire.LiveTransport.Local ||
            (mount.transport() === wire.LiveTransport.Sse && mount.sharing() === wire.ConnectionSharing.Shared)) continue;
        if (cursors.has(mount.channelId()) && cursors.get(mount.channelId()) !== mount.resumeFrom()) throw new Error("Conflicting stream cursors");
        cursors.set(mount.channelId(), mount.resumeFrom());
      }
      for (const stream of expired) if (!cursors.has(stream)) throw new Error("Snapshot is missing the expired stream");
      const state = this.viewState(root);
      for (const gap of state.gaps.values()) this.timer.clear(gap.timer);
      state.gaps.clear();
      state.cursors.clear();
      for (const [stream, cursor] of cursors) this.setCursor(stream,cursor,root);
      if (!this.receive(bytes, {viewMount:root})) throw new Error("Invalid view snapshot");
      this.flush();
    }, options);
    this.loaders.set(root,recovery);
    return recovery;
  }
  registerElement(name, tag, node) {
    const valid = /^[A-Za-z_][A-Za-z0-9_-]*$/;
    if (!valid.test(name) || !valid.test(tag) || tag.toLowerCase() === "view") throw problem("InvalidName");
    let root = node;
    while (root.parentNode && ![...this.views.values(),this.deps.viewMount].includes(root)) root = root.parentNode;
    const other = this.namedElements(root).get(name);
    if (other && other !== node) throw problem("DuplicateName", {detail:name});
    node.setAttribute("id",name);
    this.sharedKinds.set(node,tag);
  }
  receivingView() {
    const supplied = this.origin && "viewMount" in this.origin ? this.origin.viewMount : null;
    if (supplied) {
      for (let node = supplied; node; node = node.parentNode) {
        if (node === this.deps.viewMount || [...this.views.values()].includes(node)) return node;
      }
      return supplied;
    }
    const roots = new Set(this.views.values());
    if (this.deps.viewMount) roots.add(this.deps.viewMount);
    if (roots.size > 1) throw problem("AmbiguousView");
    return roots.values().next().value;
  }
  namedElements(root) {
    const names = new Map(), pending = [root];
    const otherViews = new Set(this.views.values());
    while (pending.length) {
      const node = pending.pop();
      if (!node || (node !== root && otherViews.has(node))) continue;
      const name = node.getAttribute?.("id");
      if (name) {
        if (names.has(name)) throw problem("DuplicateName", {detail:name});
        names.set(name,node);
      }
      for (const child of node.children ?? []) pending.push(child);
    }
    return names;
  }
  placementTarget(selector, parent, scope) {
    if (!selector) return null;
    const kinds = wire.SelectorKind;
    if (selector.kind === kinds.NodeId) {
      const node = this.registry.get(selector.nodeId);
      return node && (!parent || node.parentNode === parent) ? node : null;
    }
    if (selector.kind === kinds.View) return this.views.get(selector.text);
    if (selector.kind === kinds.Receiving) return this.receivingView();
    if (selector.kind === kinds.Id || selector.kind === kinds.TypedId) {
      const typed = selector.kind === kinds.TypedId;
      const parts = typed ? selector.text?.split("#") : [null,selector.text];
      if (parts?.length !== 2 || (typed && !/^[A-Za-z_][A-Za-z0-9_-]*$/.test(parts[0]))) throw problem("InvalidName");
      const [tag, name] = parts;
      if (!/^[A-Za-z_][A-Za-z0-9_-]*$/.test(name)) throw problem("InvalidName");
      const root = scope ?? this.receivingView() ?? this.document.body;
      const node = this.namedElements(root).get(name);
      if (!node || (parent && node.parentNode !== parent)) return null;
      if (typed) {
        const compact = text => text?.replace(/[-_]/g,"").toLowerCase();
        const expected = compact(tag) === "sheet" ? "bottomsheet" : compact(tag);
        if (compact(this.sharedKinds.get(node) ?? node.localName) !== expected)
          throw problem("WrongTargetKind", {detail:selector.text});
      }
      return node;
    }
    const query = this.selectorQuery(selector);
    const matches = parent ? Array.from(parent.children).filter(node => node.matches(query)) :
      this.document.querySelectorAll(query);
    return matches.length === 1 ? matches[0] : null;
  }
  validatePlacementNames(reader, scope, outgoing) {
    const names = this.namedElements(scope);
    for (const [name,node] of names) if (outgoing?.some(root => root === node || root.contains?.(node))) names.delete(name);
    const check = name => {
      if (!name || !/^[A-Za-z_][A-Za-z0-9_-]*$/.test(name)) throw problem("InvalidName");
      if (names.has(name)) throw problem("DuplicateName", {detail:name});
      names.set(name,true);
    };
    for (let i = 0; i < reader.operationsLength(); i++) {
      const unit = reader.operations(i);
      if (unit.uiType() === wire.UiOperation.SetProp) {
        const prop = unit.ui(new wire.SetProp());
        if (prop.prop() === wire.PropId.ID) check(prop.value(new wire.TextValue())?.v());
      }
      const batch = unit.columnar();
      if (!batch) continue;
      const bytes = batch.blobBytesArray() ?? new Uint8Array();
      for (let row = 0; row < batch.opCodesLength(); row++) {
        if (batch.opCodes(row) !== wire.UiOperation.SetProp || (batch.argA(row) & 0xffff) !== wire.PropId.ID) continue;
        const start = batch.blobAOff(row), end = batch.blobAOff(row + 1);
        if (start > end || end > bytes.length) throw problem("BadFrame");
        check(utf8.decode(bytes.subarray(start,end)));
      }
    }
  }
  place(reader, index) {
    const target = reader.target();
    const root = target?.kind() === wire.SelectorKind.View ? this.views.get(target.text()) : this.origin?.viewMount;
    const previous = this.registry;
    this.registry = this.viewState(root).registry;
    try { this.applyPlacement(reader, index); }
    finally { this.registry = previous; }
  }
  applyPlacement(reader, index) {
    const kind = wire.PlacementKind[reader.kind()];
    if (kind === "FullView") {
      const mount = this.origin && "viewMount" in this.origin ? this.origin.viewMount : this.receivingView();
      if (!mount || mount.isConnected === false) throw problem("UnknownNode", { detail: "Missing receiving view mount" });
      this.validatePlacementNames(reader, mount, Array.from(mount.childNodes));
      for (const node of Array.from(mount.childNodes)) { this.discard(node); node.remove(); }
      this.applyNested(reader, index);
      mount.appendChild(this.node(reader.rootNode(), true));
      return;
    }
    const selector = reader.target()?.unpack();
    const scope = selector?.kind === wire.SelectorKind.View ? this.views.get(selector.text) : this.receivingView() ?? this.document.body;
    const target = this.placementTarget(selector, null, scope);
    const missing = value => [wire.SelectorKind.Id, wire.SelectorKind.View, wire.SelectorKind.TypedId, wire.SelectorKind.Receiving].includes(value?.kind)
      ? "MissingTarget" : "UnknownNode";
    if (!target) throw problem(missing(selector), { detail: "Placement needs one target" });
    const childSelector = reader.child()?.unpack();
    const child = childSelector && this.placementTarget(childSelector, target, scope);
    if (childSelector && (!child || child.parentNode !== target)) throw problem(missing(childSelector), { detail: "Placement child missing" });
    this.validatePlacementNames(reader, scope, kind === "Swap" ? [target] : []);
    if (kind === "Remove") { this.discard(target); target.remove(); return; }
    if (kind === "Swap") this.discard(target);
    this.applyNested(reader, index);
    const root = this.node(reader.rootNode(), true);
    switch (kind) {
      case "Append": target.appendChild(root); break;
      case "Swap": target.replaceWith(root); break;
      case "Before": target.insertBefore(root, child ?? target.firstChild); break;
      case "After": target.insertBefore(root, child?.nextSibling ?? null); break;
      default: throw problem("UnknownOp");
    }
  }
  applyUi(name, m, index) {
    const d = this.document;
    switch (name) {
      case "CreateNode": {
        if (!this.cssInstalled) {
          const style = d.createElement("style"); style.setAttribute("data-ewe-core", "");
          style.textContent = vocab.CORE_CSS; d.head.appendChild(style); this.cssInstalled = true;
        }
        const kind = vocab.WEB_KINDS[m.kind];
        if (!kind) this.report("UnknownKind", { unitIndex: index, node: m.nodeId });
        const el = d.createElement(kind?.tag ?? "div");
        this.sharedKinds.set(el, wire.NodeKind[m.kind]);
        el.className = kind?.cls ?? "ewe-unknown";
        if (m.kind === wire.NodeKind.DRAWING_SURFACE) this.drawingTargets.push([el, m.nodeId]);
        for (const [attr, value] of kind?.attrs ?? []) el.setAttribute(attr, value);
        el.setAttribute("primal-id", String(m.nodeId));
        if (m.kind === wire.NodeKind.OVERLAY && el.showPopover) {
          el.setAttribute("popover", "manual");
          this.layers.add(el);
        }
        if ([wire.NodeKind.MODAL, wire.NodeKind.CONFIRMATION_DIALOG, wire.NodeKind.BOTTOM_SHEET].includes(m.kind)) {
          const Controller = d.defaultView?.AbortController ?? globalThis.AbortController;
          const controller = new Controller();
          this.containerEvents.set(el, controller);
          const options = { signal: controller.signal };
          if (m.kind !== wire.NodeKind.BOTTOM_SHEET) {
            this.layers.add(el);
            el.addEventListener("cancel", event => {
              event.preventDefault();
              const cancel = Array.from(el.querySelectorAll('.ewe-cancel-button'))
                .find(node => node.closest("dialog") === el && !node.disabled && !node.closest("[hidden]")
                  && node.hasAttribute("primal:onclick"));
              cancel?.click();
            }, options);
          } else {
            wireBottomSheet(el, options);
          }
        }
        if (m.sharing !== wire.ConnectionSharing.TransportDefault) {
          el.setAttribute("data-connection-shared", String(m.sharing === wire.ConnectionSharing.Shared));
        }
        if (m.keepAlive) el.setAttribute("data-connection-keep-alive", "true");
        this.origin?.onMount?.(m.nodeId, el);
        this.registry.register(m.nodeId, el);
        break;
      }
      case "SetText": {
        const node = this.node(m.nodeId);
        for (const child of Array.from(node.childNodes)) this.discard(child);
        node.textContent = m.text ?? ""; break;
      }
      case "SetProp": applyProp(this.node(m.nodeId), m.prop, m.valueType, m.value); break;
      case "AppendChild": this.node(m.parentId).appendChild(this.node(m.childId, true)); break;
      case "InsertBefore": {
        const parent = this.node(m.parentId);
        const before = this.node(m.beforeId);
        parent.insertBefore(this.node(m.childId, true), before);
        break;
      }
      case "Remove": { const node = this.node(m.nodeId); this.discard(node); node.remove(); this.registry.unregister(m.nodeId); break; }
      case "BindAction": case "UnbindAction": {
        const el = this.node(m.nodeId), event = vocab.WEB_EVENTS[m.event];
        if (!event) throw problem("UnknownOp");
        const attr = "primal:on" + event;
        const dispatcher = this.deps.dispatcher ?? this.deps.runtime?.dispatcher;
        if (name === "BindAction") { el.setAttribute(attr, m.action); dispatcher?.scanAndWire?.(el); }
        else { el.removeAttribute(attr); dispatcher?.off?.(el, event); }
        break;
      }
      case "AttachState": this.node(m.rootNode).setAttribute("primal::state", base64urlEncode(m.state ?? [])); break;
      case "UiBehaviour":
        this.bindings.push({registry: this.registry, node:this.node(m.nodeId),id:m.nodeId,index,name:m.behaviour,args:m.args,shared:true});
        break;
      case "LiveMount": {
        const el = d.createElement(m.kind === wire.LiveKind.Stream ? "mount-stream" : "mount-data");
        const transport = ["local", "sse", "websocket", "fetch"][m.transport];
        for (const [key, value] of Object.entries({ api: m.endpoint, transport, protocol: "frame",
          "data-channel-id": m.channelId, "data-resume-from": m.resumeFrom, "primal-id": m.nodeId })) el.setAttribute(key, String(value));
        if (m.sharing !== wire.ConnectionSharing.TransportDefault) {
          el.setAttribute("data-connection-shared", String(m.sharing === wire.ConnectionSharing.Shared));
        }
        if (m.keepAlive) el.setAttribute("data-connection-keep-alive", "true");
        this.origin?.onMount?.(m.nodeId, el);
        this.registry.register(m.nodeId, el); break;
      }
      default: throw problem("UnknownOp", { field: "ui" });
    }
  }
  applyWebReader(name, reader, index) {
    const m = reader.unpack();
    const d = this.document;
    const id = m.nodeId;
    const resolveName = (id, custom, names) => {
      const value = id ? names[id - 1] : custom;
      if (!value) throw problem("BadFrame");
      return value;
    };
    const property = () => resolveName(m.nameId, m.customName, vocab.ATTR_NAMES);
    switch (name) {
      case "WebElement": {
        const el = d.createElement(resolveName(m.tagId, m.customTag, vocab.TAG_NAMES));
        if (m.className) el.className = m.className;
        this.origin?.onMount?.(id, el);
        this.nodeIds.set(el, id); this.registry.stage(id, el); break;
      }
      case "WebText": {
        const node = d.createTextNode(m.content ?? "");
        this.origin?.onMount?.(id, node);
        this.nodeIds.set(node, id); this.registry.stage(id, node); break;
      }
      case "WebRegisterNode": this.registry.applyRegister(id, d); break;
      case "WebUnregisterNode": this.registry.unregister(id); break;
      case "WebReplaceNode": {
        const old = this.node(m.oldId), next = this.node(m.newId, true);
        this.discard(old); old.replaceWith(next);
        this.registry.unregister(m.oldId); this.registry.register(m.newId, next); break;
      }
      case "WebDetachChild": this.node(m.parentId).removeChild(this.node(m.childId)); break;
      case "WebSetAttribute": case "WebRemoveAttribute": {
        const attr = resolveName(m.attrId, m.customAttr, vocab.ATTR_NAMES);
        if (attr.toLowerCase() === "primal::state") throw problem("BadFrame");
        if (name === "WebSetAttribute") this.node(id).setAttribute(attr, m.value ?? "");
        else this.node(id).removeAttribute(attr);
        break;
      }
      case "HtmlMorph": {
        const target = this.resolve(m.target);
        if (!target) throw problem("Morph");
        if (m.action === wire.MorphAction.ReplaceChildren && this.deps.morph && d.createRange) {
          const previous = Array.from(target.querySelectorAll("*"));
          this.deps.morph(target, d.createRange().createContextualFragment(m.content), d);
          const removed = new Set(previous.filter(n => !target.contains(n)));
          for (const n of removed) if (!removed.has(n.parentNode)) this.discard(n);
        } else {
          switch (m.action) {
            case wire.MorphAction.ReplaceChildren:
              for (const child of Array.from(target.childNodes ?? [])) this.discard(child);
              target.innerHTML = m.content;
              break;
            case wire.MorphAction.ReplaceElement:
              target.insertAdjacentHTML("afterend", m.content);
              this.discard(target);
              target.remove();
              break;
            case wire.MorphAction.InsertBefore: target.insertAdjacentHTML("beforebegin", m.content); break;
            case wire.MorphAction.InsertAfter: target.insertAdjacentHTML("afterend", m.content); break;
            case wire.MorphAction.AppendSibling:
              (target.parentNode ?? target.parent)?.insertAdjacentHTML("beforeend", m.content);
              break;
            default: throw problem("Morph");
          }
        }
        break;
      }
      case "ScriptNode": {
        const el = d.createElement("script");
        el.setAttribute("primal:script", ""); el.setAttribute("scoped", "");
        // The hydrator runs the body in the parent scope.
        el.type = "application/primal-script";
        el.textContent = m.body;
        const parent = this.node(m.parentId);
        parent.appendChild(el);
        this.deps.hydrate?.(parent, d); break;
      }
      case "WebBehaviour": {
        this.bindings.push({registry: this.registry, node:this.node(id),id,index,name:m.custom,args:m.args,shared:false});
        break;
      }
      case "WebProperty": {
        let value = m.value;
        try { value = JSON.parse(value); } catch { /* A plain string is a valid property. */ }
        this.node(id)[property()] = value;
        break;
      }
      case "WebStyle": this.node(id).style[property()] = m.value; break;
      case "WebAddClass": this.node(id).classList.add(m.className); break;
      case "WebRemoveClass": this.node(id).classList.remove(m.className); break;
      case "WebAddListener": case "WebRemoveListener": {
        const node = this.node(id), event = property();
        let listeners = this.elementListeners.get(node);
        if (!listeners) this.elementListeners.set(node, listeners = new Map());
        if (name === "WebAddListener" && !listeners.has(event)) {
          const deliver = this.origin?.onEvent ?? this.deps.onEvent;
          const handler = value => deliver?.(event, id, value, node);
          listeners.set(event, handler);
          node.addEventListener(event, handler);
        } else if (name === "WebRemoveListener" && listeners.has(event)) {
          node.removeEventListener(event, listeners.get(event));
          listeners.delete(event);
        }
        break;
      }
      default: throw problem("UnknownOp", {field:"web"});
    }
  }
  applyColumnar(batch, index) {
    const codes = batch.opCodesArray() ?? [];
    const cols = ["nodeIds", "argA", "argB", "blobAOff", "blobBOff"].map((name, i) =>
      vector(batch, name, 6 + 2*i, Uint32Array, this.stats));
    const [nodes, args, values, starts, second] = cols;
    const bytes = batch.blobBytesArray() ?? new Uint8Array();
    if (cols.some((v,i) => v.length !== codes.length + Number(i >= 3)) ||
        starts[0] !== 0 || starts.at(-1) !== bytes.length ||
        (second[0] !== 0 && second[0] !== bytes.length)) throw problem("BadFrame");
    const bits = new DataView(new ArrayBuffer(4));
    for (let i = 0; i < codes.length; i++) {
      try {
        if (starts[i] > starts[i+1] || starts[i+1] > bytes.length || second[i] !== second[i+1]) throw problem("BadFrame");
        const blob = bytes.subarray(starts[i], starts[i+1]);
        const name = wire.UiOperation[codes[i]];
        const nodeId = nodes[i], a = args[i], b = values[i];
        const m = { nodeId };
        switch (name) {
          case "CreateNode": m.kind = a; break;
          case "SetText": m.text = utf8.decode(blob); break;
          case "SetProp": {
            m.prop = a & 0xffff;
            const kind = vocab.PROP_KIND[m.prop];
            m.valueType = wire.PropValue[kind];
            bits.setUint32(0, b, true);
            switch (kind) {
              case "Text": m.value = {v: utf8.decode(blob)}; break;
              case "Length": m.value = {v: {value: bits.getFloat32(0, true), unit: a >>> 16}}; break;
              case "ColorToken": case "FontToken": m.value = {id: b}; break;
              case "Bool": m.value = {v: Boolean(b)}; break;
              case "Axis": case "Alignment": m.value = {v: b}; break;
              default: throw problem("BadFrame");
            }
            break;
          }
          case "InsertBefore": case "AppendChild": Object.assign(m, {parentId: nodeId, childId: a, beforeId: b}); break;
          case "Remove": break;
          case "BindAction": m.action = utf8.decode(blob); m.event = a; break;
          case "UnbindAction": m.event = a; break;
          case "AttachState": m.rootNode = nodeId; m.state = blob; break;
          default: throw problem("UnknownOp", {field: "columnar"});
        }
        this.applyUi(name, m, index); this.stats.rows++;
      } catch (e) { this.report(e.report?.code ?? "BadFrame", {unitIndex: index, ...(e.report ?? {}), row: i}); }
    }
  }
  applyData(data) {
    const table = data.table();
    let detail;
    if (table) {
      const columns = [];
      const types = { I32: [wire.I32s, Int32Array], I64: [wire.I64s, BigInt64Array],
        F64: [wire.F64s, Float64Array], Bool: [wire.Bools, Uint8Array], Utf8: [wire.Strings] };
      for (let i = 0; i < table.columnsLength(); i++) {
        const col = table.columns(i);
        const type = types[wire.ColumnData[col.dataType()]];
        if (!type) throw problem("BadFrame");
        const values = col.data(new type[0]());
        const items = type[1] ? vector(values, "values", 4, type[1]) :
          Array.from({length: values.valuesLength()}, (_, j) => values.values(j));
        columns.push({name: col.name(), data: items, validity: col.validityArray()});
      }
      detail = {kind: "table", table: {rows: table.rows(), columns}};
    } else {
      const blob = data.arrow() ?? data.msgpack();
      if (!blob) throw problem("BadFrame");
      detail = {kind: data.arrow() ? "arrow-ipc" : "messagepack", bytes: blob.bytesArray()};
    }
    const target = data.targetNode() ? this.node(data.targetNode()) : this.receivingView();
    dispatchEvent(target || this.document, "ewe:data", detail);
  }
  applyControl(control) {
    if (control.caps()) this.deps.onCaps?.(control.caps().unpack());
    for (const [field, code] of [["stateExpired", "StateExpired"], ["updateRequired", "UpdateRequired"]]) {
      if (control[field]()) this.deps.onError?.({ ...control[field]().unpack(), code });
    }
    if (control.error()) {
      const error = control.error().unpack();
      const detail = { ...error, code: wire.ErrorCode[error.code] };
      this.deps.onError?.(detail); dispatchEvent(this.document, "ewe:error", detail);
    }
  }
}

export function installFrameRuntime(wasmUi, options = {}) {
  const doc = options.document ?? globalThis.document;
  const runtime = wasmUi.Patcher.runtime;
  if (runtime.frames?.document === doc) return runtime.frames;
  if (runtime.document && runtime.document !== doc) runtime.dispatcher = null;
  runtime.frames = new FrameApplicator({document: doc, registry: wasmUi.Patcher.ensureRegistry(doc),
    morph: wasmUi.MorphDom.morph, hydrate: wasmUi.Hydrator.hydrate, runtime, onApplied: wasmUi.Patcher.markApplied,
    ...options });
  bindLiveMounts(runtime, runtime.frames);
  wasmUi.Hydrator.behaviourMarkers = registryBehaviours.hydrateMarkers;
  if (doc.body) registryBehaviours.hydrateMarkers(doc.body);
  wasmUi.ProtocolHandler.frameHook = {isFrame, splitFrames, readFrames, readViewResponse, maxFrameBytes: runtime.frames.maxFrameBytes};
  return runtime.frames;
}

globalThis.EweFrames = {installFrameRuntime};
