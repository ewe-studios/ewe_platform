import test from "node:test";
import assert from "node:assert/strict";
import {FetchActionTransport, WasmActionTransport, WebSocketActionTransport} from "../../runtimes/foundation-ui-actions.js";

test("fetch reads split prefixes and frames, including multiple frames per chunk", async () => {
  let sent;
  const frame = Uint8Array.of(0,0,0,0,69,87,70,49);
  const body = new ReadableStream({ start(controller) {
    for (const bytes of [[8,0], [0,0,0], [...frame.slice(1),8,0,0,0,...frame]]) controller.enqueue(Uint8Array.from(bytes));
    controller.close();
  }});
  const transport = new FetchActionTransport({url:"/action", csrfToken:"token", hello:() => Uint8Array.of(255),
    fetch:(url, init) => { sent = {url, init}; return new Response(body); }});
  const bytes = Uint8Array.of(5);
  assert.deepEqual(await transport.send(bytes), [frame, frame]);
  assert.equal(sent.init.headers["x-ewe-hello"], "_w");
  assert.equal(sent.init.headers["x-csrf-token"], "token");
  assert.equal(sent.init.body, bytes);
});
test("http failure checks status before reading, and deadline retries need a policy", async () => {
  let calls = 0;
  const bad = {status:503, body:{cancel(){}, getReader(){ assert.fail("error body read"); }}};
  const fetch = () => { calls++; return bad; };
  await assert.rejects(new FetchActionTransport({fetch}).send(new Uint8Array()), /503/);
  assert.equal(calls, 1);
  calls = 0;
  const timeout = {fetch:(_, {signal}) => {calls++; signal.throwIfAborted();}, requestSignal:() => AbortSignal.abort(Error("deadline"))};
  await assert.rejects(new FetchActionTransport(timeout).send(new Uint8Array()), /deadline/);
  assert.equal(calls, 1);
  calls = 0;
  await assert.rejects(new FetchActionTransport({...timeout, retry:(_, attempt) => attempt < 2}).send(new Uint8Array()), /deadline/);
  assert.equal(calls, 2);
});
test("fetch rejects truncated and oversized replies", async () => {
  for (const bytes of [[1], [2,0,0,0,9], [100,0,0,0]]) {
    const transport = new FetchActionTransport({fetch:() => new Response(Uint8Array.from(bytes)), maxResponseBytes:20});
    await assert.rejects(transport.send(new Uint8Array()), /Truncated|Invalid/);
  }
});
test("socket accepts one message and does not wait for a reply", async () => {
  const sent = [];
  const socket = {readyState:1, send:bytes => sent.push(bytes)};
  const transport = new WebSocketActionTransport(socket);
  const bytes = Uint8Array.of(4);
  assert.deepEqual(await transport.send(bytes), []);
  socket.readyState = 3;
  await assert.rejects(transport.send(bytes), /closed/);
  socket.readyState = 1;
  assert.deepEqual(sent, [bytes]);
});
test("wasm calls complete in reverse order, free late replies and support slot zero", async () => {
  let next = 0n;
  const memory = new Uint8Array(100);
  const slots = new Set(), calls = [];
  const bridge = {memory:() => memory, exports:{
    create_allocation() {const slot = next++; slots.add(slot); return slot;},
    allocation_start_pointer: Number, allocation_length: () => 1,
    dispose_allocation(slot) {assert.ok(slots.delete(slot));},
    ewe_ui_action(call, slot) {calls.push(call); this.dispose_allocation(slot); return 0;},
  }};
  const transport = new WasmActionTransport(bridge);
  const first = transport.send(Uint8Array.of(8)), second = transport.send(Uint8Array.of(9));
  function finish(call, slot, value) {slots.add(slot); memory[Number(slot)] = value; transport.complete(call, slot);}
  finish(calls[1], 0n, 4);
  finish(calls[0], 2n, 5);
  assert.deepEqual(await first, [Uint8Array.of(5)]);
  assert.deepEqual(await second, [Uint8Array.of(4)]);
  const missing = transport.send(Uint8Array.of(1));
  transport.complete(calls.at(-1), -1n);
  await assert.rejects(missing, /failed/);
  const pending = transport.send(Uint8Array.of(1));
  transport.close();
  await assert.rejects(pending, /closed/);
  finish(calls.at(-1), 2n, 7);
  assert.equal(slots.size, 0);
});
