const { describe, it } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const process = require("node:process");

const megatron = require("./megatron.js");

megatron.LOGGER.mode = process.env.DEBUG
  ? megatron.LEVELS.DEBUG
  : megatron.LEVELS.INFO;

const EXECUTING_DIR = path.dirname(__filename);

const wasm_buffer = fs.readFileSync(path.join(EXECUTING_DIR, "./module.wasm"));

const mock = {
  calls: [],
};

describe("Megatron.apply_instructions", async () => {
  const runtime = new megatron.MegatronMiddleware();
  runtime.mock = mock;

  const wasm_module = await WebAssembly.instantiate(wasm_buffer, {
    abi: runtime.web_abi,
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
      const expected_arguments = [
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
      ];

      const mocked_arguments = mock.calls[0].arguments;
      const remaining_arguments = mocked_arguments.slice(
        0,
        mocked_arguments.length - 1,
      );

      const typed_slice_incoming =
        mocked_arguments[mocked_arguments.length - 1];
      assert.equal(
        true,
        typed_slice_incoming instanceof megatron.TypedArraySlice,
      );
      assert.equal(
        true,
        typed_slice_incoming.slice_content instanceof Uint8Array,
      );

      console.log("Calls: ", mocked_arguments);
      console.log("Expected: ", expected_arguments);

      assert.deepEqual(
        { method: "select", arguments: remaining_arguments },
        {
          method: "select",
          arguments: expected_arguments,
        },
      );
    });
  });
});
