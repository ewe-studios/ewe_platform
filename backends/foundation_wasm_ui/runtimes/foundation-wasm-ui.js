// foundation-wasm-ui.js — DOM layer of the runtime (built on foundation-wasm.js).
//
// WHY: foundation-wasm.js is the pure ABI (memory, transport, timers). The DOM half
// lives here so the ABI stays reusable by non-DOM hosts — mirroring the
// foundation_wasm / foundation_wasm_ui crate split.
//
// WHAT: the first DOM increment —
//   - ColumnarParser        : decodes the Arrow columnar payload (the exact layout
//                          `foundation_ui_traits::ColumnarEncoder` produces)
//   - NodeRegistry       : primal-id (u32) → DOM node
//   - DomOpApplicator : applies a parsed batch to a DOM `document`
//   - columnarHandler()     : a ProtocolDispatcher handler that ties parse → apply
//
// HOW: register `columnarHandler(...)` on a FoundationWasm dispatcher for protocol byte
// 1; when WASM ships an Arrow batch via host_apply, it's parsed and applied here.
//
// The op codes match decision 010 (shared with the Rust encoders).

// ─── Operation codes (decision 010) ─────────────────────────────────────────────

export const Op = Object.freeze({
  CREATE_ELEMENT: 0,
  CREATE_TEXT_NODE: 1,
  SET_TEXT_CONTENT: 2,
  SET_ATTRIBUTE: 3,
  REMOVE_ATTRIBUTE: 4,
  SET_PROPERTY: 5,
  ADD_EVENT_LISTENER: 6,
  REMOVE_EVENT_LISTENER: 7,
  APPEND_CHILD: 8,
  REMOVE_CHILD: 9,
  REMOVE_NODE: 10,
  INSERT_BEFORE: 11,
  REPLACE_NODE: 12,
  SET_STYLE: 13,
  ADD_CLASS: 14,
  REMOVE_CLASS: 15,
  MORPH_NODE: 16,
  REGISTER_NODE: 17,
  UNREGISTER_NODE: 18,
});

// Reserved node ids (gap inventory G1): 0=<head>, 1=<body>, 2=<html>. Macro ids ≥ 1000.
export const RESERVED = Object.freeze({ HEAD: 0, BODY: 1, HTML: 2 });

// ─── Known tag/attribute id tables (decision G12/G13/G14) ───────────────────
//
// EXACT mirrors of `foundation_ui_traits::html::TAG_NAMES` / `ATTR_NAMES`:
// index + 1 == wire id, `"id:<n>"` strings in the columns resolve here.
// Append-only — ORDER IS ABI. Regenerate from html.rs when extending.

export const TAG_NAMES = Object.freeze([
  "div", "span", "input", "button", "html", "head", "body", "title", "base",
  "link", "meta", "style", "script", "noscript", "template", "slot", "main", "section",
  "nav", "article", "aside", "header", "footer", "address", "h1", "h2", "h3",
  "h4", "h5", "h6", "hgroup", "p", "hr", "pre", "blockquote", "ol",
  "ul", "menu", "li", "dl", "dt", "dd", "figure", "figcaption", "search",
  "a", "em", "strong", "small", "s", "cite", "q", "dfn", "abbr",
  "ruby", "rt", "rp", "data", "time", "code", "var", "samp", "kbd",
  "sub", "sup", "i", "b", "u", "mark", "bdi", "bdo", "br",
  "wbr", "ins", "del", "picture", "source", "img", "iframe", "embed", "object",
  "video", "audio", "track", "map", "area", "svg", "math", "canvas", "table",
  "caption", "colgroup", "col", "tbody", "thead", "tfoot", "tr", "td", "th",
  "form", "label", "select", "datalist", "optgroup", "option", "textarea", "output", "progress",
  "meter", "fieldset", "legend", "details", "summary", "dialog",
]);

export const ATTR_NAMES = Object.freeze([
  "class", "id", "style", "value", "title", "lang", "dir", "hidden", "tabindex",
  "accesskey", "draggable", "contenteditable", "spellcheck", "translate", "role", "slot", "part", "is",
  "href", "src", "srcset", "sizes", "alt", "rel", "target", "download", "referrerpolicy",
  "crossorigin", "integrity", "loading", "media", "type", "name", "placeholder", "disabled", "readonly",
  "required", "checked", "selected", "multiple", "min", "max", "step", "pattern", "minlength",
  "maxlength", "autocomplete", "autofocus", "for", "form", "action", "method", "enctype", "novalidate",
  "accept", "rows", "cols", "wrap", "list", "size", "colspan", "rowspan", "headers",
  "scope", "width", "height", "controls", "autoplay", "loop", "muted", "preload", "poster",
  "playsinline", "charset", "content", "http-equiv", "open", "label", "datetime", "cite", "data",
]);

