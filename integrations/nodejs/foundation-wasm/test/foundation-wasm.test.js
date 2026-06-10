// Unit tests for the foundation-wasm.js core ABI runtime, using a mock WASM bridge
// (no compiled .wasm needed). Run with `node --test`.

import { test } from "node:test";
import assert from "node:assert/strict";

import {
  WasmEnvelope,
  ProtocolDispatcher,
  FoundationWasm,
} from "../../../../backends/foundation_wasm/runtime/foundation-wasm.js";
import { makeMockWasm } from "../mock-wasm.js";

/** Build a FoundationWasm wired to a mock instance. */
function bootWithMock(opts = {}) {
  const mock = makeMockWasm();
  const rt = new FoundationWasm(opts);
  // The runtime takes `memory` from instance.exports.memory.
  rt.init({ exports: { ...mock.exports, memory: mock.memory } });
  return { rt, mock };
}

// ─── WasmEnvelope ────────────────────────────────────────────────────────────

test("WasmEnvelope.write/parse round-trips", () => {
  const payload = new TextEncoder().encode("arrow-ipc-bytes — café");
  const memId = (1n << 32n) | 3n; // index=1, generation=3
  const framed = WasmEnvelope.write(1, 0, memId, payload);
  assert.equal(framed.length, WasmEnvelope.HEADER_LEN + payload.length);

  const env = WasmEnvelope.parse(framed);
  assert.equal(env.protocol, 1);
  assert.equal(env.version, 0);
  assert.equal(env.memoryId, memId);
  assert.equal(env.length, payload.length);
  assert.deepEqual([...env.payload], [...payload]);
});

test("WasmEnvelope exact little-endian byte layout", () => {
  const memId = 0x0000_0001_0000_0003n;
  const payload = new Uint8Array([0xaa, 0xbb]);
  const framed = WasmEnvelope.write(1, 0, memId, payload);
  assert.equal(framed[0], 1); // protocol
  assert.equal(framed[1], 0); // version
  const view = new DataView(framed.buffer);
  assert.equal(view.getBigUint64(2, true), memId);
  assert.equal(view.getUint32(10, true), 2);
  assert.deepEqual([...framed.subarray(14)], [0xaa, 0xbb]);
});

test("WasmEnvelope.parse rejects short header and truncated payload", () => {
  assert.throws(() => WasmEnvelope.parse(new Uint8Array(13)), RangeError);
  const framed = WasmEnvelope.write(2, 0, 0n, new Uint8Array([1, 2]));
  framed[10] = 200; // claim 200-byte payload that isn't there
  assert.throws(() => WasmEnvelope.parse(framed), RangeError);
});

// ─── ProtocolDispatcher ──────────────────────────────────────────────────────

test("ProtocolDispatcher routes each protocol byte to its handler", () => {
  for (const proto of [0, 1, 2]) {
    const seen = [];
    const dispatcher = new ProtocolDispatcher();
    for (const p of [0, 1, 2]) {
      dispatcher.setHandler(p, { apply: (id, payload) => seen.push([p, id, payload.length]) });
    }
    const memId = 7n;
    const payload = new Uint8Array([9, 9, 9]);
    dispatcher.dispatch(WasmEnvelope.write(proto, 0, memId, payload));
    assert.deepEqual(seen, [[proto, memId, 3]]);
  }
});

test("ProtocolDispatcher throws on unknown protocol", () => {
  const dispatcher = new ProtocolDispatcher();
  const framed = WasmEnvelope.write(255, 0, 0n, new Uint8Array([1]));
  assert.throws(() => dispatcher.dispatch(framed), /unknown protocol: 255/);
});

// ─── MemoryAllocations (via mock bridge) ─────────────────────────────────────

test("MemoryAllocations create/write/get/dispose round-trips through the arena", () => {
  const { rt, mock } = bootWithMock();
  const data = new Uint8Array([10, 20, 30, 40]);
  const id = rt.memory.create(data.length);
  rt.memory.write(id, data);

  const { bytes, len } = rt.memory.get(id);
  assert.equal(len, 4);
  assert.deepEqual([...bytes], [...data]);

  rt.memory.dispose(id);
  assert.deepEqual(mock.calls.dispose_allocation, [BigInt(id)]);
  assert.equal(mock.exports.__slotCount(), 0);
});

