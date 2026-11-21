import test from "node:test";
import assert from "node:assert/strict";
import {JsActionRegistry, WebActionManager, installActionRuntime} from "../../runtimes/foundation-ui-actions.js";
import * as wire from "../../runtimes/foundation-ui-wire.js";

function element(id, state, root) {
  const attrs = new Map([["primal-id", String(id)]]);
  if (state !== undefined) attrs.set("primal::state", state);
  return {
    value: "buy milk", checked: false, attributes: [],
    getAttribute: key => attrs.get(key) ?? null,
    closest: key => key === "[data-ewe-root]" ? root : null,
    toggleAttribute(key, on) { this[key] = on; },
  };
}
function setup(extra = {}) {
  const registry = new JsActionRegistry();
  registry.declare("server", "save");
  const errors = [], requests = [];
  const manager = new WebActionManager({registry, onError: e => errors.push(e.code),
    server: {send(bytes) { requests.push(wire.Frame.getRootAsFrame(new wire.ByteBuffer(bytes))); return []; }},
    ...extra});
  return {registry, manager, errors, requests};
}
test("bound element supplies the state, input and node on each request", async () => {
  const {manager, errors, requests} = setup();
  const parent = element(40, "BQ");
  const withState = element(42, "_wA", parent);
  withState.parentElement = parent;
  await manager.trigger("server:save", withState, {type:"submit", ctrlKey:true});
  const without = element(43, undefined, parent);
  without.parentElement = parent;
  await manager.trigger("server:save", without, {type:"tap"});
  assert.deepEqual(errors, []);
  assert.deepEqual(requests.map(r => r.sequenceId()), [1n, 2n]);
  const [a,b] = requests.map(r => r.kind(new wire.FrameRequest()).action());
  assert.equal(a.action(), "server:save");
  assert.equal(a.nodeId(), 42);
  assert.equal(a.input().parentNode(), 40);
  assert.equal(a.input().modifiers(), 2);
  assert.equal(a.input().value(), "buy milk");
  assert.deepEqual([...a.stateArray()], [255,0]);
  assert.equal(b.stateArray(), null);
});
test("busy group releases after a failed request", async () => {
  let reject;
  const {manager, errors} = setup({server:{send:() => new Promise((_, fail) => { reject = fail; })}});
  const el = element(3, undefined, {});
  const first = manager.trigger("server:save", el);
  await manager.trigger("server:save", el);
  assert.deepEqual(errors, ["Busy"]);
  reject(Error("disconnected"));
  await first;
  const next = manager.trigger("server:save", el);
  reject(Error("closed"));
  await next;
  assert.deepEqual(errors, ["Busy", "Transport", "Transport"]);
});
test("page handlers receive the bound element and can return frames", async () => {
  const frames = [], el = element(9);
  const {manager, registry, errors} = setup({scope:{go({node}) { assert.equal(node, el); return Uint8Array.of(7); }},
    frames:{receive:b => frames.push([...b])}});
  registry.declarePage("go");
  await manager.trigger("host:go", el);
  assert.deepEqual(frames, [[7]]);
  assert.deepEqual(errors, []);
});
test("ordered frame callbacks install caps and reload once", async () => {
  const errors = [], controls = [], scopes = [], reloads = [];
  const frames = {deps:{}, receive(b) {controls.push(b); return true;}};
  const registry = new JsActionRegistry();
  const behaviours = [];
  const manager = installActionRuntime({EventDispatcher:{}, Hydrator:{scopeExtensions:scopes}, Patcher:{runtime:{}}},
    {frames, registry, behaviours:{registerWebBehaviour:(...args) => behaviours.push(args)}, reload:() => reloads.push(1), onError:e => errors.push(e.code)});
  const scope = {};
  scopes[0](scope);
  const fn = () => {};
  scope.action("save", fn);
  scope.behaviour("focus", fn);
  assert.equal(registry.hostFunction("save"), fn);
  assert.deepEqual(behaviours, [["focus",fn]]);
  await manager.receive(Uint8Array.of(1));
  assert.equal(controls.length, 1);
  frames.deps.onCaps({streamId:7n, actions:[]});
  frames.deps.onError({code:"StateExpired"});
  frames.deps.onError({code:"UpdateRequired"});
  assert.equal(manager.streamId, 7n);
  assert.deepEqual(errors, ["StateExpired","UpdateRequired"]);
  assert.equal(reloads.length, 1);
});

test("a declared web host action can call a global function", async () => {
  let calls = 0;
  const {manager, registry, errors} = setup({scope: {notify() {calls++;}}});
  registry.declare("host", "notify", "web");
  await manager.trigger("host:notify", element(2));
  assert.equal(calls, 1);
  assert.deepEqual(errors, []);
});

test("a remount can replace the callback owned by its node", () => {
  const registry = new JsActionRegistry();
  const next = () => {};
  registry.registerHost("notify", () => {}, element(2));
  registry.registerHost("notify", next, element(2));
  assert.equal(registry.hostFunction("notify"), next);
  assert.throws(() => registry.registerHost("notify", next, element(3)));
});

test("caps mark missing controls and reject server requests before sending", async () => {
  const el = element(2);
  el.attributes = [{name:"primal:onclick", value:"server:save"}];
  const {manager, requests, errors} = setup({document:{querySelectorAll:() => [el]}});
  manager.applyCaps({streamId:80n, actions:[]});
  assert.equal(el["data-ewe-unavailable"], true);
  await manager.trigger("server:save", el);
  assert.deepEqual(errors, ["UnavailableOrigin"]);
  manager.applyCaps({streamId:80n, actions:[{name:"save"}]});
  assert.equal(el["data-ewe-unavailable"], false);
  await manager.trigger("server:save", el);
  assert.equal(requests.length, 1);
  assert.equal(requests[0].streamId(), 80n);
});

test("server-only installation wires an existing control", async () => {
  const ui = await import("../../runtimes/foundation-wasm-ui.js");
  const {MockDocument} = await import("../mock-dom.js");
  const doc = new MockDocument();
  doc.body = doc.createElement("body");
  const button = doc.createElement("button");
  button.setAttribute("primal:onclick", "server:save");
  button.closest = () => null;
  doc.body.appendChild(button);
  const registry = new JsActionRegistry();
  registry.declare("server", "save");
  let count = 0;
  const wasmUi = {...ui, Patcher:{runtime:{}}};
  installActionRuntime(wasmUi, {document:doc, registry, frames:{deps:{}}, server:{send:() => { count++; return []; }}});
  button.dispatchEvent("click");
  assert.equal(count, 1);
});

test("a sheet button with its own action keeps its state and submits the sibling field value", async () => {
  const {manager, requests} = setup();
  const sheet = element(60);
  const button = element(62, "BQ", sheet);
  const field = {tagName: "INPUT", value: "Read chapter three", hasAttribute: key => key === "primal:onsubmit"};
  const attr = button.getAttribute;
  button.getAttribute = key => key === "data-ewe-presentation" ? "submit-action" : attr(key);
  button.closest = selector => selector === ".ewe-bottom-sheet" ? sheet : null;
  button.parentElement = sheet;
  sheet.children = [field, button];

  await manager.trigger("server:save", button, {type: "click"});
  assert.equal(requests.length, 1);
  const action = requests[0].kind(new wire.FrameRequest()).action();
  assert.equal(action.nodeId(), 62);
  assert.deepEqual([...action.stateArray()], [5]);
  assert.equal(action.input().value(), "Read chapter three");
});
