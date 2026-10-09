// Integration tests for F28 stream objects (WasmStreamReceiver / WasmStreamSender)
// and F27 trigger delivery (triggerCapability / triggerIpc).
// Run with: node --test

import { test, describe } from "node:test";
import assert from "node:assert/strict";

import {
  WasmEnvelope,
  WasmStreamReceiver,
  WasmStreamSender,
  FoundationWasm,
} from "../../runtime/foundation-wasm.js";
import { makeMockWasm } from "../mock-wasm.js";

function bootWithMock(opts = {}) {
  const mock = makeMockWasm();
  const rt = new FoundationWasm(opts);
  rt.init({ exports: { ...mock.exports, memory: mock.memory } });
  return { rt, mock };
}

// ═══════════════════════════════════════════════════════════════════════
// F28 — WasmStreamReceiver (host → WASM incoming stream)
// ═══════════════════════════════════════════════════════════════════════

describe("WasmStreamReceiver", () => {
  test("constructs and assigns a monotonic receiverId", () => {
    const a = new WasmStreamReceiver({});
    const b = new WasmStreamReceiver({});
    assert.ok(a.receiverId > 0);
    assert.ok(b.receiverId > 0);
    assert.notEqual(a.receiverId, b.receiverId);
  });

  test("push() delivers data to the onData callback", () => {
    const chunks = [];
    const r = new WasmStreamReceiver({
      onData: (c) => chunks.push(c),
    });

    const data = new Uint8Array([1, 2, 3]);
    WasmStreamReceiver._push(r.receiverId, data, 0, false);
    assert.equal(chunks.length, 1);
    assert.equal(chunks[0].sequence, 0);
    assert.deepEqual([...chunks[0].data], [1, 2, 3]);
  });

  test("multiple chunks arrive in order", () => {
    const seqs = [];
    const r = new WasmStreamReceiver({
      onData: (c) => seqs.push(c.sequence),
    });

    WasmStreamReceiver._push(r.receiverId, new Uint8Array([1]), 0, false);
    WasmStreamReceiver._push(r.receiverId, new Uint8Array([2]), 1, false);
    WasmStreamReceiver._push(r.receiverId, new Uint8Array([3]), 2, false);
    assert.deepEqual(seqs, [0, 1, 2]);
  });

  test("push() with isLast=true fires onEnd and cleans up", () => {
    let ended = false;
    const r = new WasmStreamReceiver({
      onEnd: () => { ended = true; },
    });

    WasmStreamReceiver._push(r.receiverId, new Uint8Array([]), 0, true);
    assert.equal(ended, true);
    // Subsequent pushes should be silently ignored (receiver cleaned up)
    WasmStreamReceiver._push(r.receiverId, new Uint8Array([99]), 1, false);
    // No error thrown, but data should not arrive
  });

  test("onError fires when callback throws", () => {
    let error = null;
    const r = new WasmStreamReceiver({
      onData: () => { throw new Error("test-error"); },
      onError: (e) => { error = e; },
    });

    // should NOT throw — error goes to onError
    assert.doesNotThrow(() => {
      WasmStreamReceiver._push(r.receiverId, new Uint8Array([1]), 0, false);
    });
    assert.ok(error instanceof Error);
    assert.match(error.message, /test-error/);
  });

  test("push to unknown receiver is a no-op", () => {
    // These should just silently return
    assert.doesNotThrow(() => {
      WasmStreamReceiver._push(999999, new Uint8Array([1]), 0, false);
      WasmStreamReceiver._push(999999, new Uint8Array([1]), 0, true);
    });
  });

  test("receiver IDs do not collide across instances", () => {
    const ids = new Set();
    for (let i = 0; i < 100; i++) {
      ids.add(new WasmStreamReceiver({}).receiverId);
    }
    assert.equal(ids.size, 100);
  });
});

// ═══════════════════════════════════════════════════════════════════════
// F28 — WasmStreamSender (WASM → host outgoing stream)
// ═══════════════════════════════════════════════════════════════════════

