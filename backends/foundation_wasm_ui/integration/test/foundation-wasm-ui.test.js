import {test} from "node:test";
import assert from "node:assert/strict";
import {decodeEnvelopeFrame, Patcher} from "../../runtimes/foundation-wasm-ui.js";
import {MockDocument} from "../mock-dom.js";

function envelope(protocol, version, payload) {
  const out = new Uint8Array(6 + payload.length);
  out[0] = protocol;
  out[1] = version;
  new DataView(out.buffer).setUint32(2, payload.length, true);
  out.set(payload, 6);
  return out;
}

/** A document the applicator can mount onto (seedDocument needs `body`). */
function docWithBody() {
  const doc = new MockDocument();
  doc.body = doc.createElement("body");
  return doc;
}

test("a JSON signal-patch array still routes to the signal bridge (not the applicator)", () => {
  // Regression: only DomOp-row arrays (elements with `operation`) become a
  // DomOp batch; signal patches (`signalId`) must keep going to the bridge.
  const patches = [{ signalId: 1, value: "x" }];
  const result = decodeEnvelopeFrame(
    envelope(2, 1, new TextEncoder().encode(JSON.stringify(patches))),
  );
  assert.equal(result.type, "json", "signal patches are NOT a DomOp batch");

  const captured = [];
  Patcher.runtime.signalBridge = { applyPatches: (p) => captured.push(p) };
  Patcher.route(result, null, docWithBody());
  assert.deepEqual(captured[0], patches, "patches reached the signal bridge");
  Patcher.runtime.signalBridge = null;
});

test("HTML protocol: route materializes full markup into the mount target", () => {
  // HTML is the non-DomOp delivery — route('html') materializes markup into the
  // target element rather than applying ops. (MockDocument has no createRange,
  // so materialize falls back to target.innerHTML; hydrate no-ops on the mock.)
  const doc = new MockDocument();
  const target = doc.createElement("div");
  Patcher.route({ type: "html", html: "<p>hi html</p>" }, target, doc);
  assert.match(target.innerHTML, /hi html/, "markup materialized into the target");
});