// ─── TimerRegistry (fake host timers) ────────────────────────────────────────

test("TimerRegistry.scheduleTimeout fires run_scheduled_callback then forgets it", () => {
  const fired = [];
  const host = {
    setTimeout: (fn) => { fired.push(fn); return fired.length; },
    clearTimeout: () => {},
    setInterval: () => 0,
    clearInterval: () => {},
  };
  const { rt, mock } = bootWithMock({ timerHost: host });
  rt.timers.scheduleTimeout(42, 4);
  assert.equal(rt.timers.timeouts.size, 1);
  fired[0](); // simulate the timer elapsing
  assert.deepEqual(mock.calls.run_scheduled_callback, [42n]);
  assert.equal(rt.timers.timeouts.size, 0);
});

test("TimerRegistry interval stops when run_interval_callback returns 0", () => {
  let intervalFn = null;
  let cleared = false;
  const host = {
    setTimeout: () => 0,
    clearTimeout: () => {},
    setInterval: (fn) => { intervalFn = fn; return 99; },
    clearInterval: () => { cleared = true; },
  };
  const { rt, mock } = bootWithMock({ timerHost: host });
  mock.exports.__stopIntervalAfter(7, 2); // 2 ticks then STOP
  rt.timers.scheduleInterval(7, 10);
  intervalFn(); // tick 1 -> REQUEUE
  assert.equal(cleared, false);
  intervalFn(); // tick 2 -> STOP
  assert.equal(cleared, true);
  assert.equal(mock.calls.run_interval_callback.length, 2);
});

// ─── CallbackRegistry ────────────────────────────────────────────────────────

test("CallbackRegistry.invoke allocates a slot and calls invoke_callback", () => {
  const { rt, mock } = bootWithMock();
  const data = new Uint8Array([1, 2, 3, 4, 5]);
  rt.callbacks.invoke(11, data);
  assert.equal(mock.calls.invoke_callback.length, 1);
  const [cbId, slotId] = mock.calls.invoke_callback[0];
  assert.equal(cbId, 11n);
  // The slot should contain the data we handed in.
  const { bytes } = rt.memory.get(slotId);
  assert.deepEqual([...bytes], [...data]);
});

// ─── host_apply transport (end-to-end through web_abi) ───────────────────────

test("web_abi.host_apply dispatches the framed message and disposes the slot", () => {
  const { rt, mock } = bootWithMock();
  const seen = [];
  rt.setProtocolHandler(1, { apply: (id, payload) => seen.push([id, [...payload]]) });

  // WASM would write a framed Arrow message into a slot, then call host_apply.
  const payload = new Uint8Array([7, 7, 7]);
  const slot = rt.memory.create(WasmEnvelope.HEADER_LEN + payload.length);
  const framed = WasmEnvelope.write(1, 0, slot, payload);
  rt.memory.write(slot, framed);

  rt.web_abi.host_apply(slot, 0n, 0n);

  assert.equal(seen.length, 1);
  assert.deepEqual(seen[0][1], [7, 7, 7]);
  assert.deepEqual(mock.calls.dispose_allocation, [BigInt(slot)]); // ACK happened
});

test("web_abi.host_apply still disposes the slot when the handler throws", () => {
  const { rt, mock } = bootWithMock();
  rt.setProtocolHandler(1, { apply: () => { throw new Error("boom"); } });
  const payload = new Uint8Array([1]);
  const slot = rt.memory.create(WasmEnvelope.HEADER_LEN + payload.length);
  rt.memory.write(slot, WasmEnvelope.write(1, 0, slot, payload));

  assert.throws(() => rt.web_abi.host_apply(slot, 0n, 0n), /boom/);
  assert.deepEqual(mock.calls.dispose_allocation, [BigInt(slot)]); // freed despite throw
});
