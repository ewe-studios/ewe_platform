// PARITY ORACLE: runs the megatron-era compiled fixtures (integrations/nodejs/
// integrations/*/module.wasm — built against the OLD runtime) on the NEW
// foundation-wasm.js. Their registered functions use the megatron context API
// (`this.mock.*`, `this.asMemorySlice(0)`, `this.asUint8(10)`), so green here means
// the new runtime is a drop-in replacement for megatron on real modules.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import {
  FoundationWasm, ExternalPointer, InternalPointer, TypedArraySliceValue,
} from "../../runtime/foundation-wasm.js";

const here = dirname(fileURLToPath(import.meta.url));
const legacyRoot = join(here, "..", "..", "..", "..", "integrations", "nodejs", "integrations");

function bootLegacy(name) {
  const wasmPath = join(legacyRoot, name, "module.wasm");
  if (!existsSync(wasmPath)) return null;
  const rt = new FoundationWasm();
  const mock = { calls: [] };
  mock.logs = (...args) => {
    mock.calls.push({ method: "log", arguments: [args[0]] });
  };
  mock.select = (args) => {
    mock.calls.push({ method: "select", arguments: args });
  };
  // Mock methods the typed-return fixtures call (same shapes as the old suite).
  mock.is_sample = (v1) => {
    mock.calls.push({ method: "is_sample", arguments: [v1] });
    return v1;
  };
  mock.returnArg = (v1) => {
    mock.calls.push({ method: "returnArg", arguments: [v1] });
    return v1;
  };
  mock.calculateAge = (...vs) => {
    const rounded = vs.map((v, i) => (i >= 12 ? Math.round(v * 100) / 100 : v));
    mock.calls.push({ method: "calculateAge", arguments: rounded });
    return 100 * vs[0];
  };
  rt.functions.mock = mock; // context = the registry; `this.mock` + `this.as*` resolve
  const instance = new WebAssembly.Instance(new WebAssembly.Module(readFileSync(wasmPath)), {
    abi: rt.web_abi,
  });
  rt.init(instance);
  return { rt, mock, instance };
}

async function waitFor(cond, ms = 3000) {
  const start = Date.now();
  while (!cond()) {
    if (Date.now() - start > ms) throw new Error("waitFor: condition not met in time");
    await new Promise((r) => setTimeout(r, 10));
  }
}

function stopTimers(rt) {
  for (const key of [...rt.timers.timeouts.keys()]) rt.timers.cancelTimeout(key);
  for (const key of [...rt.timers.intervals.keys()]) rt.timers.cancelInterval(key);
  rt.animation.stop();
}

const missing = "legacy fixture not present";

test("legacy tests_callfunction: register + invoke logs through this.mock", { skip: bootLegacy("tests_callfunction") ? false : missing }, () => {
  const { mock, instance } = bootLegacy("tests_callfunction");
  instance.exports.main();
  assert.deepEqual(mock.calls, [{ method: "log", arguments: ["Hello from intro"] }]);
});

test("legacy tests_registerfunction: function lands in the heap and is callable", { skip: bootLegacy("tests_registerfunction") ? false : missing }, () => {
  const { rt, instance } = bootLegacy("tests_registerfunction");
  instance.exports.main();
  assert.equal(rt.functions.heap.items.length, 1);
  const slot = rt.functions.heap.items[0];
  assert.equal(slot.active, true);
  assert.equal(slot.generation, 0n);
  assert.equal(typeof slot.item, "function");
  // This fixture's registered fn calls `this.mock.runtime.logs(message)`.
  const runtimeMock = { calls: [] };
  runtimeMock.logs = (message) => runtimeMock.calls.push({ method: "log", arguments: [message] });
  slot.item.call({ mock: { runtime: runtimeMock } }, "hello");
  assert.deepEqual(runtimeMock.calls, [{ method: "log", arguments: ["hello"] }]);
});

test("legacy tests_js_invoke_function: invoke path logs through this.mock", { skip: bootLegacy("tests_js_invoke_function") ? false : missing }, () => {
  const { mock, instance } = bootLegacy("tests_js_invoke_function");
  instance.exports.main();
  assert.deepEqual(mock.calls, [{ method: "log", arguments: ["Hello from intro"] }]);
});

test("legacy tests_js_invoke_async_function: async invoke resolves + delivers", { skip: bootLegacy("tests_js_invoke_async_function") ? false : missing }, async () => {
  const { mock, instance } = bootLegacy("tests_js_invoke_async_function");
  instance.exports.main();
  assert.deepEqual(mock.calls, [{ method: "log", arguments: ["Hello from intro"] }]);
  await new Promise((r) => setTimeout(r, 0)); // drain the callback delivery
});

test("legacy tests_instructions_function: V2 batch decodes every scalar param type", { skip: bootLegacy("tests_instructions_function") ? false : missing }, () => {
  const { mock, instance } = bootLegacy("tests_instructions_function");
  instance.exports.main();
  assert.equal(mock.calls.length, 1);
  assert.equal(mock.calls[0].method, "select");
  assert.deepEqual(mock.calls[0].arguments, [true, false, 10, 10, 10, 10, 10, 10, 10, 10, 10.0, 10.0]); // quantized i64/u64 surface as numbers (megatron parity)
});

