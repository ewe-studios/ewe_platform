// Browser document registry, events, transport and component lifecycle.

// Reserved node ids (gap inventory G1): 0=<head>, 1=<body>, 2=<html>. Macro ids ≥ 1000.
export const RESERVED = Object.freeze({ HEAD: 0, BODY: 1, HTML: 2 });

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
// Morph identity: `id` wins, else `primal-id` (spec-42 feature 00 §4 — slot
// spans and template nodes carry primal-id; server re-renders of the same
// template have STABLE primal-ids, so they anchor preservation exactly like
// user ids do).
const morphIdentity = (n) => morphAttr(n, "id") ?? morphAttr(n, "primal-id");

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
      const id = morphIdentity(el);
      if (!id) return;
      if (this.oldIdTagMap.has(id)) this.duplicates.add(id);
      else this.oldIdTagMap.set(id, morphTag(el));
    });
    morphWalk(newRoot, (el) => {
      const id = morphIdentity(el);
      if (!id || this.duplicates.has(id)) return;
      if (this.oldIdTagMap.get(id) === morphTag(el)) this.persistentIds.add(id);
    });
  }

  // Phase 2 — bottom-up: each node -> the persistent ids inside its subtree.
  populateIdMap(root) {
    const build = (node) => {
      const ids = new Set();
      const own = morphIsElement(node) ? morphIdentity(node) : null;
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
    const id = morphIsElement(node) ? morphIdentity(node) : null;
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
  static actionHook = null;
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
    if (/^(server|host):/.test(String(handlerRef))) {
      return event => {
        if (EventDispatcher.actionHook) return EventDispatcher.actionHook(String(handlerRef), el, event, eventType);
        console.warn("No action runtime for", handlerRef);
      };
    }
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
        // Hydrate scoped styles/scripts on runtime-inserted subtrees (machinery
        // delivery, spec-42 feature 05 §M-delivery). Idempotent: hydrate runs
        // each scoped script once then removes it. Islands self-hydrate via
        // connectedCallback, so they're skipped above.
        Hydrator.hydrate(node);
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
    const memory = typeof bridge.memory === "function" ? bridge.memory() : bridge.memory;
    new Uint8Array(memory.buffer, ptr, bytes.length).set(bytes);
    bridge.exports.invoke_signal_callback(BigInt(setterId), memId);
  };
}

