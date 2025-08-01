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

describe("Megatron.tests_js_invoke_function_and_return_types", async () => {
  const runtime = new megatron.MegatronMiddleware();
  runtime.mock = mock;

  const wasm_module = await WebAssembly.instantiate(wasm_buffer, {
    abi: runtime.web_abi,
  });
  runtime.init(wasm_module);

  mock.returnArg = (v1) => {
    console.log("ReturnArg got: ", v1);
    mock.calls.push({
      method: "returnArg",
      arguments: [v1],
    });
    return v1;
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
      const type_slice = mock.calls[0];
      const type_slice_item = type_slice.arguments[0];
      console.log("Slice: ", type_slice, type_slice_item);
      assert.equal(true, type_slice_item instanceof megatron.TypedArraySlice);

      expected = [
        type_slice,
        {
          method: "returnArg",
          arguments: [5],
        },
        {
          method: "returnArg",
          arguments: [5],
        },
        {
          method: "returnArg",
          arguments: [5n],
        },
        {
          method: "returnArg",
          arguments: [5],
        },
        {
          method: "returnArg",
          arguments: [5],
        },
        {
          method: "returnArg",
          arguments: [5],
        },
        {
          method: "returnArg",
          arguments: [5n],
        },
        {
          method: "returnArg",
          arguments: [5],
        },
        {
          method: "returnArg",
          arguments: [5],
        },
        {
          method: "returnArg",
          arguments: [5],
        },
        {
          method: "returnArg",
          arguments: ["alex"],
        },
        {
          method: "returnArg",
          arguments: ["hello"],
        },
        {
          method: "returnArg",
          arguments: [new megatron.ErrorCode(50)],
        },
        {
          method: "returnArg",
          arguments: [undefined],
        },
        {
          method: "returnArg",
          arguments: [new megatron.InternalPointer(0)],
        },
        {
          method: "returnArg",
          arguments: [new megatron.ExternalPointer(0)],
        },
        {
          method: "returnArg",
          arguments: [10n],
        },
        {
          method: "returnArg",
          arguments: [10n],
        },
      ];

      console.log(mock.calls);
      console.log(expected);

      assert.deepEqual(mock.calls, expected);
      assert.equal(runtime.dom_heap.length(), 5);
      assert.equal(runtime.object_heap.length(), 0);
      assert.equal(runtime.function_heap.length(), 2);
    });
  });
});