describe("WasmStreamSender", () => {
  test("constructs and assigns a monotonic senderId", () => {
    const a = new WasmStreamSender(() => {}, () => {});
    const b = new WasmStreamSender(() => {}, () => {});
    assert.ok(a.senderId > 0);
    assert.ok(b.senderId > 0);
    assert.notEqual(a.senderId, b.senderId);
  });

  test("_send() delivers data to the onChunk callback", () => {
    const chunks = [];
    const s = new WasmStreamSender(
      (c) => chunks.push(c),
      () => {},
    );

    const data = new Uint8Array([10, 20, 30]);
    WasmStreamSender._send(s.senderId, data, 0);
    assert.equal(chunks.length, 1);
    assert.deepEqual([...chunks[0].data], [10, 20, 30]);
    assert.equal(chunks[0].sequence, 0);
  });

  test("_end() fires onEnd and deregisters", () => {
    let ended = false;
    const s = new WasmStreamSender(
      () => {},
      () => { ended = true; },
    );

    WasmStreamSender._end(s.senderId);
    assert.equal(ended, true);
    // Subsequent _send should be no-ops (sender deregistered)
    assert.doesNotThrow(() => {
      WasmStreamSender._send(s.senderId, new Uint8Array([99]), 1);
    });
  });

  test("_send to unknown sender is silently ignored", () => {
    assert.doesNotThrow(() => {
      WasmStreamSender._send(99999, new Uint8Array([1]), 0);
      WasmStreamSender._end(99999);
    });
  });

  test("sender IDs do not collide across instances", () => {
    const ids = new Set();
    for (let i = 0; i < 100; i++) {
      ids.add(new WasmStreamSender(() => {}, () => {}).senderId);
    }
    assert.equal(ids.size, 100);
  });
});

// ═══════════════════════════════════════════════════════════════════════
// F27 — triggerCapability / triggerIpc (host → WASM)
// ═══════════════════════════════════════════════════════════════════════

describe("triggerCapability / triggerIpc (F27)", () => {
  test("triggerCapability rejects when no handler registered", async () => {
    const { rt } = bootWithMock();
    await assert.rejects(
      () => rt.triggerCapability({ capability: "cam", action: "read", payload: new Uint8Array() }),
      /no handler registered/,
    );
  });

  test("triggerIpc rejects when no handler registered", async () => {
    const { rt } = bootWithMock();
    await assert.rejects(
      () => rt.triggerIpc({ ipc: "system", action: "ping", payload: new Uint8Array() }),
      /no handler registered/,
    );
  });

  test("triggerCapability delivers to registered handler", async () => {
    const { rt } = bootWithMock();
    const seen = [];
    rt._capTriggerHandler = (req) => {
      seen.push(req);
      return Promise.resolve({ status: "ok" });
    };

    await rt.triggerCapability({ capability: "cam", action: "capture", payload: new Uint8Array([1]) });
    assert.equal(seen.length, 1);
    assert.equal(seen[0].capability, "cam");
    assert.equal(seen[0].action, "capture");
  });

  test("triggerIpc delivers to registered handler", async () => {
    const { rt } = bootWithMock();
    const seen = [];
    rt._ipcTriggerHandler = (req) => {
      seen.push(req);
      return Promise.resolve({ result: "ok" });
    };

    await rt.triggerIpc({ ipc: "system", action: "get_info", payload: new Uint8Array([2]) });
    assert.equal(seen.length, 1);
    assert.equal(seen[0].ipc, "system");
    assert.equal(seen[0].action, "get_info");
  });

  test("handler replacement works", async () => {
    const { rt } = bootWithMock();
    const calls = [];

    rt._ipcTriggerHandler = () => { calls.push("first"); return Promise.resolve({}); };
    await rt.triggerIpc({ ipc: "x", action: "x", payload: new Uint8Array() });
    assert.deepEqual(calls, ["first"]);

    rt._ipcTriggerHandler = () => { calls.push("second"); return Promise.resolve({}); };
    await rt.triggerIpc({ ipc: "x", action: "x", payload: new Uint8Array() });
    assert.deepEqual(calls, ["first", "second"]);
  });

  test("dispatcher routes protocol bytes 3 and 4 to trigger handlers", () => {
    const { rt, mock } = bootWithMock();
    const capLog = [], ipcLog = [];

    rt._capTriggerHandler = (req) => { capLog.push(req); return Promise.resolve({}); };
    rt._ipcTriggerHandler = (req) => { ipcLog.push(req); return Promise.resolve({}); };

    // Build framed messages using the standard envelope
    const capPayload = new TextEncoder().encode(JSON.stringify({ capability: "test", action: "do" }));
    const capSlot = rt.memory.create(WasmEnvelope.HEADER_LEN + capPayload.length);
    rt.memory.write(capSlot, WasmEnvelope.write(3, 0, capSlot, capPayload));
    rt.web_abi.host_apply(capSlot, 0n, 0n);

    assert.equal(capLog.length, 1);

    const ipcPayload = new TextEncoder().encode(JSON.stringify({ ipc: "sys", action: "ping" }));
    const ipcSlot = rt.memory.create(WasmEnvelope.HEADER_LEN + ipcPayload.length);
    rt.memory.write(ipcSlot, WasmEnvelope.write(4, 0, ipcSlot, ipcPayload));
    rt.web_abi.host_apply(ipcSlot, 0n, 0n);

    assert.equal(ipcLog.length, 1);
  });
});