test("legacy tests_instructions_multi_return: V2 batch with multi-return hints", { skip: bootLegacy("tests_instructions_multi_return") ? false : missing }, () => {
  const { mock, instance } = bootLegacy("tests_instructions_multi_return");
  instance.exports.main();
  assert.equal(mock.calls.length, 1);
  assert.equal(mock.calls[0].method, "select");
  assert.deepEqual(mock.calls[0].arguments, [true, false, 10, 10, 10, 10, 10, 10, 10, 10, 10.0, 10.0]); // quantized i64/u64 surface as numbers (megatron parity)
});

test("legacy tests_instructions_array: V2 batch decodes text/refs/typed arrays", { skip: bootLegacy("tests_instructions_array") ? false : missing }, () => {
  const { mock, instance } = bootLegacy("tests_instructions_array");
  instance.exports.main();
  assert.equal(mock.calls.length, 1);
  const args = mock.calls[0].arguments;
  assert.equal(args[0], "alex");
  assert.ok(args[1] instanceof ExternalPointer);
  assert.equal(args[1].value, 1n);
  assert.ok(args[2] instanceof InternalPointer);
  assert.equal(args[2].value, 2n);
  assert.deepEqual(args[3], new Uint8Array([1, 1]));
  assert.deepEqual(args[4], new Int8Array([1, 1]));
  assert.deepEqual(args[5], new Uint16Array([1, 1]));
  assert.deepEqual(args[6], new Int16Array([1, 1]));
  assert.deepEqual(args[7], new Uint32Array([1, 1]));
  assert.deepEqual(args[8], new Int32Array([1, 1]));
  assert.deepEqual(args[9], new BigInt64Array([2n, 2n]));
  assert.deepEqual(args[10], new BigUint64Array([3n, 3n]));
  assert.deepEqual(args[11], new Float32Array([1.0, 1.0]));
  assert.deepEqual(args[12], new Float64Array([1.0, 1.0]));
  assert.ok(args[13] instanceof TypedArraySliceValue);
  assert.equal(args[13].sliceType, 5); // TypedSlice.Uint8
  assert.deepEqual(new Uint8Array(args[13].content.buffer, args[13].content.byteOffset, 2), new Uint8Array([4, 4]));
});

// ── Timers / animation frames ──────────────────────────────────────────────────────

test("legacy tests_js_timeout: schedule_timeout fires the WASM callback", { skip: bootLegacy("tests_js_timeout") ? false : missing }, async () => {
  const { rt, mock, instance } = bootLegacy("tests_js_timeout");
  instance.exports.main();
  await waitFor(() => mock.calls.length >= 1);
  stopTimers(rt);
  assert.deepEqual(mock.calls, [{ method: "log", arguments: ["Hello from intro"] }]);
});

test("legacy tests_js_interval: schedule_interval fires repeatedly", { skip: bootLegacy("tests_js_interval") ? false : missing }, async () => {
  const { rt, mock, instance } = bootLegacy("tests_js_interval");
  instance.exports.main();
  await waitFor(() => mock.calls.length >= 3);
  stopTimers(rt);
  assert.deepEqual(mock.calls.slice(0, 3), [
    { method: "log", arguments: ["Hello from intro"] },
    { method: "log", arguments: ["Hello from intro"] },
    { method: "log", arguments: ["Hello from intro"] },
  ]);
});

test("legacy tests_js_raf: animation-frame loop drives the WASM callback", { skip: bootLegacy("tests_js_raf") ? false : missing }, async () => {
  const { rt, mock, instance } = bootLegacy("tests_js_raf");
  instance.exports.main();
  await waitFor(() => mock.calls.length >= 1);
  stopTimers(rt);
  assert.deepEqual(mock.calls.slice(0, 1), [{ method: "log", arguments: ["Hello from intro"] }]);
});

// ── Async failure + async batch callbacks ──────────────────────────────────────────

test("legacy tests_js_invoke_failed_async_function: rejection delivers ErrorCode", { skip: bootLegacy("tests_js_invoke_failed_async_function") ? false : missing }, async () => {
  const { mock, instance } = bootLegacy("tests_js_invoke_failed_async_function");
  instance.exports.main(); // wasm self-asserts the ErrorCode(101) callback (trap = fail)
  assert.deepEqual(mock.calls, [{ method: "log", arguments: ["Hello from intro"] }]);
  await new Promise((r) => setTimeout(r, 0)); // drain the rejection delivery
});

test("legacy tests_instructions_array_callback: batch InvokeAsync + rich params", { skip: bootLegacy("tests_instructions_array_callback") ? false : missing }, async () => {
  const { mock, instance } = bootLegacy("tests_instructions_array_callback");
  instance.exports.main();
  assert.equal(mock.calls.length, 1);
  assert.equal(mock.calls[0].method, "select");
  assert.equal(mock.calls[0].arguments[0], "alex");
  await new Promise((r) => setTimeout(r, 0)); // drain the Uint8 callback delivery
});

