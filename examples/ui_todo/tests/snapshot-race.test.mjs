// Concurrent adds make the open count equal the committed sequence at each snapshot.
import test from "node:test";
import assert from "node:assert/strict";
import {spawn} from "node:child_process";
import {once} from "node:events";
import {createInterface} from "node:readline";
import * as wire from "../../../backends/foundation_wasm_ui/runtimes/foundation-ui-wire.js";
import {base64urlEncode,readViewResponse} from "../../../backends/foundation_wasm_ui/runtimes/foundation-ui-frames.js";
import {FetchActionTransport,JsActionRegistry,WebActionManager} from "../../../backends/foundation_wasm_ui/runtimes/foundation-ui-actions.js";

test("concurrent snapshots report the revision of their displayed count",{timeout:30000},async t => {
  const server = spawn(process.env.EWE_TODO_BIN ?? "target/uat/ui_todo",["127.0.0.1:0"]);
  t.after(() => server.kill());
  const [line] = await once(createInterface({input:server.stdout}),"line");
  const base = new URL(line.match(/http:\/\/\S+/)[0]);
  const registry = new JsActionRegistry();
  registry.declare("server","add_todo");
  const sender = new FetchActionTransport({url:new URL("/action",base),hello:() => actions.hello()});
  const actions = new WebActionManager({registry,server:sender,frames:{receive(){return true}},
    onError:error => assert.fail(JSON.stringify(error))});
  const [greeting] = await sender.send(actions.hello());
  const frame = wire.Frame.getRootAsFrame(new wire.ByteBuffer(greeting));
  actions.applyCaps(frame.kind(new wire.FrameMessage()).messages(0).control().caps().unpack());
  async function capture() {
    const res = await fetch(new URL("/initial",base),{headers:{"X-Ewe-Hello":base64urlEncode(actions.hello())}});
    const packet = wire.Frame.getRootAsFrame(new wire.ByteBuffer(await readViewResponse(res)));
    const place = packet.kind(new wire.FrameMessage()).messages(0).operation().ui(new wire.Place());
    let cursor, count;
    for (let i=0; i<place.operationsLength(); i++) {
      const op = place.operations(i);
      if (op.uiType() === wire.UiOperation.LiveMount) cursor = op.ui(new wire.LiveMount()).resumeFrom();
      if (op.uiType() === wire.UiOperation.SetText) {
        const label = op.ui(new wire.SetText()).text()?.match(/^(\d+) open todos?$/);
        if (label) count = BigInt(label[1]);
      }
    }
    assert.notEqual(cursor,undefined);
    assert.equal(count,cursor);
    return count;
  }
  const input = {value:"Revision",closest:() => null,getAttribute:() => null};
  assert.equal(await capture(),0n);
  await Promise.all([
    (async () => { for (let i=0;i<20;i++) await actions.triggerStructured("server:add_todo",input,{id:crypto.randomUUID(), title:input.value},{type:"submit"}); })(),
    ...Array.from({length:3},async () => { for(let i=0;i<10;i++) await capture(); }),
  ]);
  assert.equal(await capture(),20n);
});
