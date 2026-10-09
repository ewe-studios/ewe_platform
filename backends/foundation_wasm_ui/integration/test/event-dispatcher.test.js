// Tests for foundation-wasm-ui.js EventDispatcher — wiring primal:on* attributes to
// WASM callbacks (decision 018). Uses the mock DOM; `deliver` is spied so we don't
// need a live WASM instance.

import { test } from "node:test";
import assert from "node:assert/strict";

import {
  EventDispatcher,
  resolveFunctionRef,
  parseCallbackId,
  callbackDeliver,
} from "../../runtimes/foundation-wasm-ui.js";
import { MockDocument } from "../mock-dom.js";

function deliverSpy() {
  const calls = [];
  const fn = (callbackId, eventData) => calls.push({ callbackId, eventData });
  return { fn, calls };
}

test("parseCallbackId accepts numeric and callback- forms, rejects JS refs", () => {
  assert.equal(parseCallbackId("7"), 7);
  assert.equal(parseCallbackId("callback-7"), 7);
  assert.equal(parseCallbackId(" 42 "), 42);
  assert.equal(parseCallbackId("controller.delete"), null);
  assert.equal(parseCallbackId(null), null);
});

test("scanAndWire wires primal:on* and delivers EventData on dispatch", () => {
  const doc = new MockDocument();
  const btn = doc.createElement("button");
  btn.setAttribute("primal-id", "42:3");
  btn.setAttribute("primal:onclick", "callback-7");
  btn.value = "hello";

  const spy = deliverSpy();
  new EventDispatcher(spy.fn).scanAndWire(btn);

  btn.dispatchEvent("click", { keyCode: 13, altKey: true });

  assert.equal(spy.calls.length, 1);
  const { callbackId, eventData } = spy.calls[0];
  assert.equal(callbackId, 7);
  assert.equal(eventData.type, "click");
  assert.equal(eventData.primalId, "42:3");
  assert.equal(eventData.value, "hello");
  assert.equal(eventData.keyCode, 13);
  assert.equal(eventData.modifiers.alt, true);
  assert.equal(eventData.modifiers.ctrl, false);
});

test("scanAndWire reaches descendants", () => {
  const doc = new MockDocument();
  const root = doc.createElement("div");
  const child = doc.createElement("button");
  child.setAttribute("primal:onclick", "3");
  root.appendChild(child);

  const spy = deliverSpy();
  new EventDispatcher(spy.fn).scanAndWire(root);
  child.dispatchEvent("click");
  assert.equal(spy.calls.length, 1);
  assert.equal(spy.calls[0].callbackId, 3);
});

test("rewiring is idempotent (G2) — no duplicate listeners", () => {
  const doc = new MockDocument();
  const btn = doc.createElement("button");
  btn.setAttribute("primal:onclick", "1");

  const spy = deliverSpy();
  const d = new EventDispatcher(spy.fn);
  d.scanAndWire(btn);
  d.scanAndWire(btn); // re-scan must not stack a second listener
  assert.equal(btn.listenerCount("click"), 1);

  btn.dispatchEvent("click");
  assert.equal(spy.calls.length, 1);
});

test("off and removeListeners detach listeners", () => {
  const doc = new MockDocument();
  const root = doc.createElement("div");
  const btn = doc.createElement("button");
  btn.setAttribute("primal:onclick", "1");
  root.appendChild(btn);

  const spy = deliverSpy();
  const d = new EventDispatcher(spy.fn);
  d.scanAndWire(root);

  d.off(btn, "click");
  assert.equal(btn.listenerCount("click"), 0);

  d.scanAndWire(root);
  d.removeListeners(root);
  assert.equal(btn.listenerCount("click"), 0);
  btn.dispatchEvent("click");
  assert.equal(spy.calls.length, 0);
});

test("non-callback handler refs do not deliver", () => {
  const doc = new MockDocument();
  const btn = doc.createElement("button");
  btn.setAttribute("primal:onclick", "controller.delete");

  const spy = deliverSpy();
  new EventDispatcher(spy.fn).scanAndWire(btn);
  btn.dispatchEvent("click");
  assert.equal(spy.calls.length, 0); // resolved as a JS function ref later (not this increment)
});

