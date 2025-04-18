// Runtime module for the megatron WASM bindgen which provides core functionality
// required to enjoy and interact with the browser and APIs.
//
// It's meant to both be simple but also extensively useful.

const ARENA = (function () {
  // Javascript module implements the necessary ArenaList used for
  // representing different javascript objects that are cached and
  // controlled by WebAssembly, though the structure can be used for
  // any usecase where control and optimized usage of a list is important.

  const MAX_SIZE = BigInt(0xfffffff0);
  const BIT_MASK = BigInt(0xffffffff);
  const BIT_SIZE = BigInt(32);

  function create_entry(index, generation) {
    return {
      index,
      generation,
    };
  }

  function uid_to_entry(entry_as_u32) {
    // shifts the bit to the leftmost bit which means
    // we shift the value of the index stored in the MSB  to the LSB
    // and clear the MSB to 0000 and then Mask with 0xFFFF so we can
    // only get the values in the LSB
    let extracted_index = (BigInt(entry_as_u32) >> BIT_SIZE) & BIT_MASK; // using <<< to instruct js we want an unsigned 16 bit number;
    let extracted_generation = BigInt(entry_as_u32) & BIT_MASK; // mask the LSB which is the generation 16 bit and extract the number.

    return create_entry(extracted_index, extracted_generation);
  }

  // [entry_to_uid] generates a unsigned 32 bit integer representing
  // a given entry which basically just uses packing to create a singular
  // number that contains both so we can reduce storage size.
  function entry_to_uid(entry) {
    const index_as_16bit = BigInt(entry.index) & BIT_MASK; // Max to ensure we have 65,535 bits (16bit numbers)
    const generation_as_16bit = BigInt(entry.generation) & BIT_MASK; // Max to ensure we have 65,535 bits (16bit numbers)

    // shift the index to the MSB(Most Significant Bits, starting from the leftmost bit).
    // others say: shifting to the upper bits (so leftmost)
    // Then: shift the generation uid to the rightmost bit (Least Significant bit).
    // others say: shifting to the lower bits (so rightmost)
    let packed_uid = (BigInt(index_as_16bit) << BIT_SIZE) | generation_as_16bit;
    return packed_uid;
  }

  function create_arena() {
    let arena = {
      items: [],
      free: [],
    };

    let create_slot = (item, index, generation) => {
      return { item, index, generation, active: true };
    };

    let allocate_slot = () => {
      if (arena.free.length > 0) {
        let available_slot_index = arena.free.pop();
        let slot_entry = arena.items[available_slot_index];
        if (slot_entry.active > 0) {
          throw Error("Active slot cant be in free list");
        }
        slot_entry.active = true;
        slot_entry.generation += 1;
        return slot_entry;
      }

      let slot = create_slot(null, arena.items.length, 0);
      arena.items.push(slot);
      return slot;
    };

    // return an active entry if entry.active is true else
    // return null;
    let get_entry_at_index = (index) => {
      let entry = arena.items[index];
      if (entry.active) return entry;
      return null;
    };

    let get_index = (index) => {
      if (index >= arena.items.length) {
        return null;
      }
      entry = arena.items[index];
      return entry.item;
    };

    let get_entry = (uid) => {
      let candidate = uid_to_entry(uid);
      if (candidate.index >= arena.items.length) {
        return null;
      }
      entry = get_entry_at_index(candidate.index);
      if (entry == null) return null;
      return entry.item;
    };

    let add_entry = (item) => {
      let slot = allocate_slot();
      slot.item = item;
      slot.active = true;
      return entry_to_uid(slot);
    };

    let remove_entry = (uid) => {
      let candidate = uid_to_entry(uid);
      if (candidate.index >= arena.items.length) {
        return false;
      }
      let entry = arena.items[candidate.index];
      if (candidate.generation != entry.generation) {
        return false;
      }

      entry.item = null;
      entry.active = false;

      // if item generation is more than our allowed
      // max generation then do not add back to freelist
      // which then makes it unusable and we do not nee to
      // worry about free-list.
      if (entry.generation >= MAX_SIZE) {
        return false;
      }

      arena.free.push(entry);
      return true;
    };

    return {
      __arena: arena,
      get: get_entry,
      index: get_index,
      create: add_entry,
      destroy: remove_entry,
      to_uid: entry_to_uid,
      from_uid: uid_to_entry,
      _entry: create_entry,
    };
  }

  let arena_instance = create_arena();

  // add entries for base JS types at the top,
  // and allocate 1-4 for use
  // always for these.
  //
  // 0 is reserved for undefined
  // 1 is reserved for null
  // 2 is reserved for self
  // 3 is reserved for document
  // 4 is reserved for window
  // 5 is reserved for document.body
  // 6 is reserved for false
  // 7 is reserved for true
  //
  arena_instance.create(undefined);
  arena_instance.create(null);
  arena_instance.create(self);
  arena_instance.create(typeof window != "undefined" ? window : null);
  arena_instance.create(typeof document != "undefined" ? document : null);
  arena_instance.create(
    typeof document != "undefined" && document && document.body
      ? document.body
      : null,
  );
  arena_instance.create(false);
  arena_instance.create(true);

  // wrap the destroy to ensure it never clears our first 5 fixed
  // objects
  const SECURE_INSTANCE_OFFSET = 7n;
  const _destroy = arena_instance.destroy;
  arena_instance.destroy = function (uid) {
    let candidate = uid_to_entry(uid);
    if (candidate.index < SECURE_INSTANCE_OFFSET) {
      return false;
    }
    return _destroy(uid);
  };

  return arena_instance;
})();

