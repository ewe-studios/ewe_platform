const { describe, it } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const process = require("node:process");

const megatron = require("./megatron.js");

megatron.LOGGER.mode =  process.env.DEBUG ? megatron.LEVELS.DEBUG : megatron.LEVELS.INFO

const EXECUTING_DIR = path.dirname(__filename);

const wasm_buffer = fs.readFileSync(path.join(EXECUTING_DIR, "./module.wasm"));

const mock = {
  calls: [],
};

describe("Megatron.js_invoke_function", async () => {
  const runtime = new megatron.MegatronMiddleware();
  runtime.mock = mock;

  const wasm_module = await WebAssembly.instantiate(wasm_buffer, {
    abi: runtime.web_abi,
  });
  runtime.init(wasm_module);

  mock.select = (args) => {
    mock.calls.push({ method: "select", arguments: args });
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

    it("validate all argument types", async () => {
      assert.deepEqual(mock.calls, [
        {
          method: "select",
          arguments: [true, false, 10, 10, 10, 10, 10, 10, 10, 10, 10.0, 10.0],
        },
      ]);
    });
  });
});