function jsonEncodeEventData(eventData) {
  return new TextEncoder().encode(JSON.stringify(eventData));
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

/** Probe `HEAD /primal/messages` (feature 11 §2): 200 = server batches. */
export async function probeBatching(fetchFn = globalThis.fetch) {
  try {
    const res = await fetchFn("/primal/messages", { method: "HEAD" });
    return !!res && res.ok === true;
  } catch {
    return false;
  }
}

/**
 * HTTP request bundling (feature 11 / decision 026): when the server supports
 * `/primal/messages`, queued requests flush as ONE id'd batch per microtask
 * tick (`[{id, url, method, headers, body}]`); a lone request skips the
 * wrapper and goes out as itself.
 */
export class RequestQueue {
  constructor(transport) {
    this.queue = [];
    this.scheduled = false;
    this.transport = transport;
    this.bundlingEnabled = false; // set after the probe
    this.nextId = 1;
  }

  /** Probe once; enable bundling on 200 OK. */
  async probe(fetchFn = globalThis.fetch) {
    this.bundlingEnabled = await probeBatching(fetchFn);
    return this.bundlingEnabled;
  }

  enqueue(request) {
    if (!this.bundlingEnabled) {
      return this.transport.send(request.url, request.method, request.data);
    }
    this.queue.push({
      id: this.nextId++,
      url: request.url,
      method: request.method ?? "GET",
      headers: request.headers,
      body: request.data,
    });
    if (!this.scheduled) {
      this.scheduled = true;
      queueMicrotask(() => this.flush());
    }
    return null;
  }

  flush() {
    this.scheduled = false;
    if (this.queue.length === 0) return; // no network call (test 7)
    const batch = this.queue.splice(0);
    if (batch.length === 1) {
      // Single request: as-is, no batch wrapper (test 5).
      const only = batch[0];
      this.transport.send(only.url, only.method, only.body);
      return;
    }
    this.transport.send("/primal/messages", "POST", batch);
  }
}

/**
 * Parse one batched-response entry (`{id, status, headers, body}`) per its
 * declared content type and produce the same result objects the protocol
 * handlers emit — so `Patcher.route` works on batch members too (§5).
 */
export function parseBatchEntry(entry) {
  const ct = entry.headers?.["content-type"] ?? entry.headers?.["Content-Type"] ?? "";
  if (ct.includes("primal-json")) return { id: entry.id, ...routeJson(entry.body) };
  if (ct.includes("primal-html") || ct.includes("text/html")) {
    return { id: entry.id, ...routeHtml(String(entry.body)) };
  }
  if (ct.includes("primal-arrow")) {
    return {id:entry.id, ...removedArrow()};
  }
  return { id: entry.id, type: "raw", text: String(entry.body ?? "") };
}

/**
 * WebSocket frame coalescing (feature 11 §6): one frame per microtask; a
 * single message ships unwrapped, several wrap as `{ batch: [...] }`.
 */
export class WSBatchQueue {
  constructor(ws) {
    this.ws = ws;
    this.queue = [];
    this.scheduled = false;
  }

  send(data) {
    this.queue.push(data);
    if (!this.scheduled) {
      this.scheduled = true;
      queueMicrotask(() => this.flush());
    }
  }

  flush() {
    this.scheduled = false;
    if (this.queue.length === 1) {
      this.ws.send(JSON.stringify(this.queue.splice(0, 1)[0]));
    } else if (this.queue.length > 1) {
      this.ws.send(JSON.stringify({ batch: this.queue.splice(0) }));
    }
  }
}

/**
 * Worker postMessage coalescing (feature 11 §7): one structured-clone
 * transfer per microtask, transferables forwarded rather than copied.
 */
export class WorkerBatchQueue {
  constructor(worker) {
    this.worker = worker;
    this.queue = [];
    this.scheduled = false;
  }

  postMessage(data) {
    this.queue.push(data);
    if (!this.scheduled) {
      this.scheduled = true;
      queueMicrotask(() => this.flush());
    }
  }

  flush() {
    this.scheduled = false;
    if (this.queue.length === 0) return;
    const batch = this.queue.splice(0);
    const transfers = batch.map((d) => d?.transferable).filter(Boolean);
    this.worker.postMessage({ batch }, transfers);
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
    const handler =
      ProtocolHandler.named(this.config.protocol) ?? ProtocolHandler.fromContentType(response);
    return handler.process(response);
  }

  disconnect() {}
}

// ─── Owned SSE: parser + fetch-based EventSource (feature 21) ─────────────────
//
// The browser EventSource API is GET-only — no bodies, no custom verbs, no
// headers. Datastar's answer (and ours): fetch() the stream with ANY method
// and parse the SSE protocol from the response ReadableStream. The parser
// MIRRORS foundation_netio's Rust SseParser semantics exactly (id rejects
// NUL, one leading value space stripped, multi-line data joined with \n,
// comments surfaced, empty line dispatches only with data, EOF flushes,
// no-colon lines ignored), plus the byte-stream concerns the Rust reader
// doesn't face here: UTF-8 sequences and CRLF pairs split across chunks, and
// partial trailing lines buffered until their terminator arrives.

export class SseParser {
  constructor() {
    this.decoder = new TextDecoder("utf-8"); // {stream:true} handles split runes
    this.textBuffer = "";
    this.sawCarriageReturn = false; // CRLF split across chunks
    this.lastEventId = null;
    this.#resetBuilder();
  }

  #resetBuilder() {
    this.id = null;
    this.eventType = null;
    this.data = [];
    this.retry = null;
  }

  /** Feed one byte chunk; returns the COMPLETE events it finished. */
  push(chunk) {
    let text = this.decoder.decode(chunk, { stream: true });
    // A \r at a previous chunk's end: swallow a leading \n (split CRLF).
    if (this.sawCarriageReturn) {
      this.sawCarriageReturn = false;
      if (text.startsWith("\n")) text = text.slice(1);
    }
    if (text.endsWith("\r")) {
      this.sawCarriageReturn = true;
    }
    this.textBuffer += text;

    const events = [];
    for (;;) {
      const cut = this.#nextLineEnd();
      if (cut === null) break;
      const [line, rest] = cut;
      this.textBuffer = rest;
      const event = this.#processLine(line);
      if (event) events.push(event);
    }
    return events;
  }

  /** EOF: flush any accumulated data as a final event (Rust parity). */
  end() {
    const tail = this.decoder.decode(); // flush a dangling partial rune
    if (tail) this.textBuffer += tail;
    const events = [];
    if (this.textBuffer.length > 0) {
      const event = this.#processLine(this.textBuffer.replace(/\r$/, ""));
      this.textBuffer = "";
      if (event) events.push(event);
    }
    const flushed = this.#dispatch();
    if (flushed) events.push(flushed);
    return events;
  }

  /** Find the next complete line, honoring \r\n, \n, and lone \r. */
  #nextLineEnd() {
    const buf = this.textBuffer;
    for (let i = 0; i < buf.length; i++) {
      const ch = buf[i];
      if (ch === "\n") return [buf.slice(0, i), buf.slice(i + 1)];
      if (ch === "\r") {
        if (i + 1 < buf.length) {
          const skip = buf[i + 1] === "\n" ? 2 : 1;
          return [buf.slice(0, i), buf.slice(i + skip)];
        }
        return null; // lone \r at buffer end — wait for the next chunk
      }
    }
    return null;
  }

  #processLine(line) {
    if (line.length === 0) {
      return this.#dispatch(); // empty line — dispatch IF data accumulated
    }
    if (line.startsWith(":")) {
      // Comments surface immediately (Rust parity).
      return { type: "comment", comment: line.slice(1).replace(/^ /, "").trimStart(), lastEventId: this.lastEventId };
    }
    const colon = line.indexOf(":");
    if (colon === -1) return null; // no-colon lines ignored (Rust parity)
    const field = line.slice(0, colon);
    let value = line.slice(colon + 1);
    if (value.startsWith(" ")) value = value.slice(1); // exactly ONE space

    switch (field) {
      case "id":
        if (!value.includes("\0")) this.id = value;
        break;
      case "event":
        this.eventType = value;
        break;
      case "data":
        this.data.push(value);
        break;
      case "retry": {
        const ms = Number(value);
        if (Number.isInteger(ms) && ms >= 0 && /^\d+$/.test(value)) this.retry = ms;
        break;
      }
      default: // unknown fields ignored
    }
    return null;
  }

  #dispatch() {
    if (this.data.length === 0) {
      this.#resetBuilder(); // reset even without an event (Rust parity)
      return null;
    }
    if (this.id !== null) this.lastEventId = this.id;
    const event = {
      type: "message",
      event: this.eventType,
      data: this.data.join("\n"),
      id: this.id,
      lastEventId: this.lastEventId,
      retry: this.retry,
    };
    this.#resetBuilder();
    return event;
  }
}