test("callbackDeliver ships JSON-encoded EventData through a CallbackRegistry", () => {
  const invoked = [];
  const fakeRegistry = { invoke: (id, bytes) => invoked.push({ id, bytes }) };

  const doc = new MockDocument();
  const input = doc.createElement("input");
  input.setAttribute("primal-id", "9");
  input.setAttribute("primal:onchange", "callback-2");
  input.value = "typed";

  new EventDispatcher(callbackDeliver(fakeRegistry)).scanAndWire(input);
  input.dispatchEvent("change");

  assert.equal(invoked.length, 1);
  assert.equal(invoked[0].id, 2);
  const decoded = JSON.parse(new TextDecoder().decode(invoked[0].bytes));
  assert.equal(decoded.type, "change");
  assert.equal(decoded.primalId, "9");
  assert.equal(decoded.value, "typed");
});

// ─── Feature 08: dot-path resolution (spec tests 1, 5, 7) ───────────────────────

test("dot-path handler resolves against scope and binds `this` to the element", () => {
  const doc = new MockDocument();
  const el = doc.createElement("button");
  el.setAttribute("primal:onclick", "controller.remove");

  let got = null;
  const scope = {
    controller: {
      remove() {
        got = this; // bound element
      },
    },
  };
  const dispatcher = new EventDispatcher(() => {}, { scope });
  dispatcher.scanAndWire(el);
  el.dispatchEvent("click");
  assert.equal(got, el, "this === the attributed element");
});

test("unresolvable dot-path warns and attaches nothing", () => {
  const doc = new MockDocument();
  const el = doc.createElement("button");
  el.setAttribute("primal:onclick", "nonexistent.fn");
  const dispatcher = new EventDispatcher(() => {}, { scope: {} });

  const warnings = [];
  const original = console.warn;
  console.warn = (msg) => warnings.push(msg);
  try {
    dispatcher.scanAndWire(el);
  } finally {
    console.warn = original;
  }
  assert.equal(el.listenerCount("click"), 0, "no listener attached");
  assert.ok(warnings.some((w) => String(w).includes("nonexistent.fn")));
});

// ─── Feature 08: primal:setter signal wiring (F03 two-way binding leg) ───────────

test("primal:setter routes through deliverSignal with the setter id", () => {
  const doc = new MockDocument();
  const el = doc.createElement("input");
  el.setAttribute("primal:onchange", "true"); // the html! marker
  el.setAttribute("primal:setter", "42");
  el.value = "typed";

  const signals = [];
  const dispatcher = new EventDispatcher(
    () => assert.fail("registry deliver must not be used"),
    { deliverSignal: (id, data) => signals.push({ id, data }) },
  );
  dispatcher.scanAndWire(el);
  el.dispatchEvent("change");

  assert.equal(signals.length, 1);
  assert.equal(signals[0].id, 42);
  assert.equal(signals[0].data.value, "typed");
});

// ─── Feature 08: delegation (spec tests 9-13) ────────────────────────────────────

function delegationSetup() {
  const doc = new MockDocument();
  const container = doc.createElement("div");
  container.setAttribute("id", "container");
  doc.root.appendChild(container);
  const button = doc.createElement("button");
  button.setAttribute("primal-id", "7");
  container.appendChild(button);
  // Mocks have no ownerDocument; point the dispatcher at this doc.
  const dispatcher = new EventDispatcher(() => {});
  dispatcher.documentOf = () => doc;
  return { doc, container, button, dispatcher };
}

test("delegate '#selector' attaches the listener on the container", () => {
  const { container, button, dispatcher } = delegationSetup();
  button.setAttribute("primal:onclick:delegate", "#container");
  dispatcher.scanAndWire(button);
  assert.equal(container.listenerCount("click"), 1, "listener on #container");
  assert.equal(button.listenerCount("click"), 0, "not on the button");
});

test("delegate 'parent' and 'body' resolve structurally", () => {
  const { doc, container, button, dispatcher } = delegationSetup();
  dispatcher.wireDelegated(button, "click", "parent");
  assert.equal(container.listenerCount("click"), 1);

  doc.body = doc.createElement("body");
  const loose = doc.createElement("a");
  dispatcher.wireDelegated(loose, "keydown", "body");
  assert.equal(doc.body.listenerCount("keydown"), 1);
});

test("delegated events stamp delegateTarget when originating inside", () => {
  const { container, button, dispatcher } = delegationSetup();
  dispatcher.wireDelegated(button, "click", "#container");

  let seen = null;
  container._listeners.get("click").forEach((fn) => {
    // Fire as if a child of button was clicked.
    const inner = { parent: button };
    const event = { target: inner };
    fn(event);
    seen = event.delegateTarget;
  });
  assert.equal(seen, button);
});

