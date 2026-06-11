// Validates the CUSTOM BINARY protocol (byte 0 = the batch-instructions format,
// decision 022) end-to-end against a REAL compiled module: Rust BatchInstructionsV1
// packs an Operations stream + texts pool into one envelope slot and ships it via
// host_apply; the core dispatcher routes byte 0 into the existing BatchInstructions
// runtime; the registered BATCH_OP_APPLY_DOM operation applies each DomOp row
// (quantized V2 params, strings via the texts pool) to the DOM.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { FoundationWasm } from "../../../foundation_wasm/runtime/foundation-wasm.js";
import {
  ArrowDomApplicator,
  NodeRegistry,
  DomHeap,
  domAbi,
  registerDomBatchOperation,
  BATCH_OP_APPLY_DOM,
} from "../../runtimes/foundation-wasm-ui.js";
import { MockDocument } from "../mock-dom.js";

const here = dirname(fileURLToPath(import.meta.url));
const wasmPath = join(here, "..", "fixtures", "foundation_wasm_ui_e2e.wasm");
const skip = existsSync(wasmPath) ? false : "wasm fixture not built (run ./build-module.sh)";

test("byte-0 batch protocol: DomOps ride the Instructions format end-to-end", { skip }, () => {
  const doc = new MockDocument();
  const registry = new NodeRegistry();
  // The module emits SetText(5, "batch hi") and Remove(6); seed those nodes.
  const n5 = registry.register(5, doc.createElement("span"));
  const n6 = registry.register(6, doc.createElement("div"));
  const applicator = new ArrowDomApplicator(registry, doc);

  const rt = new FoundationWasm();
  // The byte-0 handler is PRE-WIRED by the core runtime; the DOM layer only
  // registers its batch operation (the selective opt-in extension point).
  registerDomBatchOperation(rt, applicator);
  assert.ok(rt.batches.operations.has(BATCH_OP_APPLY_DOM));

  const abi = { ...rt.web_abi, ...domAbi(rt, new DomHeap({ window: null, document: null })) };
  const instance = new WebAssembly.Instance(new WebAssembly.Module(readFileSync(wasmPath)), {
    abi,
  });
  rt.init(instance);

  instance.exports.emit_batch_dom_ops();

  assert.equal(n5.textContent, "batch hi", "SetText applied through the batch protocol");
  assert.equal(n6.removed, true, "Remove applied through the batch protocol");
  assert.equal(registry.get(6), undefined, "removed node unregistered");
});
