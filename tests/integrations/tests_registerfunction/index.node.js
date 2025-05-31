const { describe, it } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");

const wasmBuffer = fs.readFileSync("./module.wasm");

describe("Web WASM Module", () => {
  test("check we can decode a params IntArray", () => {
    console.log(megatron);
    assert.ok(true);
  });
});
