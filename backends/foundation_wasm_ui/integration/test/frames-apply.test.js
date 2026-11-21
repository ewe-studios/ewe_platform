import {test} from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {FoundationWasm} from "../../../foundation_wasm/runtime/foundation-wasm.js";
import * as wire from "../../runtimes/foundation-ui-wire.js";
import * as ui from "../../runtimes/foundation-wasm-ui.js";
import {FrameApplicator, installFrameRuntime, base64urlDecode, splitFrames} from "../../runtimes/foundation-ui-frames.js";
import {registerWebBehaviour} from "../../runtimes/foundation-ui-behaviours.js";
import {MockDocument} from "../mock-dom.js";

const fixture = name => Uint8Array.from(readFileSync(new URL("../../../foundation_ui_wire/fixtures/"+name+".bin", import.meta.url)));
const operation = (name, ...args) => new wire.OperationT(wire.UiOperation[name], new wire[name+"T"](...args));
const web = (name,...args) => new wire.OperationT(0,null,wire.WebOperation[name],new wire[name+"T"](...args));
function frame(ops, seq = 0n, ack = 0n) {
  const msg = new wire.FrameMessageT(ack, ops.map(op=>new wire.MessageT(op)));
  const value = new wire.FrameT(1,1n,seq,wire.FrameKind.FrameMessage,msg);
  const builder = new wire.Builder();
  wire.Frame.finishFrameBuffer(builder,value.pack(builder));
  return builder.asUint8Array();
}
function setup(options = {}) {
  const doc = new MockDocument();
  doc.head = doc.createElement("head");
  doc.body = doc.createElement("body");
  doc.root.appendChild(doc.body);
  const events = [], reports = [], errors = [];
  doc.dispatchEvent = ev=>events.push(ev);
  const registry = new ui.NodeRegistry().seedDocument(doc);
  const reader = new FrameApplicator({document:doc,registry,schedule:fn=>fn(),
    onReport:r=>reports.push(r),onError:r=>errors.push(r),...options});
  return {registry,reader,reports,errors,events,doc,apply: (...rows)=>reader.receive(frame(rows))};
}
test("shared nodes, text, properties, action bindings and signed state", () => {
  const {apply,registry,reports,doc} = setup();
  const bytes = [0,255,241,4];
  apply(operation("CreateNode",42,wire.NodeKind.BUTTON),operation("SetText",42,"Buy milk"),
    operation("AttachState",42,bytes),operation("BindAction",42,wire.EventId.TAP,"server:buy"),
    operation("SetProp",42,wire.PropId.ENABLED,wire.PropValue.Bool,new wire.BoolValueT(false)),
    operation("AppendChild",1,42));
  const el = registry.get(42);
  assert.equal(el.parentNode,doc.body);
  assert.equal(el.textContent,"Buy milk");
  assert.deepEqual([...base64urlDecode(el.getAttribute("primal::state"))],bytes);
  assert.equal(el.getAttribute("disabled"),"");
  assert.equal(el.getAttribute("primal:onclick"),"server:buy");
  apply(web("WebSetAttribute",42,0,"PRIMAL::STATE","bad"),web("WebRemoveAttribute",42,0,"primal::state"),
    operation("SetText",4242,"missing"),operation("SetText",42,"next"),operation("UnbindAction",42,wire.EventId.TAP));
  assert.equal(el.textContent,"next");
  assert.equal(el.getAttribute("primal:onclick"),null);
  assert.deepEqual([...base64urlDecode(el.getAttribute("primal::state"))],bytes);
  assert.deepEqual(reports.map(r=>r.code),["BadFrame","BadFrame","UnknownNode"]);
});
test("selector placement resolves first and disposes removed scopes", () => {
  const {apply,registry,reports} = setup();
  const node = id=> operation("CreateNode",id,wire.NodeKind.TEXT);
  const select = id => new wire.TargetSelectorT(wire.SelectorKind.NodeId,id);
  const place = (kind,target,child,id,ops) =>
    operation("Place",wire.PlacementKind[kind],select(target),child ? select(child) : null,id,ops);
  apply(node(10),operation("AppendChild",1,10),place("Append",10,null,11,[node(11)]),
    place("Before",10,11,12,[node(12)]),place("After",10,11,13,[node(13)]));
  assert.deepEqual(registry.get(10).children.map(n=>n.getAttribute("primal-id")),["12","11","13"]);
  const before = registry.get(11);
  apply(place("Swap",11,null,11,[node(11),operation("SetText",11,"replaced")]),
    place("Remove",12,null,0,[]),place("Append",99,null,14,[node(14)]));
  assert.notEqual(registry.get(11),before);
  assert.equal(registry.get(11).textContent,"replaced");
  assert.equal(registry.get(12),undefined);
  assert.equal(registry.get(14),undefined);
  assert.deepEqual(reports.map(r=>r.code),["UnknownNode"]);
});
test("custom behaviours run once and unmount on removal", () => {
  const {apply,registry,reader,reports} = setup();
  let added=0, removed=0, disposed=0;
  registerWebBehaviour("once",()=>{added++; return ()=>removed++;});
  const bind = web("WebBehaviour",10,0,"once",[]);
  apply(operation("CreateNode",10,wire.NodeKind.TEXT),bind,bind,web("WebBehaviour",10,0,"missing",[]));
  reader.registerUnmount(registry.get(10),()=>disposed++);
  apply(operation("Remove",10));
  assert.deepEqual([added,removed,disposed],[1,1,1]);
  assert.deepEqual(reports.map(r=>r.code),["UnknownBehaviour"]);
});
test("stream order, acknowledgements, replay and gap timeout", () => {
  const queue=[], timers=new Map(), gaps=[];
  const {registry,reader,doc} = setup({schedule:fn=>queue.push(fn),timer:{set:fn=>{timers.set(fn,fn);return fn;},clear:id=>timers.delete(id)},onGap:g=>gaps.push(g)});
  const node=doc.createElement("p");
  registry.register(42,node);
  const send=(seq,text,ack=0n)=>reader.receive(frame([operation("SetText",42,text)],seq,ack));
  send(1n,"one"); send(3n,"three"); send(2n,"two"); send(2n,"old");
  assert.equal(queue.length,1); queue.shift()();
  assert.equal(node.textContent,"three");
  send(4n,"reply",1n); queue.shift()(); assert.equal(node.textContent,"reply");
  assert.equal(reader.cursor(1n),3n);
  send(6n,"six"); [...timers.values()][0]();
  assert.deepEqual(gaps,[{streamId:1n,resumeFrom:3n}]);
});
test("aligned and unaligned columns give the same text for all three rows", () => {
  const bytes=fixture("frame_columnar_3rows");
  for (const offset of [0,1]) {
    const {registry,doc,reader,reports} = setup();
    for (const id of [1,2,3]) registry.register(id,doc.createElement("p"));
    const copy=new Uint8Array(bytes.length+offset); copy.set(bytes,offset);
    assert.ok(reader.receive(copy.subarray(offset)));
    assert.deepEqual(reports,[]);
    assert.deepEqual([1,2,3].map(i=>registry.get(i).textContent),["a","bb","ccc"]);
    assert.equal(reader.stats[offset?"copiedColumns":"zeroCopyColumns"],5);
  }
});
test("wrong kind, short input, size limit, controls and data", () => {
  const {reader,reports,errors,events} = setup();
  assert.equal(reader.receive(fixture("hello_web")),false);
  assert.equal(reader.receive(fixture("frame_ops_counter").subarray(0,9)),false);
  reader.receive(fixture("frame_error")); reader.receive(fixture("frame_state_expired"));
  reader.receive(fixture("frame_data_table"));
  assert.deepEqual(errors.map(e=>e.code),["ActionFailed","StateExpired"]);
  const data=events.find(e=>e.type==="ewe:data");
  assert.equal(data.detail.kind,"table");
  assert.ok(data.detail.table.columns.some(c=>ArrayBuffer.isView(c.data)));
  assert.deepEqual(reports.map(r=>r.code),["BadFrame","BadFrame"]);
  const small=setup({maxFrameBytes:8});
  assert.equal(small.reader.receive(fixture("frame_ops_counter")),false);
  assert.equal(small.reports[0].code,"TooLarge");
});
test("transport reads split lengths, rejects truncated replies and routes binary frames", async () => {
  const {doc}=setup();
  const reader=installFrameRuntime(ui,{document:doc,schedule:fn=>fn()});
  assert.equal(installFrameRuntime(ui,{document:doc}),reader);
  const bytes=fixture("frame_ops_counter");
  const body=new Uint8Array(4+bytes.length);
  new DataView(body.buffer).setUint32(0,bytes.length,true); body.set(bytes,4);
  const response=new Response(new ReadableStream({start(ctrl){for(const b of body)ctrl.enqueue(Uint8Array.of(b));ctrl.close();}}),
    {headers:{"content-type":"application/primal-frame"}});
  const result=await ui.ProtocolHandler.fromContentType(response).process(response);
  assert.deepEqual(result.frames,[bytes]);
  const p=doc.createElement("p"); reader.registry.register(42,p);
  ui.Patcher.applyRoute(ui.decodeEnvelopeFrame(bytes),null,doc);
  assert.equal(p.textContent,"1");
  assert.throws(()=>splitFrames(body.subarray(0,-1)),/Truncated/);
  await assert.rejects(ui.ProtocolHandler.named("frame").process(new Response(body,{status:401})),/401/);
});
test("placement requires one target and one matching direct child", () => {
  const {apply,registry,doc,reports} = setup();
  const query = selector => new wire.TargetSelectorT(wire.SelectorKind.Query,0,selector);
  const add = id=>operation("CreateNode",id,wire.NodeKind.TEXT);
  apply(add(10),add(11),add(12),add(13),operation("AppendChild",10,11),operation("AppendChild",11,12),operation("AppendChild",10,13));
  const parent=registry.get(10), branch=registry.get(11), nested=registry.get(12), anchor=registry.get(13);
  nested.matches = sel=>sel===".anchor";
  branch.matches=()=>false; anchor.matches=sel=>sel===".anchor";
  doc.querySelectorAll=sel=>sel===".parent"?[parent]:[branch,anchor];
  const before = (sel,id)=>operation("Place",wire.PlacementKind.Before,query(sel),query(".anchor"),id,[add(id)]);
  apply(before(".duplicate",20),before(".parent",21));
  assert.equal(registry.get(20),undefined);
  assert.deepEqual(parent.children,[branch,registry.get(21),anchor]);
  registry.get(21).matches = sel=>sel===".anchor";
  apply(before(".parent",22));
  assert.equal(registry.get(22),undefined);
  assert.deepEqual(reports.map(r=>r.code),["UnknownNode","UnknownNode"]);
});

