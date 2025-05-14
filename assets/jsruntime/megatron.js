class ArenaAllocator {
  static MAX_SIZE = BigInt(0xfffffff0);
  static BIT_MASK = BigInt(0xffffffff);
  static BIT_SIZE = BigInt(32);

  constructor() {
    this.items = [];
    this.free = [];
  }

  destroy(uid) {
    return this.remove_entry(uid);
  }

  create(item) {
    return this.add_entry(item);
  }

  create_slot(item, index, generation) {
    return { item, index, generation, active: true };
  }

  allocate_slot() {
    if (this.free.length > 0) {
      let available_slot_index = this.free.pop();
      let slot_entry = this.items[available_slot_index];
      if (slot_entry.active > 0) {
        throw Error("Active slot cant be in free list");
      }
      slot_entry.active = true;
      slot_entry.generation += 1;
      return slot_entry;
    }

    let slot = this.create_slot(null, this.items.length, 0);
    this.items.push(slot);
    return slot;
  }

  // return an active entry if entry.active is true else
  // return null;
  get_entry_at_index(index) {
    let entry = this.items[index];
    if (entry.active) return entry;
    return null;
  }

  get_index(index) {
    if (index >= this.items.length) {
      return null;
    }
    entry = this.items[index];
    return entry.item;
  }

  get_entry(uid) {
    let candidate = ArenaAllocator.uid_to_entry(uid);
    if (candidate.index >= this.items.length) {
      return null;
    }
    entry = get_entry_at_index(candidate.index);
    if (entry == null) return null;
    return entry.item;
  }

  add_entry(item) {
    let slot = this.allocate_slot();
    slot.item = item;
    slot.active = true;
    return ArenaAllocator.entry_to_uid(slot);
  }

  remove_entry(uid) {
    let candidate = ArenaAllocator.uid_to_entry(uid);
    if (candidate.index >= this.items.length) {
      return false;
    }
    let entry = this.items[candidate.index];
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

    this.free.push(entry);
    return true;
  }

  static create_entry(index, generation) {
    return {
      index,
      generation,
    };
  }

  static uid_to_entry(entry_as_u32) {
    // shifts the bit to the leftmost bit which means
    // we shift the value of the index stored in the MSB  to the LSB
    // and clear the MSB to 0000 and then Mask with 0xFFFF so we can
    // only get the values in the LSB
    let extracted_index =
      (BigInt(entry_as_u32) >> ArenaAllocator.BIT_SIZE) &
      ArenaAllocator.BIT_MASK; // using <<< to instruct js we want an unsigned 16 bit number;
    let extracted_generation = BigInt(entry_as_u32) & ArenaAllocator.BIT_MASK; // mask the LSB which is the generation 16 bit and extract the number.

    return ArenaAllocator.create_entry(extracted_index, extracted_generation);
  }

  // [entry_to_uid] generates a unsigned 32 bit integer representing
  // a given entry which basically just uses packing to create a singular
  // number that contains both so we can reduce storage size.
  static entry_to_uid(entry) {
    const index_as_16bit = BigInt(entry.index) & ArenaAllocator.BIT_MASK; // Max to ensure we have 65,535 bits (16bit numbers)
    const generation_as_16bit =
      BigInt(entry.generation) & ArenaAllocator.BIT_MASK; // Max to ensure we have 65,535 bits (16bit numbers)

    // shift the index to the MSB(Most Significant Bits, starting from the leftmost bit).
    // others say: shifting to the upper bits (so leftmost)
    // Then: shift the generation uid to the rightmost bit (Least Significant bit).
    // others say: shifting to the lower bits (so rightmost)
    let packed_uid =
      (BigInt(index_as_16bit) << ArenaAllocator.BIT_SIZE) | generation_as_16bit;
    return packed_uid;
  }
}

/// [`DOMArena`] provides a special arena for use specifically to interact with
//  both DOM nodes and non-dom elements, generally I would expect this to really
//  be used for interacting with the DOM APIs.
//
//  And it will be enhanced in the future to have methods specifically suited for such
//  needs.
class DOMArena extends ArenaAllocator {
  static SECURE_INSTANCE_OFFSET = 7n;