/**
 * Resolve a decision-010 string-column name: `"id:<n>"` looks up `table`
 * (1-based), anything else is already the literal name.
 * @param {string} wire @param {readonly string[]} table
 */
export function resolveWireName(wire, table) {
  if (wire.startsWith("id:")) {
    const id = Number(wire.slice(3));
    if (Number.isInteger(id) && id >= 1 && id <= table.length) return table[id - 1];
  }
  return wire;
}


// ─── ColumnarParser (compact columnar payload — protocol 1, wire v1) ─────────────

/**
 * Decode the COMPACT COLUMNAR payload (protocol byte 1, wire VERSION 1 — the
 * owned, Arrow-INSPIRED layout; wire version 2 is real Arrow IPC, server-side)
 * into per-column arrays. Layout (little-endian), mirroring
 * `foundation_ui_traits::ColumnarEncoder::encode`:
 *
 *   [row_count:u32]
 *   [op_id:    u32 × N]
 *   [node_id:  u32 × N]
 *   [operation:u8  × N]
 *   [attribute string-column]
 *   [value     string-column]
 *   [text_val  string-column]
 *
 * A string-column is `[(N+1) offsets:u32][data_len:u32][utf8 bytes]`.
 */
export class ColumnarParser {
  /**
   * @param {Uint8Array} payload
   * @returns {{ count:number, nodeIds:Uint32Array, operations:Uint8Array,
   *            attribute:string[], value:string[], textVal:string[] }}
   */
  static parse(payload) {
    const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
    let off = 0;
    const u32 = () => {
      const v = view.getUint32(off, true);
      off += 4;
      return v;
    };

    const count = u32();

    // op_id column (sequential, not needed for apply — skip).
    off += count * 4;

    // node_id column (zero-copy view).
    const nodeIds = new Uint32Array(count);
    for (let i = 0; i < count; i++) nodeIds[i] = u32();

    // operation column.
    const operations = new Uint8Array(count);
    for (let i = 0; i < count; i++) operations[i] = view.getUint8(off++);

    const decoder = new TextDecoder();
    const readStringColumn = () => {
      const offsets = new Uint32Array(count + 1);
      for (let i = 0; i <= count; i++) offsets[i] = u32();
      const dataLen = u32();
      const base = payload.byteOffset + off;
      const out = new Array(count);
      for (let i = 0; i < count; i++) {
        out[i] = decoder.decode(
          new Uint8Array(payload.buffer, base + offsets[i], offsets[i + 1] - offsets[i]),
        );
      }
      off += dataLen;
      return out;
    };

    const attribute = readStringColumn();
    const value = readStringColumn();
    const textVal = readStringColumn();

    return { count, nodeIds, operations, attribute, value, textVal };
  }
}

// ─── NodeRegistry ────────────────────────────────────────────────────────────

/**
 * Maps primal-ids (u32) to DOM nodes. CreateElement/Text register; RemoveNode and
 * ReplaceNode unregister. Reserved ids 0-2 map to head/body/html when seeded.
 */
export class NodeRegistry {
  constructor() {
    this.nodes = new Map();
    // Created-but-not-yet-registered nodes (CreateElement/CreateTextNode do NOT
    // auto-register — feature 01): REGISTER_NODE promotes them into `nodes`.
    this.pending = new Map();
  }

  /** Seed the reserved ambient nodes from a document (browser) — optional in tests. */
  seedDocument(doc) {
    if (doc.head) this.nodes.set(RESERVED.HEAD, doc.head);
    if (doc.body) this.nodes.set(RESERVED.BODY, doc.body);
    if (doc.documentElement) this.nodes.set(RESERVED.HTML, doc.documentElement);
    return this;
  }

  register(id, node) {
    this.nodes.set(id, node);
    return node;
  }

  get(id) {
    return this.nodes.get(id);
  }

