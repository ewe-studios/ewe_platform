// Validates the ported FunctionRegistry codec against a REAL compiled module: the
// module registers JS functions (source strings) and invokes them with real `Params`
// (flat encoding), so these assertions exercise the full Rust↔JS round-trip —
// register → flat param encode (Rust) → ParameterParser decode (JS) → call →
// return encode (JS) → typed decode (Rust).

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { FoundationWasm } from "../../runtime/foundation-wasm.js";

const here = dirname(fileURLToPath(import.meta.url));
const wasmPath = join(here, "..", "fixtures", "foundation_wasm_e2e.wasm");
const skip = existsSync(wasmPath) ? false : "wasm fixture not built (run ./build-module.sh)";

function boot() {
  const rt = new FoundationWasm();
  const instance = new WebAssembly.Instance(new WebAssembly.Module(readFileSync(wasmPath)), {
    abi: rt.web_abi,
  });
  rt.init(instance);
  return { rt, instance };
}

test("roundtrip_i32: Int32 param + i32 return through the registered fn", { skip }, () => {
  const { instance } = boot();
  assert.equal(instance.exports.roundtrip_i32(21), 42); // x*2
  assert.equal(instance.exports.roundtrip_i32(-5), -10);
});

test("roundtrip_f64: Float64 param + f64 return", { skip }, () => {
  const { instance } = boot();
  assert.equal(instance.exports.roundtrip_f64(2.0), 2.5); // x+0.5
});

test("roundtrip_bool_and: two Bool params + bool return", { skip }, () => {
  const { instance } = boot();
  assert.equal(instance.exports.roundtrip_bool_and(1, 1), 1);
  assert.equal(instance.exports.roundtrip_bool_and(1, 0), 0);
  assert.equal(instance.exports.roundtrip_bool_and(0, 1), 0);
});

test("roundtrip_via_reply_i32: generic encoded return (Begin..End frame → from_binary)", { skip }, () => {
  const { instance } = boot();
  assert.equal(instance.exports.roundtrip_via_reply_i32(35), 42); // x+7 via slot+from_binary
});

test("roundtrip_string_len: string return via Text8 slot", { skip }, () => {
  const { instance } = boot();
  assert.equal(instance.exports.roundtrip_string_len(), 11); // "hello world"
});

test("roundtrip_string_echo_len: Text8 param in + string out", { skip }, () => {
  const { instance } = boot();
  assert.equal(instance.exports.roundtrip_string_echo_len(), 4); // "ab"+"ab"
});

test("roundtrip_list_sum: List(One(Int32)) → homogeneous multi-value frame", { skip }, () => {
  const { instance } = boot();
  assert.equal(instance.exports.roundtrip_list_sum(), 6); // [1,2,3] summed
});

test("roundtrip_multi: Multi[Int32, Bool] → tuple decoded value-by-state", { skip }, () => {
  const { instance } = boot();
  assert.equal(instance.exports.roundtrip_multi(), 7); // [7, true] → 7 when bool true
});

test("invoke_async_test: async fn result framed + delivered via invoke_callback", { skip }, async () => {
  const { rt, instance } = boot();
  // Spy on the callback registry so we observe the encoded reply WITHOUT calling back
  // into WASM at an unregistered id. invokeAsync delivers through rt.callbacks.invoke.
  const calls = [];
  rt.callbacks.invoke = (id, data) => calls.push({ id, data: Uint8Array.from(data) });

  instance.exports.invoke_async_test(7n); // callback id = 7
  await Promise.resolve(); // drain the .then() microtask
  await Promise.resolve();

  assert.equal(calls.length, 1);
  assert.equal(calls[0].id, 7n);
  const d = calls[0].data; // [Begin=100][Int32=5][42 LE u32][End=101]
  assert.equal(d[0], 100); // ReturnValueMarker.Begin
  assert.equal(d[1], 5); // ReturnType.Int32
  const dv = new DataView(d.buffer, d.byteOffset, d.byteLength);
  assert.equal(dv.getInt32(2, true), 42); // 41 + 1
  assert.equal(d[6], 101); // ReturnValueMarker.End
});

