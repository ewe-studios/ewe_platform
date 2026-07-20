// E2E test for F28: WASM ↔ JS streaming.
//
// WASM-led:  host_stream_create() → ID → host_sender_send/end
//            JS binds callbacks, WASM pushes chunks (buffered until bound).
// JS→WASM:   stream_create/send/close — JS pushes chunks to WASM registry.
//
// Rebuild: cd integration && ./build-module.sh

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { FoundationWasm } from "../../runtime/foundation-wasm.js";

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

function encodeInWasm(rt, str) {
  const buf = new TextEncoder().encode(str);
  const slot = rt.memory.create(buf.length);
  rt.memory.write(slot, buf);
  const sv = rt.memory.get(slot);
  return { ptr: Number(sv.ptr), len: Number(sv.len), data: sv.bytes };
}

// ═══════════════════════════════════════════════════════════════════════
// WASM-led: WASM creates a host stream, pushes chunks (buffered),
//           JS binds callbacks later → drains buffer.
// ═══════════════════════════════════════════════════════════════════════

test("e2e: WASM-led stream — create, push (buffered), bind, drain, end", { skip }, () => {
  const { rt, instance } = bootE2E();

  // 1. WASM creates a host-side stream.
  const streamId = Number(instance.exports.e2e_stream_create());
  assert.ok(streamId > 0);

  // 2. Verify the stream is in the registry, with empty buffer.
  const stream = rt._streamRegistry[streamId];
  assert.ok(stream !== undefined);
  assert.deepEqual(stream.chunks, []);
  assert.equal(stream.onChunk, null);
  assert.equal(stream.onEnd, null);
  assert.equal(stream.ended, false);

  // 3. WASM pushes chunks — buffered because onChunk is null.
  const d1 = encodeInWasm(rt, "chunk-one");
  const d2 = encodeInWasm(rt, "chunk-two");
  instance.exports.e2e_stream_send(BigInt(streamId), d1.ptr, d1.len, 0n);
  instance.exports.e2e_stream_send(BigInt(streamId), d2.ptr, d2.len, 1n);
  assert.equal(stream.chunks.length, 2);
  assert.deepEqual(stream.chunks[0].data, d1.data);
  assert.equal(stream.chunks[0].sequence, 0);
  assert.deepEqual(stream.chunks[1].data, d2.data);
  assert.equal(stream.chunks[1].sequence, 1);

  // 4. JS binds callbacks — drains the buffer.
  const received = [];
  let ended = false;
  stream.bind(
    (c) => received.push(c),
    () => { ended = true; },
  );
  assert.equal(received.length, 2);

  // 5. WASM pushes more — delivered directly now.
  const d3 = encodeInWasm(rt, "chunk-three");
  instance.exports.e2e_stream_send(BigInt(streamId), d3.ptr, d3.len, 2n);
  assert.equal(received.length, 3);

  // 6. WASM ends the stream.
  instance.exports.e2e_stream_end(BigInt(streamId));
  assert.equal(ended, true);
  assert.ok(rt._streamRegistry[streamId] === undefined); // cleaned up
});

// ═══════════════════════════════════════════════════════════════════════
// WASM-led: create, end immediately (no chunks).
// ═══════════════════════════════════════════════════════════════════════

test("e2e: WASM-led stream — create and end immediately", { skip }, () => {
  const { rt, instance } = bootE2E();

  const streamId = Number(instance.exports.e2e_stream_create());
  const stream = rt._streamRegistry[streamId];
  let ended = false;
  stream.bind(null, () => { ended = true; });

  instance.exports.e2e_stream_end(BigInt(streamId));
  assert.equal(ended, true);
});

// ═══════════════════════════════════════════════════════════════════════
// JS→WASM: JS pushes chunks to WASM-side registry.
// ═══════════════════════════════════════════════════════════════════════

test("e2e: JS→WASM — create, send, close", { skip }, () => {
  const { rt, instance } = bootE2E();

  const streamId = instance.exports.stream_create();
  assert.ok(streamId > 0n);

  const c1 = encodeInWasm(rt, "hello-wasm");
  const ok1 = instance.exports.stream_send(streamId, c1.ptr, c1.len, 0n);
  assert.equal(ok1, 1);

  const closed = instance.exports.stream_close(streamId);
  assert.equal(closed, 1);
});
