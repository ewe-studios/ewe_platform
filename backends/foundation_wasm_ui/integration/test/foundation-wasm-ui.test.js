// Tests for foundation-wasm-ui.js — the DOM layer. Covers ColumnarParser/Applicator on
// a synthetic batch, and end-to-end: a REAL wasm module's Arrow batch applied to a
// mock DOM (the full WASM → JS → DOM loop).

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { FoundationWasm } from "../../../foundation_wasm/runtime/foundation-wasm.js";
import {
  ColumnarParser,
  decodeEnvelopeFrame,
  DomOpApplicator,
  NodeRegistry,
  Op,
  Patcher,
} from "../../runtimes/foundation-wasm-ui.js";
import { MockDocument } from "../mock-dom.js";

/** Envelope-frame a payload: `[protocol][version][len u32 LE][payload]`. */
function envelope(protocol, version, payload) {
  const out = new Uint8Array(6 + payload.length);
  out[0] = protocol;
  out[1] = version;
  new DataView(out.buffer).setUint32(2, payload.length, true);
  out.set(payload, 6);
  return out;
}

/** A document the applicator can mount onto (seedDocument needs `body`). */
function docWithBody() {
  const doc = new MockDocument();
  doc.body = doc.createElement("body");
  return doc;
}

const here = dirname(fileURLToPath(import.meta.url));
const wasmPath = join(here, "..", "..", "..", "foundation_wasm", "integration", "fixtures", "foundation_wasm_e2e.wasm");

test("DomOpApplicator applies create/register/attribute/class/append ops", () => {
  const doc = new MockDocument();
  const registry = new NodeRegistry();
  const parent = registry.register(1000, doc.createElement("div"));

  // Synthetic batch: create #1001 (known tag rides as id:4 = button), REGISTER
  // it (creation no longer auto-registers — feature 01), set its id attr
  // (known attr id:2), add class, append to parent.
  const batch = {
    count: 5,
    nodeIds: Uint32Array.from([1001, 1001, 1001, 1001, 1000]),
    operations: Uint8Array.from([
      Op.CREATE_ELEMENT,
      Op.REGISTER_NODE,
      Op.SET_ATTRIBUTE,
      Op.ADD_CLASS,
      Op.APPEND_CHILD,
    ]),
    attribute: ["id:4", "", "id:2", "", "1001"],
    value: ["btn", "", "submit", "primary", ""],
    textVal: ["", "", "", "", ""],
  };
  new DomOpApplicator(registry, doc).apply(batch);

  const btn = registry.get(1001);
  assert.equal(btn.tag, "button", "id:4 resolves through the mirrored TAG_NAMES table");
  assert.equal(btn.className, "btn");
  assert.equal(btn.getAttribute("id"), "submit", "id:2 resolves to the `id` attribute");
  assert.ok(btn.classList.contains("primary"));
  assert.equal(parent.children[0], btn);
});

test("created nodes stay staged until REGISTER_NODE promotes them", () => {
  const doc = new MockDocument();
  const registry = new NodeRegistry();
  const applicator = new DomOpApplicator(registry, doc);

  applicator.applyOne(Op.CREATE_ELEMENT, 7, "id:1", "", "");
  assert.equal(registry.get(7), undefined, "no auto-register");
  // Referencing the staged node before REGISTER_NODE is a producer bug.
  assert.throws(() => applicator.applyOne(Op.SET_TEXT_CONTENT, 7, "", "", "x"), /unknown node id 7/);

  applicator.applyOne(Op.REGISTER_NODE, 7, "", "", "");
  assert.equal(registry.get(7).tag, "div");
  // Re-register is a no-op, not an error.
  applicator.applyOne(Op.REGISTER_NODE, 7, "", "", "");

  applicator.applyOne(Op.UNREGISTER_NODE, 7, "", "", "");
  assert.equal(registry.get(7), undefined, "unregister clears the mapping only");
});

test("REPLACE_NODE takes its replacement from staged nodes and re-registers", () => {
  const doc = new MockDocument();
  const registry = new NodeRegistry();
  const applicator = new DomOpApplicator(registry, doc);
  const parent = registry.register(1, doc.createElement("div"));
  const oldNode = registry.register(2, doc.createElement("span"));
  parent.appendChild(oldNode);

  applicator.applyOne(Op.CREATE_ELEMENT, 3, "p", "", ""); // staged
  applicator.applyOne(Op.REPLACE_NODE, 2, "3", "", ""); // attribute = new id

  assert.equal(registry.get(2), undefined, "old id implicitly unregistered");
  assert.equal(registry.get(3).tag, "p", "new id implicitly registered");
  assert.equal(parent.children[0], registry.get(3));
});

