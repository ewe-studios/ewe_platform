const { describe, it } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const megatron = require("./megatron.js");
megatron.LOGGER.mode = megatron.LEVELS.DEBUG;

const EXECUTING_DIR = path.dirname(__filename);

const wasm_buffer = fs.readFileSync(path.join(EXECUTING_DIR, "./module.wasm"));

const mock = {
  calls: [],
};

describe("Megatron.js_invoke_function_and_return_big_int", async () => {
  const runtime = new megatron.MegatronMiddleware();
  runtime.mock = mock;

  const wasm_module = await WebAssembly.instantiate(wasm_buffer, {
    v1: runtime.v1_mappings,
    v2: runtime.v2_mappings,
  });
  runtime.init(wasm_module);

  mock.calculateAge = (
    v1,
    v2,
    v3,
    v4,
    v5,
    v6,
    v7,
    v8,
    v9,
    v10,
    v11,
    v12,
    v13,
    v14,
  ) => {
    mock.calls.push({
      method: "calculateAge",
      arguments: [
        v1,
        v2,
        v3,
        v4,
        v5,
        v6,
        v7,
        v8,
        v9,
        v10,
        v11,
        v12,
        Math.round(v13 * 100) / 100,
        Math.round(v14 * 100) / 100,
      ],
    });
    return 100 * v1;
  };

  describe("Validate::setup", () => {
    const { module, instance } = wasm_module;

    it("able to validate module and instance", () => {
      assert.ok(module != undefined && module != null);
      assert.ok(instance != undefined && instance != null);
    });

    it("able to access instance memory", () => {
      assert.ok(instance.exports.memory != undefined);
    });
  });

  describe("Validate::Behaviour", async () => {
    const { module, instance } = wasm_module;
    instance.exports.main();

    it("validate registered functions effect", async () => {
      assert.deepEqual(mock.calls, [
        {
          method: "calculateAge",
          arguments: [
            5,
            5,
            10,
            10,
            5,
            5,
            10,
            10,
            10,
            10,
            true,
            false,
            10.2,
            10.4,
          ],
        },
      ]);
      assert.equal(runtime.dom_heap.length(), 5);
      assert.equal(runtime.object_heap.length(), 0);
      assert.equal(runtime.function_heap.length(), 1);
    });
  });
});