test("browser follows the shared native placement trace", () => {
  const {apply,registry,reports} = setup();
  const create = id => operation("CreateNode",id,wire.NodeKind.TEXT);
  const selector = id => new wire.TargetSelectorT(wire.SelectorKind.NodeId,id);
  const steps = JSON.parse(readFileSync(new URL("../../../foundation_ui_runtime/fixtures/placement_order.json",import.meta.url)));
  apply(create(10), create(20), create(21), operation("AppendChild",1,10), operation("AppendChild",20,21));
  for (const step of steps) {
    const before = reports.length;
    apply(operation("Place", wire.PlacementKind[step.kind], selector(step.target), step.child ? selector(step.child) : null,
      step.root ?? 0, step.root ? [create(step.root)] : []));
    assert.equal(reports.length > before, step.error ?? false);
    if (step.error) assert.equal(registry.get(step.root), undefined);
    assert.deepEqual(registry.get(10).children.map(n => Number(n.getAttribute("primal-id"))), step.order);
  }
});

test("parent-checked detachment keeps the child available for reattachment", () => {
  const {doc, registry, reports, apply} = setup();
  for (const id of [10,11,12]) {
    apply(operation("CreateNode",id,wire.NodeKind.STACK));
  }
  apply(operation("AppendChild",10,11));
  apply(web("WebDetachChild",12,11));
  assert.equal(registry.get(11).parentNode,registry.get(10));
  apply(web("WebDetachChild",10,11));
  apply(operation("AppendChild",12,11));
  assert.equal(registry.get(11).parentNode,registry.get(12));
  apply(operation("Remove",11));
  assert.equal(registry.get(11),undefined);
  assert.equal(reports.length,1);
});
test("Rust row and column encodings apply every eligible shared operation", () => {
  function read(name) {
    const {doc, registry, reports, reader} = setup();
    for (const id of [2,3,40]) registry.register(id,doc.createElement("div"));
    const bytes = readFileSync(new URL("../fixtures/frames/"+name+".bin",import.meta.url));
    assert.ok(reader.receive(bytes));
    assert.deepEqual(reports,[]);
    return [...registry.nodes].map(([id,n])=>[id,n.textContent,[...n.attributes],n.children.map(c=>c.tag)]);
  }
  assert.deepEqual(read("shared-units"),read("shared-columns"));
});
test("all operation fields decode using the prerequisites of each fixture row", () => {
  const contents = fixture("frame_ops_all_forms");
  const all = wire.Frame.getRootAsFrame(new wire.ByteBuffer(contents)).kind(new wire.FrameMessage());
  const codes=[];
  for (let i=0; i<all.messagesLength(); i++) {
    const {registry,doc,reader,reports}=setup({behaviours:{applyBehaviour:()=>true,applyWebBehaviour:()=>true}});
    for(const id of [1,2,3,40]) registry.register(id,doc.createElement("div"));
    registry.get(1).appendChild(registry.get(3));
    const op=all.messages(i).operation().unpack();
    assert.ok(reader.receive(frame([op])));
    codes.push(...reports.map(r=>r.code));
  }
  assert.deepEqual(codes,Array(3).fill("UnsupportedPlatformOp"));
});
test("receive handles every truncated prefix without throwing", () => {
  const {reader}=setup();
  const bytes=fixture("frame_ops_counter");
  for(let i=0; i<bytes.length; i++) assert.doesNotThrow(()=>reader.receive(bytes.subarray(0,i)));
});
test("application of a frame needs no network or WebAssembly", context => {
  const {apply,registry} = setup();
  context.mock.method(globalThis,"fetch",()=>assert.fail("unexpected fetch"));
  context.mock.method(WebAssembly,"instantiate",()=>assert.fail("unexpected WebAssembly instantiation"));
  apply(operation("CreateNode",41,wire.NodeKind.STACK),operation("SetText",41,"done"));
  assert.equal(registry.get(41).textContent,"done");
});
test("all vocabulary kinds and property types map to browser controls", () => {
  const {apply,registry,reports}=setup();
  const tags=["div","span","img","button","input","input","div","div","div","div"];
  tags.forEach((tag,i)=>{
    apply(operation("CreateNode",40+i,i+1));
    assert.equal(registry.get(40+i).tag,tag);
  });
  const rows=[
    ["PADDING","Length",new wire.LengthValueT(new wire.LengthT(3,wire.LengthUnit.Point)),"padding","3px"],
    ["SPACING","Length",new wire.LengthValueT(new wire.LengthT(4,wire.LengthUnit.Percent)),"gap","4%"],
    ["ALIGN","Alignment",new wire.AlignmentValueT(wire.Alignment.Center),"alignItems","center"],
    ["COLOR","ColorToken",new wire.ColorTokenT(2),"color","var(--ewe-color-2)"],
    ["FONT","FontToken",new wire.FontTokenT(1),"font","var(--ewe-font-1)"],
    ["AXIS","Axis",new wire.AxisValueT(wire.Axis.Horizontal),"flexDirection","row"],
    ["BACKGROUND","ColorToken",new wire.ColorTokenT(3),"backgroundColor","var(--ewe-color-3)"],
    ["WIDTH","Length",new wire.LengthValueT(new wire.LengthT(0,wire.LengthUnit.Auto)),"width","auto"],
    ["HEIGHT","Length",new wire.LengthValueT(new wire.LengthT(9,wire.LengthUnit.Point)),"height","9px"],
  ];
  const el=registry.get(40);
  for(const [prop,kind,val,attr,expected] of rows) {
    apply(operation("SetProp",40,wire.PropId[prop],wire.PropValue[kind],val));
    assert.equal(el.style[attr],expected);
  }
  for(const [prop,attr] of [["SOURCE","src"],["PLACEHOLDER","placeholder"],["A11Y_LABEL","aria-label"],["PRESENTATION","data-ewe-presentation"]]) {
    apply(operation("SetProp",40,wire.PropId[prop],wire.PropValue.Text,new wire.TextValueT("label")));
    assert.equal(el.getAttribute(attr),"label");
  }
  apply(operation("SetProp",40,wire.PropId.VALUE,wire.PropValue.Text,new wire.TextValueT("value")),
    operation("SetProp",40,wire.PropId.CHECKED,wire.PropValue.Bool,new wire.BoolValueT(true)),
    operation("SetProp",40,wire.PropId.HIDDEN,wire.PropValue.Bool,new wire.BoolValueT(true)));
  assert.equal(el.value,"value");
  assert.equal(el.checked,true);
  assert.equal(el.getAttribute("hidden"),"");
  assert.deepEqual(reports,[]);
});
test("shared dismiss and web composite install listeners once and release them", () => {
  const {registry,doc,reports,apply} = setup();
  apply(operation("CreateNode",44,wire.NodeKind.STACK));
  const el=registry.get(44);
  el.ownerDocument=doc.root;
  el.querySelectorAll=()=>[];
  const dismiss=operation("UiBehaviour",44,wire.BehaviourId.DISMISS,[]);
  const composite=web("WebBehaviour",44,0,"composite",[]);
  apply(dismiss,composite,dismiss,composite);
  assert.equal(doc.root.listenerCount("pointerdown"),1);
  assert.equal(el.listenerCount("keydown"),1);
  apply(operation("Remove",44));
  assert.equal(doc.root.listenerCount("pointerdown"),0);
  assert.equal(el.listenerCount("keydown"),0);
  assert.deepEqual(reports,[]);
});

