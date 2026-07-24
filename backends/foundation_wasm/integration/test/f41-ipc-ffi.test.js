// F41 — IPC FFI e2e tests (binary wire format, host_ipc_invoke import).
//
// Validates the full WASM↔JS IPC round-trip through the new host_ipc_invoke
// import. The WASM module encodes an IpcRequest in binary wire format,
// calls host_ipc_invoke(ptr, len), and the JS handler decodes, processes,
// encodes the response, and writes it back to the WASM arena.
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

// ── Binary wire format encode/decode (pure JS, no WASM) ──────────────────

test("F41: binary wire format — encode/decode request round-trip", () => {
  const req = {
    ipc: "camera",
    action: "capture",
    content_type: 0, // Json
    target: null,
    payload: new TextEncoder().encode(JSON.stringify({ facing: "back" })),
  };
  const encoded = FoundationWasm._ipcEncodeRequest(req);
  const decoded = FoundationWasm._ipcDecodeRequest(encoded);

  assert.ok(decoded !== null, "should decode successfully");
  assert.equal(decoded.ipc, "camera");
  assert.equal(decoded.action, "capture");
  assert.equal(decoded.content_type, 0);
  assert.equal(decoded.target, null);
  const decodedPayload = JSON.parse(new TextDecoder().decode(decoded.payload));
  assert.equal(decodedPayload.facing, "back");
});

test("F41: binary wire format — encode/decode request with target", () => {
  const req = {
    ipc: "emit",
    action: "broadcast",
    content_type: 2, // Binary
    target: "wv_3",
    payload: new Uint8Array([1, 2, 3]),
  };
  const encoded = FoundationWasm._ipcEncodeRequest(req);
  const decoded = FoundationWasm._ipcDecodeRequest(encoded);

  assert.ok(decoded !== null);
  assert.equal(decoded.ipc, "emit");
  assert.equal(decoded.target, "wv_3");
  assert.equal(decoded.content_type, 2);
  assert.equal(decoded.payload.length, 3);
  assert.equal(decoded.payload[0], 1);
  assert.equal(decoded.payload[2], 3);
});

test("F41: binary wire format — encode/decode response round-trip", () => {
  const respBytes = FoundationWasm._ipcEncodeResponse(0, new TextEncoder().encode(JSON.stringify({ handle: 7 })));
  const decoded = FoundationWasm._ipcDecodeResponse(respBytes);

  assert.ok(decoded !== null);
  assert.equal(decoded.content_type, 0);
  const payload = JSON.parse(new TextDecoder().decode(decoded.payload));
  assert.equal(payload.handle, 7);
});

test("F41: binary wire format — decode garbage returns null", () => {
  assert.equal(FoundationWasm._ipcDecodeRequest(new Uint8Array([0])), null);
  assert.equal(FoundationWasm._ipcDecodeResponse(new Uint8Array([0])), null);
  assert.equal(FoundationWasm._ipcDecodeResponse(new Uint8Array([])), null);
});

// ── host_ipc_invoke: WASM→JS round-trip (echo handler) ──────────────────

test("F41: host_ipc_invoke — WASM→JS echo round-trip", { skip }, () => {
  const { rt, instance } = bootE2E();

  // Register an IPC handler that echoes the payload back
  rt.registerIpcHandler({
    onIpc(req) {
      return {
        content_type: req.content_type,
        payload: req.payload, // echo
      };
    },
  });

  const result = instance.exports.e2e_ipc_invoke_echo();
  assert.equal(result, 1);
});

// ── host_ipc_invoke: WASM→JS camera:open round-trip ─────────────────────

test("F41: host_ipc_invoke — camera:open via JS handler", { skip }, () => {
  const { rt, instance } = bootE2E();

  let received = null;
  rt.registerIpcHandler({
    onIpc(req) {
      received = {
        ipc: req.ipc,
        action: req.action,
        payload: new TextDecoder().decode(req.payload),
      };
      return {
        content_type: 0,
        payload: new TextEncoder().encode(JSON.stringify({ handle: 42 })),
      };
    },
  });

  const result = instance.exports.e2e_ipc_invoke_camera_open();
  assert.equal(result, 1);
  assert.ok(received !== null, "JS handler should have been called");
  assert.equal(received.ipc, "camera");
  assert.equal(received.action, "open");
  const payload = JSON.parse(received.payload);
  assert.equal(payload.facing, "back");
});

