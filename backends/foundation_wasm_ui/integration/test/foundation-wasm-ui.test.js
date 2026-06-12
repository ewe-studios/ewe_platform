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
  DomOpApplicator,
  NodeRegistry,
  Op,
} from "../../runtimes/foundation-wasm-ui.js";
import { MockDocument } from "../mock-dom.js";

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
