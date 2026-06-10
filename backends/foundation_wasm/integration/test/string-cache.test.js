// Tests for the StringCache in foundation-wasm.js — the host_cache_string import that
// interns UTF-8/UTF-16 strings from WASM memory and returns a stable handle.

import { test } from "node:test";
import assert from "node:assert/strict";

import { FoundationWasm } from "../../runtime/foundation-wasm.js";
import { makeMockWasm } from "../mock-wasm.js";

function boot() {
  const mock = makeMockWasm();
  const rt = new FoundationWasm();
  rt.init({ exports: { ...mock.exports, memory: mock.memory } });
  return { rt, mock };
}

/** Write `bytes` into a fresh arena slot and return its pointer. */
function writeBytes(rt, bytes) {
  const id = rt.memory.create(bytes.length);
  rt.memory.write(id, bytes);
  return rt.memory.get(id).ptr;
}

test("host_cache_string interns a UTF-8 string and resolves the handle", () => {
  const { rt } = boot();
  const bytes = new TextEncoder().encode("héllo — café");
  const ptr = writeBytes(rt, bytes);

  const handle = rt.web_abi.host_cache_string(BigInt(ptr), BigInt(bytes.length), 0);
  assert.equal(typeof handle, "bigint");
  assert.equal(rt.strings.get(handle), "héllo — café");
});

test("identical strings intern to the same handle; different strings differ", () => {
  const { rt } = boot();
  const a1 = new TextEncoder().encode("same");
  const a2 = new TextEncoder().encode("same");
  const b = new TextEncoder().encode("other");

  const h1 = rt.web_abi.host_cache_string(BigInt(writeBytes(rt, a1)), BigInt(a1.length), 0);
  const h2 = rt.web_abi.host_cache_string(BigInt(writeBytes(rt, a2)), BigInt(a2.length), 0);
  const h3 = rt.web_abi.host_cache_string(BigInt(writeBytes(rt, b)), BigInt(b.length), 0);

  assert.equal(h1, h2, "same string -> same handle (interned)");
  assert.notEqual(h1, h3, "different string -> different handle");
});

test("host_cache_string decodes UTF-16LE when encoding = 1", () => {
  const { rt } = boot();
  const str = "wide ☃";
  // encode as UTF-16LE
  const u16 = new Uint8Array(str.length * 2);
  const view = new DataView(u16.buffer);
  for (let i = 0; i < str.length; i++) view.setUint16(i * 2, str.charCodeAt(i), true);

  const ptr = writeBytes(rt, u16);
  const handle = rt.web_abi.host_cache_string(BigInt(ptr), BigInt(u16.length), 1);
  assert.equal(rt.strings.get(handle), str);
});
