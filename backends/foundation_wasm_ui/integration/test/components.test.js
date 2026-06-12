// Feature 06 — web component layers under node: transport factory routing,
// content-type handler selection, wrapper detection (island/morph), the
// request queue's bundling, FetchTransport with injected fetch, CSS scoping,
// scope objects, mount target resolution, reconnect backoff, and mount-data
// end-to-end on the mock DOM with an injected transport.

import { test } from "node:test";
import assert from "node:assert/strict";

import {
  Transport,
  FetchTransport,
  SSETransport,
  ProtocolHandler,
  ArrowHandler,
  JsonHandler,
  HtmlHandler,
  RawHandler,
  Patcher,
  Hydrator,
  scopeCss,
  RequestQueue,
  reconnectDelay,
  resolveMountTarget,
  MountDataComponent,
} from "../../runtimes/foundation-wasm-ui.js";
import { MockDocument } from "../mock-dom.js";

/** Minimal Response stand-in. */
function fakeResponse({ contentType, text = "", json = null, ok = true, status = 200 }) {
  return {
    ok,
    status,
    headers: { get: (k) => (k === "content-type" ? contentType : null) },
    text: async () => text,
    json: async () => json,
    arrayBuffer: async () => new ArrayBuffer(0),
  };
}

/// Spec tests 1-3 — transport factory routing.
test("Transport.create routes by config", () => {
  assert.ok(Transport.create({ transport: "fetch" }) instanceof FetchTransport);
  assert.ok(Transport.create({}) instanceof FetchTransport, "default");
  assert.ok(Transport.create({ transport: "sse" }) instanceof SSETransport);
});

/// Spec test 4 — FetchTransport sends and routes through the handler.
test("FetchTransport.send posts JSON and processes by content type", async () => {
  let captured = null;
  const transport = new FetchTransport({
    fetchFn: async (url, init) => {
      captured = { url, init };
      return fakeResponse({ contentType: "application/primal-html", text: "<p>hi</p>" });
    },
  });
  const result = await transport.send("/api", "POST", { id: 1 });
  assert.equal(captured.url, "/api");
  assert.equal(captured.init.method, "POST");
  assert.equal(captured.init.body, '{"id":1}');
  assert.deepEqual(result, { type: "html", html: "<p>hi</p>" });
});

/// Spec test 36 — non-OK responses reject.
test("FetchTransport rejects on 500", async () => {
  const transport = new FetchTransport({
    fetchFn: async () => fakeResponse({ contentType: "text/html", ok: false, status: 500 }),
  });
  await assert.rejects(() => transport.send("/api", "GET"));
});

/// Spec tests 8-13 — content-type table.
test("ProtocolHandler.fromContentType selects per the table", () => {
  const pick = (ct) => ProtocolHandler.fromContentType(fakeResponse({ contentType: ct }));
  assert.ok(pick("application/primal-html") instanceof HtmlHandler);
  assert.ok(pick("application/primal-arrow") instanceof ArrowHandler);
  assert.ok(pick("application/primal-json") instanceof JsonHandler);
  assert.ok(pick("text/event-stream-html") instanceof HtmlHandler);
  assert.ok(pick("text/html; charset=utf-8") instanceof HtmlHandler);
  assert.ok(pick(null) instanceof RawHandler, "fallback");
});

/// Spec §3 — wrapper detection.
test("handlers detect island and morph wrappers", async () => {
  const html = await new HtmlHandler().process(
    fakeResponse({
      contentType: "text/html",
      text: '<island data-target="#main" data-action="replace-children"><div>X</div></island>',
    }),
  );
  assert.equal(html.type, "html-morph");
  assert.equal(html.target, "#main");
  assert.equal(html.content, "<div>X</div>");

  const morph = await new JsonHandler().process(
    fakeResponse({
      contentType: "application/primal-json",
      json: { morph: { target: "#m", action: "replace-children", content: "<b>y</b>" } },
    }),
  );
  assert.equal(morph.type, "json-morph");

  const patches = await new JsonHandler().process(
    fakeResponse({ contentType: "application/primal-json", json: [{ signalId: 1 }] }),
  );
  assert.equal(patches.type, "json");
});

/// Request bundling (decision 026): off until probed, then one flush per tick.
test("RequestQueue bundles only after a successful probe", async () => {
  const sent = [];
  const queue = new RequestQueue({ send: (...args) => sent.push(args) });

  queue.enqueue({ url: "/a", method: "GET", data: null });
  assert.equal(sent.length, 1, "unbundled passthrough before probe");

  await queue.probe(async () => ({ ok: true }));
  queue.enqueue({ url: "/b", method: "GET", data: 1 });
  queue.enqueue({ url: "/c", method: "GET", data: 2 });
  assert.equal(sent.length, 1, "queued, not yet flushed");
  await Promise.resolve();
  assert.equal(sent.length, 2);
  assert.equal(sent[1][0], "/primal/messages");
  assert.equal(sent[1][2].length, 2, "both requests in ONE batch");
});