/**
 * EventSource over fetch (feature 21): ANY method, headers, body; streams the
 * response through {@link SseParser}; reconnects (F06 backoff, server
 * `retry:` override, `Last-Event-ID` carried) until {@link FetchEventSource#close}.
 */
export class FetchEventSource {
  /**
   * @param {string} url
   * @param {{ method?:string, headers?:object, body?:any, fetchFn?:Function,
   *           onOpen?:Function, onEvent?:Function, onComment?:Function,
   *           onError?:Function }} [options]
   */
  constructor(url, options = {}) {
    this.url = url;
    this.options = options;
    this.fetchFn = options.fetchFn ?? ((...args) => globalThis.fetch(...args));
    this.closed = false;
    this.attempt = 0;
    this.retryOverride = null; // server `retry:` field
    this.lastEventId = null;
    this.abort = null;
  }

  /** Open the stream (returns when the FIRST connection attempt settles). */
  async connect() {
    this.closed = false;
    await this.#attemptOnce();
  }

  async #attemptOnce() {
    if (this.closed) return;
    this.abort = typeof AbortController === "undefined" ? null : new AbortController();
    const headers = {
      accept: "text/event-stream",
      ...(this.options.headers ?? {}),
    };
    if (this.lastEventId !== null) headers["last-event-id"] = this.lastEventId;

