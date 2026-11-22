// WasmLoader + AsyncTaskCollector against the real e2e fixture: loadBytes binds the
// bridge exactly like the manual instantiate pattern, and enabled task collection
// makes awaitTasks() settle after async invocations deliver their callbacks.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { WasmLoader, AsyncTaskCollector } from "../../runtime/foundation-wasm.js";

const here = dirname(fileURLToPath(import.meta.url));
const wasmPath = join(here, "..", "fixtures", "foundation_wasm_e2e.wasm");
const skip = existsSync(wasmPath) ? false : "wasm fixture not built (run ./build-module.sh)";

test("WasmLoader.loadBytes instantiates + binds the runtime bridge", { skip }, async () => {
  const loader = await new WasmLoader().loadBytes(readFileSync(wasmPath));
  assert.equal(loader.runtime.bridge.exports, loader.module.instance.exports);
  assert.equal(loader.module.instance.exports.roundtrip_i32(21), 42);
});

test("WasmLoader.run throws without a module or a main export", { skip }, async () => {
  assert.throws(() => new WasmLoader().run(), /No wasm module loaded/);
  const loader = await new WasmLoader().loadBytes(readFileSync(wasmPath));
  assert.throws(() => loader.run(), /no exported main/); // fixture has no main()
});

test("awaitTasks settles after async invocation callbacks deliver", { skip }, async () => {
  const loader = await new WasmLoader().loadBytes(readFileSync(wasmPath));
  const rt = loader.runtime;
  rt.tasks.enable();
  const calls = [];
  rt.callbacks.invoke = (id, data) => calls.push({ id, data: Uint8Array.from(data) });

  loader.module.instance.exports.invoke_async_test(3n);
  assert.equal(rt.tasks.tasks.length, 1);
  await rt.awaitTasks();
  assert.equal(calls.length, 1);
  assert.equal(calls[0].id, 3n);
});

test("AsyncTaskCollector is off by default and validates inputs", () => {
  const tasks = new AsyncTaskCollector();
  tasks.add(Promise.resolve(1)); // collection off — not retained
  assert.equal(tasks.tasks.length, 0);
  tasks.enable();
  tasks.add(Promise.resolve(2));
  assert.equal(tasks.tasks.length, 1);
  assert.throws(() => tasks.add(42), /must be a Promise/);
  tasks.clear();
  assert.equal(tasks.tasks.length, 0);
});

test("loadURL reports a missing module before it starts compilation", async () => {
  const loader = new WasmLoader();
  const body = new ReadableStream();
  await assert.rejects(loader.loadURL("/missing.wasm", () => new Response(body, {status: 404})), {
    name: "WasmLoadError", code: "HttpStatus", url: "/missing.wasm", status: 404,
  });
  assert.equal(loader.module, null);
});

test("loadURL distinguishes malformed modules and streams valid modules", async () => {
  const loader = new WasmLoader();
  const response = bytes => () => new Response(Uint8Array.from(bytes), {
    headers: {"Content-Type": "application/wasm"},
  });
  await assert.rejects(loader.loadURL("/bad.wasm", response([0])), {code: "Compile"});
  await loader.loadURL("/empty.wasm", response([0, 97, 115, 109, 1, 0, 0, 0]));
  assert.equal(loader.runtime.bridge.exports, loader.module.instance.exports);
});

test("loadURL preserves the network failure and distinguishes incompatible imports", async () => {
  const cause = new TypeError("offline");
  const loader = new WasmLoader({environment: {host: {main: 1}}});
  await assert.rejects(loader.loadURL("/module.wasm", () => Promise.reject(cause)), {
    code: "Network", cause,
  });
  // This module imports the function host.main.
  const bytes = Uint8Array.of(
    0, 97, 115, 109, 1, 0, 0, 0,
    1, 4, 1, 96, 0, 0,
    2, 13, 1, 4, 104, 111, 115, 116, 4, 109, 97, 105, 110, 0, 0,
  );
  await assert.rejects(loader.loadURL("/module.wasm", () => new Response(bytes, {
    headers: {"Content-Type": "application/wasm"},
  })), {code: "Imports"});
});