  /** Look up `id`, throwing a clear error if it isn't registered. */
  expect(id) {
    const node = this.nodes.get(id);
    if (node === undefined) throw new Error(`NodeRegistry: unknown node id ${id}`);
    return node;
  }

  unregister(id) {
    this.nodes.delete(id);
  }

  /** Park a freshly created node until an explicit REGISTER_NODE promotes it. */
  stage(id, node) {
    this.pending.set(id, node);
    return node;
  }

  /**
   * REGISTER_NODE semantics: promote a staged node, keep an existing
   * registration (no-op), or find a pre-existing element in the document via
   * `[primal-id="<id>"]`. Throws if the id is nowhere to be found.
   */
  applyRegister(id, document) {
    if (this.nodes.has(id)) return this.nodes.get(id); // re-register = no-op
    const staged = this.pending.get(id);
    if (staged !== undefined) {
      this.pending.delete(id);
      return this.register(id, staged);
    }
    const found = document.querySelector?.(`[primal-id="${id}"]`);
    if (found) return this.register(id, found);
    throw new Error(`NodeRegistry: REGISTER_NODE ${id} matches no staged node or [primal-id]`);
  }

  /**
   * Resolve an id that is allowed to still be staged — REPLACE_NODE's `new_id`
   * (its implicit registration) is the one consumer. Promotes staged nodes.
   */
  expectOrStaged(id) {
    const node = this.nodes.get(id);
    if (node !== undefined) return node;
    const staged = this.pending.get(id);
    if (staged !== undefined) {
      this.pending.delete(id);
      return staged;
    }
    throw new Error(`NodeRegistry: unknown node id ${id}`);
  }

  get size() {
    return this.nodes.size;
  }
}

// ─── DomOpApplicator ──────────────────────────────────────────────────────

/**
 * Applies a parsed Arrow batch to a DOM. `document` must provide `createElement`,
 * `createTextNode`; nodes must provide the usual mutation API. Designed so a minimal
 * stub (tests) or a real `document` both work.
 */
export class DomOpApplicator {
  /**
   * @param {NodeRegistry} registry @param {Document} document
   * @param {(eventName:string, nodeId:number, event:Event, el:Element)=>void} [onEvent]
   *   dispatch hook for ADD_EVENT_LISTENER-bound listeners (feature 08 wires WASM).
   */
  constructor(registry, document, onEvent) {
    this.registry = registry;
    this.document = document;
    this.onEvent = onEvent;
    this.listeners = new Map(); // `${nodeId}:${event}` -> bound handler
  }

  /** Apply a full batch (output of {@link ColumnarParser.parse}). */
  apply(batch) {
    const { count, nodeIds, operations, attribute, value, textVal } = batch;
    for (let i = 0; i < count; i++) {
      this.applyOne(operations[i], nodeIds[i], attribute[i], value[i], textVal[i]);
    }
  }