  constructor() {
    super();

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
    this.create(undefined);
    this.create(null);
    this.create(self);
    this.create(typeof window != "undefined" ? window : null);
    this.create(typeof document != "undefined" ? document : null);
    this.create(
      typeof document != "undefined" && document && document.body
        ? document.body
        : null,
    );
    this.create(false);
    this.create(true);
  }

  destroy(uid) {
    let candidate = ArenaAllocator.uid_to_entry(uid);
    if (candidate.index < ArenaAllocator.SECURE_INSTANCE_OFFSET) {
      return false;
    }
    return super.destroy(uid);
  }
}

class MemoryOperator {
  constructor(wasm_module) {
    this.module = wasm_module;
  }

  get_module() {
    return this.module;
  }

  get_memory() {
    return this.module.instance.exports.memory.buffer;
  }

  allocate_memory(size) {
    // create an allocation within the wasm instance by
    // calling its create_allocation exported function
    // as we expect.
    const wasmInstance = this.module;
    const allocation_id = wasmInstance.exports.create_allocation(size);
    const allocation_start_pointer =
      wasmInstance.exports.allocation_start_pointer(allocation_id);

    return [allocation_id, allocation_start_pointer];
  }

  readUint8Array(start_pointer, len) {
    const memory = this.get_memory();
    const slice = memory.slice(start_pointer, start_pointer + len);
    return new Uint8Array(slice);
  }

  writeUint8Buffer(uint8_buffer) {
    const len = uint8_buffer.length;
    const [id, start] = this.allocate_memory(len);

    const memory = this.get_memory();
    memory.set(uint8_buffer, start);

    return id;
  }

  writeUint8Array(array_buffer) {
    const uint8_buffer = new Uint8Array(array_buffer);
    return this.writeUint8Buffer(uint8_buffer);
  }
}

class TextCodec {
  constructor(memory_operator) {
    console.log(
      memory_operator instanceof MemoryOperator,
      "memory_operator must be MemoryOperator",
    );
    this.operator = memory_operator;
    this.utf8_encoder = new TextEncoder();
    this.utf8_decoder = new TextDecoder("utf-8");
    this.utf16_decoder = new TextDecoder("utf-16");
  }

  readUTF8FromMemory(start, len) {
    const memory = this.operator.get_memory();
    const data_slice = memory.slice(start, start + len);
    return utf8_decoder.decode(data_slice);
  }

  writeUTF8FromMemory(text) {
    const bytes = utf8_encoder.encode(text);
    const len = bytes.length;
    const [id, start] = this.operator.allocate_memory(len);

    const memory = this.operator.get_memory();
    memory.set(bytes, start);
    return id;
  }

  readUTF16FromMemory(start, len) {
    const bytes = this.operator.get_memory().subarray(start, start + len);
    const text = utf16_decoder.decode(bytes);
    return text;
  }

  // [utf8ArrayToStr] convets your array to UTF-16 using the decodeURIComponent.
  //
  // Taking from: https://stackoverflow.com/a/42241418/1294175
  static utf8ArrayToStr(array) {
    for (var i = 0, s = ""; i < array.length; i++) {
      var h = array[i].toString(16);
      if (h.length < 2) h = "0" + h;
      s += "%" + h;
    }
    return decodeURIComponent(s);
  }

  // [uintArrayToStr] uses the [`decodeURIComponent`] to convert
  // your uint8 array into a UTF-16 string.
  static uintArrayToStr(uintArray) {
    var encodedString = String.fromCharCode.apply(null, uintArray),
      decodedString = decodeURIComponent(escape(encodedString));
    return decodedString;
  }

  // [strToUtf8Array] convets your string to UTF-8 array.
  //
  // Taking from: https://stackoverflow.com/a/42241418/1294175
  static strToUtf8Array(s) {
    for (var i = 0, enc = encodeURIComponent(s), a = []; i < enc.length; ) {
      if (enc[i] === "%") {
        a.push(parseInt(enc.substr(i + 1, 2), 16));
        i += 3;
      } else {
        a.push(enc.charCodeAt(i++));
      }
    }
    return a;
  }