test("MORPH_NODE resolves selectors and applies the packed action", () => {
  const doc = new MockDocument();
  const registry = new NodeRegistry();
  const applicator = new DomOpApplicator(registry, doc);

  // kind 0 (node id) + action 0 (replace children).
  const byId = registry.register(4, doc.createElement("div"));
  applicator.applyOne(Op.MORPH_NODE, 4, "0:0:", "", "<p>new</p>");
  assert.equal(byId.innerHTML, "<p>new</p>");

  // kind 3 (CSS query containing `:`) + action 3 (insert after).
  const target = doc.createElement("section");
  doc.root.appendChild(target);
  applicator.applyOne(Op.MORPH_NODE, 0, "3:3:section", "", "<aside/>");
  assert.deepEqual(target.adjacentHTML, [{ position: "afterend", html: "<aside/>" }]);
});

test("DomOpApplicator throws on an unknown node id", () => {
  const applicator = new DomOpApplicator(new NodeRegistry(), new MockDocument());
  assert.throws(
    () =>
      applicator.apply({
        count: 1,
        nodeIds: Uint32Array.from([999]),
        operations: Uint8Array.from([Op.SET_TEXT_CONTENT]),
        attribute: [""],
        value: [""],
        textVal: ["x"],
      }),
    /unknown node id 999/,
  );
});

test("Patcher.route('arrow') lazily installs a body-seeded applicator and mounts", () => {
  // The streaming path (Mode 1: a native App → SSE → mount-stream) decodes to
  // {type:"arrow", columns} and calls Patcher.route. Without an installed
  // applicator the apply was a SILENT no-op; route must lazily build one whose
  // NodeRegistry is seeded from the document so RESERVED.BODY (1) resolves.
  const doc = new MockDocument();
  doc.body = doc.createElement("body");
  Patcher.runtime.applicator = null; // isolate from any prior test's singleton

  // create #20 (div), register it, append to body (reserved id 1), set its text.
  const batch = {
    count: 4,
    nodeIds: Uint32Array.from([20, 20, 1, 20]),
    operations: Uint8Array.from([
      Op.CREATE_ELEMENT,
      Op.REGISTER_NODE,
      Op.APPEND_CHILD,
      Op.SET_TEXT_CONTENT,
    ]),
    attribute: ["div", "", "20", ""],
    value: ["", "", "", ""],
    textVal: ["", "", "", "hi from stream"],
  };

  Patcher.route({ type: "arrow", columns: batch }, null, doc);

  assert.ok(Patcher.runtime.applicator, "route installed the applicator");
  const mounted = doc.body.children[0];
  assert.ok(mounted, "the streamed node landed in the seeded document.body");
  assert.equal(mounted.tag, "div");
  assert.equal(mounted.textContent, "hi from stream");

  Patcher.runtime.applicator = null; // don't leak the singleton to other tests
});

// ─── mount renders identically across protocols ──────────────────────────────
// The App streams a DomOp batch; the wire encoding (custom binary columnar vs
// JSON) is an envelope detail. Both must reach the SAME DomOpApplicator so a
// mount looks identical whichever protocol it streamed over. (HTML is a
// different delivery — full markup via materialize, not a DomOp batch — covered
// last.) These guard the streaming path the Rust browser e2e exercises.

/** The greeting batch, as the columnar parser would hand it to the applicator. */
function greetingColumnarBatch() {
  return {
    count: 4,
    nodeIds: Uint32Array.from([20, 20, 1, 20]),
    operations: Uint8Array.from([
      Op.CREATE_ELEMENT,
      Op.REGISTER_NODE,
      Op.APPEND_CHILD,
      Op.SET_TEXT_CONTENT,
    ]),
    attribute: ["div", "", "20", ""],
    value: ["", "", "", ""],
    textVal: ["", "", "", "hi"],
  };
}

/** The SAME batch in the JsonEncoder flat-row form (snake_case, nulls). */
function greetingJsonRows() {
  return [
    { op_id: 0, node_id: 20, operation: Op.CREATE_ELEMENT, attribute: "div", value: null, text_val: null },
    { op_id: 1, node_id: 20, operation: Op.REGISTER_NODE, attribute: null, value: null, text_val: null },
    { op_id: 2, node_id: 1, operation: Op.APPEND_CHILD, attribute: "20", value: null, text_val: null },
    { op_id: 3, node_id: 20, operation: Op.SET_TEXT_CONTENT, attribute: null, value: null, text_val: "hi" },
  ];
}

function assertMounted(doc) {
  const mounted = doc.body.children[0];
  assert.ok(mounted, "node mounted onto the seeded document.body");
  assert.equal(mounted.tag, "div");
  assert.equal(mounted.textContent, "hi");
}

