// Spec-42 Feature 05 — in-browser verification of the JS machinery behaviors
// (M5 composite roving, M3 dismiss) against a MockDocument.
// These verify the ALGORITHMS actually work, not just that the JS strings
// contain the right tokens (the Rust tests do that).

import { test } from "node:test";
import assert from "node:assert/strict";
import * as wire from "../../../foundation_wasm_ui/runtimes/foundation-ui-wire.js";
import * as ui from "../../../foundation_wasm_ui/runtimes/foundation-wasm-ui.js";
import { installFrameRuntime } from "../../../foundation_wasm_ui/runtimes/foundation-ui-frames.js";

// ─── Mock DOM ────────────────────────────────────────────────────────────────

class MockElement {
  constructor(tag) {
    this.tag = tag;
    this.attributes = new Map();
    this.children = [];
    this.parent = null;
    this.tabIndex = -1;
    this.id = null;
    this._listeners = new Map();
    this._focused = false;
  }
  setAttribute(n, v) { this.attributes.set(n, v); }
  getAttribute(n) { return this.attributes.has(n) ? this.attributes.get(n) : null; }
  hasAttribute(n) { return this.attributes.has(n); }
  removeAttribute(n) { this.attributes.delete(n); }
  appendChild(c) { c.parent = this; this.children.push(c); return c; }
  querySelector(sel) {
    if (sel.startsWith("#")) {
      return findId(this, sel.slice(1));
    }
    return findByAttr(this, sel);
  }
  querySelectorAll(sel) {
    return collectByAttr(this, sel);
  }
  contains(node) {
    for (let cur = node; cur; cur = cur.parent) {
      if (cur === this) return true;
    }
    return false;
  }
  addEventListener(type, fn) {
    if (!this._listeners.has(type)) this._listeners.set(type, new Set());
    this._listeners.get(type).add(fn);
  }
  dispatchEvent(type, init = {}) {
    const event = { type, target: init.target || this, ...init };
    if (init.preventDefault) event.preventDefault = init.preventDefault;
    else event.preventDefault = () => {};
    for (const fn of this._listeners.get(type) ?? []) fn(event);
  }
  focus() { this._focused = true; }
  click() { this.dispatchEvent("click", { target: this }); }
}

class MockDocument {
  constructor() {
    this.body = new MockElement("body");
    this._listeners = new Map();
    this.activeElement = null;
    // Make `document` available globally for the JS behaviors.
    globalThis.document = this;
  }
  createElement(tag) { return new MockElement(tag); }
  getElementById(id) { return findId(this.body, id); }
  addEventListener(type, fn) {
    if (!this._listeners.has(type)) this._listeners.set(type, new Set());
    this._listeners.get(type).add(fn);
  }
  dispatchEvent(type, init = {}) {
    const event = { type, target: init.target || this, ...init };
    if (init.preventDefault) event.preventDefault = init.preventDefault;
    else event.preventDefault = () => {};
    for (const fn of this._listeners.get(type) ?? []) fn(event);
  }
}

// Use the same wire messages and behaviour registry as the browser.
function applyBehaviour(document, element, operation) {
  const frames = installFrameRuntime(ui, {document, schedule: run => run()});
  frames.registry.register(42, element);
  const message = new wire.FrameMessageT(0n, [new wire.MessageT(operation)]);
  const frame = new wire.FrameT(1, 1n, 0n, wire.FrameKind.FrameMessage, message);
  const builder = new wire.Builder();
  wire.Frame.finishFrameBuffer(builder, frame.pack(builder));
  frames.receive(builder.asUint8Array());
}

function composite(document, root) {
  applyBehaviour(document, root, new wire.OperationT(0, null, wire.WebOperation.WebBehaviour,
    new wire.WebBehaviourT(42, wire.WebBehaviourId.UNKNOWN, "composite")));
}

