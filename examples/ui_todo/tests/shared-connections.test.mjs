// Run against the compiled todo server. Each test run starts a fresh store.
import test from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { createInterface } from "node:readline";
import * as ui from "../../../backends/foundation_wasm_ui/runtimes/foundation-wasm-ui.js";
import * as w from "../../../backends/foundation_wasm_ui/runtimes/foundation-ui-wire.js";
import { isFrame, base64urlEncode, readFrames } from "../../../backends/foundation_wasm_ui/runtimes/foundation-ui-frames.js";
import { WebActionManager, WebSocketActionTransport, JsActionRegistry } from "../../../backends/foundation_wasm_ui/runtimes/foundation-ui-actions.js";

function packet(bytes) {
  const frame = w.Frame.getRootAsFrame(new w.ByteBuffer(bytes));
  const body = frame.kind(new w.FrameMessage());
  return {frame, body, control: body.messagesLength() ? body.messages(0)?.control() : null};
}
function olderGreeting(bytes, version) {
  const frame = w.Frame.getRootAsFrame(new w.ByteBuffer(bytes)).unpack();
  frame.kind.hello.vocabularyVersion = version;
  const builder = new w.Builder();
  w.Frame.finishFrameBuffer(builder, frame.pack(builder));
  return base64urlEncode(builder.asUint8Array());
}
function resume(session, subscription, after) {
  const builder = new w.Builder();
  const request = new w.FrameRequestT(null, null, new w.ResumeSubscriptionT(session, subscription, 1n, after));
  const frame = new w.FrameT(1, session, 0n, w.FrameKind.FrameRequest, request);
  w.Frame.finishFrameBuffer(builder, frame.pack(builder));
  return builder.asUint8Array();
}
test("shared socket actions and per-subscription replay on SSE and streamed HTTP", {timeout: 20000}, async t => {
  const child = spawn(process.env.EWE_TODO_BIN ?? "target/uat/ui_todo", ["127.0.0.1:0"]);
  t.after(() => child.kill());
  const lines = createInterface({input: child.stdout});
  const [address] = await once(lines, "line");
  const base = address.match(/^Open (http:\/\/[^/]+)/)[1];
  ui.ProtocolHandler.frameHook = {isFrame};
  let opened = 0;
  const connections = ui.frameConnections({wsFactory(url) { opened++; return new WebSocket(url); }});
  t.after(() => connections.close());
  const socket = base + "/socket";
  const screens = [0, 1].map(() => {
    const server = WebSocketActionTransport.connect(connections, socket, {handshakeTimeoutMs: 4000});
    const actions = new WebActionManager({server, registry: new JsActionRegistry(),
      frames: {deps:{}, receive() {return true;}}, onError(error) {throw Error(JSON.stringify(error));}});
    return {server, actions};
  });
  const sessions = await Promise.all(screens.map(async ({server,actions}) => {
    const [bytes] = await server.send(actions.hello());
    const caps = packet(bytes).control.caps().unpack();
    actions.applyCaps(caps);
    assert.equal(caps.requestTag, server.requestTag);
    assert.ok(caps.subscriptionResume);
    return caps.streamId;
  }));
  assert.notEqual(sessions[0], sessions[1]);
  assert.equal(opened, 1);
  const latest = Promise.withResolvers();
  const second = Promise.withResolvers();
  const replay = Promise.withResolvers();
  const firstHistory = [], secondHistory = [];
  const laterReplay = Promise.withResolvers();
  const expired = Promise.withResolvers();
  const listener = connections.acquire({transport:"websocket", endpoint:socket}, {
    receive(bytes) {
      const {frame, body, control} = packet(bytes);
      if (frame.streamId() !== 1n) return;
      if (body.subscriptionId() === 77n && control?.replayExpired()) expired.resolve();
      if (body.subscriptionId() === 76n) {
        firstHistory.push(frame.sequenceId());
        if (frame.sequenceId() === 2n) replay.resolve();
      }
      if (body.subscriptionId() === 78n) {
        secondHistory.push(frame.sequenceId());
        if (frame.sequenceId() === 2n) laterReplay.resolve();
      }
      if (!body.subscriptionId() && frame.sequenceId() === 2n) second.resolve();
      if (!body.subscriptionId() && frame.sequenceId() === 1n) latest.resolve(bytes);
    },
  });
  const result = Promise.withResolvers();
  let wrongScreen = 0;
  screens[0].server.onReply = result.resolve;
  screens[1].server.onReply = () => { wrongScreen++; };
  const field = {value:"Socket todo",closest:()=>null,getAttribute:name=>name==="primal-id"?"44":null};
  await screens[0].actions.triggerStructured("server:add_todo",field,{id:crypto.randomUUID(), title:field.value},{type:"submit"});
  const {body,control} = packet(await result.promise);
  assert.equal(body.ack(), 1n);
  assert.equal(control?.error(), undefined);
  const update = packet(await latest.promise).body.unpack();
  const received = update.messages.find(message => message.data?.msgpack)?.data.msgpack;
  const confirmed = body.unpack().messages.find(message => message.data?.msgpack)?.data.msgpack;
  assert.ok(received?.bytes.length);
  assert.deepEqual(received.bytes, confirmed.bytes);
  const nextReply = Promise.withResolvers();
  screens[0].server.onReply = nextReply.resolve;
  await screens[0].actions.triggerStructured("server:add_todo", field, {id:crypto.randomUUID(), title:"Second task"}, {type:"submit"});
  await Promise.all([second.promise, nextReply.promise]);
  await listener.send(resume(sessions[0], 76n, 0n));
  await listener.send(resume(sessions[1], 78n, 1n));
  await Promise.all([replay.promise, laterReplay.promise]);
  assert.deepEqual(firstHistory, [1n, 2n]);
  assert.deepEqual(secondHistory, [2n]);
  await listener.send(resume(sessions[0], 77n, 99n));
  await expired.promise;
  assert.equal(wrongScreen, 0);
  assert.equal(opened, 1);

  const events = new ui.SSETransport({protocol:"frame", headers: {"X-Ewe-Hello":olderGreeting(screens[0].actions.hello(), 11)}});
  t.after(() => events.disconnect());
  const delivery = Promise.withResolvers(), targeted = Promise.withResolvers();
  const streamUrl = base + "/live";
  events.connect(streamUrl, undefined, ({frames}) => {
    for (const data of frames) {
      const {frame,body,control} = packet(data);
      if (control?.caps()) delivery.resolve(control.caps().streamId());
      if (body.subscriptionId() === 91n) targeted.resolve(frame.sequenceId());
    }
  });
  const session = await delivery.promise;
  const post = bytes => fetch(streamUrl,{method:"POST", body:bytes});
  assert.equal((await post(resume(sessions[0],91n,0n))).status,400);
  assert.equal((await post(resume(session,91n,0n))).status,204);
  assert.equal(await targeted.promise,1n);

  const cancel = new AbortController();
  t.after(() => cancel.abort());
  const binary = await fetch(streamUrl, {signal: cancel.signal, headers: {
    Accept: "application/primal-frame",
    "X-Ewe-Hello": olderGreeting(screens[1].actions.hello(), 10),
  }});
  assert.equal(binary.headers.get("content-type"), "application/primal-frame");
  const frames = readFrames(binary.body);
  const hello = packet((await frames.next()).value).control.caps();
  assert.ok(hello.subscriptionResume());
  assert.equal((await post(resume(hello.streamId(), 92n, 0n))).status, 204);
  const binaryUpdate = packet((await frames.next()).value);
  assert.equal(binaryUpdate.body.subscriptionId(), 92n);
  assert.equal(binaryUpdate.frame.sequenceId(), 1n);
  await frames.return();
});