// ── No handler: decode fails on error byte ─────────────────────────────

test("F41: host_ipc_invoke — no handler returns decode error", { skip }, () => {
  const { rt, instance } = bootE2E();
  // No handler: _ipcErrorAlloc writes [0xFF], decode_response fails → -3
  const result = instance.exports.e2e_ipc_invoke_echo();
  assert.equal(result, -3, "should fail decode on error marker");
});

// ── Stream open/read/close ──────────────────────────────────────────────

test("F41: host_ipc_stream_open/read/close — basic lifecycle", { skip }, () => {
  const { rt } = bootE2E();

  rt.registerIpcHandler({
    onIpcStream(req, stream) {
      stream.write({ content_type: 0, payload: new TextEncoder().encode("chunk1") });
      stream.write({ content_type: 0, payload: new TextEncoder().encode("chunk2") });
      stream.end();
    },
  });

  // Create the stream via WASM calling host_ipc_stream_open
  const encoded = FoundationWasm._ipcEncodeRequest({
    ipc: "camera", action: "snap", content_type: 0, target: null,
    payload: new Uint8Array(0),
  });
  // Write into WASM memory so ptr/len resolve
  const reqSlot = rt.memory.create(encoded.length);
  rt.memory.write(reqSlot, encoded);
  const info = rt.memory.get(reqSlot);

  const allocId = rt._dispatchIpcStreamOpen(info.ptr, info.len);
  rt.memory.dispose(reqSlot);
  assert.ok(allocId !== 0n, "stream should be created: " + allocId);

  // Read chunks
  const chunk1Id = rt._dispatchIpcStreamRead(Number(allocId));
  assert.ok(chunk1Id !== 0n, "chunk1 should be available");
  const chunk1Bytes = new Uint8Array(
    rt.bridge.memory.buffer,
    Number(rt.bridge.exports.allocation_start_pointer(chunk1Id)),
    Number(rt.bridge.exports.allocation_length(chunk1Id)),
  );
  const chunk1Dec = FoundationWasm._ipcDecodeResponse(chunk1Bytes);
  assert.equal(new TextDecoder().decode(chunk1Dec.payload), "chunk1");
  rt.bridge.exports.dispose_allocation(chunk1Id);

  const chunk2Id = rt._dispatchIpcStreamRead(Number(allocId));
  assert.ok(chunk2Id !== 0n, "chunk2 should be available");
  rt.bridge.exports.dispose_allocation(chunk2Id);

  // Should be closed now
  const closedId = rt._dispatchIpcStreamRead(Number(allocId));
  assert.equal(closedId, 0n, "stream should be closed");
});

// ═══════════════════════════════════════════════════════════════════════
// WASM E2E stream round-trip (WASM→host→stream→WASM reads chunks)
// ═══════════════════════════════════════════════════════════════════════

test("F41: WASM stream E2E — open/write 3 chunks/read all/close via exports", { skip }, () => {
  const { rt, instance } = bootE2E();

  rt.registerIpcHandler({
    onIpcStream(req, stream) {
      stream.write({ content_type: 0, payload: new TextEncoder().encode(JSON.stringify({ frame: 1 })) });
      stream.write({ content_type: 0, payload: new TextEncoder().encode(JSON.stringify({ frame: 2 })) });
      stream.write({ content_type: 0, payload: new TextEncoder().encode(JSON.stringify({ frame: 3 })) });
      stream.end();
    },
  });

  const result = instance.exports.e2e_ipc_stream_roundtrip();
  assert.equal(result, 1, "stream roundtrip should read 3 chunks");
});

test("F41: WASM stream — open returns non-zero ID", { skip }, () => {
  const { rt, instance } = bootE2E();

  rt.registerIpcHandler({
    onIpcStream(req, stream) {
      stream.write({ content_type: 0, payload: new Uint8Array([1]) });
      stream.end();
    },
  });

  const streamId = instance.exports.e2e_ipc_stream_open();
  assert.ok(streamId !== 0n && streamId !== 0, "stream ID should be non-zero: " + streamId);
});

test("F41: WASM stream — read returns 0 when no handler", { skip }, () => {
  const { instance } = bootE2E();
  // No handler registered — host_ipc_stream_open returns 0
  const streamId = instance.exports.e2e_ipc_stream_open();
  assert.equal(streamId, 0n, "stream should fail without handler");
});

