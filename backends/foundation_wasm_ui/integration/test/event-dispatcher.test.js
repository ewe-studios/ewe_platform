// Tests for foundation-wasm-ui.js EventDispatcher — wiring primal:on* attributes to
// WASM callbacks (decision 018). Uses the mock DOM; `deliver` is spied so we don't
// need a live WASM instance.

import { test } from "node:test";
import assert from "node:assert/strict";

import {
  EventDispatcher,
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
