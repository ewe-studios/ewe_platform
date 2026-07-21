// E2E tests for F23 (capabilities) and F25 (IPC) — both directions.
//
// WASM→host: WASM registers JS bridge function via register_function,
//            invokes it with typed params, gets result back.
// Host→WASM: host calls triggerCapability/triggerIpc on FoundationWasm,
//            WASM-side TriggerRegistry dispatches to registered handler.
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

// ═══════════════════════════════════════════════════════════════════════
// F23 — Capability: WASM→host (register JS fn + invoke)
// ═══════════════════════════════════════════════════════════════════════

test("e2e: capability WASM→host — register bridge fn and invoke", { skip }, () => {
  const { instance } = bootE2E();
  const result = instance.exports.e2e_invoke_capability();
  assert.equal(result, 1, "e2e_invoke_capability should return 1 on success");
});

test("e2e: capability host→WASM — trigger via host_apply", { skip }, () => {
  const { rt, instance } = bootE2E();

  let triggerReceived = null;
  rt._capTriggerHandler = (req) => { triggerReceived = req; };

  // Use the existing e2e_trigger_capability export (protocol byte 3)
  const payload = JSON.stringify({ capability: "test", action: "do", payload: {} });
  const buf = new TextEncoder().encode(payload);
  const slot = rt.memory.create(buf.length);
  rt.memory.write(slot, buf);
  const sv = rt.memory.get(slot);

  instance.exports.e2e_trigger_capability(BigInt(sv.ptr), sv.len);

  assert.ok(triggerReceived !== null);
  assert.equal(triggerReceived.capability, "test");
  assert.equal(triggerReceived.action, "do");
});

// ═══════════════════════════════════════════════════════════════════════
// F25 — IPC: WASM→host (register JS fn + invoke)
// ═══════════════════════════════════════════════════════════════════════

test("e2e: IPC WASM→host — register bridge fn and invoke", { skip }, () => {
  const { instance } = bootE2E();
  const result = instance.exports.e2e_invoke_ipc();
  assert.equal(result, 1, "e2e_invoke_ipc should return 1 on success");
});

test("e2e: IPC host→WASM — trigger via host_apply", { skip }, () => {
  const { rt, instance } = bootE2E();

  let triggerReceived = null;
  rt._ipcTriggerHandler = (req) => { triggerReceived = req; };

  // Use the existing e2e_trigger_ipc export (protocol byte 4)
  const payload = JSON.stringify({ ipc: "system", action: "ping", payload: {} });
  const buf = new TextEncoder().encode(payload);
  const slot = rt.memory.create(buf.length);
  rt.memory.write(slot, buf);
  const sv = rt.memory.get(slot);

  instance.exports.e2e_trigger_ipc(BigInt(sv.ptr), sv.len);

  assert.ok(triggerReceived !== null);
  assert.equal(triggerReceived.ipc, "system");
  assert.equal(triggerReceived.action, "ping");
});