  applyOne(op, nodeId, attribute, value, textVal) {
    const reg = this.registry;
    switch (op) {
      case Op.CREATE_ELEMENT: {
        // attribute = tag (id: form or literal), value = class.
        const el = this.document.createElement(resolveWireName(attribute, TAG_NAMES));
        if (value) el.className = value;
        reg.stage(nodeId, el); // NO auto-register — REGISTER_NODE promotes.
        break;
      }
      case Op.CREATE_TEXT_NODE:
        reg.stage(nodeId, this.document.createTextNode(textVal));
        break;
      case Op.SET_TEXT_CONTENT:
        reg.expect(nodeId).textContent = textVal;
        break;
      case Op.SET_ATTRIBUTE:
        reg.expect(nodeId).setAttribute(resolveWireName(attribute, ATTR_NAMES), value);
        break;
      case Op.REMOVE_ATTRIBUTE:
        reg.expect(nodeId).removeAttribute(resolveWireName(attribute, ATTR_NAMES));
        break;
      case Op.SET_PROPERTY: {
        // value is the SERIALIZED property value; JSON covers the primitive
        // cases (numbers, booleans, quoted strings) with raw-string fallback.
        let parsed = value;
        try {
          parsed = JSON.parse(value);
        } catch {
          /* raw string property */
        }
        reg.expect(nodeId)[resolveWireName(attribute, ATTR_NAMES)] = parsed;
        break;
      }
      case Op.ADD_EVENT_LISTENER: {
        // value = event name. The bound listener forwards into the runtime's
        // event dispatch hook (feature 08 wires the WASM callback path).
        const eventName = resolveWireName(value, ATTR_NAMES);
        const el = reg.expect(nodeId);
        const key = `${nodeId}:${eventName}`;
        if (!this.listeners.has(key)) {
          const handler = (event) => this.onEvent?.(eventName, nodeId, event, el);
          this.listeners.set(key, handler);
          el.addEventListener(eventName, handler);
        }
        break;
      }
      case Op.REMOVE_EVENT_LISTENER: {
        const eventName = resolveWireName(value, ATTR_NAMES);
        const el = reg.expect(nodeId);
        const key = `${nodeId}:${eventName}`;
        const handler = this.listeners.get(key);
        if (handler) {
          el.removeEventListener(eventName, handler);
          this.listeners.delete(key);
        }
        break;
      }
      case Op.APPEND_CHILD:
        reg.expect(nodeId).appendChild(reg.expect(Number(attribute))); // attribute = child id
        break;
      case Op.REMOVE_CHILD:
        reg.expect(nodeId).removeChild(reg.expect(Number(attribute)));
        break;
      case Op.INSERT_BEFORE:
        reg
          .expect(nodeId)
          .insertBefore(reg.expect(Number(attribute)), reg.expect(Number(textVal))); // text_val = ref id
        break;
      case Op.REMOVE_NODE: {
        const node = reg.expect(nodeId);
        node.remove();
        reg.unregister(nodeId); // implicit unregister
        break;
      }
      case Op.REPLACE_NODE: {
        // attribute = new id. Implicitly unregisters old, registers new — the
        // replacement may still be staged (no REGISTER_NODE needed for it).
        const newId = Number(attribute);
        const oldNode = reg.expect(nodeId);
        const newNode = reg.expectOrStaged(newId);
        oldNode.replaceWith(newNode);
        reg.unregister(nodeId);
        reg.register(newId, newNode);
        break;
      }
      case Op.SET_STYLE:
        reg.expect(nodeId).style[resolveWireName(attribute, ATTR_NAMES)] = value;
        break;
      case Op.ADD_CLASS:
        reg.expect(nodeId).classList.add(value); // value = class
        break;
      case Op.REMOVE_CLASS:
        reg.expect(nodeId).classList.remove(value);
        break;
      case Op.MORPH_NODE:
        this.applyMorph(nodeId, attribute, textVal);
        break;
      case Op.REGISTER_NODE:
        reg.applyRegister(nodeId, this.document);
        break;
      case Op.UNREGISTER_NODE:
        reg.unregister(nodeId); // registry only — DOM untouched
        break;
      default:
        throw new Error(`DomOpApplicator: unknown operation ${op}`);
    }
  }

  /**
   * MORPH_NODE (op 16): `packed` is `"<action>:<kind>:<selector>"` (split only
   * the first two colons — CSS queries contain `:`); `nodeId` carries the
   * target for kind 0. Minimal application — full Datastar-style morphing with
   * state preservation is feature 07 (decision 027).
   */
  applyMorph(nodeId, packed, content) {
    const first = packed.indexOf(":");
    const second = packed.indexOf(":", first + 1);
    if (first < 0 || second < 0) {
      throw new Error(`DomOpApplicator: malformed morph packing \`${packed}\``);
    }
    const action = Number(packed.slice(0, first));
    const kind = packed.slice(first + 1, second);
    const selector = packed.slice(second + 1);

    let target;
    if (kind === "0") target = this.registry.expect(nodeId);
    else if (kind === "1") target = this.document.querySelector(`#${selector}`);
    else if (kind === "2") target = this.document.querySelector(`.${selector}`);
    else if (kind === "3") target = this.document.querySelector(selector);
    else throw new Error(`DomOpApplicator: unknown morph selector kind ${kind}`);
    if (!target) throw new Error(`DomOpApplicator: morph target not found (${kind}:${selector})`);

    switch (action) {
      case 0: // ReplaceChildren
        target.innerHTML = content;
        break;
      case 1: // ReplaceElement
        target.insertAdjacentHTML("afterend", content);
        target.remove();
        break;
      case 2: // InsertBefore
        target.insertAdjacentHTML("beforebegin", content);
        break;
      case 3: // InsertAfter
        target.insertAdjacentHTML("afterend", content);
        break;
      case 4: // AppendSibling — last child of the target's parent
        (target.parent ?? target.parentNode)?.insertAdjacentHTML("beforeend", content);
        break;
      default:
        throw new Error(`DomOpApplicator: unknown morph action ${action}`);
    }
  }
}