  // [utf16BinaryStringToUTF16String] converts a utf16 encoded binary string
  // into actual UTF-16 string usable in the web.
  //
  // You probably have seen this "agreement" before: it is the String's character
  // encoding (and the usual "agreement terms" are, for example,
  // Unicode UTF-16 and iso8859-1).
  //
  // See https://stackoverflow.com/a/58510156/1294175
  static utf16BinaryStringToUTF16String(binary_str) {
    return String.fromCharCode.apply(null, new Uint8Array(binary_str));
  }

  // [arrayBufferToStr] lets you supply a plain ArrayBuffer object which
  // contains UTF-16 encoded bytes.
  //
  // You probably have seen this "agreement" before: it is the String's character
  // encoding (and the usual "agreement terms" are, for example,
  // Unicode UTF-16 and iso8859-1).
  //
  //
  // Buffer can be Uinit8/Uint32 depending upon your buffer value type.
  //
  // But the bytes must be in UTF-16 encoding already.
  //
  // See https://stackoverflow.com/a/65540824/1294175
  static arrayBufferToStr(buf) {
    return String.fromCharCode.apply(null, new Uint16Array(buf));
  }

  // See https://stackoverflow.com/a/65540824/1294175
  static string_to_array_buffer(str) {
    var buf = new ArrayBuffer(str.length * 2); // 2 bytes for each char
    var bufView = new Uint16Array(buf);
    for (var i = 0, strLen = str.length; i < strLen; i++) {
      bufView[i] = str.charCodeAt(i);
    }
    return buf;
  }

  // see https://stackoverflow.com/a/44614927/1294175
  static unicodeToString(strBytes) {
    var MAX_SIZE = 0x4000;
    var codeUnits = [];
    var highSurrogate;
    var lowSurrogate;
    var index = -1;

    var result = "";

    while (++index < strBytes.length) {
      var codePoint = Number(strBytes[index]);

      if (codePoint === (codePoint & 0x7f)) {
      } else if (0xf0 === (codePoint & 0xf0)) {
        codePoint ^= 0xf0;
        codePoint = (codePoint << 6) | (strBytes[++index] ^ 0x80);
        codePoint = (codePoint << 6) | (strBytes[++index] ^ 0x80);
        codePoint = (codePoint << 6) | (strBytes[++index] ^ 0x80);
      } else if (0xe0 === (codePoint & 0xe0)) {
        codePoint ^= 0xe0;
        codePoint = (codePoint << 6) | (strBytes[++index] ^ 0x80);
        codePoint = (codePoint << 6) | (strBytes[++index] ^ 0x80);
      } else if (0xc0 === (codePoint & 0xc0)) {
        codePoint ^= 0xc0;
        codePoint = (codePoint << 6) | (strBytes[++index] ^ 0x80);
      }

      if (
        !isFinite(codePoint) ||
        codePoint < 0 ||
        codePoint > 0x10ffff ||
        Math.floor(codePoint) != codePoint
      )
        throw RangeError("Invalid code point: " + codePoint);

      if (codePoint <= 0xffff) codeUnits.push(codePoint);
      else {
        codePoint -= 0x10000;
        highSurrogate = (codePoint >> 10) | 0xd800;
        lowSurrogate = codePoint % 0x400 | 0xdc00;
        codeUnits.push(highSurrogate, lowSurrogate);
      }
      if (index + 1 == strBytes.length || codeUnits.length > MAX_SIZE) {
        result += String.fromCharCode.apply(null, codeUnits);
        codeUnits.length = 0;
      }
    }

    return result;
  }

  // See https://stackoverflow.com/a/38858004/1294175
  static arrayBufferToBase64(buffer) {
    var binary = "";
    var bytes = new Uint8Array(buffer);
    var len = bytes.byteLength;
    for (var i = 0; i < len; i++) {
      binary += String.fromCharCode(bytes[i]);
    }
    return window.btoa(binary);
  }

  // See https://stackoverflow.com/a/38858004/1294175
  static base64ToArrayBuffer(base64) {
    var binary_string = window.atob(base64);
    var len = binary_string.length;
    var bytes = new Uint8Array(len);
    for (var i = 0; i < len; i++) {
      bytes[i] = binary_string.charCodeAt(i);
    }
    return bytes.buffer;
  }