test("protocol 1 (custom binary columnar): envelope routes to the applicator and mounts", () => {
  const doc = docWithBody();
  Patcher.runtime.applicator = null;
  // Mirror the wire: protocol byte 1 v1 → {type:"arrow", columns}. (Encoding the
  // columnar bytes is Rust-side; the Rust browser e2e drives the real bytes —
  // here we assert the route+apply contract directly from a parsed batch.)
  const result = { type: "arrow", columns: greetingColumnarBatch() };
  Patcher.route(result, null, doc);
  assertMounted(doc);
  Patcher.runtime.applicator = null;
});

test("protocol 2 (JSON DomOp batch): envelope decodes + routes to the SAME applicator and mounts", () => {
  const doc = docWithBody();
  Patcher.runtime.applicator = null;
  const payload = new TextEncoder().encode(JSON.stringify(greetingJsonRows()));
  const result = decodeEnvelopeFrame(envelope(2, 1, payload));
  assert.equal(result.type, "arrow", "a JSON DomOp batch routes through the DomOp applicator path");

  Patcher.route(result, null, doc);
  assertMounted(doc);
  Patcher.runtime.applicator = null;
});

test("protocol 1 v2 (Apache Arrow IPC): route('arrow-ipc') reads the table into the SAME applicator", () => {
  const doc = docWithBody();
  Patcher.runtime.applicator = null;
  // A minimal table stub (numRows + getChild(name).get(i)) stands in for an
  // apache-arrow Table; the real IPC bytes are exercised by the Rust browser e2e.
  const rows = greetingJsonRows();
  const table = { numRows: rows.length, getChild: (name) => ({ get: (i) => rows[i][name] }) };
  Patcher.route({ type: "arrow-ipc", table }, null, doc);
  assertMounted(doc);
  Patcher.runtime.applicator = null;
});

test("a JSON signal-patch array still routes to the signal bridge (not the applicator)", () => {
  // Regression: only DomOp-row arrays (elements with `operation`) become a
  // DomOp batch; signal patches (`signalId`) must keep going to the bridge.
  const patches = [{ signalId: 1, value: "x" }];
  const result = decodeEnvelopeFrame(
    envelope(2, 1, new TextEncoder().encode(JSON.stringify(patches))),
  );
  assert.equal(result.type, "json", "signal patches are NOT a DomOp batch");

  const captured = [];
  Patcher.runtime.signalBridge = { applyPatches: (p) => captured.push(p) };
  Patcher.route(result, null, docWithBody());
  assert.deepEqual(captured[0], patches, "patches reached the signal bridge");
  Patcher.runtime.signalBridge = null;
});

test("HTML protocol: route materializes full markup into the mount target", () => {
  // HTML is the non-DomOp delivery — route('html') materializes markup into the
  // target element rather than applying ops. (MockDocument has no createRange,
  // so materialize falls back to target.innerHTML; hydrate no-ops on the mock.)
  const doc = new MockDocument();
  const target = doc.createElement("div");
  Patcher.route({ type: "html", html: "<p>hi html</p>" }, target, doc);
  assert.match(target.innerHTML, /hi html/, "markup materialized into the target");
});

test(
  "e2e: a real module's columnar batch parses and applies to the DOM",
  { skip: existsSync(wasmPath) ? false : "wasm fixture not built (run ./build-module.sh)" },
  () => {
    const doc = new MockDocument();
    const registry = new NodeRegistry();
    // The module emits SetText(5,"hi") and Remove(6); seed those nodes.
    const n5 = registry.register(5, doc.createElement("span"));
    const n6 = registry.register(6, doc.createElement("div"));
    const applicator = new DomOpApplicator(registry, doc);

    const rt = new FoundationWasm();
    let parsed = null;
    rt.setProtocolHandler(1, {
      apply: (_id, payload) => {
        parsed = ColumnarParser.parse(payload);
        applicator.apply(parsed);
      },
    });

    const instance = new WebAssembly.Instance(new WebAssembly.Module(readFileSync(wasmPath)), {
      abi: rt.web_abi,
    });
    rt.init(instance);
    instance.exports.emit_columnar_batch();

    // Parser decoded the Rust ColumnarEncoder output correctly.
    assert.equal(parsed.count, 2);
    assert.deepEqual([...parsed.nodeIds], [5, 6]);
    assert.deepEqual([...parsed.operations], [Op.SET_TEXT_CONTENT, Op.REMOVE_NODE]);
    assert.equal(parsed.textVal[0], "hi");

    // DOM effects applied.
    assert.equal(n5.textContent, "hi");
    assert.equal(n6.removed, true);
    assert.equal(registry.get(6), undefined);
  },
);