test("legacy tests_instructions_none_return_callback: batch async fire-and-forget", { skip: bootLegacy("tests_instructions_none_return_callback") ? false : missing }, async () => {
  const { mock, instance } = bootLegacy("tests_instructions_none_return_callback");
  instance.exports.main();
  assert.equal(mock.calls.length, 1);
  assert.equal(mock.calls[0].method, "select");
  await new Promise((r) => setTimeout(r, 0));
});

// ── Typed returns (modules self-assert the decoded values; trap = fail) ────────────

test("legacy return_big_int: 14 mixed params incl. 128-bit + naked big-int return", { skip: bootLegacy("tests_js_invoke_function_and_return_big_int") ? false : missing }, () => {
  const { rt, mock, instance } = bootLegacy("tests_js_invoke_function_and_return_big_int");
  instance.exports.main();
  assert.deepEqual(mock.calls, [{
    method: "calculateAge",
    arguments: [5, 5, 10, 10, 5, 5, 10, 10, 10, 10, true, false, 10.2, 10.4],
  }]);
  assert.equal(rt.objects.items.length, 0);
  assert.equal(rt.functions.heap.items.length, 1);
});

test("legacy return_bool: naked bool return", { skip: bootLegacy("tests_js_invoke_function_and_return_bool") ? false : missing }, () => {
  const { rt, mock, instance } = bootLegacy("tests_js_invoke_function_and_return_bool");
  instance.exports.main();
  assert.deepEqual(mock.calls, [{ method: "is_sample", arguments: [true] }]);
  assert.equal(rt.objects.items.length, 0);
  assert.equal(rt.functions.heap.items.length, 1);
});

test("legacy return_none: explicit asNone() return", { skip: bootLegacy("tests_js_invoke_function_and_return_none") ? false : missing }, () => {
  const { rt, mock, instance } = bootLegacy("tests_js_invoke_function_and_return_none");
  instance.exports.main();
  assert.deepEqual(mock.calls, [{ method: "is_sample", arguments: [true] }]);
  assert.equal(rt.functions.heap.items.length, 2); // error logger + the fn
});

test("legacy return_object: object return interns into the object heap", { skip: bootLegacy("tests_js_invoke_function_and_return_object") ? false : missing }, () => {
  const { rt, mock, instance } = bootLegacy("tests_js_invoke_function_and_return_object");
  instance.exports.main();
  assert.deepEqual(mock.calls, [{ method: "log", arguments: ["Hello from intro"] }]);
  assert.equal(rt.objects.items.length, 1);
});

test("legacy return_string: string return decodes back in WASM", { skip: bootLegacy("tests_js_invoke_function_and_return_string") ? false : missing }, () => {
  const { mock, instance } = bootLegacy("tests_js_invoke_function_and_return_string");
  instance.exports.main(); // wasm self-asserts the round-tripped string
  assert.deepEqual(mock.calls, [{ method: "log", arguments: ["Hello from intro"] }]);
});

test("legacy return_types: every scalar type echoes through returnArg", { skip: bootLegacy("tests_js_invoke_function_and_return_types") ? false : missing }, () => {
  const { mock, instance } = bootLegacy("tests_js_invoke_function_and_return_types");
  instance.exports.main(); // wasm self-asserts every echoed return value
  assert.ok(mock.calls.length > 1);
  assert.ok(mock.calls[0].arguments[0] instanceof TypedArraySliceValue);
  for (const call of mock.calls.slice(1)) {
    assert.equal(call.method, "returnArg");
    assert.equal(call.arguments.length, 1);
  }
});

test("legacy with_array: V1 flat invoke decodes text/refs/typed arrays", { skip: bootLegacy("tests_js_invoke_function_with_array") ? false : missing }, () => {
  const { mock, instance } = bootLegacy("tests_js_invoke_function_with_array");
  instance.exports.main();
  assert.equal(mock.calls.length, 1);
  const args = mock.calls[0].arguments;
  assert.equal(args[0], "alex");
  assert.ok(args[1] instanceof ExternalPointer);
  assert.equal(args[1].value, 1n);
  assert.ok(args[2] instanceof InternalPointer);
  assert.equal(args[2].value, 2n);
  assert.deepEqual(args[3], new Uint8Array([1, 1]));
  assert.deepEqual(args[4], new Int8Array([1, 1]));
  assert.deepEqual(args[5], new Uint16Array([1, 1]));
  assert.deepEqual(args[6], new Int16Array([1, 1]));
  assert.deepEqual(args[7], new Uint32Array([1, 1]));
  assert.deepEqual(args[8], new Int32Array([1, 1]));
  assert.deepEqual(args[9], new BigInt64Array([2n, 2n]));
  assert.deepEqual(args[10], new BigUint64Array([3n, 3n]));
  assert.deepEqual(args[11], new Float32Array([1.0, 1.0]));
  assert.deepEqual(args[12], new Float64Array([1.0, 1.0]));
});
