// Validates the DOM ABI extension (DomHeap + domAbi) against a REAL compiled module:
// foundation_wasm_ui's `allocate_dom_reference`, the naked `invoke_for_dom` fast-path
// (host_invoke_function_as_dom → DOM heap intern → handle crosses raw), and
// `drop_dom_reference` — the full Rust↔JS DOM round-trip.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { FoundationWasm } from "../../../foundation_wasm/runtime/foundation-wasm.js";
import { DomHeap, domAbi } from "../../runtimes/foundation-wasm-ui.js";

const here = dirname(fileURLToPath(import.meta.url));
const wasmPath = join(here, "..", "fixtures", "foundation_wasm_ui_e2e.wasm");
const skip = existsSync(wasmPath) ? false : "wasm fixture not built (run ./build-module.sh)";

function boot() {
  const rt = new FoundationWasm();
  const dom = new DomHeap({ window: null, document: null }); // no real DOM under node
  const abi = { ...rt.web_abi, ...domAbi(rt, dom) };
  const instance = new WebAssembly.Instance(new WebAssembly.Module(readFileSync(wasmPath)), {
    abi,
  });
  rt.init(instance);
  return { rt, dom, instance };
}

test("DomHeap reserves slots 0-4 and refuses to destroy them", () => {
  const dom = new DomHeap({ window: "w", document: { body: "b" } });
  assert.equal(dom.get(2n), "w"); // window
  assert.equal(dom.get(4n), "b"); // document.body
  assert.equal(dom.destroy(2n), false); // reserved
  const id = dom.create("node");
  assert.equal(dom.get(id), "node");
  assert.equal(dom.destroy(id), true);
  assert.equal(dom.get(id), undefined);
});

test("dom_allocate: pre-allocation lands after the reserved slots", { skip }, () => {
  const { dom, instance } = boot();
  const handle = instance.exports.dom_allocate();
  assert.equal(handle, BigInt(DomHeap.RESERVED_SLOTS) << 32n); // index 5, generation 0
  assert.equal(dom.get(BigInt(handle)), null); // empty until bound
});

test("dom_invoke_for_dom: naked as_dom interns the node, handle crosses raw", { skip }, () => {
  const { rt, dom, instance } = boot();
  const node = { nodeName: "DIV" };
  rt.functions.context = { testNode: node };
  const handle = instance.exports.dom_invoke_for_dom();
  assert.equal(dom.get(BigInt(handle)), node);
});

test("dom_drop: host_dom_drop_external_pointer retires the handle", { skip }, () => {
  const { rt, dom, instance } = boot();
  rt.functions.context = { testNode: { nodeName: "SPAN" } };
  const handle = instance.exports.dom_invoke_for_dom();
  assert.notEqual(dom.get(BigInt(handle)), undefined);
  instance.exports.dom_drop(BigInt(handle));
  assert.equal(dom.get(BigInt(handle)), undefined);
});
