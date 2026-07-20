// E2E test for F28: stream (WASM ↔ JS).
// FOUR functions, two on each side:
//   WASM exports:  stream_create, stream_send, stream_close
//   WASM imports:  host_sender_send, host_sender_end (JS provides)
// Drives the real compiled WASM module.
// Rebuild: cd integration && ./build-module.sh

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import {
  FoundationWasm,
  WasmStreamSender,
} from "../../runtime/foundation-wasm.js";

const here = dirname(fileURLToPath(import.meta.url));
const wasmPath = join(here, "..", "fixtures", "foundation_wasm_e2e.wasm");
const skip = existsSync(wasmPath) ? false : "wasm fixture not built";

function bootE2E() {
  const bytes = readFileSync(wasmPath);
  const rt = new FoundationWasm();
  const instance = new WebAssembly.Instance(new WebAssembly.Module(bytes), {
    abi: rt.web_abi,
  });
  rt.init(instance);
  return { rt, instance };
}

/** Encode a string into WASM linear memory; returns { ptr, len, data }.
 *  ptr and len are Numbers (safe for passing as u32 to WASM exports). */
function encodeInWasm(rt, str) {
  const buf = new TextEncoder().encode(str);
  const slot = rt.memory.create(buf.length);
  rt.memory.write(slot, buf);
  const sv = rt.memory.get(slot);
  return {
    ptr: Number(sv.ptr),
    len: Number(sv.len),
    data: sv.bytes,
  };
}

// ═══════════════════════════════════════════════════════════════════════
// JS → WASM: JS calls WASM exports to push chunks to the WASM-side
//           stream registry (ConcurrentQueue inside WASM).
// ═══════════════════════════════════════════════════════════════════════

test("e2e: JS pushes chunks to WASM-side stream", { skip }, () => {
  const { rt, instance } = bootE2E();

  // 1. WASM creates a stream — returns the ID.
  const streamId = instance.exports.stream_create();
  assert.ok(streamId > 0n);

  // 2. JS writes data into WASM linear memory.
  const c1 = encodeInWasm(rt, "hello");
  const c2 = encodeInWasm(rt, "world");

  // 3. JS pushes chunks to the WASM-side stream.
  const ok1 = instance.exports.stream_send(streamId, c1.ptr, c1.len, 0n);
  const ok2 = instance.exports.stream_send(streamId, c2.ptr, c2.len, 1n);
  assert.equal(ok1, 1);
  assert.equal(ok2, 1);

  // 4. JS closes the stream.
  const closed = instance.exports.stream_close(streamId);
  assert.equal(closed, 1);

  // 5. Sending to a closed stream returns 0.
  const fail = instance.exports.stream_send(streamId, c1.ptr, c1.len, 2n);
  assert.equal(fail, 0);
});

// ═══════════════════════════════════════════════════════════════════════
// WASM → JS: WASM calls host_sender_send / host_sender_end (FFI imports)
//           to push chunks to a JS-side WasmStreamSender.
// ═══════════════════════════════════════════════════════════════════════

test("e2e: WASM pushes chunks to JS-side stream", { skip }, () => {
  const { rt, instance } = bootE2E();

  const chunks = [];
  let ended = false;

  // 1. Create a JS-side stream.
  const sender = new WasmStreamSender(
    (c) => chunks.push(c),
    () => { ended = true; },
  );

  // 2. Write data into WASM memory so it can pass pointers.
  const d1 = encodeInWasm(rt, "from-wasm-1");
  const d2 = encodeInWasm(rt, "from-wasm-2");

  // 3. WASM calls host_sender_send via the e2e export.
  instance.exports.e2e_wasm_to_js_send(
    BigInt(sender.senderId), d1.ptr, d1.len, 0n,
  );
  assert.equal(chunks.length, 1);
  assert.deepEqual(chunks[0].data, new TextEncoder().encode("from-wasm-1"));
  assert.equal(chunks[0].sequence, 0);

  instance.exports.e2e_wasm_to_js_send(
    BigInt(sender.senderId), d2.ptr, d2.len, 1n,
  );
  assert.equal(chunks.length, 2);
  assert.deepEqual(chunks[1].data, new TextEncoder().encode("from-wasm-2"));

  // 4. WASM signals end-of-stream.
  instance.exports.e2e_wasm_to_js_end(BigInt(sender.senderId));
  assert.equal(ended, true);

  // 5. Further sends to ended stream are no-ops.
  assert.doesNotThrow(() => {
    instance.exports.e2e_wasm_to_js_send(
      BigInt(sender.senderId), d1.ptr, d1.len, 99n,
    );
  });
});

// ═══════════════════════════════════════════════════════════════════════
// Independence: WASM→JS and JS→WASM are separate.
// ═══════════════════════════════════════════════════════════════════════

test("e2e: both directions work independently", { skip }, () => {
  const { rt, instance } = bootE2E();

  // JS→WASM
  const wasmStream = instance.exports.stream_create();
  const data = encodeInWasm(rt, "js-to-wasm");
  assert.equal(instance.exports.stream_send(wasmStream, data.ptr, data.len, 0n), 1);
  assert.equal(instance.exports.stream_close(wasmStream), 1);

  // WASM→JS
  const chunks = [];
  let ended = false;
  const sender = new WasmStreamSender(
    (c) => chunks.push(c),
    () => { ended = true; },
  );
  const jsData = encodeInWasm(rt, "wasm-to-js");
  instance.exports.e2e_wasm_to_js_send(BigInt(sender.senderId), jsData.ptr, jsData.len, 0n);
  instance.exports.e2e_wasm_to_js_end(BigInt(sender.senderId));

  assert.equal(chunks.length, 1);
  assert.deepEqual(chunks[0].data, new TextEncoder().encode("wasm-to-js"));
  assert.equal(ended, true);
});

// ═══════════════════════════════════════════════════════════════════════
// Exports exist.
// ═══════════════════════════════════════════════════════════════════════

test("e2e: F28 stream exports exist on the WASM instance", { skip }, () => {
  const { instance } = bootE2E();
  assert.equal(typeof instance.exports.stream_create, "function");
  assert.equal(typeof instance.exports.stream_send, "function");
  assert.equal(typeof instance.exports.stream_close, "function");
  assert.equal(typeof instance.exports.e2e_wasm_to_js_send, "function");
  assert.equal(typeof instance.exports.e2e_wasm_to_js_end, "function");
});