  static utf8ArrayToString(u8Array, idx, maxBytesToRead) {
    return TextCodec.emscriptenUTF8ArrayToString(
      u8Array,
      idx,
      maxBytesToRead,
      true,
    );
  }

  // See https://github.com/emscripten-core/emscripten/blob/main/test/js_optimizer/applyImportAndExportNameChanges2-output.js#L30-L64
  static emscriptenUTF8ArrayToString(
    u8Array,
    idx,
    maxBytesToRead,
    skipTextDecoder,
  ) {
    var endIdx = idx + maxBytesToRead;
    var endPtr = idx;
    while (u8Array[endPtr] && !(endPtr >= endIdx)) ++endPtr;

    var UTF8Decoder =
      typeof TextDecoder != "undefined" ? new TextDecoder("utf8") : undefined;
    if (
      skipTextDecoder ||
      (endPtr - idx > 16 && u8Array.subarray && UTF8Decoder)
    ) {
      return UTF8Decoder.decode(u8Array.subarray(idx, endPtr));
    } else {
      var str = "";
      while (idx < endPtr) {
        var u0 = u8Array[idx++];
        if (!(u0 & 128)) {
          str += String.fromCharCode(u0);
          continue;
        }
        var u1 = u8Array[idx++] & 63;
        if ((u0 & 224) == 192) {
          str += String.fromCharCode(((u0 & 31) << 6) | u1);
          continue;
        }
        var u2 = u8Array[idx++] & 63;
        if ((u0 & 240) == 224) {
          u0 = ((u0 & 15) << 12) | (u1 << 6) | u2;
        } else {
          u0 =
            ((u0 & 7) << 18) | (u1 << 12) | (u2 << 6) | (u8Array[idx++] & 63);
        }
        if (u0 < 65536) {
          str += String.fromCharCode(u0);
        } else {
          var ch = u0 - 65536;
          str += String.fromCharCode(55296 | (ch >> 10), 56320 | (ch & 1023));
        }
      }
    }
    return str;
  }

  // [legacyUtf16to8] is a legacy conversion code that mostly works with
  // well formed utf16 arrays but also good to have to see other ways
  // to perform conversion
  //
  // See http://www.onicos.com/staff/iz/amuse/javascript/expert/utf.txt
  static legacyUtf16to8(str) {
    var out, i, len, c;

    out = "";
    len = str.length;
    for (i = 0; i < len; i++) {
      c = str.charCodeAt(i);
      if (c >= 0x0001 && c <= 0x007f) {
        out += str.charAt(i);
      } else if (c > 0x07ff) {
        out += String.fromCharCode(0xe0 | ((c >> 12) & 0x0f));
        out += String.fromCharCode(0x80 | ((c >> 6) & 0x3f));
        out += String.fromCharCode(0x80 | ((c >> 0) & 0x3f));
      } else {
        out += String.fromCharCode(0xc0 | ((c >> 6) & 0x1f));
        out += String.fromCharCode(0x80 | ((c >> 0) & 0x3f));
      }
    }
    return out;
  }

  // [legacyUtf8to16] is a legacy conversion code that mostly works with
  // well formed utf8 arrays but also good to have to see other ways
  // to perform conversion
  //
  // See http://www.onicos.com/staff/iz/amuse/javascript/expert/utf.txt
  static legacyUtf8to16(str) {
    var out, i, len, c;
    var char2, char3;

    out = "";
    len = str.length;
    i = 0;
    while (i < len) {
      c = str.charCodeAt(i++);
      switch (c >> 4) {
        case 0:
        case 1:
        case 2:
        case 3:
        case 4:
        case 5:
        case 6:
        case 7:
          // 0xxxxxxx
          out += str.charAt(i - 1);
          break;
        case 12:
        case 13:
          // 110x xxxx   10xx xxxx
          char2 = str.charCodeAt(i++);
          out += String.fromCharCode(((c & 0x1f) << 6) | (char2 & 0x3f));
          break;
        case 14:
          // 1110 xxxx  10xx xxxx  10xx xxxx
          char2 = str.charCodeAt(i++);
          char3 = str.charCodeAt(i++);
          out += String.fromCharCode(
            ((c & 0x0f) << 12) | ((char2 & 0x3f) << 6) | ((char3 & 0x3f) << 0),
          );
          break;
      }
    }

    return out;
  }
}