// ─── Protocol handler factory ────────────────────────────────────────────────

/**
 * Build a ProtocolDispatcher handler (`{ apply(memoryId, payload) }`) for
 * protocol byte 1 wire v1 (compact columnar): parse + apply to the DOM.
 * @param {DomOpApplicator} applicator
 */
export function columnarHandler(applicator) {
  return {
    apply(_memoryId, payload) {
      applicator.apply(ColumnarParser.parse(payload));
    },
  };
}

// ─── EventDispatcher (primal:on* → WASM callbacks) ───────────────────────────────

const PRIMAL_ON = "primal:on";

/**
 * Build the EventData object handed to a WASM callback (decision 018 / G4):
 * type, the element's primal-id, current value/checked, key code, and modifier keys.
 */
export function buildEventData(eventType, event, el) {
  return {
    type: eventType,
    primalId: el.getAttribute ? el.getAttribute("primal-id") : null,
    value: el.value ?? null,
    checked: el.checked ?? null,
    keyCode: event.keyCode ?? null,
    modifiers: {
      alt: !!event.altKey,
      ctrl: !!event.ctrlKey,
      shift: !!event.shiftKey,
      meta: !!event.metaKey,
    },
  };
}

/**
 * Parse a `primal:on*` attribute value into a WASM callback id.
 * Accepts `"7"` or `"callback-7"`. Returns `null` for non-callback refs (e.g. a
 * stimulus-style `"controller.delete"`), which a later increment will resolve as a
 * JS function ref.
 */
export function parseCallbackId(ref) {
  if (ref == null) return null;
  const s = String(ref).trim();
  const body = s.startsWith("callback-") ? s.slice("callback-".length) : s;
  return /^\d+$/.test(body) ? Number(body) : null;
}

/**
 * WHY: DOM events have to cross back into WASM. The event runtime wires
 * `primal:on{event}` attributes to direct listeners (decision 018 — direct binding
 * is the default, works for non-bubbling events too) that serialise EventData and
 * invoke the WASM callback.
 *
 * WHAT: scan/wire/unwire + programmatic helpers, with idempotent rewiring (G2).
 *
 * HOW: `deliver(callbackId, eventData)` is injected so the EventData *wire format*
 * (F08) stays swappable — see {@link callbackDeliver} for the default that ships it
 * through a `CallbackRegistry`.
 */
export class EventDispatcher {
  /** @param {(callbackId:number, eventData:object) => void} deliver */
  constructor(deliver) {
    this.deliver = deliver;
    // element -> Map<eventType, listenerFn>, so rewiring/cleanup is exact.
    this.listeners = new WeakMap();
  }

  /** Wire every `primal:on*` attribute on `root` and its descendants. */
  scanAndWire(root) {
    this.#visit(root, (el) => {
      for (const name of el.getAttributeNames()) {
        if (name.startsWith(PRIMAL_ON)) {
          this.wire(el, name.slice(PRIMAL_ON.length), el.getAttribute(name));
        }
      }
    });
  }

  #visit(node, fn) {
    if (typeof node.getAttributeNames === "function") fn(node);
    for (const child of node.children || []) this.#visit(child, fn);
  }

  /**
   * Wire one `eventType` on `el` to `handlerRef`. Idempotent (G2): removes any prior
   * listener for that event first, so re-scans (or MutationObserver re-fires) don't
   * stack duplicates.
   */
  wire(el, eventType, handlerRef) {
    this.off(el, eventType);
    const listener = (event) => {
      const callbackId = parseCallbackId(handlerRef);
      if (callbackId !== null) {
        this.deliver(callbackId, buildEventData(eventType, event, el));
      }
    };
    el.addEventListener(eventType, listener);
    let map = this.listeners.get(el);
    if (!map) {
      map = new Map();
      this.listeners.set(el, map);
    }
    map.set(eventType, listener);
  }

  /** Remove the listener for `eventType` on `el` (if any). */
  off(el, eventType) {
    const map = this.listeners.get(el);
    const listener = map?.get(eventType);
    if (listener) {
      el.removeEventListener(eventType, listener);
      map.delete(eventType);
    }
  }

  /** Remove all listeners on `el` and its descendants (cleanup on node removal). */
  removeListeners(root) {
    this.#visit(root, (el) => {
      const map = this.listeners.get(el);
      if (map) {
        for (const [eventType, listener] of map) el.removeEventListener(eventType, listener);
        this.listeners.delete(el);
      }
    });
  }
}