    let response;
    try {
      response = await this.fetchFn(this.options.nextUrl?.(this.url) ?? this.url, {
        method: this.options.method ?? "GET",
        headers,
        body: this.options.body,
        signal: this.abort?.signal,
      });
      if (!response.ok) throw new Error(`FetchEventSource: ${response.status}`);
    } catch (error) {
      this.options.onError?.(error);
      this.#scheduleReconnect();
      return;
    }

    this.options.onOpen?.(response);
    this.attempt = 0; // a successful open resets the backoff
    const parser = new SseParser();
    const reader = response.body.getReader();
    try {
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        for (const event of parser.push(value)) this.#deliver(event);
      }
      for (const event of parser.end()) this.#deliver(event);
    } catch (error) {
      if (!this.closed) this.options.onError?.(error);
    }
    this.lastEventId = parser.lastEventId ?? this.lastEventId;
    // Stream ended — SSE semantics: reconnect unless closed.
    this.#scheduleReconnect();
  }

  #deliver(event) {
    if (event.type === "comment") {
      this.options.onComment?.(event);
      return;
    }
    if (event.retry !== null && event.retry !== undefined) {
      this.retryOverride = event.retry;
    }
    if (event.id !== null) this.lastEventId = event.id;
    this.options.onEvent?.(event);
  }

  #scheduleReconnect() {
    if (this.closed) return;
    const delay = this.retryOverride ?? reconnectDelay(this.attempt);
    this.attempt += 1;
    this.reconnectTimer = setTimeout(() => {
      if (!this.closed) this.#attemptOnce();
    }, delay);
  }

  /** Stop: abort the in-flight fetch and cancel reconnection. */
  close() {
    this.closed = true;
    clearTimeout(this.reconnectTimer);
    this.abort?.abort?.();
  }
}

export class SSETransport {
  constructor(config = {}) {
    this.config = config;
    this.source = null;
  }

  /**
   * Open an SSE stream with ANY method (feature 21 — the browser EventSource
   * is GET-only; this rides {@link FetchEventSource}). `config.method`
   * defaults to POST when `data` is given, GET otherwise.
   */
  connect(url, data, onResult) {
    const method = this.config.method ?? (data === undefined ? "GET" : "POST");
    this.source = new FetchEventSource(url, {
      method,
      headers: {
        ...(data === undefined ? {} : { "content-type": "application/json" }),
        ...(this.config.headers ?? {}),
      },
      body: data === undefined ? undefined : JSON.stringify(data),
      fetchFn: this.config.fetchFn,
      // G46: the `event:` field names the payload kind; unnamed = html.
      // The mount `protocol` attribute overrides per feature-04 precedence.
      onEvent: (event) =>
        onResult(streamEventResult(this.config.protocol ?? event.event ?? "html", event.data)),
      onError: this.config.onError,
      nextUrl: this.config.nextUrl,
    });
    return this.source.connect();
  }

