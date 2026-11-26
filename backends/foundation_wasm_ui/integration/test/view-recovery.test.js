import {test} from "node:test";
import assert from "node:assert/strict";
import {ViewRecovery} from "../../runtimes/foundation-ui-recovery.js";

test("one recovery retains content, combines expired streams and discards late results", async () => {
  let content = "old", loads = 0;
  const gates = [Promise.withResolvers(), Promise.withResolvers(), Promise.withResolvers()];
  const seen = [];
  const r = new ViewRecovery(() => gates[loads++].promise,(value, required) => {
    seen.push([...required]); content = value;
  });
  const initial = r.open();
  gates[0].resolve("first");
  await initial;
  const previousGeneration = r.generation;
  const one = r.expire(1n);
  assert.equal(r.expire(2n), one);
  assert.equal(content, "first");
  assert.equal(r.accepts(previousGeneration),false);
  gates[1].resolve("second");
  await one;
  assert.deepEqual(seen,[[],[1n,2n]]);
  const pending = r.expire(1n);
  r.close();
  gates[2].resolve("late");
  await pending;
  assert.equal(content,"second");
});

test("a failed snapshot preserves the view and explicit retry resets the budget", async () => {
  let content = "old", loads = 0;
  const errors = [];
  const r = new ViewRecovery(async () => ++loads, (value, required) => {
    if (value === 1) throw new Error("Missing cursor");
    assert.deepEqual([...required],[7n]);
    content = value;
  },{maxAttempts:1, onError:error => errors.push(error.message)});
  await assert.rejects(r.expire(7n),/Missing cursor/);
  assert.equal(content,"old");
  r.expire(7n);
  assert.equal(loads,1);
  await r.retry();
  assert.equal(content,2);
  assert.deepEqual(errors,["Missing cursor","View recovery attempt limit reached"]);
});

test("an expired listener keeps its shared connection while the other view advances", async () => {
  const {ConnectionManager} = await import("../../runtimes/foundation-ui-connections.js");
  const manager = new ConnectionManager();
  let channel, closed = 0;
  manager.register("websocket",(_,events) => { channel = events; return {close(){closed++;}}; });
  const gate = Promise.withResolvers();
  const first = new ViewRecovery(() => gate.promise,() => {});
  const second = new ViewRecovery(async () => 0,() => {});
  await second.open();
  const secondGeneration = second.generation, updates = [];
  const options = {endpoint:"ws://localhost/live",transport:"websocket"};
  let pending;
  const a = manager.acquire(options,{owner:first,accept:bytes => bytes[0] === 1,receive:() => { pending = first.expire(7n); }});
  const b = manager.acquire(options,{owner:second,accept:bytes => bytes[0] === 2,receive:bytes => {
    if (second.accepts(secondGeneration)) updates.push(bytes[1]);
  }});
  channel.deliver(Uint8Array.of(1));
  channel.deliver(Uint8Array.of(2,21));
  assert.deepEqual(updates,[21]);
  a.close();
  assert.equal(closed,0);
  gate.resolve("snapshot");
  await pending;
  channel.deliver(Uint8Array.of(2,22));
  assert.deepEqual(updates,[21,22]);
  b.close();
  assert.equal(closed,1);
});
