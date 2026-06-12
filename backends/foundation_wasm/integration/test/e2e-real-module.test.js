// End-to-end test: drive foundation-wasm.js against a REAL compiled wasm module
// (fixtures/foundation_wasm_e2e.wasm). Proves the full WASM→JS transport: the module
// frames a compact-columnar batch and calls `host_apply`; foundation-wasm.js parses the
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
  "e2e: real wasm module ships a columnar batch through host_apply",
  { skip: existsSync(wasmPath) ? false : "wasm fixture not built (run ./build-module.sh)" },
  () => {
    const bytes = readFileSync(wasmPath);

    const rt = new FoundationWasm();
    let captured = null;
    rt.setProtocolHandler(1, {
      apply: (memoryId, payload) => {
        // First u32 of the compact columnar layout is the row count.
        const rows = new DataView(
          payload.buffer,
          payload.byteOffset,
          payload.byteLength,
        ).getUint32(0, true);
        captured = { memoryId, len: payload.length, rows };
      },
    });

    const instance = new WebAssembly.Instance(new WebAssembly.Module(bytes), {
      abi: rt.web_abi,
    });
    rt.init(instance);

    // The module allocates a slot, frames the batch, and calls host_apply.
    instance.exports.emit_columnar_batch();

    assert.ok(captured, "the columnar handler was invoked");
    assert.equal(captured.rows, 2, "the batch carried both DomOps");
    assert.ok(captured.len > 0, "payload is non-empty");

    // The slot was ACKed/freed in host_apply's finally — reading it now traps.
    assert.throws(() => rt.memory.get(captured.memoryId));
  },
);