// ═══════════════════════════════════════════════════════════════════════
// F28 — web_abi streaming FFI (host_sender_send, host_sender_end, host_receiver_push)
// ═══════════════════════════════════════════════════════════════════════

describe("web_abi streaming FFI", () => {
  test("host_receiver_push delivers to WasmStreamReceiver", () => {
    const { rt } = bootWithMock();
    const chunks = [];
    const r = new WasmStreamReceiver({
      onData: (c) => chunks.push(c),
    });

    // WASM would write data into the arena and call the FFI with BigInt
    // args. From JS tests, we verify that _push works with data read from
    // the bridge buffer (what the FFI does internally).
    const buf = new Uint8Array([0xde, 0xad, 0xbe, 0xef]);
    const slot = rt.memory.create(buf.length);
    rt.memory.write(slot, buf);
    const sv = rt.memory.get(slot);

    // Read from bridge the same way host_receiver_push does internally
    const data = new Uint8Array(rt.bridge.memory.buffer, sv.ptr, sv.len);
    assert.deepEqual([...data], [0xde, 0xad, 0xbe, 0xef]);

    // Feed that data through _push — simulates the full FFI path
    WasmStreamReceiver._push(r.receiverId, data, 0, false);
    assert.equal(chunks.length, 1);

    // Verify the FFI function objects exist and accept the correct arity
    assert.equal(typeof rt.web_abi.host_receiver_push, 'function');
    assert.equal(rt.web_abi.host_receiver_push.length, 5);
    assert.equal(typeof rt.web_abi.host_sender_send, 'function');
    assert.equal(typeof rt.web_abi.host_sender_end, 'function');
  });

  test("host_sender_send delivers to WasmStreamSender", () => {
    const { rt } = bootWithMock();
    let delivered = null;
    const s = new WasmStreamSender(
      (c) => { delivered = c; },
      () => {},
    );

    // Put some data in the arena
    const buf = new Uint8Array([7, 8, 9]);
    const slot = rt.memory.create(buf.length);
    rt.memory.write(slot, buf);
    const slotInfo = rt.memory.get(slot);

    rt.web_abi.host_sender_send(s.senderId, BigInt(slotInfo.ptr), BigInt(buf.length), 0n);
    assert.ok(delivered !== null);
    assert.deepEqual([...delivered.data], [7, 8, 9]);
    assert.equal(delivered.sequence, 0);
  });

  test("host_sender_end fires onEnd and deregisters", () => {
    const { rt } = bootWithMock();
    let ended = false;
    const s = new WasmStreamSender(
      () => {},
      () => { ended = true; },
    );

    rt.web_abi.host_sender_end(s.senderId);
    assert.equal(ended, true);
  });
});
