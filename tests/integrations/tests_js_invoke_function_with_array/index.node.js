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

describe("Megatron.apply_instructions", async () => {
  const runtime = new megatron.MegatronMiddleware();
  runtime.mock = mock;

  const wasm_module = await WebAssembly.instantiate(wasm_buffer, {
    v1: runtime.v1_mappings,
    v2: runtime.v2_mappings,
  });
  runtime.init(wasm_module);

  mock.select = (args) => {
    console.log("SelectArgs: ", args);
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
          arguments: [
            "alex",
            new megatron.ExternalPointer(1),
            new megatron.InternalPointer(2),
            new Uint8Array([1, 1]),
            new Int8Array([1, 1]),
            new Uint16Array([1, 1]),
            new Int16Array([1, 1]),
            new Uint32Array([1, 1]),
            new Int32Array([1, 1]),
            new BigInt64Array([BigInt(2), BigInt(2)]),
            new BigUint64Array([BigInt(3), BigInt(3)]),
            new Float32Array([1.0, 1.0]),
            new Float64Array([1.0, 1.0]),
            new megatron.TypedArraySlice(
              megatron.TypedSlice.Uint8,
              new Uint8Array([4, 4]),
            ),
          ],
        },
      ]);
    });
  });
});
