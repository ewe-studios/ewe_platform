import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {JsActionRegistry, route} from "../../runtimes/foundation-ui-actions.js";

const vectors = JSON.parse(readFileSync(new URL("../../../foundation_ui_runtime/fixtures/action_routes.json", import.meta.url)));
for (const [index, row] of vectors.entries()) {
  test("shared routing vector " + index + " " + row.reference, () => {
    const registry = new JsActionRegistry();
    const kind = row.declaration;
    if (kind === "page") registry.declarePage("save");
    else if (kind !== "none") registry.declare(kind.startsWith("plain") ? null : kind, "save", row.scope, kind.endsWith("-local"));
    const has = Object.fromEntries(row.transports.map(k => [k, true]));
    const result = route(row.reference, row.surface, registry.lookup.bind(registry), has, () => row.advertised);
    assert.equal(result.error ?? result.route + ":save", row.expect);
  });
}

test("overlapping declarations fail and disjoint scopes coexist", () => {
  const registry = new JsActionRegistry();
  const fn = () => {};
  registry.registerHost("notify", fn);
  registry.declare("host", "notify", "mobile");
  assert.equal(registry.hostFunction("notify"), fn);
  assert.deepEqual(registry.refs("web"), ["host:notify"]);
  assert.throws(() => registry.declare(null, "notify"));
  assert.throws(() => registry.registerHost("notify", fn));
});
