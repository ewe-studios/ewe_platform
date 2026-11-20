// End-to-end test: drive foundation-wasm.js against a REAL compiled wasm module
// (fixtures/foundation_wasm_e2e.wasm). Proves the full WASM→JS transport: the module
// frames a host message and calls `host_apply`; foundation-wasm.js parses the
// envelope, routes to the registered handler, and ACKs (frees) the slot.
//
// The fixture is committed; rebuild it with ./build-module.sh after changing the
// module or the WasmEnvelope/encoder formats. The test skips if it's missing.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { FoundationWasm } from "../../runtime/foundation-wasm.js";

const here = dirname(fileURLToPath(import.meta.url));
const wasmPath = join(here, "..", "fixtures", "foundation_wasm_e2e.wasm");

test(
  "e2e: real wasm module ships a host message through host_apply",
  { skip: existsSync(wasmPath) ? false : "wasm fixture not built (run ./build-module.sh)" },
  () => {
    const bytes = readFileSync(wasmPath);

    const rt = new FoundationWasm();
    let captured = null;
    rt.setProtocolHandler(42, {
      apply: (memoryId, payload) => {
        captured = { memoryId, text: new TextDecoder().decode(payload) };
      },
    });

    const instance = new WebAssembly.Instance(new WebAssembly.Module(bytes), {
      abi: rt.web_abi,
    });
    rt.init(instance);

    // The module allocates a slot, frames the batch, and calls host_apply.
    instance.exports.emit_sample_message();

    assert.equal(captured?.text, "host message");

    // The slot was ACKed/freed in host_apply's finally — reading it now traps.
    assert.throws(() => rt.memory.get(captured.memoryId));
  },
);
