import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { FoundationWasm } from "../../../foundation_wasm/runtime/foundation-wasm.js";
import * as ui from "../../runtimes/foundation-wasm-ui.js";
import * as w from "../../runtimes/foundation-ui-wire.js";
import { installFrameRuntime } from "../../runtimes/foundation-ui-frames.js";
import { MockDocument } from "../mock-dom.js";

const frame = (seq, text, id=42) => {
  const op=new w.OperationT(w.UiOperation.SetText,new w.SetTextT(id,text));
  const msg=new w.FrameMessageT(0n,[new w.MessageT(op)]);
  const value=new w.FrameT(1,7n,seq,w.FrameKind.FrameMessage,msg);
  const b=new w.Builder(); w.Frame.finishFrameBuffer(b,value.pack(b)); return b.asUint8Array();
};

function page(deps={}) {
  const doc=new MockDocument();
  doc.baseURI="https://test.invalid/page";
  doc.head=doc.createElement("head"); doc.body=doc.createElement("body");
  doc.root.appendChild(doc.body);
  const frames=installFrameRuntime(ui,{document:doc,schedule:f=>f(),...deps});
  for(const id of [42,43]) frames.registry.register(id,doc.createElement("span"));
  return {doc,frames};
}

function mount(doc, attrs, transport) {
  const el=doc.createElement("mount-stream"); el.ownerDocument=doc;
  el.setAttribute("protocol","frame");
  for(const [key,val] of Object.entries(attrs)) el.setAttribute(key,val);
  el._transportOverride=transport;
  const component=ui.MountStreamComponent.prototype;
  component.connectedCallback.call(el);
  return {el, close:()=>component.disconnectedCallback.call(el)};
}

test("gap resumes from the last complete render and ignores the old connection",()=>{
  let timeout,disconnected=0;
  const gap=[];
  const {doc,frames}=page({timer:{set:f=>{timeout=f;return 1;},clear(){}},onGap:g=>gap.push(g)});
  const connections=[];
  const transport={connect(url,data,receive){connections.push({url,receive});},disconnect(){disconnected++;}};
  const live=mount(doc,{api:"/live?section=todo",transport:"sse"},transport);
  const push=(n,section=42)=>({type:"frame",frames:[frame(BigInt(n),String(n),section)]});
  connections[0].receive(push(10)); connections[0].receive(push(12,43));
  assert.equal(frames.registry.get(43).textContent,"");
  timeout();
  assert.equal(disconnected,1);
  const retry=new URL(connections[1].url);
  assert.equal(retry.searchParams.get("section"),"todo");
  assert.equal(retry.searchParams.get("stream_id"),"7");
  assert.equal(retry.searchParams.get("resume_from"),"10");
  connections[0].receive(push(11));
  assert.equal(frames.cursor(7n),10n);
  connections[1].receive(push(11)); connections[1].receive(push(12,43));
  assert.equal(frames.registry.get(42).textContent,"11");
  assert.equal(frames.registry.get(43).textContent,"12");
  live.close();
  connections[1].receive(push(13)); assert.equal(frames.cursor(7n),12n);
  assert.equal(gap.length,1);
});
test("a local mount opens no transport and a resumed network mount preserves its cursor",()=>{
  const {doc,frames}=page();
  let urls=[];
  const transport={connect:url=>urls.push(url),disconnect(){}};
  mount(doc,{"data-channel-id":"7","data-resume-from":"9",transport:"local"},transport).close();
  assert.equal(urls.length,0);
  assert.equal(frames.cursor(7n),9n);
  const network=mount(doc,{api:"/updates","data-channel-id":"7","data-resume-from":"5",transport:"websocket"},transport);
  assert.equal(new URL(urls[0]).searchParams.get("resume_from"),"9");
  network.close();
});
test("host_apply copies a frame before acknowledging the actual Rust allocation",()=>{
  const jobs=[];
  const {doc,frames}=page({schedule:fn=>jobs.push(fn)});
  const runtime=new FoundationWasm();
  const binary=readFileSync(new URL("../fixtures/foundation_wasm_ui_e2e.wasm",import.meta.url));
  const abi={...runtime.web_abi,...ui.domAbi(runtime,new ui.DomHeap({window:null,document:null}))};
  runtime.init(new WebAssembly.Instance(new WebAssembly.Module(binary),{abi}));
  ui.registerWasmApp(runtime,{document:doc});
  const bytes=frame(0n,"copied");
  const api=runtime.bridge.exports;
  const slot=api.create_allocation(BigInt(bytes.length));
  const ptr=api.allocation_start_pointer(slot);
  const borrowed=new Uint8Array(runtime.bridge.memory.buffer,Number(ptr),bytes.length);
  borrowed.set(bytes);
  const disposed = [];
  const dispose = runtime.memory.dispose.bind(runtime.memory);
  runtime.memory.dispose = id => { disposed.push(id); return dispose(id); };
  runtime.web_abi.host_apply(slot,ptr,BigInt(bytes.length));
  assert.deepEqual(disposed,[slot]);
  const next=api.create_allocation(BigInt(bytes.length));
  const replacement=api.allocation_start_pointer(next);
  new Uint8Array(runtime.bridge.memory.buffer,Number(replacement),bytes.length).fill(0);
  assert.equal(replacement,ptr);
  assert.equal(frames.registry.get(42).textContent,"");
  jobs.shift()();
  assert.equal(frames.registry.get(42).textContent,"copied");
});

test("a reopened WebSocket requests replay and ignores the old socket", t => {
  t.mock.timers.enable({apis:["setTimeout"]});
  const {doc,frames} = page();
  const sockets = [];
  const transport = new ui.WebSocketTransport({ wsFactory(url) {
    const socket = {url, binaryType:"", close() {}};
    sockets.push(socket);
    return socket;
  }});
  const live = mount(doc, {api:"wss://test.invalid/live",transport:"websocket"}, transport);
  sockets[0].onmessage({data:frame(1n, "first")});
  assert.equal(frames.cursor(7n), 1n);
  sockets[0].onclose();
  t.mock.timers.tick(ui.reconnectDelay(0));
  const retry = new URL(sockets[1].url);
  assert.equal(retry.searchParams.get("resume_from"), "1");
  assert.equal(retry.searchParams.get("stream_id"), "7");
  sockets[0].onmessage({data:frame(2n, "old")});
  assert.equal(frames.cursor(7n), 1n);
  sockets[1].onmessage({data:frame(2n, "replayed")});
  assert.equal(frames.registry.get(42).textContent, "replayed");
  live.close();
});

test("chunked HTTP emits complete length-prefixed updates before the next chunk", async () => {
  const {frames} = page();
  const first = frame(1n,"one"), second = frame(2n,"two");
  const chunks = [first, second].map(bytes => {
    const data = new Uint8Array(4+bytes.length);
    new DataView(data.buffer).setUint32(0,bytes.length,true);
    data.set(bytes,4);
    return data;
  });
  const stream = new ReadableStream({start(controller) {
    controller.enqueue(chunks[0].slice(0,2));
    controller.enqueue(chunks[0].slice(2));
    controller.enqueue(chunks[1]); controller.close();
  }});
  const transport = new ui.ChunkedTransport({protocol:"frame",fetchFn:async()=>new Response(stream)});
  const received = [];
  await transport.connect("/live",undefined,result => {
    frames.receive(result.frames[0]); received.push(frames.registry.get(42).textContent);
  });
  assert.deepEqual(received,["one","two"]);
});