test("external frame replacement disposes a real mounted Rust scope through WebAssembly", () => {
  const {doc} = setup();
  const frames = installFrameRuntime(ui,{document:doc,schedule:fn=>fn()});
  const runtime = new FoundationWasm();
  const wasm = readFileSync(new URL("../fixtures/foundation_wasm_ui_e2e.wasm",import.meta.url));
  const heap = new ui.DomHeap({window:null,document:null});
  const instance = new WebAssembly.Instance(new WebAssembly.Module(wasm), {
    abi:{...runtime.web_abi, ...ui.domAbi(runtime,heap)}
  });
  runtime.init(instance);
  ui.registerWasmApp(runtime,{document:doc});
  instance.exports.mount_scoped_button();
  assert.equal(frames.registry.get(77).parentNode,doc.body);
  const target = new wire.TargetSelectorT(wire.SelectorKind.NodeId,77);
  const next = operation("CreateNode",77,wire.NodeKind.TEXT);
  frames.receive(frame([operation("Place",wire.PlacementKind.Swap,target,null,77,[next])]));
  assert.equal(frames.registry.get(77).tag,"span");
  assert.equal(instance.exports.local_scope_releases(),1);
  frames.receive(frame([operation("Remove",77)]));
  assert.equal(instance.exports.local_scope_releases(),1);
});

