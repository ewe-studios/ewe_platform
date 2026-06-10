// Tests for foundation-wasm-ui.js — the DOM layer. Covers ArrowParser/Applicator on
// a synthetic batch, and end-to-end: a REAL wasm module's Arrow batch applied to a
// mock DOM (the full WASM → JS → DOM loop).

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { FoundationWasm } from "../../../foundation_wasm/runtime/foundation-wasm.js";
import {
  ArrowParser,
  ArrowDomApplicator,
  NodeRegistry,
  Op,
} from "../../runtimes/foundation-wasm-ui.js";
import { MockDocument } from "../mock-dom.js";

const here = dirname(fileURLToPath(import.meta.url));
const wasmPath = join(here, "..", "..", "..", "foundation_wasm", "integration", "fixtures", "foundation_wasm_e2e.wasm");

test("ArrowDomApplicator applies create/attribute/class/append ops", () => {
  const doc = new MockDocument();
  const registry = new NodeRegistry();
  const parent = registry.register(1000, doc.createElement("div"));

  // Synthetic batch: create #1001, set its id attr, add class, append to parent.
  const batch = {
    count: 4,
    nodeIds: Uint32Array.from([1001, 1001, 1001, 1000]),
    operations: Uint8Array.from([Op.CREATE_ELEMENT, Op.SET_ATTRIBUTE, Op.ADD_CLASS, Op.APPEND_CHILD]),
    attribute: ["button", "id", "", "1001"],
    value: ["btn", "submit", "primary", ""],
    textVal: ["", "", "", ""],
  };
  new ArrowDomApplicator(registry, doc).apply(batch);

  const btn = registry.get(1001);
  assert.equal(btn.tag, "button");
  assert.equal(btn.className, "btn");
  assert.equal(btn.getAttribute("id"), "submit");
  assert.ok(btn.classList.contains("primary"));
  assert.equal(parent.children[0], btn);
});

test("ArrowDomApplicator throws on an unknown node id", () => {
  const applicator = new ArrowDomApplicator(new NodeRegistry(), new MockDocument());
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
  "e2e: a real module's Arrow batch parses and applies to the DOM",
  { skip: existsSync(wasmPath) ? false : "wasm fixture not built (run ./build-module.sh)" },
  () => {
    const doc = new MockDocument();
    const registry = new NodeRegistry();
    // The module emits SetText(5,"hi") and Remove(6); seed those nodes.
    const n5 = registry.register(5, doc.createElement("span"));
    const n6 = registry.register(6, doc.createElement("div"));
    const applicator = new ArrowDomApplicator(registry, doc);

    const rt = new FoundationWasm();
    let parsed = null;
    rt.setProtocolHandler(1, {
      apply: (_id, payload) => {
        parsed = ArrowParser.parse(payload);
        applicator.apply(parsed);
      },
    });

    const instance = new WebAssembly.Instance(new WebAssembly.Module(readFileSync(wasmPath)), {
      abi: rt.web_abi,
    });
    rt.init(instance);
    instance.exports.emit_arrow_batch();

    // Parser decoded the Rust ArrowEncoder output correctly.
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