function dismiss(document, popup) {
  applyBehaviour(document, popup, new wire.OperationT(wire.UiOperation.UiBehaviour,
    new wire.UiBehaviourT(42, wire.BehaviourId.DISMISS)));
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

function findId(node, id) {
  if (node.id === id) return node;
  for (const child of node.children) {
    const found = findId(child, id);
    if (found) return found;
  }
  return null;
}

function findByAttr(node, sel) {
  const m = sel.match(/\[([^\]=]+)(?:="([^"]*)")?\]/);
  if (!m) return null;
  const [, name] = m;
  if (node.attributes?.has(name)) return node;
  for (const child of node.children) {
    const found = findByAttr(child, sel);
    if (found) return found;
  }
  return null;
}

function collectByAttr(node, sel) {
  const results = [];
  const m = sel.match(/\[([^\]=]+)(?:="([^"]*)")?\]/);
  if (!m) return results;
  const [, name] = m;
  function walk(n) {
    if (n.attributes?.has(name)) results.push(n);
    for (const child of n.children) walk(child);
  }
  walk(node);
  return results;
}

function clickTracker(el) {
  let clicked = false;
  el.addEventListener("click", () => { clicked = true; });
  return () => clicked;
}

// ─── M5: Composite roving focus ──────────────────────────────────────────────



test("M5 composite: arrows move roving tabindex horizontally", () => {
  const doc = new MockDocument();
  const root = doc.createElement("div");
  root.setAttribute("data-composite", "");
  root.setAttribute("data-orientation", "horizontal");
  const items = [0, 1, 2].map(i => {
    const el = doc.createElement("button");
    el.setAttribute("data-composite-item", "");
    el.setAttribute("data-label", `item-${i}`);
    el.tabIndex = -1;
    root.appendChild(el);
    return el;
  });
  doc.body.appendChild(root);
  composite(doc, root);

  assert.equal(items[0].tabIndex, 0, "first item seeded as active");
  assert.equal(items[1].tabIndex, -1);
  assert.equal(items[2].tabIndex, -1);

  doc.activeElement = items[0];
  root.dispatchEvent("keydown", { key: "ArrowRight" });
  assert.equal(items[0].tabIndex, -1, "old active lost tabindex");
  assert.equal(items[1].tabIndex, 0, "ArrowRight moved to next");
  assert.equal(items[2].tabIndex, -1);
});

test("M5 composite: Home/End jump to first/last", () => {
  const doc = new MockDocument();
  const root = doc.createElement("div");
  root.setAttribute("data-composite", "");
  const items = [0, 1, 2].map(i => {
    const el = doc.createElement("button");
    el.setAttribute("data-composite-item", "");
    el.tabIndex = -1;
    root.appendChild(el);
    return el;
  });
  doc.body.appendChild(root);
  composite(doc, root);
  doc.activeElement = items[0];

  root.dispatchEvent("keydown", { key: "End" });
  assert.equal(items[2].tabIndex, 0, "End jumps to last");

  root.dispatchEvent("keydown", { key: "Home" });
  assert.equal(items[0].tabIndex, 0, "Home jumps to first");
});

test("M5 composite: disabled items are skipped", () => {
  const doc = new MockDocument();
  const root = doc.createElement("div");
  root.setAttribute("data-composite", "");
  root.setAttribute("data-orientation", "horizontal");
  const items = [0, 1, 2].map(i => {
    const el = doc.createElement("button");
    el.setAttribute("data-composite-item", "");
    el.tabIndex = -1;
    root.appendChild(el);
    return el;
  });
  items[1].setAttribute("aria-disabled", "true");
  doc.body.appendChild(root);
  composite(doc, root);
  doc.activeElement = items[0];

  root.dispatchEvent("keydown", { key: "ArrowRight" });
  assert.equal(items[2].tabIndex, 0, "skipped disabled item 1");
});

test("M5 composite: typeahead prefix match", () => {
  const doc = new MockDocument();
  const root = doc.createElement("div");
  root.setAttribute("data-composite", "");
  root.setAttribute("data-orientation", "horizontal");
  const items = ["apple", "apricot", "banana"].map(label => {
    const el = doc.createElement("button");
    el.setAttribute("data-composite-item", "");
    el.setAttribute("data-label", label);
    el.tabIndex = -1;
    root.appendChild(el);
    return el;
  });
  doc.body.appendChild(root);
  composite(doc, root);
  doc.activeElement = items[0];

  root.dispatchEvent("keydown", { key: "b" });
  assert.equal(items[2].tabIndex, 0, "typeahead 'b' → banana");
});

test("M5 composite: vertical orientation maps to up/down", () => {
  const doc = new MockDocument();
  const root = doc.createElement("div");
  root.setAttribute("data-composite", "");
  root.setAttribute("data-orientation", "vertical");
  const items = [0, 1].map(i => {
    const el = doc.createElement("div");
    el.setAttribute("data-composite-item", "");
    el.tabIndex = -1;
    root.appendChild(el);
    return el;
  });
  doc.body.appendChild(root);
  composite(doc, root);
  doc.activeElement = items[0];

  root.dispatchEvent("keydown", { key: "ArrowDown" });
  assert.equal(items[1].tabIndex, 0, "ArrowDown moves in vertical");
});

test("M5 composite: data-composite-select clicks on move", () => {
  const doc = new MockDocument();
  const root = doc.createElement("div");
  root.setAttribute("data-composite", "");
  root.setAttribute("data-composite-select", "");
  root.setAttribute("data-orientation", "horizontal");
  const items = [0, 1].map(i => {
    const el = doc.createElement("button");
    el.setAttribute("data-composite-item", "");
    el.tabIndex = -1;
    root.appendChild(el);
    return el;
  });
  const wasClicked = [clickTracker(items[0]), clickTracker(items[1])];
  doc.body.appendChild(root);
  composite(doc, root);
  doc.activeElement = items[0];

  root.dispatchEvent("keydown", { key: "ArrowRight" });
  assert.ok(wasClicked[1](), "select mode clicked the moved-to item");
});

// ─── M3: Dismiss (Escape + outside pointer) ──────────────────────────────────



test("M3 dismiss: Escape clicks the action", () => {
  const doc = new MockDocument();
  const popup = doc.createElement("div");
  popup.setAttribute("data-dismiss", "");
  const action = doc.createElement("button");
  action.setAttribute("data-dismiss-action", "");
  action.setAttribute("hidden", "");
  const clicked = clickTracker(action);
  popup.appendChild(action);
  doc.body.appendChild(popup);
  dismiss(doc, popup);

  doc.dispatchEvent("keydown", { key: "Escape" });
  assert.ok(clicked(), "Escape triggered the dismiss action");
});

test("M3 dismiss: outside pointer clicks the action", () => {
  const doc = new MockDocument();
  const popup = doc.createElement("div");
  popup.setAttribute("data-dismiss", "");
  const action = doc.createElement("button");
  action.setAttribute("data-dismiss-action", "");
  const clicked = clickTracker(action);
  popup.appendChild(action);
  const outside = doc.createElement("div");
  doc.body.appendChild(popup);
  doc.body.appendChild(outside);
  dismiss(doc, popup);

  doc.dispatchEvent("pointerdown", { target: outside });
  assert.ok(clicked(), "outside pointer triggered the dismiss action");
});

test("M3 dismiss: pointer inside popup does NOT dismiss", () => {
  const doc = new MockDocument();
  const popup = doc.createElement("div");
  popup.setAttribute("data-dismiss", "");
  const action = doc.createElement("button");
  action.setAttribute("data-dismiss-action", "");
  const clicked = clickTracker(action);
  popup.appendChild(action);
  doc.body.appendChild(popup);
  dismiss(doc, popup);

  doc.dispatchEvent("pointerdown", { target: popup });
  assert.ok(!clicked(), "inside pointer did NOT dismiss");
});

test("M3 dismiss: pointer on trigger (anchor) does NOT dismiss", () => {
  const doc = new MockDocument();
  const popup = doc.createElement("div");
  popup.setAttribute("data-dismiss", "");
  popup.setAttribute("data-anchor", "trigger-1");
  const action = doc.createElement("button");
  action.setAttribute("data-dismiss-action", "");
  const clicked = clickTracker(action);
  popup.appendChild(action);
  const trigger = doc.createElement("button");
  trigger.id = "trigger-1";
  doc.body.appendChild(popup);
  doc.body.appendChild(trigger);
  dismiss(doc, popup);

  doc.dispatchEvent("pointerdown", { target: trigger });
  assert.ok(!clicked(), "trigger pointer did NOT dismiss");
});

test("M3 dismiss: stamps dismiss reason on the action", () => {
  const doc = new MockDocument();
  const popup = doc.createElement("div");
  popup.setAttribute("data-dismiss", "");
  const action = doc.createElement("button");
  action.setAttribute("data-dismiss-action", "");
  popup.appendChild(action);
  doc.body.appendChild(popup);
  dismiss(doc, popup);

  doc.dispatchEvent("keydown", { key: "Escape" });
  assert.equal(action.getAttribute("data-dismiss-reason"), "escape-key",
    "reason stamped as escape-key");
});

test("M3 dismiss: data-dismiss-escape=false disables Escape", () => {
  const doc = new MockDocument();
  const popup = doc.createElement("div");
  popup.setAttribute("data-dismiss", "");
  popup.setAttribute("data-dismiss-escape", "false");
  const action = doc.createElement("button");
  action.setAttribute("data-dismiss-action", "");
  const clicked = clickTracker(action);
  popup.appendChild(action);
  doc.body.appendChild(popup);
  dismiss(doc, popup);

  doc.dispatchEvent("keydown", { key: "Escape" });
  assert.ok(!clicked(), "Escape ignored when data-dismiss-escape=false");
});

test("M3 dismiss: data-dismiss-outside=false disables outside pointer", () => {
  const doc = new MockDocument();
  const popup = doc.createElement("div");
  popup.setAttribute("data-dismiss", "");
  popup.setAttribute("data-dismiss-outside", "false");
  const action = doc.createElement("button");
  action.setAttribute("data-dismiss-action", "");
  const clicked = clickTracker(action);
  popup.appendChild(action);
  const outside = doc.createElement("div");
  doc.body.appendChild(popup);
  doc.body.appendChild(outside);
  dismiss(doc, popup);

  doc.dispatchEvent("pointerdown", { target: outside });
  assert.ok(!clicked(), "outside pointer ignored when data-dismiss-outside=false");
});

test("every shipped matrix entry names a real browser binding", async () => {
  const { readFileSync } = await import("node:fs");
  const { SHARED, WEB_ONLY } = await import("../../../foundation_wasm_ui/runtimes/foundation-ui-behaviours.js");
  const text = readFileSync(new URL("../../behaviours.toml",import.meta.url),"utf8");
  const names = [...text.matchAll(/^\[behaviours\.(\w+)\]$/gm)].map(match => match[1]);
  const browser = [...Object.keys(SHARED).map(name => name.toLowerCase()), ...Object.keys(WEB_ONLY)];
  assert.deepEqual(names.sort(),browser.sort());
});