test("web properties, classes, styles and listeners use generated operation fields", () => {
  let calls = 0;
  const {apply,registry,reports} = setup({onEvent(name,id) {
    assert.equal(name,"click"); assert.equal(id,88); calls++;
  }});
  const add = web("WebAddListener",88,0,"click");
  const remove = web("WebRemoveListener",88,0,"click");
  apply(web("WebElement",88,0,"button",""),web("WebRegisterNode",88),
    web("WebProperty",88,0,"disabled","true"),web("WebStyle",88,0,"color","red"),
    web("WebAddClass",88,"active"),add,add);
  const button = registry.get(88);
  assert.equal(button.disabled,true);
  assert.equal(button.style.color,"red");
  assert.ok(button.classList.contains("active"));
  button.dispatchEvent("click");
  assert.equal(calls,1);
  apply(remove,web("WebRemoveClass",88,"active"));
  button.dispatchEvent("click");
  assert.equal(calls,1);
  assert.ok(!button.classList.contains("active"));
  apply(add,operation("Remove",88));
  button.dispatchEvent("click");
  assert.equal(calls,1);
  assert.deepEqual(reports,[]);
});

test("the application loader delivers a callback reference to the WebAssembly registry", () => {
  const {doc} = setup();
  const button = doc.createElement("button");
  doc.body.appendChild(button);
  button.setAttribute("primal:onclick","callback-17");
  const calls = [];
  ui.registerWasmApp({
    bridge:{}, dispatcher:{setRawHandler() {}},
    callbacks:{invoke(id,bytes){calls.push([id,JSON.parse(new TextDecoder().decode(bytes))]);}},
  }, {document:doc});
  button.dispatchEvent("click");
  assert.equal(calls.length,1);
  assert.equal(calls[0][0],17);
  assert.equal(calls[0][1].type,"click");
});