const MemoryOperations = function (getWasmInstanceFunc) {
  const allocate_memory = function (size) {
    // create an allocation within the wasm instance by
    // calling its create_allocation exported function
    // as we expect.
    const wasmInstance = getWasmInstanceFunc();
    const allocation_id = wasmInstance.exports.create_allocation(size);
    const allocation_start_pointer =
      wasmInstance.exports.allocation_start_pointer(allocation_id);
    return [allocation_id, allocation_start_pointer];
  };

  const get_memory = function () {
    return getWasmInstanceFunc().instance.exports.memory.buffer;
  };

  const read_uint8_buffer = function (start_pointer, len) {
    const memory = get_memory();
    const slice = memory.slice(start_pointer, start_pointer + len);
    return new Uint8Array(slice);
  };

  const write_int8_buffer = function (uint8_buffer) {
    const len = uint8_buffer.length;
    const [id, start] = allocate_memory(len);

    const memory = get_memory();
    memory.set(uint8_buffer, start);

    return id;
  };

  const write_array_buffer = function (array_buffer) {
    const uint8_buffer = new Uint8Array(array_buffer);
    return write_int8_buffer(uint8_buffer);
  };

  return {
    get_memory,
    allocate_memory,
    buffer: {
      read: read_uint8_buffer,
      write: write_int8_buffer,
    },
    array_buffer: {
      write: write_array_buffer,
    },
  };
};

/// Runtime implementation for using TextEncoder and TextDecoder for utf8
//  string conversion from rust native UTF8 to webs UTF16 as desired.
const UTFCodec = function (get_memory_function, allocate_memory_function) {
  const utf8_encoder = new TextEncoder();
  const utf8_decoder = new TextDecoder("utf-8");
  const utf16_decoder = new TextDecoder("utf-16");

  const readUTF8FromMemory = function (start, len) {
    const memory = get_memory_function();
    const data_slice = memory.slice(start, start + len);
    return utf8_decoder.decode(data_slice);
  };

  const writeUTF8FromMemory = function (text) {
    const bytes = utf8_encoder.encode(text);
    const len = bytes.length;
    const [id, start] = allocate_memory_function(len);

    const memory = get_memory_function();
    memory.set(bytes, start);
    return id;
  };

  const readUTF16FromMemory = function (start, len) {
    const bytes = get_memory_function().subarray(start, start + len);
    const text = utf16_decoder.decode(bytes);
    return text;
  };

  return {
    read_utf8: readUTF8FromMemory,
    write_utf8: writeUTF8FromMemory,
    read_utf16: readUTF16FromMemory,
  };
};

/// FlatBuffer runtime implementation for using FlatBuffer as a serialization
//  and deserialization system for communication between rust and the web.
const FlatbufferCodec = function (get_memory_function) {};

