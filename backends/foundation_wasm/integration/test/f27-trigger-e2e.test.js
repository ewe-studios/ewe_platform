// E2E test for F27: capability/IPC trigger dispatch (WASM → JS through host_apply).
// Drives a REAL compiled wasm module (fixtures/foundation_wasm_e2e.wasm) that
// frames a capability/IPC trigger envelope (protocol bytes 3/4) and ships it
// through the uniform host_apply transport. The JS runtime dispatches to the
// registered trigger handlers, proving the full WASM→JS trigger path.
//
// Rebuild the fixture: cd integration && ./build-module.sh

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { FoundationWasm } from "../../runtime/foundation-wasm.js";

const here = dirname(fileURLToPath(import.meta.url));
const wasmPath = join(here, "..", "fixtures", "foundation_wasm_e2e.wasm");
const skip = existsSync(wasmPath) ? false : "wasm fixture not built (run ./build-module.sh)";

function bootE2E() {
  const bytes = readFileSync(wasmPath);
  const rt = new FoundationWasm();
  const instance = new WebAssembly.Instance(new WebAssembly.Module(bytes), {
    abi: rt.web_abi,
  });
  rt.init(instance);
  return { rt, instance };
}

function encodeIntoArena(rt, str) {
  const encoded = new TextEncoder().encode(str);
  const slot = rt.memory.create(encoded.length);
  rt.memory.write(slot, encoded);
  return rt.memory.get(slot);
}

// ═══════════════════════════════════════════════════════════════════════
// F27 — Capability Trigger (protocol byte 3)
// ═══════════════════════════════════════════════════════════════════════

test("e2e: capability trigger dispatches through host_apply to JS handler", { skip }, () => {
  const { rt, instance } = bootE2E();

  let triggerReceived = null;
  rt._capTriggerHandler = (req) => {
    triggerReceived = req;
  };

  const payload = JSON.stringify({ capability: "camera", action: "capture", params: { res: "1080p" } });
  const sv = encodeIntoArena(rt, payload);

  // WASM exports: e2e_trigger_capability(ptr, len) — frames and ships via host_apply
  instance.exports.e2e_trigger_capability(BigInt(sv.ptr), payload.length);

  assert.ok(triggerReceived !== null, "capability trigger handler was invoked");
  assert.equal(triggerReceived.capability, "camera");
  assert.equal(triggerReceived.action, "capture");
  assert.deepEqual(triggerReceived.params, { res: "1080p" });
});

test("e2e: capability trigger with no handler does not throw", { skip }, () => {
  const { rt, instance } = bootE2E();

  // No handler registered — _dispatchCapTrigger should catch and ignore
  rt._capTriggerHandler = null;

  const payload = JSON.stringify({ capability: "none", action: "ignore" });
  const sv = encodeIntoArena(rt, payload);

  // Should NOT throw — _dispatchCapTrigger catches errors and disposes the slot
  assert.doesNotThrow(() => {
    instance.exports.e2e_trigger_capability(BigInt(sv.ptr), payload.length);
  });
});

// ═══════════════════════════════════════════════════════════════════════
// F27 — IPC Trigger (protocol byte 4)
// ═══════════════════════════════════════════════════════════════════════

test("e2e: IPC trigger dispatches through host_apply to JS handler", { skip }, () => {
  const { rt, instance } = bootE2E();

  let triggerReceived = null;
  rt._ipcTriggerHandler = (req) => {
    triggerReceived = req;
  };

  const payload = JSON.stringify({ ipc: "system", action: "get_info" });
  const sv = encodeIntoArena(rt, payload);

  instance.exports.e2e_trigger_ipc(BigInt(sv.ptr), payload.length);

  assert.ok(triggerReceived !== null, "IPC trigger handler was invoked");
  assert.equal(triggerReceived.ipc, "system");
  assert.equal(triggerReceived.action, "get_info");
});

test("e2e: trigger handlers are independent (IPC ≠ capability)", { skip }, () => {
  const { rt, instance } = bootE2E();

  const capLog = [], ipcLog = [];
  rt._capTriggerHandler = (req) => capLog.push(req);
  rt._ipcTriggerHandler = (req) => ipcLog.push(req);

  // Send IPC trigger — only ipcLog should receive
  let payload = JSON.stringify({ ipc: "test", action: "ping" });
  let sv = encodeIntoArena(rt, payload);
  instance.exports.e2e_trigger_ipc(BigInt(sv.ptr), payload.length);
  assert.equal(capLog.length, 0);
  assert.equal(ipcLog.length, 1);

  // Send capability trigger — only capLog should receive
  payload = JSON.stringify({ capability: "test", action: "do" });
  sv = encodeIntoArena(rt, payload);
  instance.exports.e2e_trigger_capability(BigInt(sv.ptr), payload.length);
  assert.equal(capLog.length, 1);
  assert.equal(ipcLog.length, 1);
});

// ═══════════════════════════════════════════════════════════════════════
// Verify the e2e module exports exist
// ═══════════════════════════════════════════════════════════════════════

test("e2e module exports trigger functions", { skip }, () => {
  const { instance } = bootE2E();
  assert.equal(typeof instance.exports.e2e_trigger_capability, "function");
  assert.equal(typeof instance.exports.e2e_trigger_ipc, "function");
});
