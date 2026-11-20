import {test} from "node:test";
import assert from "node:assert/strict";
import {readFileSync, readdirSync} from "node:fs";
import {MockDocument} from "../mock-dom.js";
import {NodeRegistry, DomOpApplicator, decodeEnvelopeFrame} from "../../runtimes/foundation-wasm-ui.js";
import {FrameApplicator} from "../../runtimes/foundation-ui-frames.js";

const dir = new URL("../fixtures/frames/", import.meta.url);
function snapshot(node) {
  return {
    tag: node.tag, text: node.textContent, html: node.innerHTML, className: node.className,
    attrs: [...node.attributes], classes: [...node.classList._set], checked: node.checked,
    clickListeners: node.listenerCount("click"), style: node.style, children: node.children.map(snapshot),
  };
}
function registryState(reg) {
  return [reg.nodes, reg.pending].map(table => [...table].map(([id,node]) => [id,snapshot(node)]));
}
const pairs = readdirSync(dir).filter(file => file.endsWith(".old.bin"));
test("all legacy operation variants have fixtures", () => assert.equal(pairs.length, 19));
for (const file of pairs) {
  test(file.replace(".old.bin","") + " has the same DOM and node registry", () => {
    const doc = new MockDocument();
    const old = new NodeRegistry(), next = new NodeRegistry();
    const reference = new DomOpApplicator(old, doc);
    reference.apply(decodeEnvelopeFrame(readFileSync(new URL(file,dir))).columns);
    const reports = [];
    const reader = new FrameApplicator({registry:next, document:doc, schedule:fn=>fn(), onReport:r=>reports.push(r)});
    assert.ok(reader.receive(readFileSync(new URL(file.replace(".old.",".new."),dir))));
    assert.deepEqual(reports, []);
    assert.deepEqual(registryState(next), registryState(old));
  });
}