test("missing delegate target warns and attaches nothing", () => {
  const { button, dispatcher } = delegationSetup();
  const warnings = [];
  const original = console.warn;
  console.warn = (msg) => warnings.push(msg);
  try {
    dispatcher.wireDelegated(button, "click", "#missing");
  } finally {
    console.warn = original;
  }
  assert.ok(warnings.length === 1);
});

test("two elements delegating to one container do not collide", () => {
  const { doc, container, dispatcher } = delegationSetup();
  const a = doc.createElement("a");
  a.setAttribute("primal-id", "1");
  container.appendChild(a);
  const b = doc.createElement("b");
  b.setAttribute("primal-id", "2");
  container.appendChild(b);
  dispatcher.wireDelegated(a, "click", "#container");
  dispatcher.wireDelegated(b, "click", "#container");
  assert.equal(container.listenerCount("click"), 2, "compound keys keep both");
});

// ─── Feature 08: mutation handling + island boundary (spec tests 14-19) ──────────

test("handleMutations wires added subtrees and cleans removed ones", () => {
  const doc = new MockDocument();
  const delivered = [];
  const dispatcher = new EventDispatcher((id) => delivered.push(id));

  const added = doc.createElement("div");
  const child = doc.createElement("button");
  child.setAttribute("primal:onclick", "callback-3");
  added.appendChild(child);

  dispatcher.handleMutations([
    { type: "childList", addedNodes: [added], removedNodes: [] },
  ]);
  child.dispatchEvent("click");
  assert.deepEqual(delivered, [3], "added subtree wired (all descendants)");

  dispatcher.handleMutations([
    { type: "childList", addedNodes: [], removedNodes: [added] },
  ]);
  child.dispatchEvent("click");
  assert.deepEqual(delivered, [3], "removed subtree cleaned");
});

test("island subtrees are skipped entirely (boundary rule)", () => {
  const doc = new MockDocument();
  const dispatcher = new EventDispatcher(() => assert.fail("must not deliver"));

  const island = doc.createElement("island");
  const inside = doc.createElement("button");
  inside.setAttribute("primal:onclick", "callback-9");
  island.appendChild(inside);

  // Adding a node INSIDE an island: skipped.
  dispatcher.handleMutations([
    { type: "childList", addedNodes: [inside], removedNodes: [] },
  ]);
  assert.equal(inside.listenerCount("click"), 0);

  // Adding the island itself: closest('island') matches self — skipped.
  dispatcher.handleMutations([
    { type: "childList", addedNodes: [island], removedNodes: [] },
  ]);
  assert.equal(inside.listenerCount("click"), 0);
});

// ─── Feature 08: trackRemoved microtask batching (spec tests 24-26) ──────────────

test("trackRemoved batches synchronous removals into one microtask", async () => {
  const doc = new MockDocument();
  const dispatcher = new EventDispatcher(() => {});
  const nodes = [];
  for (let i = 0; i < 5; i++) {
    const el = doc.createElement("div");
    dispatcher.wire(el, "click", "callback-1");
    nodes.push(el);
  }
  for (const el of nodes) dispatcher.trackRemoved(el);
  assert.equal(nodes[0].listenerCount("click"), 1, "not yet — batched");
  await Promise.resolve(); // drain the microtask
  for (const el of nodes) assert.equal(el.listenerCount("click"), 0);

  // A second batch after the first drained.
  const late = doc.createElement("div");
  dispatcher.wire(late, "click", "callback-2");
  dispatcher.trackRemoved(late);
  await Promise.resolve();
  assert.equal(late.listenerCount("click"), 0, "second batch ran separately");
});

// ─── Feature 08: programmatic API + convenience methods (spec tests 27-31) ───────

test("on/off programmatic API incl. delegation option and convenience methods", () => {
  const { doc, container, dispatcher } = delegationSetup();
  const delivered = [];
  dispatcher.deliver = (id) => delivered.push(id);

  const el = doc.createElement("span");
  dispatcher.on(el, "click", "callback-5");
  el.dispatchEvent("click");
  assert.deepEqual(delivered, [5]);

  dispatcher.off(el, "click");
  el.dispatchEvent("click");
  assert.deepEqual(delivered, [5], "off removed it");

  dispatcher.on(el, "click", null, { delegate: "#container" });
  assert.equal(container.listenerCount("click"), 1, "delegate option routes");

  const conv = doc.createElement("a");
  dispatcher.onclick(conv, "callback-6");
  conv.dispatchEvent("click");
  assert.deepEqual(delivered, [5, 6], "convenience method wires");

  // off(el) with no type clears everything incl. compound keys.
  dispatcher.off(container);
  assert.equal(container.listenerCount("click"), 0);
});