/**
 * Default `deliver` for {@link EventDispatcher}: serialise EventData and ship it to
 * the WASM callback via a `CallbackRegistry` (from foundation-wasm.js).
 *
 * `encode(eventData) -> Uint8Array` is injectable; the default is UTF-8 JSON. The
 * authoritative EventData wire format is defined by feature 08 (event-runtime) — this
 * keeps the seam swappable without changing the dispatcher.
 *
 * @param {{invoke:(id:number, bytes:Uint8Array)=>void}} callbackRegistry
 * @param {(eventData:object)=>Uint8Array} [encode]
 */
export function callbackDeliver(callbackRegistry, encode = jsonEncodeEventData) {
  return (callbackId, eventData) => callbackRegistry.invoke(callbackId, encode(eventData));
}

function jsonEncodeEventData(eventData) {
  return new TextEncoder().encode(JSON.stringify(eventData));
}

// ─── DomOps over the batch protocol (Custom Binary, byte 0) ────────────────────────

/**
 * The registered batch opcode carrying one DomOp row:
 * `[BATCH_OP_APPLY_DOM][ArgStart (operation, nodeId, attribute, value, textVal)
 * ArgStop][Operations.End]` — params quantized (V2), strings via the texts pool.
 * Mirrors `foundation_wasm_ui::BATCH_OP_APPLY_DOM` on the Rust side.
 */
export const BATCH_OP_APPLY_DOM = 10;

/**
 * Register the DomOp batch operation on a core runtime: byte-0 batch messages
 * carrying [`BATCH_OP_APPLY_DOM`] ops apply straight to the DOM through the same
 * applicator Arrow uses. This is the selective-opt-in pattern of the batch system
 * (`BatchInstructions.registerOperation`) — other processes register their own
 * opcodes the same way.
 *
 * @param {{batches:{registerOperation:Function, params:object}}} rt  core runtime
 * @param {DomOpApplicator} applicator
 */
export function registerDomBatchOperation(rt, applicator) {
  rt.batches.registerOperation(BATCH_OP_APPLY_DOM, (batch, _opId, i, view, texts) => {
    // Markers are the shared cross-language contract: ArgumentOperations.Start = 1,
    // Operations.End = 254 (foundation_wasm base.rs).
    if (view.getUint8(i) !== 1) {
      throw new Error(`dom batch op: expected ArgumentOperations.Start, got ${view.getUint8(i)}`);
    }
    i += 1;
    let args;
    [i, args] = batch.params.parseParams(view, i, texts);
    if (view.getUint8(i) !== 254) {
      throw new Error(`dom batch op: expected Operations.End, got ${view.getUint8(i)}`);
    }
    i += 1;
    const [operation, nodeId, attribute, value, textVal] = args;
    const thunk = () => {
      applicator.applyOne(operation, nodeId, attribute, value, textVal);
      return null;
    };
    return [i, thunk];
  });
  return rt;
}

// ─── DomHeap (DOM external-pointer arena, = megatron DOMArena) ────────────────────

/**
 * Generation-arena heap for DOM nodes referenced across the ABI by `ExternalPointer`
 * ids (uid = `(index << 32) | generation`, bigint — same scheme as the core runtime's
 * ExternalHeap; implemented here so this file stays import-free for the browser).
 *
 * Slots 0–4 are RESERVED at construction (megatron DOMArena parity):
 * 0 = self (or this heap when no `self`), 1 = the heap itself, 2 = window,
 * 3 = document, 4 = document.body. Reserved slots refuse `destroy`.
 */
export class DomHeap {
  static RESERVED_SLOTS = 5;

  /** @param {{window?:object, document?:object}} [host] overrides for tests/SSR */
  constructor(host = globalThis) {
    this.items = []; // { item, generation, active }
    this.free = [];
    const doc = host.document ?? null;
    this.create(typeof self !== "undefined" ? self : this);
    this.create(this);
    this.create(host.window ?? null);
    this.create(doc);
    this.create(doc && doc.body ? doc.body : null);
  }

