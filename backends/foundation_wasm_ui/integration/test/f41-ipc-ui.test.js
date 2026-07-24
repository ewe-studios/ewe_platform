// F41 — Unified IPC integration tests for foundation_wasm_ui.
//
// Validates IPC through the FULL runtime stack: foundation-wasm.js +
// foundation-wasm-ui.js (domAbi) + ipc-bridge.js. Tests:
//   1. WASM→host IPC (host_ipc_invoke) through merged web_abi + domAbi
//   2. Chrome/camera capability calls from the UI context
//   3. Host→WASM event push (ipc_handle_event via TriggerRegistry)
//   4. ipc-bridge.js registerIpcTriggers registration
//   5. Streaming IPC with domAbi present
//
// Rebuild: cd integration && ./build-module.sh

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
  const dom = new DomHeap({ window: null, document: null });
  const abi = { ...rt.web_abi, ...domAbi(rt, dom) };
  const mod = new WebAssembly.Module(readFileSync(wasmPath));
  const instance = new WebAssembly.Instance(mod, { abi });
  rt.init(instance);
  return { rt, dom, instance };
}

// ═══════════════════════════════════════════════════════════════════════
// WASM→host IPC through the full runtime stack (web_abi + domAbi merged)
// ═══════════════════════════════════════════════════════════════════════

test("F41-UI: host_ipc_invoke — echo round-trip through merged ABI", { skip }, () => {
  const { rt, instance } = boot();

  rt.registerIpcHandler({
    onIpc(req) {
      return { content_type: 0, payload: req.payload };
    },
  });

  const result = instance.exports.e2e_ui_ipc_echo();
  assert.equal(result, 1, "echo should succeed");
});

test("F41-UI: host_ipc_invoke — chrome:set_title through merged ABI", { skip }, () => {
  const { rt, instance } = boot();

  let received = null;
  rt.registerIpcHandler({
    onIpc(req) {
      received = { ipc: req.ipc, action: req.action, payload: new TextDecoder().decode(req.payload) };
      return { content_type: 0, payload: new TextEncoder().encode(JSON.stringify({ ok: true })) };
    },
  });

  const result = instance.exports.e2e_ui_chrome_set_title();
  assert.equal(result, 1, "chrome:set_title should succeed");
  assert.ok(received !== null);
  assert.equal(received.ipc, "chrome");
  assert.equal(received.action, "set_title");
  const payload = JSON.parse(received.payload);
  assert.equal(payload.title, "Test Page");
});

test("F41-UI: host_ipc_invoke — no handler returns decode error", { skip }, () => {
  const { instance } = boot();
  const result = instance.exports.e2e_ui_ipc_echo();
  assert.equal(result, -3, "decode should fail on error marker");
});

// ═══════════════════════════════════════════════════════════════════════
// Host→WASM event push (ipc_handle_event via TriggerRegistry)
// ═══════════════════════════════════════════════════════════════════════

test("F41-UI: host→WASM — trigger event with handler registered", { skip }, () => {
  const { rt, instance } = boot();

  // Register a handler on the WASM-side TriggerRegistry
  instance.exports.e2e_ipc_register_handler(42);

  // Now trigger an event — WASM calls ipc_handle_event internally
  const result = instance.exports.e2e_ui_trigger_event();
  // The handler echoes "handled-42-toolbar_tap"
  assert.equal(result, 1, "trigger event should be handled");
});

test("F41-UI: host→WASM — trigger with no handler returns 0", { skip }, () => {
  const { rt, instance } = boot();
  // No handler registered — will get empty trigger or stale state
  // The export returns -1 on alloc_id == 0
  const result = instance.exports.e2e_ui_trigger_event();
  assert.equal(result, -1, "trigger should fail without handler");
});

// ═══════════════════════════════════════════════════════════════════════
// ipc-bridge.js: registerIpcTriggers with full runtime
// ═══════════════════════════════════════════════════════════════════════