/// Spec test 7 — reconnect backoff sequence.
test("reconnect backoff is 1s/2s/4s capped at 30s", () => {
  assert.deepEqual(
    [0, 1, 2, 3, 4, 5, 6].map(reconnectDelay),
    [1000, 2000, 4000, 8000, 16000, 30000, 30000],
  );
});

/// Spec test 29 — CSS scoping incl. compound selectors.
test("scopeCss prefixes every selector", () => {
  const scoped = scopeCss(".title { color: red } .a, .b { x: y }", "#island-1");
  assert.match(scoped, /#island-1 \.title \{/);
  assert.match(scoped, /#island-1 \.a, #island-1 \.b \{/);
});

/// Spec tests 22-24 — scope object.
test("Hydrator scope addEvent/cleanup", () => {
  const d = new MockDocument();
  const root = d.createElement("div");
  const btn = d.createElement("button");
  btn.setAttribute("class", "btn");
  root.appendChild(btn);
  d.root.appendChild(root);

  // Mock querySelector works on document; give the root a scoped variant.
  root.querySelector = (sel) => (sel === ".btn" ? btn : null);
  const scope = Hydrator.createScope(root);

  let fired = 0;
  scope.addEvent(".btn", "click", () => (fired += 1));
  btn.dispatchEvent("click");
  assert.equal(fired, 1);
  scope.cleanup();
  btn.dispatchEvent("click");
  assert.equal(fired, 1, "listener removed");
  assert.equal(scope.parent(), root);
});

/// Spec tests 33-35 + 37 — target resolution table.
test("resolveMountTarget covers the placement table", () => {
  const d = new MockDocument();
  const parent = d.createElement("section");
  const mount = d.createElement("mount-data");
  parent.appendChild(mount);
  d.root.appendChild(parent);

  // "parent": cleared and returned.
  parent.appendChild(d.createElement("p"));
  const got = resolveMountTarget(mount, "parent", d);
  assert.equal(got, parent);
  assert.equal(parent.children.length, 0, "cleared");

  // "#id": found, cleared, returned.
  const list = d.createElement("ul");
  list.setAttribute("id", "list");
  list.appendChild(d.createElement("li"));
  d.root.appendChild(list);
  const byId = resolveMountTarget(mount, "#list", d);
  assert.equal(byId, list);
  assert.equal(list.children.length, 0);

  // missing target throws.
  assert.throws(() => resolveMountTarget(mount, "#missing", d), /mount target not found/);

  // omitted: self-replacement with a container div (G29).
  const holder = d.createElement("div");
  const mount2 = d.createElement("mount-data");
  holder.appendChild(mount2);
  const container = resolveMountTarget(mount2, null, d);
  assert.equal(container.tag, "div");
  assert.equal(holder.children[0], container, "mount replaced in place");
});

/// Spec tests 31-33 — mount-data end-to-end with an injected transport.
test("mount-data fetches and routes the result", async () => {
  const d = new MockDocument();
  const target = d.createElement("div");
  target.setAttribute("id", "out");
  d.root.appendChild(target);

  const mount = new MountDataComponent();
  // Wire the mock element surface the component reads.
  const attrs = new Map([
    ["api", "/items"],
    ["method", "PUT"],
    ["data", '{"id":1}'],
    ["target", "#out"],
  ]);
  mount.getAttribute = (k) => attrs.get(k) ?? null;
  Object.defineProperty(mount, "ownerDocument", { value: d });

  let sent = null;
  mount._transportOverride = {
    send: async (url, method, data) => {
      sent = { url, method, data };
      return { type: "raw", text: "DONE" };
    },
    disconnect() {},
  };

  await mount.connectedCallback();
  assert.deepEqual(sent, { url: "/items", method: "PUT", data: { id: 1 } });
  assert.equal(target.textContent, "DONE", "raw result landed in #out");
});

/// Patcher seams: signal patches warn-and-drop without a bridge, route via it
/// when installed.
test("Patcher.applySignalPatches uses the injected bridge", () => {
  const got = [];
  Patcher.runtime.signalBridge = { applyPatches: (p) => got.push(p) };
  Patcher.applySignalPatches([{ s: 1 }]);
  assert.deepEqual(got, [[{ s: 1 }]]);
  Patcher.runtime.signalBridge = null;
});