test("bind after child insertion and keep distinct gesture modes on one parent", () => {
  const {doc,apply,registry,reports} = setup();
  const modes = [];
  const stage = doc.createElement("div");
  registry.register(42,stage);
  const render = new wire.BehaviourArgT("mode",wire.PropValue.Text,new wire.TextValueT("swipe"));
  const snap = new wire.BehaviourArgT("mode",wire.PropValue.Text,new wire.TextValueT("snap"));
  stage.ownerDocument = {defaultView:doc.createElement("window")};
  stage.style.setProperty = (name,value) => stage.style[name] = value;
  stage.querySelector = () => null;
  registerWebBehaviour("probe-after-children", (root) => modes.push(root.children.length));
  apply(web("WebBehaviour",42,0,"probe-after-children"),operation("CreateNode",43,wire.NodeKind.TEXT),
    operation("AppendChild",42,43),web("WebBehaviour",42,0,"gestures",[render]),
    web("WebBehaviour",42,0,"gestures",[snap]),web("WebBehaviour",42,0,"gestures",[snap]));
  assert.deepEqual(modes,[1]);
  assert.ok(stage.__swipe && stage.__snap);
  assert.deepEqual(reports,[]);
});

test("anonymous scoped functions receive their own element and release their listeners", () => {
  const {doc} = setup();
  const parent = doc.createElement("button");
  const script = doc.createElement("script");
  script.textContent = "function(scope){scope.addEvent(scope.parent(),'click',()=>scope.parent().textContent+='x')}";
  parent.appendChild(script);
  parent.querySelectorAll = query => query.startsWith("script") ? [script] : [];
  ui.Hydrator.hydrate(parent,doc);
  parent.dispatchEvent("click");
  assert.equal(parent.textContent,"x");
  ui.Patcher.releaseScope(parent);
  parent.dispatchEvent("click");
  assert.equal(parent.textContent,"x");
});