  #unpack(uid) {
    const v = BigInt(uid);
    // The well-known DOM handles (Rust DOM_SELF..DOM_BODY) are the RAW values 0–4,
    // not packed uids. Reserved slots are never destroyed, so their generation stays
    // 0 and small raw values stay unambiguous (a packed index-n uid is n<<32).
    if (v < BigInt(DomHeap.RESERVED_SLOTS)) return { index: Number(v), generation: 0n };
    return { index: Number(v >> 32n), generation: v & 0xffffffffn };
  }

  /** Allocate a slot for `item` (may be null) → packed uid (bigint). */
  create(item) {
    let index;
    if (this.free.length > 0) {
      index = this.free.pop();
      const slot = this.items[index];
      slot.generation += 1n;
      slot.active = true;
      slot.item = item;
    } else {
      index = this.items.length;
      this.items.push({ item, generation: 0n, active: true });
    }
    return (BigInt(index) << 32n) | this.items[index].generation;
  }

  /** Resolve a uid → node (undefined when stale/missing). */
  get(uid) {
    const { index, generation } = this.#unpack(uid);
    const slot = this.items[index];
    if (!slot || !slot.active || slot.generation !== generation) return undefined;
    return slot.item;
  }

  /** Fill a pre-allocated uid. False when stale. */
  update(uid, item) {
    const { index, generation } = this.#unpack(uid);
    const slot = this.items[index];
    if (!slot || slot.generation !== generation) return false;
    slot.item = item;
    slot.active = true;
    return true;
  }

  /** Retire a uid (reserved slots 0–4 refuse, megatron parity). */
  destroy(uid) {
    const { index, generation } = this.#unpack(uid);
    if (index < DomHeap.RESERVED_SLOTS) return false;
    const slot = this.items[index];
    if (!slot || !slot.active || slot.generation !== generation) return false;
    slot.item = null;
    slot.active = false;
    this.free.push(index);
    return true;
  }
}

// ─── DOM ABI extension ─────────────────────────────────────────────────────────────

/**
 * Wire the DOM layer onto a core `FoundationWasm` runtime: creates the DomHeap,
 * registers it as the ReplyEncoder's DOM heap (DOMObject returns intern here), and
 * returns the DOM-specific import fragment to spread into the import object:
 *
 *   const rt = new FoundationWasm();
 *   const dom = new DomHeap();
 *   const abi = { ...rt.web_abi, ...domAbi(rt, dom) };
 *   const instance = new WebAssembly.Instance(module, { abi });
 *
 * @param {{functions:{reply:{dom:object}}}} rt  the core runtime
 * @param {DomHeap} dom
 * @returns {object} import fragment (`dom_allocate_external_pointer`, …)
 */
export function domAbi(rt, dom) {
  rt.functions.reply.dom = dom;
  return {
    // Pre-allocate an external-pointer slot earmarked for a DOM node
    // (foundation_wasm_ui's allocate_dom_reference).
    dom_allocate_external_pointer: () => dom.create(null),
    // Retire a DOM handle (reserved slots 0–4 refuse; stale ids no-op).
    host_dom_drop_external_pointer: (handle) => {
      dom.destroy(BigInt(handle));
    },
    // DOM fast-path: the returned node interns into the DOM heap; its handle
    // crosses naked (29 = ReturnTypeId.DOMObject, the shared contract value).
    host_invoke_function_as_dom: (h, p, l) => {
      const v = rt.functions.invokeNakedAs(h, p, l, 29);
      return typeof v === "bigint" ? v : BigInt(v);
    },
  };
}

// ─── Global registration ──────────────────────────────────────────────────────────
//
// The ESM exports above are canonical (`<script type="module">` / import). This
// mirror lets classic (non-module) scripts on the same page reach the DOM runtime as
// `globalThis.FoundationWasmUiRuntime` once the module has loaded.
globalThis.FoundationWasmUiRuntime = Object.freeze({
  Op,
  RESERVED,
  ColumnarParser,
  NodeRegistry,
  DomOpApplicator,
  columnarHandler,
  buildEventData,
  parseCallbackId,
  EventDispatcher,
  callbackDeliver,
  DomHeap,
  domAbi,
  BATCH_OP_APPLY_DOM,
  registerDomBatchOperation,
});