test("F41: WASM stream — close is idempotent", { skip }, () => {
  const { rt, instance } = bootE2E();

  rt.registerIpcHandler({
    onIpcStream(req, stream) {
      stream.write({ content_type: 0, payload: new TextEncoder().encode("data") });
      stream.end();
    },
  });

  const streamId = instance.exports.e2e_ipc_stream_open();
  assert.ok(streamId !== 0n, "open should return non-zero stream ID");

  // Read once — alloc_id could be 0 (first valid allocation)
  const cid = instance.exports.e2e_ipc_stream_read(streamId);
  const ptr = instance.exports.allocation_start_pointer(cid);
  const len = instance.exports.allocation_length(cid);
  const bytes = new Uint8Array(instance.exports.memory.buffer, Number(ptr), Number(len));
  const decoded = FoundationWasm._ipcDecodeResponse(bytes);
  assert.ok(decoded !== null, "first chunk should be valid");
  assert.equal(new TextDecoder().decode(decoded.payload), "data");
  instance.exports.dispose_allocation(cid);

  instance.exports.e2e_ipc_stream_close(streamId);
  // Close again — should not crash
  instance.exports.e2e_ipc_stream_close(streamId);
});

// ═══════════════════════════════════════════════════════════════════════
// Content-type round-trips: Json / Arrow / Binary through full host_ipc_invoke
// ═══════════════════════════════════════════════════════════════════════

test("F41: content_type — JS wire format round-trip Arrow (ct=1)", () => {
  const req = {
    ipc: "analytics", action: "query",
    content_type: 1, // Arrow
    target: null,
    payload: new Uint8Array([0, 1, 2, 3]),
  };
  const enc = FoundationWasm._ipcEncodeRequest(req);
  const dec = FoundationWasm._ipcDecodeRequest(enc);
  assert.ok(dec !== null);
  assert.equal(dec.content_type, 1);
  assert.equal(dec.payload.length, 4);
  assert.equal(dec.payload[0], 0);
});

test("F41: content_type — decode_response handles all content types", () => {
  for (let ct = 0; ct <= 2; ct++) {
    const respBytes = FoundationWasm._ipcEncodeResponse(ct, new Uint8Array([1, 2, 3]));
    const decoded = FoundationWasm._ipcDecodeResponse(respBytes);
    assert.ok(decoded !== null, "should decode content_type " + ct);
    assert.equal(decoded.content_type, ct);
    assert.equal(decoded.payload.length, 3);
  }
});

test("F41: WASM E2E — Arrow content_type round-trip", { skip }, () => {
  const { rt, instance } = bootE2E();

  rt.registerIpcHandler({
    onIpc(req) {
      // Echo with same content_type but reversed payload
      return { content_type: req.content_type, payload: new Uint8Array([3, 2, 1, 0]) };
    },
  });

  const result = instance.exports.e2e_ipc_invoke_arrow();
  assert.equal(result, 1, "Arrow round-trip should succeed");
});

test("F41: WASM E2E — Binary content_type round-trip", { skip }, () => {
  const { rt, instance } = bootE2E();

  rt.registerIpcHandler({
    onIpc(req) {
      // Echo with same content_type but different payload
      return { content_type: req.content_type, payload: new Uint8Array([0xCA, 0xFE]) };
    },
  });

  const result = instance.exports.e2e_ipc_invoke_binary();
  assert.equal(result, 1, "Binary round-trip should succeed");
});

test("F41: content_type — dispatch is content_type agnostic", { skip }, () => {
  const { rt, instance } = bootE2E();

  // Handler echoes same content_type but reverses payload — matching WASM expectation
  rt.registerIpcHandler({
    onIpc(req) {
      // Reverse the payload bytes for the Arrow test
      var reversed = new Uint8Array(req.payload.length);
      for (var i = 0; i < req.payload.length; i++) reversed[i] = req.payload[req.payload.length - 1 - i];
      return { content_type: req.content_type, payload: reversed };
    },
  });

  // Arrow E2E: WASM sends [0,1,2,3], expects reversed [3,2,1,0]
  const result = instance.exports.e2e_ipc_invoke_arrow();
  assert.equal(result, 1, "Arrow round-trip with reversed payload should succeed");
});
