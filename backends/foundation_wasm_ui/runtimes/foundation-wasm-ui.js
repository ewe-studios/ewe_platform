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
 * Decode the COMPACT COLUMNAR payload v1.1 (protocol byte 1, wire VERSION 1 —
 * the owned layout; wire version 2 is real Arrow IPC, server-side). Layout
 * (little-endian), mirroring `foundation_ui_traits::ColumnarBatch::serialize`:
 *
 *   [pad_len:u8][0x00 × pad_len]      // alignment shim
 *   [row_count:u32][flags:u32]        // 8-byte header — 8-ALIGNED by contract
 *   [op_id:    u32 × N]
 *   [node_id:  u32 × N]
 *   [operation:u8  × N][pad to 4]
 *   [attribute column][value column][text_val column]
 *
 * A column is `[(N+1) offsets:u32][data_len:u32][utf8 bytes][pad to 4]`.
 *
 * ZERO-COPY (feature 19): every producer pads so the header lands 8-aligned —
 * the pure encoder relative to the payload, the wasm framing layer against the
 * ABSOLUTE arena address. When that holds here (`byteOffset` math), the u32
 * columns become TRUE TypedArray views sharing the payload's buffer
 * (`zeroCopy: true`); otherwise we fall back to copying (correct everywhere).
 */
