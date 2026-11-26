import test from "node:test";
import assert from "node:assert/strict";
import { ConnectionManager } from "../../runtimes/foundation-ui-connections.js";

function setup(protocol) {
  const opened = [];
  const manager = new ConnectionManager();
  manager.register(protocol, ({endpoint}, events) => {
    const connection = {endpoint, events, sent: [], closed: false};
    opened.push(connection);
    return {send: bytes => connection.sent.push(bytes), close() { connection.closed = true; }};
  });
  return {manager, opened, options: {transport: protocol, endpoint: "/live", base: "https://example.test/"}};
}

test("a socket sender and two screens share a connection and keep independent bytes", async () => {
  const {manager, opened, options} = setup("websocket");
  const seen = [];
  const first = manager.acquire(options, {receive(bytes) { bytes[0] = 0; }});
  const second = manager.acquire(options, {receive(bytes) { seen.push(bytes[0]); }});
  const sender = manager.acquire({...options, endpoint: "wss://example.test/live"});
  assert.equal(opened.length, 1);
  opened[0].events.deliver(Uint8Array.of(9));
  assert.deepEqual(seen, [9]);
  first.close();
  await sender.send(Uint8Array.of(4));
  assert.equal(opened[0].sent[0][0], 4);
  second.close();
  assert.equal(opened[0].events.listening(), false);
  assert.equal(opened[0].closed, false);
  sender.close();
  assert.equal(opened[0].closed, true);
});

test("SSE defaults to dedicated and shared connections honor endpoint, authority and retention", () => {
  const {manager, opened, options} = setup("sse");
  const a = manager.acquire(options), b = manager.acquire(options);
  assert.equal(opened.length, 2);
  a.close(); b.close();
  const settings = {}, authority = {};
  const config = {...options, shared: true, settings, authority};
  const retained = manager.acquire({...config, keepAlive: true});
  const listener = manager.acquire(config);
  for (const other of [{endpoint:"/live?before=2"}, {settings:{}}, {authority:{}}]) manager.acquire({...config, ...other});
  assert.equal(opened.length, 6);
  listener.close(); retained.close();
  assert.equal(opened[2].closed, false);
  assert.equal(opened[2].events.listening(), false);
  const resumed = manager.acquire(config);
  assert.equal(opened.length, 6);
  retained.releaseRetention(); resumed.close();
  assert.equal(opened[2].closed, true);
  manager.close();
  assert.ok(opened.every(entry => entry.closed));
});

test("custom adapters discard callbacks from a closed connection", () => {
  const {manager, opened, options} = setup("custom");
  let count = 0;
  manager.acquire({...options, shared: true}, {receive() { count++; }}).close();
  opened[0].events.deliver(Uint8Array.of(7));
  assert.equal(count, 0);
});