test("F41-UI: ipc-bridge — registerIpcTriggers sets handlers", { skip }, async () => {
  const { rt } = boot();

  // ipc-bridge.js is an IIFE that sets globalThis.registerIpcTriggers
  await import("../../runtimes/ipc-bridge.js");
  const registerIpcTriggers = globalThis.registerIpcTriggers;
  assert.ok(typeof registerIpcTriggers === 'function', 'registerIpcTriggers should be a function');
  registerIpcTriggers(rt);

  assert.ok(rt._ipcHandler !== null, "IPC handler should be registered");
  assert.ok(rt._ipcTriggerHandler !== null, "IPC trigger should be registered");
  assert.ok(rt._capTriggerHandler !== null, "capability trigger should be registered");
});

// ═══════════════════════════════════════════════════════════════════════
// Streaming IPC through the full runtime
// ═══════════════════════════════════════════════════════════════════════

test("F41-UI: host_ipc_stream — open/write/read/close through merged ABI", { skip }, () => {
  const { rt } = boot();

  rt.registerIpcHandler({
    onIpcStream(req, stream) {
      stream.write({ content_type: 0, payload: new TextEncoder().encode("item-a") });
      stream.write({ content_type: 0, payload: new TextEncoder().encode("item-b") });
      stream.end();
    },
  });

  // Encode a stream-open request and write to WASM memory
  const encoded = FoundationWasm._ipcEncodeRequest({
    ipc: "camera", action: "snap", content_type: 0, target: null,
    payload: new Uint8Array(0),
  });
  const reqSlot = rt.memory.create(encoded.length);
  rt.memory.write(reqSlot, encoded);
  const info = rt.memory.get(reqSlot);

  const streamId = rt._dispatchIpcStreamOpen(info.ptr, info.len);
  rt.memory.dispose(reqSlot);
  assert.ok(streamId !== 0n, "stream should open: " + streamId);

  // Read first chunk
  const c1Id = rt._dispatchIpcStreamRead(Number(streamId));
  assert.ok(c1Id !== 0n, "chunk1 should be available");
  const c1Bytes = new Uint8Array(
    rt.bridge.memory.buffer,
    Number(rt.bridge.exports.allocation_start_pointer(c1Id)),
    Number(rt.bridge.exports.allocation_length(c1Id)),
  );
  assert.equal(
    new TextDecoder().decode(FoundationWasm._ipcDecodeResponse(c1Bytes).payload),
    "item-a"
  );
  rt.bridge.exports.dispose_allocation(c1Id);

  // Read second chunk
  const c2Id = rt._dispatchIpcStreamRead(Number(streamId));
  assert.ok(c2Id !== 0n, "chunk2 should be available");
  rt.bridge.exports.dispose_allocation(c2Id);

  // Should be done
  const done = rt._dispatchIpcStreamRead(Number(streamId));
  assert.equal(done, 0n, "stream should be closed");

  rt._dispatchIpcStreamClose(Number(streamId));
});

// ═══════════════════════════════════════════════════════════════════════
// Bidirectional: WASM→host → host→WASM reply pattern
// ═══════════════════════════════════════════════════════════════════════

test("F41-UI: bidirectional — WASM→host request triggers host→WASM reply", { skip }, () => {
  const { rt, instance } = boot();

  // Register a WASM-side handler that will receive the reply event
  instance.exports.e2e_ipc_register_handler(99);

  // Host handler: when invoked, pushes a reply event back to WASM
  rt.registerIpcHandler({
    onIpc(req) {
      // The host processes the request, then pushes a reply event
      // back to WASM via the ipc_handle_event export
      const replyData = new TextEncoder().encode(JSON.stringify({
        ipc: "reply", action: "result", payload: { original_action: req.action }
      }));
      const replySlot = rt.memory.create(replyData.length);
      rt.memory.write(replySlot, replyData);
      const rInfo = rt.memory.get(replySlot);
      rt.bridge.exports.ipc_handle_event(rInfo.ptr, rInfo.len);
      rt.memory.dispose(replySlot);
      // Return the normal response too
      return { content_type: 0, payload: req.payload };
    },
  });

  // Invoke through the WASM export
  const result = instance.exports.e2e_ui_chrome_set_title();
  assert.equal(result, 1, "bidirectional request should succeed");
});