// exposes the core runtime functions for interoperating with WASM & Web.
const WASMRuntime = function () {
  const runtime = {
    heap: ARENA,
    module: null,
  };

  runtime.get_wasm_module = function () {
    return runtime.module;
  };

  const memory_ops = MemoryOperations(runtime.get_wasm_module);
  runtime.memory_ops = memory_ops;

  const utf_codec = UTFCodec(memory_ops.get_memory, memory_ops.allocate_memory);
  runtime.utf_codec = utf_codec;

  // 32 bits is 4 bytes (as 1 byte = 8 bits), so each memory location
  // is 32 bits in Javascript, so when moving index we move by 32 bits each.
  const MOVE_INDEX_BY_32BITS = 4;

  // Javascript is 32 bit memory, 4 bit makes 32bit, to move 64bits then
  // its 4 bytes x 2 = 8 bytes.
  const MOVE_INDEX_BY_64BIT = 8;

  const parameters_readers = {
    // 0 - means we want `undefined`
    0: function (index, read_values_list, parameter_buffer) {
      read_values_list.push(undefined);
      return [0, false];
    },
    // 0 - means we want `null`
    1: function (index, read_values_list, parameter_buffer) {
      read_values_list.push(null);
      return [0, false];
    },
    // 2 means we want to read a float64;
    2: function (index, read_values_list, parameter_buffer) {
      const view = new DataView(parameter_buffer.buffer).getFloat64(
        index,
        true,
      );
      read_values_list.push(view);
      return [MOVE_INDEX_BY_64BIT, false];
    },
    // 3 means we want to read a BigInt64;
    3: function (index, read_values_list, parameter_buffer) {
      const view = new DataView(parameter_buffer.buffer).getBigInt64(
        index,
        true,
      );
      read_values_list.push(view);
      return [MOVE_INDEX_BY_64BIT, false];
    },
    // 4 = string (followed by 32-bit start and size of string in wasm memory)
    // 4 means we want to read a int32 memory size where we have 4 bytes for start, 4 bytes for length which
    // indicate the memory range we need to read;
    4: function (index, read_values_list, parameter_buffer) {
      let start_index = index;
      const start = new DataView(parameter_buffer.buffer).getInt32(
        start_index,
        true,
      );
      start_index += MOVE_INDEX_BY_32BITS;

      const length = new DataView(parameter_buffer.buffer).getInt32(
        start_index,
        true,
      );
      start_index += MOVE_INDEX_BY_32BITS;

      const data = utf_codec.read_utf8(start, length);
      read_values_list.push(data);
      return [start_index - index, false];
    },
    // 5 = extern ref
    5: function (index, read_values_list, parameter_buffer) {
      const handle_uid = new DataView(parameter_buffer.buffer).getBigInt64(
        index,
        true,
      );
      read_values_list.push(ARENA.get(handle_uid));
      return [MOVE_INDEX_BY_64BIT, false];
    },
    // 6 = array of Float32 from wasm memory (followed by 32-bit start and size of string in memory)
    6: function (index, read_values_list, parameter_buffer) {
      let start_index = index;
      const start = new DataView(parameter_buffer.buffer).getInt32(
        start_index,
        true,
      );
      start_index += MOVE_INDEX_BY_32BITS;

      const length = new DataView(parameter_buffer.buffer).getInt32(
        start_index,
        true,
      );
      start_index += MOVE_INDEX_BY_32BITS;

      const memory = memory_ops.get_memory();
      const slice = memory.buffer.slice(start, start + length * 4);
      const array = new Float32Array(slice);

      read_values_list.push(array);
      return [start_index - index, false];
    },
    // 7 = true
    7: function (index, read_values_list, parameter_buffer) {
      read_values_list.push(true);
      return [0, false];
    },
    // 8 = false
    8: function (index, read_values_list, parameter_buffer) {
      read_values_list.push(false);
      return [0, false];
    },
    // 9 = array of Float64 from wasm memory (followed by 32-bit start and size of string in memory)
    9: function (index, read_values_list, parameter_buffer) {
      let start_index = index;
      const start = new DataView(parameter_buffer).getInt32(start_index, true);
      start_index += MOVE_INDEX_BY_32BITS;

      const length = new DataView(parameter_buffer).getInt32(start_index, true);
      start_index += MOVE_INDEX_BY_32BITS;

      const memory = memory_ops.get_memory();
      const slice = memory.buffer.slice(start, start + length * 4);
      const array = new Float64Array(slice);

      read_values_list.push(array);
      return [start_index - index, false];
    },
    // 10 = array of Uint32 from wasm memory (followed by 32-bit start and size of string in memory)
    10: function (index, read_values_list, parameter_buffer) {
      let start_index = index;
      const start = new DataView(parameter_buffer).getInt32(start_index, true);
      start_index += MOVE_INDEX_BY_32BITS;

      const length = new DataView(parameter_buffer).getInt32(start_index, true);
      start_index += MOVE_INDEX_BY_32BITS;

      const memory = memory_ops.get_memory();
      const slice = memory.buffer.slice(start, start + length * 4);
      const array = new Uint32Array(slice);

      read_values_list.push(array);
      return [start_index - index, false];
    },
  };

  const read_parameters = function (start, length) {
    // handles reading out parameters of calls from wasm memory.
    const converted_values = [];

    const parameters = memory_ops.buffer.read(start, length);
    console.debug("read_parameters: ", parameters, start, length);

    let index = 0;
    while (index < parameters.length) {
      const type_id = parameters[index];
      index++;

      if (type_id in parameters_readers) {
        const reader_func = parameters_readers[type_id];
        const [move_index_by, should_break] = reader_func(
          index,
          converted_values,
          parameters,
        );
        index += move_index_by;
        if (should_break) break;
        continue;
      }

      console.debug("Read: ", converted_values);
      throw new Error(`unknown parameter type ${type_id}`);
    }

    return converted_values;
  };
  runtime.read_parameters = read_parameters;

  // list of registered functions usable on both sides
  //
  // 0 - contains a function that starts the debugger.
  runtime.functions = [
    function () {
      debugger;
      return 0;
    },
  ];

  runtime.run_wasm = function () {
    if (!runtime.module) throw new Error("No wasm module attached");
    if (!runtime.module.instance.exports.main)
      throw new Error("wasm module has no exported main");
    return runtime.module.instance.exports.main();
  };

  const environment = {
    data: {},
    refs: {},
    funcs: {},
  };

  runtime.environment = environment;

  environment.abort = function () {
    throw new Error("WasmInstance calls abort");
  };

  environment.refs.drop_external_reference = function (uid) {
    return runtime.heap.destroy(uid);
  };

  const ALLOWED_UTF8_INDICATOR = [8, 16];

  // register a function on the javascript side for execution via a handle id.
  environment.funcs.js_register_function = function (
    start,
    length,
    utf_indicator,
  ) {
    console.debug(
      "Register function: ",
      start,
      length,
      " UTF8: ",
      utf_indicator,
    );
    let function_body = null;

    if (ALLOWED_UTF8_INDICATOR.indexOf(utf_indicator) === -1) {
      throw new Error("Unsupported UTF indicator (only 8 or 16)");
    }

    if (utf_indicator === 16) {
      function_body = utf_codec.read_utf16(start, length);
    }

    if (utf_indicator === 8) {
      function_body = utf_codec.read_utf8(start, length);
    }

    if (!function_body) throw new Error("Function body must be supplied");

    const id = runtime.functions.length;
    const registered_func = Function(
      `"use strict"; return(${function_body})`,
    )();
    runtime.functions.push(registered_func);
    return id;
  };

  environment.funcs.js_invoke_function = function (
    handle,
    parameter_start,
    parameter_length,
  ) {
    console.debug("FuncHandle: ", handle, parameter_start, parameter_length);
    // read parameters and invoke function via handle.
    const parameters = read_parameters(parameter_start, parameter_length);
    console.debug("FuncHandleReadParameters: ", handle, parameters);

    if (!parameters && parameter_length > 0)
      throw new Error("No parameters returned though we expect some");
    console.debug("FuncHandleReadParameters: ", handle, parameters);

    const func = runtime.functions[handle];
    console.debug("FuncHandleFunc: ", handle, func);
    return func.call(runtime, ...parameters);
  };

  environment.funcs.js_invoke_function_and_return_object = function (
    handle,
    parameter_start,
    parameter_length,
  ) {
    // read parameters and invoke function via handle.
    const parameters = read_parameters(parameter_start, parameter_length);
    const func = functions[handle];
    const result = func.call(runtime, ...parameters);
    if (result === undefined || result === null) {
      throw new Error(
        "function returned undefined or null while trying to return an object",
      );
    }
    return runtime.heap.create(result);
  };

  environment.funcs.js_invoke_function_and_return_bool = function (
    handle,
    parameter_start,
    parameter_length,
  ) {
    // read parameters and invoke function via handle.
    const parameters = read_parameters(parameter_start, parameter_length);
    const func = functions[handle];
    const result = func.call(runtime, ...parameters);
    return result ? 1 : 0;
  };

  environment.funcs.js_invoke_function_and_return_bigint = function (
    handle,
    parameter_start,
    parameter_length,
  ) {
    // read parameters and invoke function via handle.
    const parameters = read_parameters(parameter_start, parameter_length);
    const func = functions[handle];
    return func.call(runtime, ...parameters);
  };

  environment.funcs.js_invoke_function_and_return_string = function (
    handle,
    parameter_start,
    parameter_length,
  ) {
    // read parameters and invoke function via handle.
    const parameters = read_parameters(parameter_start, parameter_length);
    const func = functions[handle];
    const result = func.call(runtime, ...parameters);
    if (result === undefined || result === null) {
      throw new Error(
        "function returned undefined or null while trying to return an object",
      );
    }
    return memory_ops.array_buffer.write(result);
  };

  return [environment, runtime];
};

