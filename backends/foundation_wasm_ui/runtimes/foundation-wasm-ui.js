// foundation-wasm-ui.js — DOM layer of the runtime (built on foundation-wasm.js).
//
// WHY: foundation-wasm.js is the pure ABI (memory, transport, timers). The DOM half
// lives here so the ABI stays reusable by non-DOM hosts — mirroring the
// foundation_wasm / foundation_wasm_ui crate split.
//
// WHAT: the first DOM increment —
//   - ArrowParser        : decodes the Arrow columnar payload (the exact layout
//                          `foundation_ui_traits::ArrowEncoder` produces)
//   - NodeRegistry       : primal-id (u32) → DOM node
//   - ArrowDomApplicator : applies a parsed batch to a DOM `document`
//   - arrowHandler()     : a ProtocolDispatcher handler that ties parse → apply
//
// HOW: register `arrowHandler(...)` on a FoundationWasm dispatcher for protocol byte
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
  APPEND_CHILD: 8,
  REMOVE_NODE: 10,
  INSERT_BEFORE: 11,
  REPLACE_NODE: 12,
  SET_STYLE: 13,
  ADD_CLASS: 14,
  REMOVE_CLASS: 15,
  MORPH_NODE: 16,
});

// Reserved node ids (gap inventory G1): 0=<head>, 1=<body>, 2=<html>. Macro ids ≥ 1000.
export const RESERVED = Object.freeze({ HEAD: 0, BODY: 1, HTML: 2 });

// ─── ArrowParser ─────────────────────────────────────────────────────────────

/**
 * Decode the Arrow-inspired columnar payload into per-column arrays. Layout
 * (little-endian), mirroring `foundation_ui_traits::ArrowEncoder::encode`:
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
export class ArrowParser {
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

  get size() {
    return this.nodes.size;
  }
}

// ─── ArrowDomApplicator ──────────────────────────────────────────────────────

/**
 * Applies a parsed Arrow batch to a DOM. `document` must provide `createElement`,
 * `createTextNode`; nodes must provide the usual mutation API. Designed so a minimal
 * stub (tests) or a real `document` both work.
 */
export class ArrowDomApplicator {
  /** @param {NodeRegistry} registry @param {Document} document */
  constructor(registry, document) {
    this.registry = registry;
    this.document = document;
  }

  /** Apply a full batch (output of {@link ArrowParser.parse}). */
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
        const el = this.document.createElement(attribute); // attribute = tag
        if (value) el.className = value; // value = class
        reg.register(nodeId, el);
        break;
      }
      case Op.CREATE_TEXT_NODE: {
        reg.register(nodeId, this.document.createTextNode(textVal));
        break;
      }
      case Op.SET_TEXT_CONTENT:
        reg.expect(nodeId).textContent = textVal;
        break;
      case Op.SET_ATTRIBUTE:
        reg.expect(nodeId).setAttribute(attribute, value);
        break;
      case Op.REMOVE_ATTRIBUTE:
        reg.expect(nodeId).removeAttribute(attribute);
        break;
      case Op.APPEND_CHILD:
        reg.expect(nodeId).appendChild(reg.expect(Number(attribute))); // attribute = child id
        break;
      case Op.INSERT_BEFORE:
        reg
          .expect(nodeId)
          .insertBefore(reg.expect(Number(attribute)), reg.expect(Number(textVal))); // text_val = ref id
        break;
      case Op.REMOVE_NODE: {
        const node = reg.expect(nodeId);
        node.remove();
        reg.unregister(nodeId);
        break;
      }
      case Op.REPLACE_NODE: {
        const oldNode = reg.expect(nodeId);
        oldNode.replaceWith(reg.expect(Number(value))); // value = new id
        reg.unregister(nodeId);
        break;
      }
      case Op.SET_STYLE:
        reg.expect(nodeId).style[attribute] = value; // attribute = prop
        break;
      case Op.ADD_CLASS:
        reg.expect(nodeId).classList.add(value); // value = class
        break;
      case Op.REMOVE_CLASS:
        reg.expect(nodeId).classList.remove(value);
        break;
      case Op.MORPH_NODE:
        // Full morph (decision 027) is a later increment; minimal fallback for now.
        reg.expect(nodeId).innerHTML = textVal;
        break;
      default:
        throw new Error(`ArrowDomApplicator: unknown operation ${op}`);
    }
  }
}

// ─── Protocol handler factory ────────────────────────────────────────────────

/**
 * Build a ProtocolDispatcher handler (`{ apply(memoryId, payload) }`) for protocol
 * byte 1 (Arrow): parse the payload and apply it to the DOM.
 * @param {ArrowDomApplicator} applicator
 */
export function arrowHandler(applicator) {
  return {
    apply(_memoryId, payload) {
      applicator.apply(ArrowParser.parse(payload));
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