// Javascript 32 bits is 4 bytes (as 1 byte = 8 bits), so each memory location
// is 32 bits and when moving index we move by 32 bits each.
const MOVE_BY_32_BITS = 4;

// Javascript is 32 bit memory, 4 bytes makes 32bit, to move 64bits then
// its 4 bytes x 2 = 8 bytes.
const MOVE_BY_64_BITS = 8;

class ParameterParserV1 {
  constructor(wasm_module) {
    this.module = wasm_module;
    this.operator = MemoryOperator(this.module);
    this.texts = TextCodec(this.operator);

    this.parsers = {
      0: this.parseUndefined,
      1: this.parseNull,
      2: this.parseFloat64,
      3: this.parseBigInt64,
      4: this.parseString,
      5: this.parseExternalReference,
      6: this.parseFloat32,
      7: this.parseTrue,
      8: this.parseFalse,
      9: this.parseFloat64Array,
      10: this.parseUint32Array,
    };
  }

  get_parser(parameter_type_id) {
    const parser = this.parsers[parameter_type_id];
    if (parser in [undefined, null]) {
      throw new Error(
        "Invalid parameter_type id provided: ",
        parameter_type_id,
      );
    }
    return parser;
  }

  parse_array(start, length) {
    const parameter_buffer = this.operator.readUint8Array(start, length);
    console.debug("read_parameters: ", parameters, start, length);

    const converted_values = [];
    while (index < parameter_buffer.length) {
      index += 1;

      const parameter_type = parameter_buffer[index];
      const parser = this.get_parser(parameter_type);

      const [move_index_by, should_break] = parser(
        index,
        converted_values,
        parameter_buffer,
      );

      index += move_index_by;
      if (should_break) break;
    }

    console.debug("Read parameters: ", converted_values);
    return converted_values;
  }

  parseUndefined(index, read_values_list, parameter_buffer) {
    read_values_list.push(undefined);
    return [0, false];
  }

  parseNull(index, read_values_list, parameter_buffer) {
    read_values_list.push(null);
    return [0, false];
  }

  parseFloat64(index, read_values_list, parameter_buffer) {
    const view = new DataView(parameter_buffer.buffer).getFloat64(index, true);
    read_values_list.push(view);
    return [MOVE_INDEX_BY_64BIT, false];
  }

  parseBigInt64(index, read_values_list, parameter_buffer) {
    const view = new DataView(parameter_buffer.buffer).getBigInt64(index, true);
    read_values_list.push(view);
    return [MOVE_INDEX_BY_64BIT, false];
  }

  parseString(index, read_values_list, parameter_buffer) {
    // 4 = string (followed by 32-bit start and size of string in wasm memory)
    // 4 means we want to read a int32 memory size where we have 4 bytes for start, 4 bytes for length which
    // indicate the memory range we need to read;
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

    const data = this.texts.readUTF8FromMemory(start, length);
    read_values_list.push(data);
    return [start_index - index, false];
  }

  parseExternalReference(index, read_values_list, parameter_buffer) {
    // 5 = extern ref
    const handle_uid = new DataView(parameter_buffer.buffer).getBigInt64(
      index,
      true,
    );
    read_values_list.push(ARENA.get(handle_uid));
    return [MOVE_INDEX_BY_64BIT, false];
  }

  parseFloat32(index, read_values_list, parameter_buffer) {
    // 6 = array of Float32 from wasm memory (followed by 32-bit start and size of string in memory)
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
  }

  parseTrue(index, read_values_list, parameter_buffer) {
    // 7 = true
    read_values_list.push(true);
    return [0, false];
  }

  parseFalse(index, read_values_list, parameter_buffer) {
    // 8 = false
    read_values_list.push(false);
    return [0, false];
  }

  parseFloat64Array(index, read_values_list, parameter_buffer) {
    // 9 = array of Float64 from wasm memory (followed by 32-bit start and size of string in memory)
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
  }

  parseUint32Array(index, read_values_list, parameter_buffer) {
    // 10 = array of Uint32 from wasm memory (followed by 32-bit start and size of string in memory)
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
  }
}