const Megatron = (function () {
  const loadURL = async function (wasmURL) {
    return await fetch(wasmURL);
  };

  const configCompiledOptions = function (compileOptions) {
    const merged_compiled_options = {
      builtins: ["js-strings"],
      importedStringConstants: "imported_strings",
    };

    if (compileOptions) {
      if ("builtins" in compileOptions)
        merged_compiled_options.builtins = compileOptions.builtins;
      if ("importedStringConstants" in compileOptions)
        merged_compiled_options.importedStringConstants =
          compileOptions.importedStringConstants;
    }

    return merged_compiled_options;
  };

  const loadWASM = async function (
    streaming,
    wasm_source,
    compileOptions,
    memory,
  ) {
    const [env, runtime] = WASMRuntime();
    const compiled_options = configCompiledOptions(compileOptions);

    if (!memory) {
      memory = new WebAssembly.Memory({ initial: 10, maximum: 200 });
    }

    let module;
    if (streaming) {
      module = await WebAssembly.instantiateStreaming(
        wasm_source,
        {
          funcs: env.funcs,
          data: env.data,
          refs: env.refs,
          js: { mem: memory },
        },
        compiled_options,
      );
    } else {
      module = await WebAssembly.instantiate(
        wasm_source,
        {
          funcs: env.funcs,
          data: env.data,
          refs: env.refs,
        },
        compiled_options,
      );
    }

    module.compiled_options = compiled_options;

    // we must set the module else things fail
    runtime.module = module;
    return [env, runtime];
  };

  const loadWASMBytes = async function (wasm_bytes, compileOptions) {
    return loadWASM(false, wasm_bytes, compileOptions);
  };

  const loadWASMResponse = async function (wasm_source, compileOptions) {
    return loadWASM(true, wasm_source, compileOptions);
  };

  const loadWASMURL = async function (wasmURL) {
    const wasm_response = await loadURL(wasmURL);
    return await loadWASMResponse(wasm_response);
  };

  const getWASMScripts = function () {
    const instantiated = [];

    const wasmScripts = document.querySelectorAll(
      "script[type='application/wasm']",
    );

    for (let i = 0; i < wasmScripts.length; i++) {
      const script = wasmScripts[i];
      const src = script.src;
      instantiated.push({ url: src, script });
    }

    return instantiated;
  };

  // loadWASMScripts will load all script marked
  // with type=`application/wasm`
  // and execute main function.
  const loadAll = async function () {
    const instantiated = [];

    let scripts = getWASMScripts();
    for (let index in scripts) {
      let script = scripts[index];
      if (script.url) {
        let instance = await loadWASMURL(script.url);
        instantiated.push(instance);
      } else {
        console.error("Properly must have 'url' property.", script);
      }
    }

    return instantiated;
  };

  // runWASMScripts will load all script marked
  // with type=`application/wasm`
  // and execute main function.
  const runAll = async function () {
    const instances = await loadAll();
    instances.forEach(([env, runtime]) => {
      runtime.run_wasm();
    });
    return instances;
  };

  return {
    loadWASM,
    loadWASMURL,
    loadWASMBytes,
    getWASMScripts,
    runAll,
    loadAll,
  };
})();
