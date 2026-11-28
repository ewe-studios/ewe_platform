import {test} from "node:test";
import assert from "node:assert/strict";
import {readFileSync, readdirSync} from "node:fs";
import {MockDocument} from "../mock-dom.js";
import {NodeRegistry} from "../../runtimes/foundation-wasm-ui.js";
import {FrameApplicator} from "../../runtimes/foundation-ui-frames.js";

const directory = new URL("../fixtures/frames/", import.meta.url);
const reference = JSON.parse(readFileSync(new URL("legacy-operation-snapshots.json", directory), "utf8"));
const fixtures = readdirSync(directory).filter(file => file.endsWith(".new.bin")).sort();

function snapshot(node) {
  return {
    tag: node.tag, text: node.textContent, html: node.innerHTML, className: node.className,
    attrs: [...node.attributes], classes: [...node.classList._set], checked: node.checked,
    clickListeners: node.listenerCount("click"), style: node.style, children: node.children.map(snapshot),
  };
}

test("each of the 19 retired operation cases has a current fixture", () => {
  assert.equal(fixtures.length, 19);
  assert.deepEqual(fixtures.map(file => file.replace(".new.bin", "")), Object.keys(reference));
});

for (const file of fixtures) {
  const name = file.replace(".new.bin", "");
  test(`${name} keeps the recorded DOM and registry result`, () => {
    const document = new MockDocument();
    const registry = new NodeRegistry();
    const reports = [];
    const reader = new FrameApplicator({
      registry, document, schedule: run => run(), onReport: report => reports.push(report),
    });
    assert.ok(reader.receive(readFileSync(new URL(file, directory))));
    assert.deepEqual(reports, []);
    const state = [registry.nodes, registry.pending].map(table =>
      [...table].map(([id, node]) => [id, snapshot(node)]));
    assert.deepEqual(JSON.parse(JSON.stringify(state)), reference[name]);
  });
}