class WASMLoader {
  // [scriptsToWasmLoader] will load all script marked with type=`application/wasm`
  // and create a WASMLoader instance which will manage the given script.
  static async scriptsToWasmLoader(
    initial_memory,
    maximum_memory,
    environment,
    compileOptions,
  ) {
    const instantiated = [];

    const scripts = getWASMScripts();
    for (let index in scripts) {
      let script = scripts[index];
      if (script.url) {
        const loader = WASMLoader(
          initial_memory,
          maximum_memory,
          environment,
          compileOptions,
        );

        loader.module = await loader.loadURL(script.url);
        instantiated.push(instance);
      } else {
        console.error("Properly must have 'url' property.", script);
      }
    }

    return instantiated;
  }

  constructor(initial_memory, maximum_memory, environment, compileOptions) {
    if (!environment) environment = {};
    if (!initial_memory) initial_memory = 10;
    if (!maximum_memory) maximum_memory = 200;

    this.module = None;
    this.initial_memory = initial_memory;
    this.maximum_memory = maximum_memory;
    this.compiled_options = compileOptions;
    this.memory = new WebAssembly.Memory({
      initial: initial_memory,
      maximum: maximum_memory,
    });

    this.env = {
      data: {},
      refs: {},
      funcs: {},
      ...environment,
    };
  }

  run() {
    if (!this.module) throw new Error("No wasm module loaded");
    if (!this.module.instance.exports.main)
      throw new Error("wasm module has no exported main function");
    return this.module.instance.exports.main();
  }

  _setup_module(module) {
    if (!(module instanceof WebAssembly.Instance)) {
      throw new Error("Module must be an instance of WebAssembly.Instance");
    }
    this.module = module;
    this.parser_v1 = ParameterParserV1(this.module);
  }

  async loadURL(wasm_url) {
    this._setup_module(
      await WASMLoader.loadWASMURL(
        wasm_url,
        this.memory,
        this.env,
        this.compiled_options,
      ),
    );
  }

  async loadBytes(wasm_bytes) {
    this._setup_module(
      await WASMLoader.loadWASMBytes(
        wasm_bytes,
        this.memory,
        this.env,
        this.compiled_options,
      ),
    );
  }

  static async loadWASMBytes(wasm_bytes, memory, environment, compileOptions) {
    return await WASMLoader.loadWASMModule(
      false,
      wasm_bytes,
      compileOptions,
      memory,
      environment,
    );
  }

  static async loadWASMURL(wasmURL, memory, environment, compileOptions) {
    const wasm_response = await WASMLoader.loadURL(wasmURL);
    return await WASMLoader.loadWASMModule(
      true,
      wasm_response,
      compileOptions,
      memory,
      environment,
    );
  }

  static async loadURL(wasmURL) {
    return await fetch(wasmURL);
  }

  static async loadWASMModule(
    streaming,
    wasm_source,
    compileOptions,
    env,
    memory,
  ) {
    const compiled_options = WASMLoader.configCompiledOptions(compileOptions);

    let module;
    if (streaming) {
      module = await WebAssembly.instantiateStreaming(
        wasm_source,
        {
          ...env,
          js: { mem: memory },
        },
        compiled_options,
      );
    } else {
      module = await WebAssembly.instantiate(
        wasm_source,
        {
          ...this.env,
          js: { mem: this.memory },
        },
        compiled_options,
      );
    }

    module.compiled_options = compiled_options;

    return module;
  }

  static configCompiledOptions(compileOptions) {
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
  }

  static getScriptsForApplicationWASM() {
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
  }
}

// WasmWebScripts owns the loading of wasm applications that are added
// to the current web page has <scripts> tags.
class WasmWebScripts {
  constructor(initial_memory, maximum_memory, environment, compileOptions) {
    this.modules = WASMLoader.scriptsToWasmLoader(
      initial_memory,
      maximum_memory,
      environment,
      compileOptions,
    );
  }

  // runAll will execute all loaded wasm module from already loaded
  // scripts with type=`application/wasm` and execute main function.
  async runAll() {
    this.modules.then((modules) => {
      modules.forEach((instance) => {
        instance.run();
      });
    });
  }
}

WasmWebScripts.default = function (env) {
  return new WasmWebScripts(10, 200, env, WASMLoader.configCompiledOptions({}));
};

// center up
