const { describe, it } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const megatron = require("./megatron.js");

const EXECUTING_DIR = path.dirname(__filename);

const wasm_buffer = fs.readFileSync(path.join(EXECUTING_DIR, "./module.wasm"));

describe("Megatron.registerfunction", async () => {
  const runtime = new megatron.MegatronMiddleware();
  const wasm_module = await WebAssembly.instantiate(wasm_buffer, {
    v1: runtime.v1_mappings,
    v2: runtime.v2_mappings,
  });
  runtime.init(wasm_module);

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

  describe("Validate::FuncRegistration", async () => {
    const { module, instance } = wasm_module;
    instance.exports.main();

    it("validate register function was called", async () => {
      assert.equal(runtime.function_heap.length(), 1);
    });
  });
});
