// Apply EWF1 message frames to the browser node registry.
import * as wire from "./foundation-ui-wire.js";
import * as vocab from "./foundation-ui-vocab.js";
import * as registryBehaviours from "./foundation-ui-behaviours.js";
import { DomOpApplicator, Op, Patcher } from "./foundation-wasm-ui.js";

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
    case "A11Y_LABEL": node.setAttribute("aria-label", v ?? ""); break;
    case "SOURCE": node.setAttribute("src", v ?? ""); break;
    case "PLACEHOLDER": node.setAttribute("placeholder", v ?? ""); break;
    case "VALUE": node.value = v ?? ""; break;
    case "CHECKED": node.checked = v; break;
    default: throw problem("UnknownOp");
  }
}

function dispatchEvent(target, name, detail) {
  const EventType = target.ownerDocument?.defaultView?.CustomEvent ?? globalThis.CustomEvent;
  target.dispatchEvent?.(EventType ? new EventType(name, { bubbles: true, detail }) : { type: name, detail, bubbles: true });
}

export class FrameApplicator {
  constructor(deps) {
    this.deps = deps;
    this.document = deps.document;
    this.registry = deps.registry;
    this.legacy = deps.applicator ?? new DomOpApplicator(deps.registry, deps.document, deps.onEvent);
    this.maxFrameBytes = deps.maxFrameBytes ?? defaultFrameBytes;
    this.gapTimeoutMs = deps.gapTimeoutMs ?? 1000;
    this.maxPendingFrames = deps.maxPendingFrames ?? 128;
    this.timer = deps.timer ?? { set: setTimeout, clear: clearTimeout };
    this.schedule = deps.schedule ?? (globalThis.requestAnimationFrame?.bind(globalThis) ?? queueMicrotask);
    this.stats = { frames: 0, rows: 0, reports: 0, copiedColumns: 0, zeroCopyColumns: 0 };
    this.cursors = new Map();
    this.gaps = new Map();
    this.queue = [];
    this.scheduled = false;
    this.cssInstalled = false;
    this.nodeIds = new WeakMap();
  }
  cursor(stream) { return this.cursors.get(stream); }
  setCursor(stream, sequence) {
    const gap = this.gaps.get(stream);
    if (gap) this.timer.clear(gap.timer);
    this.gaps.delete(stream);
    this.cursors.set(stream, sequence);
  }
  registerUnmount(node, hook) { Patcher.registerUnmount(node, hook); }
  report(code, fields = {}) {
    this.stats.reports++;
    const report = { code, ...fields };
    if (this.deps.onReport) this.deps.onReport(report);
    else console.error("Frame operation failed", report);
  }
  receive(input) {
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
      if (seq === 0n || msg.ack() !== 0n) this.enqueue(msg);
      else this.order(stream, seq, msg);
      return true;
    } catch (error) { this.report("BadFrame", { detail: error.message }); return false; }
  }
  order(stream, seq, msg) {
    const cursor = this.cursor(stream);
    if (cursor !== undefined && seq <= cursor) return;
    let gap = this.gaps.get(stream);
    if (cursor !== undefined && seq > cursor + 1n) {
      if (!gap) {
        gap = { held: new Map(), timer: this.timer.set(() => {
          this.gaps.delete(stream);
          this.deps.onGap?.({ streamId: stream, resumeFrom: this.cursor(stream) });
        }, this.gapTimeoutMs) };
        this.gaps.set(stream, gap);
      }
      if (!gap.held.has(seq) && gap.held.size >= this.maxPendingFrames) {
        this.timer.clear(gap.timer);
        this.gaps.delete(stream);
        this.deps.onGap?.({ streamId: stream, resumeFrom: cursor });
      } else gap.held.set(seq, msg);
      return;
    }
    this.enqueue(msg);
    while (gap?.held.has(seq + 1n)) {
      seq++;
      this.enqueue(gap.held.get(seq));
      gap.held.delete(seq);
    }
    this.cursors.set(stream, seq);
    if (gap && !gap.held.size) { this.timer.clear(gap.timer); this.gaps.delete(stream); }
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
      this.stats.frames++;
      for (let i = 0; i < msg.messagesLength(); i++) {
        try {
          const m = msg.messages(i);
          if (m.operation()) this.applyUnit(m.operation(), i);
          if (m.data()) this.applyData(m.data());
          if (m.control()) this.applyControl(m.control());
        } catch (e) { this.report(e.report?.code ?? "BadFrame", { unitIndex: i, ...(e.report ?? {}), detail: e.message }); }
      }
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
    for (const child of Array.from(node.childNodes ?? [])) this.discard(child);
    Patcher.releaseScope(node);
    registryBehaviours.disposeBehaviours(node);
    this.deps.onUnmount?.(node);
    (this.deps.dispatcher ?? this.deps.runtime?.dispatcher)?.off?.(node);
    const id = this.nodeIds.get(node) ?? node.getAttribute?.("primal-id");
    if (id != null && this.registry.get(Number(id)) === node) {
      this.registry.unregister(Number(id));
      for (const [key, handler] of this.legacy.listeners) {
        const prefix = String(id) + ":";
        if (key.startsWith(prefix)) {
          node.removeEventListener(key.slice(prefix.length), handler);
          this.legacy.listeners.delete(key);
        }
      }
    }
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
  placementTarget(selector, parent) {
    if (!selector) return null;
    if (selector.kind === wire.SelectorKind.NodeId) {
      const node = this.registry.get(selector.nodeId);
      return node && (!parent || node.parentNode === parent) ? node : null;
    }
    const query = this.selectorQuery(selector);
    const matches = parent ? Array.from(parent.children).filter(node => node.matches(query)) :
      this.document.querySelectorAll(query);
    return matches.length === 1 ? matches[0] : null;
  }
  place(reader, index) {
    const kind = wire.PlacementKind[reader.kind()];
    const target = this.placementTarget(reader.target()?.unpack());
    if (!target) throw problem("UnknownNode", { detail: "Placement needs one target" });
    const childSelector = reader.child()?.unpack();
    const child = childSelector && this.placementTarget(childSelector, target);
    if (childSelector && (!child || child.parentNode !== target)) throw problem("UnknownNode", { detail: "Placement child missing" });
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
        el.className = kind?.cls ?? "ewe-unknown";
        for (const [attr, value] of kind?.attrs ?? []) el.setAttribute(attr, value);
        el.setAttribute("primal-id", String(m.nodeId));
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
        if (!(this.deps.behaviours ?? registryBehaviours).applyBehaviour(this.node(m.nodeId), m.behaviour, m.args)) {
          this.report("UnknownBehaviour", { unitIndex: index, node: m.nodeId, name: String(m.behaviour) });
        }
        break;
      case "LiveMount": {
        const el = d.createElement(m.kind === wire.LiveKind.Stream ? "mount-stream" : "mount-data");
        const transport = ["local", "sse", "websocket", "fetch"][m.transport];
        for (const [key, value] of Object.entries({ api: m.endpoint, transport, protocol: "frame",
          "data-channel-id": m.channelId, "data-resume-from": m.resumeFrom, "primal-id": m.nodeId })) el.setAttribute(key, String(value));
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
    const one = (op, attr, value, text) => this.legacy.applyOne(op, id, attr, value, text);
    switch (name) {
      case "WebElement": {
        const el = d.createElement(resolveName(m.tagId, m.customTag, vocab.TAG_NAMES));
        if (m.className) el.className = m.className;
        this.nodeIds.set(el, id); this.registry.stage(id, el); break;
      }
      case "WebText": {
        const node = d.createTextNode(m.content ?? "");
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
          if (m.action === wire.MorphAction.ReplaceElement) this.discard(target);
          this.legacy.applyMorph(m.target.nodeId, [m.action, m.target.kind, m.target.text ?? ""].join(":"), m.content);
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
        const applied = (this.deps.behaviours ?? registryBehaviours).applyWebBehaviour(this.node(id), m.custom, m.args);
        if (!applied) this.report("UnknownBehaviour", { unitIndex: index, node: id, name: m.custom });
        break;
      }
      default: {
        this.node(id);
        switch (name) {
          case "WebProperty": one(Op.SET_PROPERTY, property(), m.value); break;
          case "WebStyle": one(Op.SET_STYLE, property(), m.value); break;
          case "WebAddClass": one(Op.ADD_CLASS, "", m.className); break;
          case "WebRemoveClass": one(Op.REMOVE_CLASS, "", m.className); break;
          case "WebAddListener": one(Op.ADD_EVENT_LISTENER, "", property()); break;
          case "WebRemoveListener": one(Op.REMOVE_EVENT_LISTENER, "", property()); break;
          default: throw problem("UnknownOp", { field: "web" });
        }
      }
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
    const target = data.targetNode() && this.registry.get(data.targetNode());
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
  if (runtime.applicator?.document && runtime.applicator.document !== doc) {
    runtime.applicator = null;
  }
  const base = wasmUi.Patcher.ensureApplicator(doc);
  runtime.frames = new FrameApplicator({document: doc, registry: base.registry, applicator: base,
    morph: wasmUi.MorphDom.morph, hydrate: wasmUi.Hydrator.hydrate, runtime,
    ...options });
  wasmUi.ProtocolHandler.frameHook = {isFrame, splitFrames, readFrames, maxFrameBytes: runtime.frames.maxFrameBytes};
  return runtime.frames;
}