export class ColumnarParser {
  /**
   * @param {Uint8Array} payload
   * @returns {{ count:number, opIds:Uint32Array, nodeIds:Uint32Array,
   *            operations:Uint8Array, attribute:string[], value:string[],
   *            textVal:string[], zeroCopy:boolean }}
   */
  static parse(payload) {
    const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
    const padLen = view.getUint8(0);
    const headerAt = 1 + padLen;
    // Column alignment is relative to the header; the producers' shim makes it
    // absolute. Verify — views need absolute 4-alignment.
    const aligned = (payload.byteOffset + headerAt) % 4 === 0;

    let off = headerAt;
    const u32 = () => {
      const v = view.getUint32(off, true);
      off += 4;
      return v;
    };
    const pad4 = () => {
      off += (4 - ((off - headerAt) % 4)) % 4;
    };

    const count = u32();
    u32(); // flags (reserved — future cached-string demux)

    const u32Column = () => {
      let column;
      if (aligned) {
        // TRUE view — shares the payload's buffer, zero copies.
        column = new Uint32Array(payload.buffer, payload.byteOffset + off, count);
      } else {
        column = new Uint32Array(count);
        for (let i = 0; i < count; i++) column[i] = view.getUint32(off + i * 4, true);
      }
      off += count * 4;
      return column;
    };

    const opIds = u32Column();
    const nodeIds = u32Column();
    // u8 views have no alignment requirement — always zero-copy.
    const operations = new Uint8Array(payload.buffer, payload.byteOffset + off, count);
    off += count;
    pad4();

    const decoder = new TextDecoder();
    const readStringColumn = () => {
      const offsets = u32Column.call(null);
      // (count+1) offsets — u32Column read `count`; read the extra one.
      const last = u32();
      const dataLen = u32();
      const base = payload.byteOffset + off;
      const out = new Array(count);
      for (let i = 0; i < count; i++) {
        const end = i + 1 < count ? offsets[i + 1] : last;
        out[i] = decoder.decode(
          new Uint8Array(payload.buffer, base + offsets[i], end - offsets[i]),
        );
      }
      off += dataLen;
      pad4();
      return out;
    };

    const attribute = readStringColumn();
    const value = readStringColumn();
    const textVal = readStringColumn();

    return { count, opIds, nodeIds, operations, attribute, value, textVal, zeroCopy: aligned };
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

// ─── MorphDom (feature 07 — decision 027, Datastar-style morphing) ───────────

/**
 * WHY: Server/WASM HTML patches (MORPH_NODE, op 16) must update a live subtree
 * WITHOUT destroying user state — focus, form values, CSS animations, element
 * identity. Hard replacement (op 12) loses all of it; morphing reconciles.
 *
 * WHAT: A morphdom/idiomorph-style reconciler: persistent-ID tracking with
 * tag-mismatch and duplicate exclusion, bottom-up ID maps, best-match scanning
 * with pantry retrieval and equality-lookahead anti-churn, a pantry for parked
 * nodes (retrievable within the same morph), form-state preservation, script
 * re-execution with a WeakSet guard, and the `data-ignore-morph` /
 * `data-preserve-attr` escape hatches.
 *
 * HOW: All per-morph state lives on a fresh MorphContext (G31 — re-entrant
 * morphs can't corrupt each other); `cleanup()` runs in `finally` (G33 — the
 * pantry never leaks). Node access is duck-typed (tagName/tag,
 * children/childNodes, getAttribute…) so the algorithm runs identically on
 * the real DOM and the test mock.
 */

const morphIsElement = (n) => {
  if (!n) return false;
  if (n.nodeType !== undefined) return n.nodeType === 1; // real DOM
  // Mocks: text nodes carry tag "#text" but still expose attribute methods.
  return typeof n.getAttributeNames === "function" && (n.tag ?? "") !== "#text";
};
const morphTag = (n) => (n.tagName ?? n.tag ?? "").toUpperCase();
const morphKids = (n) => Array.from(n.childNodes ?? n.children ?? []);
const morphText = (n) => (n.nodeValue !== undefined && n.nodeValue !== null ? n.nodeValue : n.textContent);
const morphSetText = (n, v) => {
  if (n.nodeValue !== undefined && n.nodeValue !== null) n.nodeValue = v;
  else n.textContent = v;
};
const morphAttr = (n, name) => (n.getAttribute ? n.getAttribute(name) : null);

/** Walk every element in a subtree (root included), depth-first. */
function morphWalk(root, fn) {
  if (morphIsElement(root)) fn(root);
  for (const child of morphKids(root)) morphWalk(child, fn);
}

/**
 * `moveBefore` keeps focus/animations/lifecycle when the platform has it
 * (G32 — Chromium 115+, Firefox 125+, Safari TP 185); the fallback is
 * structurally correct but loses that state.
 */
export function moveBefore(parent, node, ref) {
  if (typeof parent.moveBefore === "function") parent.moveBefore(node, ref);
  else {
    node.parentNode?.removeChild?.(node);
    parent.insertBefore(node, ref);
  }
}

class MorphContext {
  constructor(doc) {
    this.doc = doc;
    this.idMap = new Map(); // Node -> Set<string> (persistent ids in subtree)
    this.persistentIds = new Set();
    this.oldIdTagMap = new Map(); // id -> tagName (old tree)
    this.duplicates = new Set();
    this.pantry = null; // created lazily on first park
  }

  // Phase 1 — ids that exist in BOTH trees with the SAME tag, no duplicates.
  computePersistentIds(oldRoot, newRoot) {
    morphWalk(oldRoot, (el) => {
      const id = morphAttr(el, "id");
      if (!id) return;
      if (this.oldIdTagMap.has(id)) this.duplicates.add(id);
      else this.oldIdTagMap.set(id, morphTag(el));
    });
    morphWalk(newRoot, (el) => {
      const id = morphAttr(el, "id");
      if (!id || this.duplicates.has(id)) return;
      if (this.oldIdTagMap.get(id) === morphTag(el)) this.persistentIds.add(id);
    });
  }

  // Phase 2 — bottom-up: each node -> the persistent ids inside its subtree.
  populateIdMap(root) {
    const build = (node) => {
      const ids = new Set();
      const own = morphIsElement(node) ? morphAttr(node, "id") : null;
      if (own && this.persistentIds.has(own)) ids.add(own);
      for (const child of morphKids(node)) {
        for (const id of build(child)) ids.add(id);
      }
      if (ids.size > 0) this.idMap.set(node, ids);
      return ids;
    };
    build(root);
  }

  hasConflictingId(node) {
    const id = morphIsElement(node) ? morphAttr(node, "id") : null;
    return !!id && !this.persistentIds.has(id);
  }

  // §3 — priority 1: ID-set intersection. The scan is UNBOUNDED over the
  // remaining siblings AND the pantry: an id match is an anchor, and anchors
  // are always worth moving for (the spec's own reorder test demands it; its
  // displacement-limit example is the newIds-EMPTY case, which never enters
  // this scan at all). Priority 2: soft match guarded by anti-churn.
  findBestMatch(oldCursor, newChild) {
    if (!morphIsElement(newChild)) {
      // Text/comment: soft-match a same-kind node at the cursor.
      return oldCursor && !morphIsElement(oldCursor) ? oldCursor : null;
    }
    const newIds = this.idMap.get(newChild) ?? new Set();

    if (newIds.size > 0) {
      const intersects = (candidate) => {
        if (
          !morphIsElement(candidate) ||
          morphTag(candidate) !== morphTag(newChild) ||
          !this.idMap.has(candidate)
        ) {
          return false;
        }
        const ids = this.idMap.get(candidate);
        for (const id of newIds) {
          if (ids.has(id)) return true;
        }
        return false;
      };
      for (let candidate = oldCursor; candidate; candidate = candidate.nextSibling) {
        if (intersects(candidate)) return candidate;
      }
      // Parked earlier in THIS morph — retrievable (§4).
      if (this.pantry) {
        for (const candidate of morphKids(this.pantry)) {
          if (intersects(candidate)) return candidate;
        }
      }
    }

    if (
      oldCursor &&
      morphIsElement(oldCursor) === morphIsElement(newChild) &&
      morphTag(oldCursor) === morphTag(newChild) &&
      !this.hasConflictingId(oldCursor)
    ) {
      // Anti-churn: the spec's future-sibling counter blocks EVERY element of
      // a homogeneous list (its own narrative contradicts it). The rule that
      // satisfies both spec examples is an equality lookahead: if the NEXT new
      // sibling is structurally equal to the cursor, `newChild` is an
      // INSERTION before it — create fresh instead of morphing the cursor
      // into its successor (prepend churn) or dragging anchors (displacement).
      if (typeof oldCursor.isEqualNode === "function") {
        if (oldCursor.isEqualNode(newChild)) return oldCursor;
        const nextNew = newChild.nextSibling ?? null;
        if (nextNew && oldCursor.isEqualNode(nextNew)) return null;
      }
      return oldCursor;
    }
    return null;
  }

  // §4 — park id-bearing nodes (retrievable this morph), drop the rest.
  removeNode(node) {
    if (this.idMap.has(node)) {
      if (!this.pantry) this.pantry = this.doc.createElement("div");
      moveBefore(this.pantry, node, null);
    } else {
      node.parentNode?.removeChild?.(node);
    }
  }

  morphChildren(oldParent, newParent) {
    let oldCursor = oldParent.firstChild ?? null;
    for (const newChild of morphKids(newParent)) {
      // Escape hatch: BOTH sides carry data-ignore-morph -> leave untouched.
      if (
        oldCursor &&
        morphAttr(oldCursor, "data-ignore-morph") !== null &&
        morphAttr(newChild, "data-ignore-morph") !== null
      ) {
        oldCursor = oldCursor.nextSibling;
        continue;
      }

      const match = this.findBestMatch(oldCursor, newChild);
      if (match) {
        // Park/remove everything between the cursor and the match.
        while (oldCursor && oldCursor !== match) {
          const next = oldCursor.nextSibling;
          this.removeNode(oldCursor);
          oldCursor = next;
        }
        if (match !== oldCursor) moveBefore(oldParent, match, oldCursor);
        this.morphNode(match, newChild);
        oldCursor = match.nextSibling;
      } else {
        const clone = cloneNode(this.doc, newChild);
        oldParent.insertBefore(clone, oldCursor);
      }
    }
    while (oldCursor) {
      const next = oldCursor.nextSibling;
      this.removeNode(oldCursor);
      oldCursor = next;
    }
  }

  morphNode(oldNode, newNode) {
    if (!morphIsElement(oldNode) || !morphIsElement(newNode)) {
      if (morphText(oldNode) !== morphText(newNode)) {
        morphSetText(oldNode, morphText(newNode));
      }
      return;
    }
    if (typeof oldNode.isEqualNode === "function" && oldNode.isEqualNode(newNode)) {
      return; // identical subtree — skip entirely
    }
    this.syncAttributes(oldNode, newNode);
    preserveFormState(oldNode, newNode);
    const tag = morphTag(oldNode);
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return; // leaves
    this.morphChildren(oldNode, newNode);
  }

  syncAttributes(oldEl, newEl) {
    const preserved = (morphAttr(oldEl, "data-preserve-attr") || "")
      .split(",")
      .map((sliver) => sliver.trim())
      .filter(Boolean);
    for (const name of newEl.getAttributeNames()) {
      if (!preserved.includes(name)) oldEl.setAttribute(name, newEl.getAttribute(name));
    }
    for (const name of oldEl.getAttributeNames()) {
      if (!preserved.includes(name) && newEl.getAttribute(name) === null) {
        oldEl.removeAttribute(name);
      }
    }
  }

  cleanup() {
    if (this.pantry) {
      while (this.pantry.firstChild) this.pantry.removeChild(this.pantry.firstChild);
    }
    this.idMap.clear();
    this.persistentIds.clear();
    this.oldIdTagMap.clear();
    this.duplicates.clear();
  }
}

/** §5 — keep what the USER did to form controls across the morph. */
function preserveFormState(oldEl, newEl) {
  if (morphTag(oldEl) !== morphTag(newEl)) return;
  switch (morphTag(oldEl)) {
    case "INPUT": {
      const type = morphAttr(oldEl, "type");
      if (type === "checkbox" || type === "radio") newEl.checked = oldEl.checked;
      else if (type !== "file") newEl.value = oldEl.value;
      break;
    }
    case "TEXTAREA":
      newEl.value = oldEl.value;
      break;
    case "SELECT":
      newEl.selectedIndex = oldEl.selectedIndex;
      break;
    default:
  }
}

/** Deep clone for newly created content (mock-aware). */
function cloneNode(doc, node) {
  if (typeof node.cloneNode === "function") return node.cloneNode(true);
  if (!morphIsElement(node)) return doc.createTextNode(morphText(node) ?? "");
  const el = doc.createElement(node.tag ?? node.tagName);
  for (const name of node.getAttributeNames()) el.setAttribute(name, node.getAttribute(name));
  if (node.value != null) el.value = node.value;
  if (node.checked != null) el.checked = node.checked;
  for (const child of morphKids(node)) el.appendChild(cloneNode(doc, child));
  return el;
}

export class MorphDom {
  /** Executed scripts — never re-run across morphs (browser concern). */
  static scripts = new WeakSet();

  /**
   * Morph `target`'s children to mirror `newContent`'s children.
   * @param {Element} target  live element
   * @param {Element|DocumentFragment} newContent  desired tree (its CHILDREN)
   * @param {Document} [doc]  owning document (defaults to target's)
   */
  static morph(
    target,
    newContent,
    doc = target.ownerDocument ?? (typeof document === "undefined" ? null : document),
  ) {
    const ctx = new MorphContext(doc);
    try {
      ctx.computePersistentIds(target, newContent);
      ctx.populateIdMap(target);
      ctx.populateIdMap(newContent);
      ctx.morphChildren(target, newContent);
      MorphDom.executeNewScripts(target, doc);
    } finally {
      ctx.cleanup(); // G33 — pantry never leaks, even on throw
    }
  }

  /** §7 — re-create injected <script>s so the browser executes them, once. */
  static executeNewScripts(root, doc) {
    if (!doc || typeof root.querySelectorAll !== "function") return; // browser-only
    for (const script of root.querySelectorAll("script")) {
      if (MorphDom.scripts.has(script)) continue;
      const clone = doc.createElement("script");
      for (const name of script.getAttributeNames()) {
        clone.setAttribute(name, script.getAttribute(name));
      }
      clone.textContent = script.textContent;
      script.parentNode?.replaceChild?.(clone, script);
      MorphDom.scripts.add(clone);
    }
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
      case 0: // ReplaceChildren — full morph (decision 027) when the document
        // can parse HTML; innerHTML fallback otherwise (test mocks).
        if (typeof this.document.createRange === "function") {
          const fragment = this.document.createRange().createContextualFragment(content);
          MorphDom.morph(target, fragment, this.document);
        } else {
          target.innerHTML = content;
        }
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
 * WHY: DOM events have to reach BOTH worlds: WASM (signal setters via
 * `primal:setter`, registry callbacks via `callback-N`) and plain JS handlers
 * (dot-path refs like `"controller.delete"`). Decision 018: direct binding is
 * the default (works for non-bubbling events); delegation is opt-in per
 * attribute.
 *
 * WHAT: The feature-08 event runtime — scan/wire/unwire with idempotent
 * rewiring (G2), dot-path resolution, opt-in delegation
 * (`primal:onclick:delegate="#container"`), MutationObserver auto-wiring with
 * the island boundary rule, microtask-batched removal cleanup, and the
 * programmatic `on`/`off`/`on<event>` API.
 *
 * HOW: `deliver(callbackId, eventData)` ships registry-callback events;
 * `deliverSignal(setterId, eventData)` ships signal-setter events (two id
 * NAMESPACES — see {@link callbackDeliver} / {@link signalDeliver}).
 * Handler resolution order per element/event:
 *   1. `handlerRef` is `"N"`/`"callback-N"`  → registry callback bridge
 *   2. element carries `primal:setter="N"`   → signal bridge (two-way binding)
 *   3. `handlerRef` is a dot-path             → JS function from `scope`
 *   4. otherwise                              → console.warn, no listener
 */
export class EventDispatcher {
  /**
   * @param {(callbackId:number, eventData:object) => void} deliver
   * @param {{ deliverSignal?:(setterId:number, eventData:object)=>void,
   *           scope?:object }} [options]
   */
  constructor(deliver, options = {}) {
    this.deliver = deliver;
    this.deliverSignal = options.deliverSignal ?? deliver;
    this.scope = options.scope ?? globalThis;
    // element -> Map<key, listenerFn>; keys are "click" (direct) or
    // "click:delegate:<primal-id>" (delegated, stored on the TARGET element).
    this.listeners = new WeakMap();
    // trackRemoved microtask batching (feature 08 §9).
    this.cleanupQueue = [];
    this.cleanupScheduled = false;
    this.observer = null;
  }

  /** Wire every `primal:on*` attribute on `root` and its descendants. */
  scanAndWire(root) {
    visit(root, (el) => {
      for (const name of el.getAttributeNames()) {
        if (!name.startsWith(PRIMAL_ON)) continue;
        const rest = name.slice(PRIMAL_ON.length); // "click" | "click:delegate"
        const [eventType, mode] = rest.split(":");
        if (!eventType) continue;
        if (mode === "delegate") {
          this.wireDelegated(el, eventType, el.getAttribute(name));
        } else if (mode === undefined) {
          this.wire(el, eventType, el.getAttribute(name));
        }
      }
    });
  }

  /**
   * Wire one direct `eventType` on `el` to `handlerRef` (resolution order in
   * the class docs). Idempotent (G2): the prior listener for that event is
   * removed first, so re-scans never stack duplicates.
   */
  wire(el, eventType, handlerRef) {
    const listener = this.#buildListener(el, eventType, handlerRef);
    if (!listener) {
      console.warn(`EventDispatcher: unresolvable handler "${handlerRef}" for ${eventType}`);
      return;
    }
    this.off(el, eventType);
    el.addEventListener(eventType, listener);
    this.#listenerMap(el).set(eventType, listener);
  }

  #buildListener(el, eventType, handlerRef) {
    const callbackId = parseCallbackId(handlerRef);
    if (callbackId !== null) {
      return (event) => this.deliver(callbackId, buildEventData(eventType, event, el));
    }
    const setterId = parseCallbackId(el.getAttribute?.("primal:setter"));
    if (setterId !== null) {
      return (event) => this.deliverSignal(setterId, buildEventData(eventType, event, el));
    }
    const fn = resolveFunctionRef(this.scope, handlerRef);
    if (fn) return fn.bind(el); // clean `this` = the attributed element
    return null;
  }

  /**
   * Opt-in delegation (feature 08 §6): attach a listener on the element named
   * by `selector` that stamps `event.delegateTarget = el` whenever the event
   * originated inside `el`. Keys include `el`'s primal-id so many elements can
   * delegate the same event type to one container without colliding.
   */
  wireDelegated(el, eventType, selector) {
    const target = resolveDelegateTarget(el, selector, this.documentOf(el));
    if (!target) {
      console.warn(`EventDispatcher: delegate target "${selector}" not found`);
      return;
    }
    const listener = (event) => {
      if (el === event.target || (el.contains && el.contains(event.target))) {
        event.delegateTarget = el;
      }
    };
    const key = `${eventType}:delegate:${el.getAttribute?.("primal-id") || el.id || ""}`;
    const map = this.#listenerMap(target);
    const prior = map.get(key);
    if (prior) target.removeEventListener(eventType, prior);
    map.set(key, listener);
    target.addEventListener(eventType, listener);
  }

  /** The document an element belongs to (overridable for tests/mocks). */
  documentOf(el) {
    return el.ownerDocument ?? (typeof document === "undefined" ? null : document);
  }

  #listenerMap(el) {
    let map = this.listeners.get(el);
    if (!map) {
      map = new Map();
      this.listeners.set(el, map);
    }
    return map;
  }

  /**
   * Remove listeners on `el`: a specific event type (matching its direct key
   * AND any delegated compound keys), or — with no `eventType` — everything.
   */
  off(el, eventType) {
    const map = this.listeners.get(el);
    if (!map) return;
    if (eventType !== undefined) {
      for (const [key, listener] of map) {
        if (key === eventType || key.startsWith(`${eventType}:`)) {
          el.removeEventListener(eventType, listener);
          map.delete(key);
        }
      }
      return;
    }
    for (const [key, listener] of map) {
      el.removeEventListener(key.split(":")[0], listener);
    }
    map.clear();
    this.listeners.delete(el);
  }

  /** Remove all listeners on `el` and its descendants (node-removal cleanup). */
  removeListeners(root) {
    visit(root, (el) => this.off(el));
  }

  /**
   * Queue a removed node for cleanup; one microtask drains the whole batch
   * (feature 08 §9 — removing 50 nodes costs one pass, before paint).
   */
  trackRemoved(node) {
    this.cleanupQueue.push(node);
    if (this.cleanupScheduled) return;
    this.cleanupScheduled = true;
    queueMicrotask(() => {
      const batch = this.cleanupQueue.splice(0);
      this.cleanupScheduled = false;
      for (const item of batch) this.removeListeners(item);
    });
  }

  /**
   * Process MutationObserver-style records (feature 08 §7): wire added
   * subtrees, clean removed ones — SKIPPING anything inside an `<island>`
   * (the island custom element owns its own lifecycle, F06).
   */
  handleMutations(mutations) {
    for (const mutation of mutations) {
      if (mutation.type !== "childList") continue;
      for (const node of mutation.addedNodes) {
        if (!isElement(node) || insideIsland(node)) continue;
        this.scanAndWire(node);
      }
      for (const node of mutation.removedNodes) {
        if (!isElement(node) || insideIsland(node)) continue;
        this.removeListeners(node);
      }
    }
  }

  /** Start the document-level observer (browser only; no-op without one). */
  observe(doc) {
    if (typeof MutationObserver === "undefined" || this.observer) return;
    this.observer = new MutationObserver((mutations) => this.handleMutations(mutations));
    this.observer.observe(doc, { subtree: true, childList: true });
  }

  /**
   * Programmatic wiring (feature 08 §11): `on(el, "click", "ctrl.fn")` or
   * `on(el, "click", null, { delegate: "#box" })`.
   */
  on(el, eventType, handlerRef, options = {}) {
    if (options.delegate) this.wireDelegated(el, eventType, options.delegate);
    else this.wire(el, eventType, handlerRef);
  }
}

/** Events that get `dispatcher.on<event>(el, ref, opts)` convenience methods. */
export const CONVENIENCE_EVENTS = [
  "click", "change", "submit", "keydown", "keyup",
  "focus", "blur", "scroll", "input", "mousedown", "mouseup",
];
for (const evt of CONVENIENCE_EVENTS) {
  EventDispatcher.prototype[`on${evt}`] = function (el, handlerRef, options) {
    this.on(el, evt, handlerRef, options);
  };
}

/** Depth-first walk over attribute-bearing nodes (root first). */
function visit(node, fn) {
  if (typeof node.getAttributeNames === "function") fn(node);
  for (const child of node.children || []) visit(child, fn);
}

function isElement(node) {
  return !!node && typeof node.getAttributeNames === "function";
}

/** The island boundary rule: nearest `<island>` ancestor-or-self opts out. */
function insideIsland(node) {
  if (typeof node.closest === "function") return node.closest("island") !== null;
  // Mock fallback: walk parents by tag.
  for (let cur = node; cur; cur = cur.parent ?? cur.parentElement ?? null) {
    if ((cur.tag ?? cur.tagName ?? "").toLowerCase() === "island") return true;
  }
  return false;
}

/**
 * Dot-path resolution against `scope` (feature 08 §4): `"controller.delete"`
 * walks `scope.controller.delete`; missing segments or non-functions → null.
 */
export function resolveFunctionRef(scope, handlerRef) {
  if (typeof handlerRef !== "string" || handlerRef.length === 0) return null;
  let current = scope;
  for (const part of handlerRef.split(".")) {
    if (current == null) return null;
    current = current[part];
  }
  return typeof current === "function" ? current : null;
}

/** Delegate target resolution (feature 08 §5). */
export function resolveDelegateTarget(el, selector, doc) {
  if (!selector) return null;
  if (selector === "parent") return el.parentElement ?? el.parent ?? null;
  if (selector === "body") return doc?.body ?? null;
  return doc?.querySelector ? doc.querySelector(selector) : null;
}

/**
 * Run the initial scan + observer once the document is ready (feature 08 §10).
 * Both paths are idempotent — rewiring replaces listeners (G2).
 */
export function initEventRuntime(dispatcher, doc = typeof document === "undefined" ? null : document) {
  if (!doc) return;
  const boot = () => {
    dispatcher.scanAndWire(doc.body);
    dispatcher.observe(doc);
  };
  doc.addEventListener?.("DOMContentLoaded", boot);
  if (doc.readyState !== "loading") boot();
}

/**
 * Default `deliver` for REGISTRY callbacks (`callback-N` refs): serialise
 * EventData and ship via a `CallbackRegistry` (foundation-wasm.js), which
 * writes an arena slot and calls the `invoke_callback` export.
 *
 * @param {{invoke:(id:number, bytes:Uint8Array)=>void}} callbackRegistry
 * @param {(eventData:object)=>Uint8Array} [encode]
 */
export function callbackDeliver(callbackRegistry, encode = jsonEncodeEventData) {
  return (callbackId, eventData) => callbackRegistry.invoke(callbackId, encode(eventData));
}

/**
 * `deliverSignal` for SIGNAL setters (`primal:setter` ids — the
 * foundation_signals registry, a separate namespace): write the JSON
 * EventData into a fresh global-arena slot and call the dedicated
 * `invoke_signal_callback(setterId, memoryId)` export. Rust reads, dispatches
 * to the setter, runs `stabilize()`, and frees the slot — no JS-side dispose.
 *
 * @param {{exports:object, memory:()=>WebAssembly.Memory}} bridge
 * @param {(eventData:object)=>Uint8Array} [encode]
 */
export function signalDeliver(bridge, encode = jsonEncodeEventData) {
  return (setterId, eventData) => {
    const bytes = encode(eventData);
    const memId = bridge.exports.create_allocation(BigInt(bytes.length));
    const ptr = Number(bridge.exports.allocation_start_pointer(memId));
    new Uint8Array(bridge.memory().buffer, ptr, bytes.length).set(bytes);
    bridge.exports.invoke_signal_callback(BigInt(setterId), memId);
  };
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
// ─── Web Components (feature 06 — decisions 021/022/023/024) ─────────────────
//
// Three thin custom elements over four shared layers: Transport (how bytes
// move), ProtocolHandler (what the bytes are, by content type), Patcher (how
// parsed results land in the DOM), Hydrator (post-insertion styles/scripts —
// events stay with the F08 EventDispatcher). Browser-only APIs (EventSource,
// WebSocket, customElements, CSSStyleSheet) are guarded so the logic runs and
// tests under node.

/** Exponential reconnect backoff: 1s/2s/4s… capped at 30s (spec §12). */
export function reconnectDelay(attempt) {
  return Math.min(1000 * 2 ** attempt, 30_000);
}

/**
 * Request bundling (G26/decision 026): when the server supports
 * `/primal/messages`, queued requests flush as ONE batch per microtask tick.
 */
export class RequestQueue {
  constructor(transport) {
    this.queue = [];
    this.scheduled = false;
    this.transport = transport;
    this.bundlingEnabled = false; // set after the HEAD /primal/messages probe
  }

  /** Probe once; enable bundling on 200 OK. */
  async probe(fetchFn = globalThis.fetch) {
    try {
      const res = await fetchFn("/primal/messages", { method: "HEAD" });
      this.bundlingEnabled = !!res && res.ok === true;
    } catch {
      this.bundlingEnabled = false;
    }
    return this.bundlingEnabled;
  }

  enqueue(request) {
    if (!this.bundlingEnabled) {
      return this.transport.send(request.url, request.method, request.data);
    }
    this.queue.push(request);
    if (!this.scheduled) {
      this.scheduled = true;
      queueMicrotask(() => this.flush());
    }
    return null;
  }

  flush() {
    this.scheduled = false;
    if (this.queue.length === 0) return;
    const batch = this.queue.splice(0);
    this.transport.send("/primal/messages", "POST", batch);
  }
}

// ─── Transport layer (G28: send for request-response, connect for streams) ────

export class FetchTransport {
  constructor(config = {}) {
    this.config = config;
    this.fetchFn = config.fetchFn ?? ((...args) => globalThis.fetch(...args));
  }

  async send(url, method = "POST", data = undefined) {
    const init = { method, headers: this.config.headers };
    if (data !== undefined && method !== "GET" && method !== "HEAD") {
      init.body = typeof data === "string" ? data : JSON.stringify(data);
    }
    const response = await this.fetchFn(url, init);
    if (!response.ok) throw new Error(`FetchTransport: ${method} ${url} -> ${response.status}`);
    const handler = ProtocolHandler.fromContentType(response);
    return handler.process(response);
  }

  disconnect() {}
}

export class SSETransport {
  constructor(config = {}) {
    this.config = config;
    this.eventSource = null;
    this.attempt = 0;
    this.closed = false;
  }

  connect(url, _data, onResult) {
    if (typeof EventSource === "undefined") {
      throw new Error("SSETransport: EventSource unavailable in this environment");
    }
    this.closed = false;
    this.url = url;
    this.onResult = onResult;
    this.eventSource = new EventSource(url);
    // G46: the HTTP content type is text/event-stream; each event's `event:`
    // field names the payload kind (html/arrow/json).
    for (const kind of ["html", "arrow", "json"]) {
      this.eventSource.addEventListener(kind, (event) => {
        onResult(streamEventResult(kind, event.data));
      });
    }
    this.eventSource.onmessage = (event) => onResult(streamEventResult("html", event.data));
    this.eventSource.onerror = () => this.reconnect();
  }

  reconnect() {
    if (this.closed) return;
    this.eventSource?.close?.();
    const delay = reconnectDelay(this.attempt);
    this.attempt += 1;
    setTimeout(() => {
      if (!this.closed) this.connect(this.url, undefined, this.onResult);
    }, delay);
  }

  disconnect() {
    this.closed = true;
    this.eventSource?.close?.();
    this.eventSource = null;
  }
}

export class WebSocketTransport {
  constructor(config = {}) {
    this.config = config;
    this.ws = null;
    this.attempt = 0;
    this.closed = false;
  }

  connect(url, _data, onResult) {
    if (typeof WebSocket === "undefined") {
      throw new Error("WebSocketTransport: WebSocket unavailable in this environment");
    }
    this.closed = false;
    this.url = url;
    this.onResult = onResult;
    this.ws = new WebSocket(url);
    this.ws.onmessage = (event) => onResult(streamEventResult("json", event.data));
    this.ws.onclose = () => {
      if (this.closed) return;
      const delay = reconnectDelay(this.attempt);
      this.attempt += 1;
      setTimeout(() => {
        if (!this.closed) this.connect(this.url, undefined, this.onResult);
      }, delay);
    };
  }

  send(data) {
    this.ws?.send(typeof data === "string" ? data : JSON.stringify(data));
  }

  disconnect() {
    this.closed = true;
    this.ws?.close?.();
    this.ws = null;
  }
}

export class ChunkedTransport {
  constructor(config = {}) {
    this.config = config;
    this.abort = null;
    this.fetchFn = config.fetchFn ?? ((...args) => globalThis.fetch(...args));
  }

  async connect(url, data, onChunk) {
    this.abort = new AbortController();
    const response = await this.fetchFn(url, {
      method: data === undefined ? "GET" : "POST",
      body: data === undefined ? undefined : JSON.stringify(data),
      signal: this.abort.signal,
    });
    const handler = ProtocolHandler.fromContentType(response);
    const reader = response.body.getReader();
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      onChunk(await handler.processChunk(value));
    }
  }

  disconnect() {
    this.abort?.abort?.();
    this.abort = null;
  }
}

export class WorkerTransport {
  constructor(config = {}) {
    this.worker = config.worker;
  }

  send(_url, _method, data) {
    this.worker.postMessage(data);
  }

  onMessage(callback) {
    this.worker.onmessage = (event) => callback(event.data);
  }

  disconnect() {
    this.worker?.terminate?.();
  }
}

export class Transport {
  /** Factory (spec §2): default is fetch. */
  static create(config = {}) {
    switch (config.transport) {
      case "sse":
        return new SSETransport(config);
      case "ws":
        return new WebSocketTransport(config);
      case "chunked":
        return new ChunkedTransport(config);
      case "worker":
        return new WorkerTransport(config);
      default:
        return new FetchTransport(config);
    }
  }
}

// ─── Protocol handlers (decision 022 content types) ───────────────────────────

function streamEventResult(kind, raw) {
  if (kind === "json") {
    try {
      return routeJson(JSON.parse(raw));
    } catch {
      return { type: "raw", text: String(raw) };
    }
  }
  if (kind === "arrow") return { type: "raw", text: String(raw) }; // SSE arrow is base64/firehose — F11 follow-up
  return routeHtml(String(raw));
}

/** JSON morph-wrapper detection (spec §3). */
function routeJson(value) {
  if (value && typeof value === "object" && value.morph) {
    return { type: "json-morph", morph: value.morph };
  }
  return { type: "json", patches: value };
}

/** HTML island-wrapper detection (spec §3). */
function routeHtml(html) {
  const match = /^\s*<island\b[^>]*data-target="([^"]+)"[^>]*data-action="([^"]+)"[^>]*>([\s\S]*)<\/island>\s*$/.exec(html);
  if (match) {
    return { type: "html-morph", target: match[1], action: match[2], content: match[3] };
  }
  return { type: "html", html };
}

export class ArrowHandler {
  async process(response) {
    const buffer = await response.arrayBuffer();
    return { type: "arrow", columns: ColumnarParser.parse(new Uint8Array(buffer)) };
  }
}

export class JsonHandler {
  async process(response) {
    return routeJson(await response.json());
  }
}

export class HtmlHandler {
  async process(response) {
    return routeHtml(await response.text());
  }
}

export class RawHandler {
  async process(response) {
    return { type: "raw", text: await response.text() };
  }
}

export class ProtocolHandler {
  /** Content-type table (spec §3, G46). */
  static fromContentType(response) {
    const ct = response.headers?.get?.("content-type") || "";
    if (ct.includes("primal-arrow") || ct.includes("event-stream-arrow")) return new ArrowHandler();
    if (ct.includes("primal-json") || ct.includes("event-stream-json")) return new JsonHandler();
    if (ct.includes("primal-html") || ct.includes("event-stream-html") || ct.includes("text/html")) {
      return new HtmlHandler();
    }
    return new RawHandler();
  }
}

// ─── Hydrator (styles + scripts ONLY — events are F08's, G2) ──────────────────

/**
 * Prefix every rule selector with `prefix` — `.title{...}` inside `#island-1`
 * becomes `#island-1 .title{...}`; compound selectors each get the prefix.
 * Pure function (the CSSStyleSheet adoption around it is browser-only).
 */
export function scopeCss(cssText, prefix) {
  return cssText.replace(/(^|\})\s*([^@{}][^{}]*)\{/g, (_, brace, selectors) => {
    const scoped = selectors
      .split(",")
      .map((sel) => `${prefix} ${sel.trim()}`)
      .join(", ");
    return `${brace}\n${scoped} {`;
  });
}

export class Hydrator {
  /** Scoped-script execution scope (spec §5). */
  static createScope(targetElement) {
    const eventCleanups = [];
    return {
      targets: () => targetElement.querySelectorAll("[primal-id]"),
      parent: () => targetElement,
      querySelector: (sel) => targetElement.querySelector(sel),
      querySelectorAll: (sel) => targetElement.querySelectorAll(sel),
      addEvent(sel, event, handler) {
        const el = targetElement.querySelector(sel);
        if (el) {
          el.addEventListener(event, handler);
          eventCleanups.push({ el, event, handler });
        }
      },
      cleanup() {
        for (const { el, event, handler } of eventCleanups) {
          el.removeEventListener(event, handler);
        }
        eventCleanups.length = 0;
      },
    };
  }

  /** Styles + scripts. NOT events (F08 owns primal:on*). Browser-leaning. */
  static hydrate(root, doc = root.ownerDocument ?? globalThis.document) {
    if (typeof root.querySelectorAll !== "function") return;
    // Styles: scope + adopt.
    for (const style of root.querySelectorAll("style[primal\\:style]")) {
      const prefix = root.id ? `#${root.id}` : `[primal-id="${root.getAttribute?.("primal-id") ?? ""}"]`;
      const scoped = scopeCss(style.textContent, prefix);
      if (typeof CSSStyleSheet === "function" && doc?.adoptedStyleSheets) {
        const sheet = new CSSStyleSheet();
        sheet.replaceSync(scoped);
        doc.adoptedStyleSheets = [...doc.adoptedStyleSheets, sheet];
        (root._sheets ??= []).push(sheet);
      }
      style.remove?.();
    }
    // Scripts: run with the scope object; per-script error isolation.
    for (const script of root.querySelectorAll("script[primal\\:script]")) {
      try {
        const fn = new Function("scope", script.textContent);
        fn(Hydrator.createScope(root));
      } catch (error) {
        console.error("Hydrator: scoped script failed", error);
      }
      script.remove?.();
    }
  }
}

// ─── Patcher ───────────────────────────────────────────────────────────────────

export class Patcher {
  /** Injectable seams: the F08 dispatcher, the DomOp applicator, signals. */
  static runtime = { dispatcher: null, applicator: null, signalBridge: null };

  static materialize(html, target, doc = target.ownerDocument ?? globalThis.document) {
    if (typeof doc?.createRange === "function") {
      const fragment = doc.createRange().createContextualFragment(html);
      target.appendChild(fragment);
    } else {
      target.innerHTML = html;
    }
    Hydrator.hydrate(target, doc);
    Patcher.runtime.dispatcher?.scanAndWire?.(target); // F08 (idempotent)
  }

  static applyDomOps(columns) {
    Patcher.runtime.applicator?.apply?.(columns);
  }

  static applySignalPatches(patches) {
    const bridge = Patcher.runtime.signalBridge;
    if (bridge?.applyPatches) bridge.applyPatches(patches);
    else console.warn("Patcher: no signalBridge installed; JSON patches dropped", patches);
  }

  /** Route one ProtocolHandler result to the DOM (shared by both mounts). */
  static route(result, targetEl, doc) {
    switch (result.type) {
      case "html":
        Patcher.materialize(result.html, targetEl, doc);
        break;
      case "html-morph": {
        const morphTarget = doc.querySelector(result.target);
        if (morphTarget && typeof doc.createRange === "function") {
          MorphDom.morph(morphTarget, doc.createRange().createContextualFragment(result.content), doc);
        }
        break;
      }
      case "json-morph": {
        const morphTarget = doc.querySelector(result.morph.target);
        if (morphTarget && typeof doc.createRange === "function") {
          MorphDom.morph(morphTarget, doc.createRange().createContextualFragment(result.morph.content), doc);
        }
        break;
      }
      case "json":
        Patcher.applySignalPatches(result.patches);
        break;
      case "arrow":
        Patcher.applyDomOps(result.columns);
        break;
      default:
        targetEl.textContent = result.text ?? "";
    }
  }
}

// ─── Response placement (spec §9, shared by both mounts) ──────────────────────

export function resolveMountTarget(element, target, doc) {
  if (!target) {
    // Self-replacement: the mount is a placeholder that disappears (G29).
    const container = doc.createElement("div");
    element.parentNode?.replaceChild?.(container, element);
    return container;
  }
  if (target === "parent") {
    const parent = element.parentElement ?? element.parent ?? null;
    if (!parent) throw new Error("mount target not found: parent");
    clearChildren(parent);
    return parent;
  }
  const found = doc.querySelector(target);
  if (!found) throw new Error(`mount target not found: ${target}`);
  clearChildren(found);
  return found;
}

function clearChildren(el) {
  while (el.firstChild) el.removeChild(el.firstChild);
}

// ─── Custom elements (browser base class guarded for node) ─────────────────────

const BaseElement = typeof HTMLElement === "undefined" ? class {} : HTMLElement;

/** `<primal-island>` — no network; styles + scripts + F08 event wiring. */
export class IslandComponent extends BaseElement {
  connectedCallback() {
    Hydrator.hydrate(this);
    Patcher.runtime.dispatcher?.scanAndWire?.(this);
  }

  disconnectedCallback() {
    const doc = this.ownerDocument ?? globalThis.document;
    if (this._sheets?.length && doc?.adoptedStyleSheets) {
      doc.adoptedStyleSheets = doc.adoptedStyleSheets.filter((s) => !this._sheets.includes(s));
    }
    this._sheets = [];
    Patcher.runtime.dispatcher?.removeListeners?.(this);
  }
}

/** Shared mount bootstrap: read attrs, build transport, resolve target. */
function mountSetup(element, defaults = {}) {
  const api = element.getAttribute("api");
  if (!api) throw new Error(`${element.tagName?.toLowerCase() ?? "mount"}: missing api attribute`);
  let data = {};
  try {
    data = JSON.parse(element.getAttribute("data") || "{}");
  } catch (error) {
    console.error("mount: invalid JSON in data attribute", error);
  }
  const transport =
    element._transportOverride ??
    Transport.create({ transport: element.getAttribute("transport") ?? defaults.transport, url: api });
  return { api, data, transport, target: element.getAttribute("target") };
}

/** `<mount-data>` — one request, one response (spec §7). */
export class MountDataComponent extends BaseElement {
  async connectedCallback() {
    const doc = this.ownerDocument ?? globalThis.document;
    try {
      const { api, data, transport, target } = mountSetup(this);
      this._transport = transport;
      const method = this.getAttribute("method") || "POST";
      const result = await transport.send(api, method, data);
      const targetEl = resolveMountTarget(this, target, doc);
      Patcher.route(result, targetEl, doc);
    } catch (error) {
      console.error("mount-data:", error);
    }
  }

  disconnectedCallback() {
    this._transport?.disconnect?.();
  }
}

/** `<mount-stream>` — continuous results until disconnect (spec §8). */
export class MountStreamComponent extends BaseElement {
  connectedCallback() {
    const doc = this.ownerDocument ?? globalThis.document;
    try {
      const { api, data, transport, target } = mountSetup(this, { transport: "sse" });
      this._transport = transport;
      const targetEl = resolveMountTarget(this, target, doc);
      transport.connect(api, data, (result) => Patcher.route(result, targetEl, doc));
    } catch (error) {
      console.error("mount-stream:", error);
    }
  }

  disconnectedCallback() {
    this._transport?.disconnect?.();
    this._transport = null;
  }
}

/** Register the custom elements (browser only; idempotent). */
export function registerWebComponents() {
  if (typeof customElements === "undefined") return;
  if (!customElements.get("primal-island")) customElements.define("primal-island", IslandComponent);
  if (!customElements.get("mount-data")) customElements.define("mount-data", MountDataComponent);
  if (!customElements.get("mount-stream")) customElements.define("mount-stream", MountStreamComponent);
}

/** Build the `window.primal` namespace (G30) over injected runtime seams. */
export function createPrimal({ dispatcher, doc = globalThis.document } = {}) {
  return {
    mountData(api, data, targetNode, opts = {}) {
      const transport = Transport.create({ transport: opts.transport, url: api });
      return transport
        .send(api, opts.method || "POST", data)
        .then((result) => Patcher.route(result, targetNode, doc));
    },
    mountStream(api, data, targetNode, opts = {}) {
      const transport = Transport.create({ transport: opts.transport ?? "sse", url: api });
      transport.connect(api, data, (result) => Patcher.route(result, targetNode, doc));
      return transport;
    },
    unmount(element) {
      element.disconnectedCallback?.();
      element.remove?.();
    },
    on: (selector, event, handler) => doc.querySelector(selector)?.addEventListener(event, handler),
    onclick(selector, handler) {
      this.on(selector, "click", handler);
    },
    onchange(selector, handler) {
      this.on(selector, "change", handler);
    },
    off: (selector, event, handler) => doc.querySelector(selector)?.removeEventListener(event, handler),
    scope: (element) => Hydrator.createScope(element),
    dispatcher,
    Transport,
    ProtocolHandler,
    Patcher,
    Hydrator,
  };
}

if (typeof window !== "undefined") {
  registerWebComponents();
  window.primal ??= createPrimal({});
}

// mirror lets classic (non-module) scripts on the same page reach the DOM runtime as
// `globalThis.FoundationWasmUiRuntime` once the module has loaded.
globalThis.FoundationWasmUiRuntime = Object.freeze({
  Op,
  RESERVED,
  ColumnarParser,
  NodeRegistry,
  DomOpApplicator,
  columnarHandler,
  MorphDom,
  moveBefore,
  buildEventData,
  parseCallbackId,
  EventDispatcher,
  CONVENIENCE_EVENTS,
  callbackDeliver,
  signalDeliver,
  resolveFunctionRef,
  resolveDelegateTarget,
  initEventRuntime,
  Transport,
  ProtocolHandler,
  Patcher,
  Hydrator,
  scopeCss,
  RequestQueue,
  reconnectDelay,
  resolveMountTarget,
  IslandComponent,
  MountDataComponent,
  MountStreamComponent,
  registerWebComponents,
  createPrimal,
  DomHeap,
  domAbi,
  BATCH_OP_APPLY_DOM,
  registerDomBatchOperation,
});
