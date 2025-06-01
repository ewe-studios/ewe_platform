const { describe, it } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const EXECUTING_DIR = path.dirname(__filename);

const wasm_buffer = fs.readFileSync(path.join(EXECUTING_DIR, "./module.wasm"));

function create_imports() {
  let v2_imports = {
    apply_instruction_calls: [],
    dom_allocate_external_pointer_values: [],
    object_allocate_external_pointer_values: [],
    function_allocate_external_pointer_values: [],
  };

  v2_imports.apply_instructions = (
    op_addr,
    op_length,
    text_addr,
    text_length,
  ) => {
    v2_imports.apply_instructions.push([
      op_addr,
      op_length,
      text_addr,
      text_length,
    ]);
  };

  v2_imports.function_allocate_external_pointer = () => {
    v2_imports.function_allocate_external_pointer_values.push([
      v2_imports.function_allocate_external_pointer_values.length,
    ]);
  };

  v2_imports.dom_allocate_external_pointer = () => {
    v2_imports.dom_allocate_external_pointer_values.push([
      v2_imports.dom_allocate_external_pointer_values.length,
    ]);
  };

  v2_imports.object_allocate_external_pointer = () => {
    v2_imports.object_allocate_external_pointer_values.push([
      v2_imports.object_allocate_external_pointer_values.length,
    ]);
  };

  const v1_imports = {
    drop_reference_calls: [],
    js_invoke_function_calls: [],
    js_register_function_calls: [],
    js_unregister_function_calls: [],
    js_register_function_at_calls: [],
  };

  v1_imports.drop_reference = function (external_id) {
    v1_imports.drop_reference_calls.push(external_id);
  };

  v1_imports.js_invoke_function = function (
    handler,
    parameter_start,
    parameter_length,
  ) {
    v1_imports.js_invoke_function_calls.push([
      handler,
      parameter_start,
      parameter_length,
    ]);
  };

  v1_imports.js_register_function = function (start, len, encoding) {
    v1_imports.js_register_function_calls.push([start, len, encoding]);
    return v1_imports.js_register_function_calls.length;
  };

  v1_imports.js_unregister_function = function (handler) {
    v1_imports.js_unregister_function_calls.push([handler]);
  };

  return { v1: v1_imports, v2: v2_imports };
}

describe("Megatron.registerfunction", async () => {
  const imports = create_imports();
  const { module, instance } = await WebAssembly.instantiate(wasm_buffer, {
    ...imports,
  });

  describe("Validate::setup", () => {
    it("able to validate module and instance", () => {
      assert.ok(module != undefined && module != null);
      assert.ok(instance != undefined && instance != null);
    });

    it("able to access instance memory", () => {
      assert.ok(instance.exports.memory != undefined);
    });
  });

  describe("Validate::FuncRegistration", async () => {
    // instance.exports.main();

    it("validate register function was called", async () => {
      
    });
  });
});