  disconnect() {
    this.source?.close?.();
    this.source = null;
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
    const factory =
      this.config.wsFactory ??
      (typeof WebSocket === "undefined" ? null : (target) => new WebSocket(target));
    if (!factory) {
      throw new Error("WebSocketTransport: WebSocket unavailable in this environment");
    }
    this.closed = false;
    this.url = url;
    this.onResult = onResult;
    const socket = factory(this.config.nextUrl?.(url) ?? url);
    this.ws = socket;
    // Binary frames carry the ENVELOPE — its header tells the protocol
    // (feature 04); text frames fall back to the declared protocol, else
    // json (the pre-feature-04 behavior, preserved as the text default).
    if ("binaryType" in this.ws) this.ws.binaryType = "arraybuffer";
    this.ws.onmessage = (event) => {
      if (this.ws !== socket || this.closed) return;
      const { data } = event;
      if (data instanceof ArrayBuffer || ArrayBuffer.isView(data)) {
        const result = decodeEnvelopeFrame(
          data instanceof ArrayBuffer ? new Uint8Array(data) : new Uint8Array(data.buffer, data.byteOffset, data.byteLength),
        );
        if (result) onResult(result);
        return; // malformed frames already surfaced a typed error
      }
      onResult(streamEventResult(this.config.protocol ?? "json", data));
    };
    this.ws.onclose = () => {
      if (this.closed || this.ws !== socket) return;
      const delay = reconnectDelay(this.attempt);
      this.attempt += 1;
      setTimeout(() => {
        if (!this.closed && this.ws === socket) this.connect(this.url, undefined, this.onResult);
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
    if (!response.ok) throw new Error("Stream request failed with HTTP " + response.status);
    const handler = ProtocolHandler.named(this.config.protocol) ?? ProtocolHandler.fromContentType(response);
    if (handler instanceof FrameHandler && isViewResponse(response)) {
      onChunk(await handler.process(response));
    } else if (handler instanceof FrameHandler) {
      const hook = ProtocolHandler.frameHook;
      if (!hook) throw new Error("Install the frame runtime before opening a frame stream");
      for await (const frame of hook.readFrames(response.body,hook.maxFrameBytes)) {
        onChunk({type:"frame",frames:[frame]});
      }
    } else {
      // A single document can span several network chunks.
      const limit = this.config.maxDocumentBytes ?? ProtocolHandler.frameHook?.maxFrameBytes ?? 4 * 1024 * 1024;
      let size = 0;
      const body = response.body.pipeThrough(new TransformStream({transform(bytes, controller) {
        size += bytes.byteLength;
        if (size > limit) throw new RangeError("Streamed document exceeds byte limit");
        controller.enqueue(bytes);
      }}));
      onChunk(await handler.process(new Response(body,{headers:response.headers})));
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
      case "ws": case "websocket":
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

// A binary message carries one EWF1 frame or a generic JSON envelope.
export function decodeEnvelopeFrame(bytes) {
  if (ProtocolHandler.frameHook?.isFrame(bytes)) return { type: "frame", frames: [bytes] };
  if (!(bytes instanceof Uint8Array)) bytes = new Uint8Array(bytes);
  if (bytes.byteLength < 6) {
    console.error("decodeEnvelopeFrame: truncated header", bytes.byteLength);
    return null;
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const protocol = view.getUint8(0);
  const version = view.getUint8(1);
  const length = view.getUint32(2, true);
  const payload = bytes.subarray(6);
  if (payload.byteLength !== length) {
    console.error(`decodeEnvelopeFrame: length mismatch (header ${length}, payload ${payload.byteLength})`);
    return null;
  }
  if (protocol === 2) {
    try {
      return routeJson(JSON.parse(new TextDecoder().decode(payload)));
    } catch (error) {
      console.error("decodeEnvelopeFrame: json payload did not parse", error);
      return null;
    }
  }
  console.error(`decodeEnvelopeFrame: unknown protocol ${protocol} v${version}`);
  return null;
}

function base64ToBytes(text) {
  const bin = atob(String(text).trim());
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i += 1) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

function streamEventResult(kind, raw) {
  if (kind === "frame") return { type: "frame", frames: [base64ToBytes(raw)] };
  if (kind === "json") {
    try {
      return routeJson(JSON.parse(raw));
    } catch {
      return { type: "raw", text: String(raw) };
    }
  }
  if (kind === "arrow") return removedArrow();
  return routeHtml(String(raw));
}

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

function removedArrow() {
  console.error('mount: protocol "arrow" was removed; use "frame"');
  return {type:"raw", text:""};
}

const emptyProtocolHandler = {
  process: async () => ({type:"raw", text:""}),
};

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

/**
 * NEGOTIATION TABLE (feature 04 — the contract, finally written down):
 *
 * | channel      | decided by                                                  |
 * |--------------|--------------------------------------------------------------|
 * | HTTP fetch   | response `content-type` (`fromContentType` below)            |
 * | SSE          | per-message `event:` name (html/json/frame; unnamed = html)  |
 * | WS binary    | the EWF1 identifier or the generic JSON envelope|
 * | WS text      | declared `protocol` attr, else json                          |
 * | `protocol=`  | mount attribute OVERRIDES headers/event names entirely       |
 *
 * Precedence: attribute > headers/event name > channel default.
 */
function isViewResponse(response) {
  return response.headers?.get?.("content-type")?.split(";")[0].trim().toLowerCase() === "application/vnd.ewe.frame";
}

export class FrameHandler {
  async process(response) {
    if (!response.ok) throw new Error("Frame request failed with HTTP " + response.status);
    const hook = ProtocolHandler.frameHook;
    if (!hook) {
      console.error("frame received but the frame runtime is not installed");
      return { type: "raw", text: "" };
    }
    if (isViewResponse(response)) {
      return { type: "frame", frames: [await hook.readViewResponse(response, hook.maxFrameBytes)] };
    }
    const frames = [];
    let total = 0;
    for await (const frame of hook.readFrames(response.body, hook.maxFrameBytes)) {
      total += frame.length;
      if (total > hook.maxFrameBytes) throw new RangeError("Frame reply exceeds byte limit");
      frames.push(frame);
    }
    return { type: "frame", frames };
  }
}

export class ProtocolHandler {
  static frameHook = null;
  /** Explicit handler by name — the mount `protocol` attribute override. */
  static named(name) {
    switch (name) {
      case "frame": return new FrameHandler();
      case "arrow":
        removedArrow();
        return emptyProtocolHandler;
      case "json":
        return new JsonHandler();
      case "html":
        return new HtmlHandler();
      default:
        return null;
    }
  }

  /** Content-type table (spec §3, G46). */
  static fromContentType(response) {
    const ct = response.headers?.get?.("content-type") || "";
    if (isViewResponse(response) || ct.includes("primal-frame")) return new FrameHandler();
    if (ct.includes("primal-arrow") || ct.includes("event-stream-arrow")) return ProtocolHandler.named("arrow");
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
  static scopeExtensions = [];
  static behaviourMarkers = null;
  /** Scoped-script execution scope (spec §5). */
  static createScope(targetElement) {
    const eventCleanups = [];
    const scope = {
      targets: () => targetElement.querySelectorAll("[primal-id]"),
      parent: () => targetElement,
      querySelector: (sel) => targetElement.querySelector(sel),
      querySelectorAll: (sel) => targetElement.querySelectorAll(sel),
      addEvent(target, event, handler, options) {
        const el = typeof target === "string" ? targetElement.querySelector(target) : target;
        if (el) {
          el.addEventListener(event, handler, options);
          eventCleanups.push(() => el.removeEventListener(event, handler, options));
        }
      },
      onDispose(dispose) { eventCleanups.push(dispose); },
      cleanup() {
        for (const cleanup of eventCleanups.splice(0)) cleanup();
      },
    };
    for (const extend of Hydrator.scopeExtensions) extend(scope, targetElement);
    return scope;
  }

  /** Styles + scripts. NOT events (F08 owns primal:on*). Browser-leaning. */
  static hydrate(root, doc = root.ownerDocument ?? globalThis.document) {
    if (typeof root.querySelectorAll !== "function") return;
    Hydrator.behaviourMarkers?.(root);
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
    // Scripts: run with the scope object; per-script error isolation. Scope to
    // the script's OWN parent (spec §5.2), so hydrating a subtree containing
    // many components scopes each script to its component root, not the subtree.
    for (const script of root.querySelectorAll("script[primal\\:script]")) {
      const owner = script.parentElement ?? root;
      const scope = Hydrator.createScope(owner);
      try {
        const source = script.textContent.trim();
        const body = /^(?:async\s+)?function\s*\(/.test(source) ? `(${source})(scope)` : source;
        new Function("scope", body)(scope);
        Patcher.registerUnmount(owner, () => scope.cleanup());
      } catch (error) {
        scope.cleanup();
        console.error("Hydrator: scoped script failed", error);
      }
      script.remove?.();
    }
  }
}

// ─── Patcher ───────────────────────────────────────────────────────────────────

export class Patcher {
  /** Document registry, event dispatcher, signal bridge and frame reader. */
  static runtime = { dispatcher: null, registry: null, signalBridge: null, frames: null };
  /** Monotonic count of applied results (the frame instrument; see `route`). */
  static frameSeq = 0;
  static localScopes = new WeakMap();

  static registerUnmount(node, dispose) {
    let hooks = Patcher.localScopes.get(node);
    if (!hooks) Patcher.localScopes.set(node, hooks = new Set());
    hooks.add(dispose);
  }

  static releaseScope(node) {
    const hooks = Patcher.localScopes.get(node);
    Patcher.localScopes.delete(node);
    if (hooks) for (const dispose of hooks) {
      try { dispose(); } catch (error) { console.error("Component cleanup failed", error); }
    }
  }

  static releaseTree(node) {
    for (const child of Array.from(node.childNodes ?? [])) Patcher.releaseTree(child);
    Patcher.releaseScope(node);
  }

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

  static ensureRegistry(doc) {
    if (Patcher.runtime.document !== doc) {
      Patcher.runtime.document = doc;
      Patcher.runtime.registry = new NodeRegistry().seedDocument(doc);
    }
    return Patcher.runtime.registry;
  }

  static applySignalPatches(patches) {
    const bridge = Patcher.runtime.signalBridge;
    if (bridge?.applyPatches) bridge.applyPatches(patches);
    else console.warn("Patcher: no signalBridge installed; JSON patches dropped", patches);
  }

  /** Route one ProtocolHandler result to the DOM (shared by both mounts). */
  static route(result, targetEl, doc) {
    Patcher.applyRoute(result, targetEl, doc);
    if (result.type !== "frame") Patcher.markApplied();
  }

  static markApplied() {
    // Frame instrument: every applied result bumps a sequence + timestamp. Cheap,
    // always-on, broadly useful — a server-driven client (or a test driver) can
    // poll `globalThis.__primalFrames` to know the DOM has caught up.
    Patcher.frameSeq += 1;
    const stamp = (typeof performance !== "undefined" ? performance.now() : Date.now());
    if (typeof globalThis !== "undefined") {
      globalThis.__primalFrames = { seq: Patcher.frameSeq, at: stamp };
    }
  }

  /** Apply one routed result (the actual DOM work; instrumented by `route`). */
  static applyRoute(result, targetEl, doc) {
    switch (result.type) {
      case "frame":
        if (!Patcher.runtime.frames) console.error("frame received but the frame runtime is not installed");
        else for (const frame of result.frames) Patcher.runtime.frames.receive(frame, { viewMount: targetEl });
        break;
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
      default:
        targetEl.textContent = result.text ?? "";
    }
  }
}

// ─── Response placement (spec §9, shared by both mounts) ──────────────────────

// A frame owns replacement and disposal. Keep the mount intact until the frame applies.
export function frameMountTarget(element, target, doc) {
  if (!target) return element;
  return target === "parent" ? element.parentElement : doc.querySelector(target);
}

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
    Transport.create({
      transport: element.getAttribute("transport") ?? defaults.transport,
      protocol: element.getAttribute("protocol") ?? undefined,
      url: api,
    });
  return { api, data, transport, target: element.getAttribute("target") };
}

/** `<mount-data>` — one request, one response (spec §7). */
export class MountDataComponent extends BaseElement {
  async connectedCallback() {
    const doc = this.ownerDocument ?? globalThis.document;
    try {
      const binding = Patcher.runtime.onMountConnect?.(this, false);
      this._frameMount = binding;
      if (binding?.local) return;
      const { api, data, transport, target } = mountSetup(this);
      this._transport = transport;
      const method = this.getAttribute("method") || "POST";
      const result = await transport.send(binding ? binding.url(api) : api, method, data);
      if (binding) binding.receive(result);
      else {
        const mount = result.type === "frame" ? frameMountTarget(this, target, doc) : resolveMountTarget(this, target, doc);
        Patcher.route(result, mount, doc);
      }
    } catch (error) {
      console.error("mount-data:", error);
    }
  }

  disconnectedCallback() {
    this._frameMount?.close();
    this._transport?.disconnect?.();
  }
}

/** Open a stream while its element is connected. */
export class MountStreamComponent extends BaseElement {
  connectedCallback() {
    const doc = this.ownerDocument ?? globalThis.document;
    try {
      const binding = Patcher.runtime.onMountConnect?.(this, true);
      this._frameMount = binding;
      if (binding?.local) return;
      let generation = 0;
      const open = () => {
        const current = ++generation;
        const { api, data, transport, target } = mountSetup(this, { transport: "sse" });
        // Frame operations name their own targets. The mount owns the connection.
        const targetEl = binding ? null : resolveMountTarget(this, target, doc);
        this._transport = transport;
        if (!binding && this.isConnected === false && targetEl?.isConnected && !this._targetObserver) {
          // Legacy markup replaces its placeholder. Its replacement owns the connection.
          this._targetObserver = new MutationObserver(() => {
            if (!targetEl.isConnected) this.disconnectedCallback();
          });
          this._targetObserver.observe(doc, { childList: true, subtree: true });
        }
        if (binding && transport.config) transport.config.nextUrl = url => binding.url(url);
        const payload = binding && this.getAttribute("data") === null ? undefined : data;
        transport.connect(binding ? binding.url(api) : api, payload, result => {
          if (current !== generation || transport !== this._transport) return;
          if (binding) binding.receive(result);
          else Patcher.route(result, targetEl, doc);
        });
      };
      this._restartFrameStream = () => { this._transport?.disconnect?.(); open(); };
      open();
    } catch (error) {
      console.error("mount-stream:", error);
    }
  }

  disconnectedCallback() {
    this._frameMount?.close();
    this._restartFrameStream = null;
    this._targetObserver?.disconnect();
    this._targetObserver = null;
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

// Connect WebAssembly allocations, event delivery and local scope disposal.
export function registerWasmApp(runtime, opts = {}) {
  const doc = opts.document ?? globalThis.document;
  if (!doc) {
    console.error("registerWasmApp: no document available (non-browser environment)");
    return;
  }
  const bridge = runtime.bridge ?? runtime;
  const frames = globalThis.EweFrames?.installFrameRuntime(globalThis.FoundationWasmUiRuntime, opts);
  if (!frames) throw new Error("registerWasmApp requires foundation-ui-frames.js");
  const dispatcher = Patcher.runtime.dispatcher ?? new EventDispatcher(callbackDeliver(runtime.callbacks));
  dispatcher.deliver = callbackDeliver(runtime.callbacks);
  dispatcher.deliverSignal = signalDeliver(bridge);
  Patcher.runtime.dispatcher = dispatcher;
  const owned = new WeakSet();
  const onMount = (id, node) => {
    if (!owned.has(node) && bridge.exports?.ewe_ui_dispose_mount) {
      owned.add(node);
      Patcher.registerUnmount(node, () => bridge.exports.ewe_ui_dispose_mount(id));
    }
  };
  const onEvent = (name, id, event, el) => {
    const setter = parseCallbackId(el.getAttribute?.("primal:setter"));
    if (setter !== null) dispatcher.deliverSignal(setter, buildEventData(name, event, el));
  };
  runtime.dispatcher.setRawHandler(ProtocolHandler.frameHook.isFrame, bytes => {
    if (bytes.byteLength > frames.maxFrameBytes) return frames.receive(bytes);
    return frames.receive(Uint8Array.from(bytes), {onMount, onEvent});
  });
  // Initial scan + observer: wires primal:on* attrs that exist on page load.
  initEventRuntime(dispatcher, doc);
}

/**
 * The frame instrument: `{ seq, at }` — `seq` is the count of applied routed
 * results, `at` the timestamp of the last apply. A server-driven client (or a
 * test driver) polls this to know the DOM has caught up with the stream. Also
 * mirrored on `globalThis.__primalFrames`.
 */
export function frameStats() {
  return globalThis.__primalFrames ?? { seq: Patcher.frameSeq, at: null };
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
  queueMicrotask(registerWebComponents);
  window.primal ??= createPrimal({});
}

// mirror lets classic (non-module) scripts on the same page reach the DOM runtime as
// `globalThis.FoundationWasmUiRuntime` once the module has loaded.
globalThis.FoundationWasmUiRuntime = Object.freeze({
  RESERVED,
  NodeRegistry,
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
  SseParser,
  FetchEventSource,
  WSBatchQueue,
  WorkerBatchQueue,
  probeBatching,
  parseBatchEntry,
  reconnectDelay,
  resolveMountTarget,
  IslandComponent,
  MountDataComponent,
  MountStreamComponent,
  registerWebComponents,
  registerWasmApp,
  createPrimal,
  DomHeap,
  domAbi,
});