test("roundtrip_object_handle: object return interned in the host object heap", { skip }, () => {
  const { rt, instance } = boot();
  const handle = instance.exports.roundtrip_object_handle();
  assert.ok(handle >= 0n, `expected a heap handle, got ${handle}`);
  assert.deepEqual(rt.objects.get(BigInt(handle)), { a: 1 });
});

test("roundtrip_typed_slice_sum: TypedArray return via raw memory location", { skip }, () => {
  const { instance } = boot();
  assert.equal(instance.exports.roundtrip_typed_slice_sum(), 6); // 1+2+3
});

test("roundtrip_object_naked: as_object fast-path returns the heap handle raw", { skip }, () => {
  const { rt, instance } = boot();
  const handle = instance.exports.roundtrip_object_naked();
  assert.deepEqual(rt.objects.get(BigInt(handle)), { b: 2 });
});

test("drop_object_test: host_object_drop_external_pointer retires the handle", { skip }, () => {
  const { rt, instance } = boot();
  const handle = instance.exports.roundtrip_object_naked();
  assert.deepEqual(rt.objects.get(BigInt(handle)), { b: 2 });
  instance.exports.drop_object_test(BigInt(handle));
  assert.equal(rt.objects.get(BigInt(handle)), undefined);
});

test("cache_and_drop_string: string-cache eviction round-trip", { skip }, () => {
  const { rt, instance } = boot();
  const handle = instance.exports.cache_and_drop_string();
  assert.equal(rt.strings.get(BigInt(handle)), undefined);
});

test("batch_register_invoke_i32: V2 MakeFunction+Invoke, quantized param, group return", { skip }, () => {
  const { instance } = boot();
  assert.equal(instance.exports.batch_register_invoke_i32(14), 42); // x*3
  assert.equal(instance.exports.batch_register_invoke_i32(-4), -12);
});

test("batch_capture_mixed_params: V2 quantized decode of Int32/Text8/Bool/Float64", { skip }, () => {
  const { rt, instance } = boot();
  const ctx = {};
  rt.functions.context = ctx;
  instance.exports.batch_capture_mixed_params();
  assert.deepEqual(ctx.batch_captured, [10, "hi", true, 2.5]);
});

test("batch_invoke_async: V2 InvokeAsync delivers framed reply via invoke_callback", { skip }, async () => {
  const { rt, instance } = boot();
  const calls = [];
  rt.callbacks.invoke = (id, data) => calls.push({ id, data: Uint8Array.from(data) });

  instance.exports.batch_invoke_async(9n); // callback id = 9
  await Promise.resolve();
  await Promise.resolve();

  assert.equal(calls.length, 1);
  assert.equal(calls[0].id, 9n);
  const d = calls[0].data; // [Begin=100][Int32=5][10 LE u32][End=101]
  assert.equal(d[0], 100);
  assert.equal(d[1], 5);
  const dv = new DataView(d.buffer, d.byteOffset, d.byteLength);
  assert.equal(dv.getInt32(2, true), 10); // 1 + 9
  assert.equal(d[6], 101);
});

test("batch_quantized_spread: i8-quantized and full-width i32 params in one call", { skip }, () => {
  const { instance } = boot();
  assert.equal(instance.exports.batch_quantized_spread(), 100007); // 7 + 100000
});

test("capture_mixed_params: flat decode of Int32/Text8/Bool/Float64", { skip }, () => {
  const { rt, instance } = boot();
  const ctx = {};
  rt.functions.context = ctx; // registered fn sets `this.captured`
  instance.exports.capture_mixed_params();
  assert.deepEqual(ctx.captured, [10, "hi", true, 2.5]);
});
