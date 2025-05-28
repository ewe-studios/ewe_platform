const NULL_AND_UNDEFINED = [null, undefined];

const LEVELS = {
  INFO: 1,
  ERROR: 2,
  WARNINGS: 3,
  DEBUG: 4,
};

const LOGGER = {
  mode: LEVELS.ERROR,
};

LOGGER.info = function () {
  console.log.apply(console, arguments);
};

LOGGER.trace = function () {
  console.trace.apply(console, arguments);
};

LOGGER.warning = function () {
  if (LOGGER.mode < LEVELS.WARNINGS) return;
  console.warn.apply(console, arguments);
};

LOGGER.error = function () {
  if (LOGGER.mode < LEVELS.ERROR) return;
  console.error.apply(console, arguments);
};

LOGGER.debug = function () {
  if (LOGGER.mode < LEVELS.DEBUG) return;
  console.debug.apply(console, arguments);
};

const Move = {
  // Move the index by 1 8-bits movement - a DataView represent 1 move: 8 bits (1 byte)
  // 8 bits is 1 Bytes, so we move by 1.
  MOVE_BY_1_BYTES: 1,

  // Move the index by 2 8-bits movement - a DataView represent 1 move: 8 bits (1 byte)
  // 16 bits is 2 Bytes, so we move by 2.
  MOVE_BY_16_BYTES: 2,

  // Move the index by 4 8-bits movement - a DataView represent 1 move: 8 bits (1 byte)
  // 32 bits is 4 Bytes, so we move by 4.
  MOVE_BY_32_BYTES: 4,

  // Move the index by 8 8-bits movement - a DataView represent 1 move: 8 bits (1 byte)
  // 64 bits is 8 Bytes, so we move by 8.
  MOVE_BY_64_BYTES: 8,

  // Move the index by 16 8-bits movement - a DataView represent 1 move: 8 bits (1 byte)
  // 128 bits is 16 Bytes, so we move by 16.
  MOVE_BY_128_BYTES: 16,
};

const UtfEncoding = {
  // Represents as 16 to indicate encoding is UTF-8.
  UTF8: 8,

  // Represents as 16 to indicate encoding is UTF-16.
  UTF16: 16,
};

UtfEncoding.__INVERSE__ = {
  8: UtfEncoding.UTF8,
  16: UtfEncoding.UTF16,
};

const ALLOWED_UTF8_INDICATOR = [UtfEncoding.UTF8, UtfEncoding.UTF16];

const TypeOptimization = {
  None: 0,

  // optimize ints
  QuantizedInt16AsI8: 1,
  QuantizedInt32AsI8: 2,
  QuantizedInt32AsI16: 3,
  QuantizedInt64AsI8: 4,
  QuantizedInt64AsI16: 5,
  QuantizedInt64AsI32: 6,

  // optimize uints
  QuantizedUint16AsU8: 7,
  QuantizedUint32AsU8: 8,
  QuantizedUint32AsU16: 9,
  QuantizedUint64AsU8: 10,
  QuantizedUint64AsU16: 11,
  QuantizedUint64AsU32: 12,

  // optimize floats
  QuantizedF64AsF32: 13,
  QuantizedF128AsF32: 14,
  QuantizedF128AsF64: 15,

  // optimize i128 bits
  QuantizedInt128AsI8: 16,
  QuantizedInt128AsI16: 17,
  QuantizedInt128AsI32: 18,
  QuantizedInt128AsI64: 19,

  // optimize u128 bits
  QuantizedUint128AsU8: 20,
  QuantizedUint128AsU16: 21,
  QuantizedUint128AsU32: 22,
  QuantizedUint128AsU64: 23,

  // optimize pointers bits
  QuantizedPtrAsU8: 24,
  QuantizedPtrAsU16: 25,
  QuantizedPtrAsU32: 26,
  QuantizedPtrAsU64: 27,
};

TypeOptimization.__INVERSE__ = Object.keys(TypeOptimization)
  .map((key) => {
    return [key, TypeOptimization[key]];
  })
  .reduce((prev, current) => {
    let [key, value] = current;
    prev[value] = key;
    return prev;
  }, {});

const Operations = {
  /// Begin - Indicative of the start of a operation in a batch, generally
  /// you should only ever see this once until the batch ends with a [`Operations::Stop`].
  /// After the begin is seen, you should see other operations indicative of what the
  /// sub-operation in the batch represent and it's specific layout.
  ///
  /// Memory wise: This is 1 Byte: 8 bits.
  ///
  Begin: 0,

  /// MakeFunction represents the operation to create/register
  /// a function on the other side at a specific ExternalReference
  /// usually pre-allocated via some API call.
  ///
  /// The layout will have the function address followed by the
  /// binary representation of the location of the function
  /// definition in batch memory, the starting index of the
  /// string and the length of the string bytes.
  ///
  /// Layout: {1{ [MemoryAllocationAddress}, PreAllocatedExternalReference, StartIndex, Length}
  ///
  /// In Actual Layout:
  ///
  /// Memory Layout: {
  ///     1 Byte / 8 Bits for Operations type,
  ///     4 Bytes for Memory Address for Location,
  ///     8 Bytes for External Reference that is 64bit long,
  ///     4 Bytes for Start Index,
  ///     4 bytes for Length,
  /// }
  ///
  /// All together its: 21 Bytes: 168 bits Long.
  ///
  /// Adding the Begin (1 Byte) and Stop (1 Byte) bytes then we have additional 2 bytes: 16 bits
  ///
  /// So in total we will have 23 Bytes: 184 bits long.
  ///
  ///
  MakeFunction: 1,

  /// InvokeNoReturnFunction represents the desire to call a
  /// function across boundary that does not return any value
  /// in response to being called.
  ///
  /// It has two layout formats:
  ///
  /// A. with no argument: Begin, 3, FunctionHandle(u64), End
  ///
  /// B. with arguments: Begin, 3, FunctionHandle(u64), FunctionArguments, {Arguments}, End
  InvokeNoReturnFunction: 2,

  /// InvokeReturningFunction represents the desire to call a
  /// function across boundary that returns a value of
  /// defined type matching [`ReturnType`]
  /// in response to being called.
  ///
  /// It has two layout formats:
  ///
  /// A. with no argument: Begin, 3, FunctionHandle(u64), ReturnType, End
  ///
  /// B. with arguments: Begin, 3, FunctionHandle(u64), ReturnType, Arguments*, End
  InvokeReturningFunction: 3,

  /// InvokeCallbackFunction represents the desire to call a
  /// function across boundary that takes a callback external reference
  /// which it will use to supply appropriate response when ready (say async call)
  /// as response to being called.
  ///
  /// Layout format: Begin, 3, FunctionHandle(u64), ArgStart, ArgBegin, ExternReference, ArgEnd, ArgStop,
  ///  End
  InvokeCallbackFunction: 4,

  /// Stop - indicates the end of an operation in a batch, since
  /// a memory will contain multiple operations batched into a single
  /// memory slot, until you see this 1 byte signal then you should
  /// consider that batch yet to finish.
  ///
  /// Memory wise: This is 1 Byte: 8 bits.
  ///
  Stop: 255,
};

Operations.__INVERSE__ = Object.keys(Operations)
  .map((key) => {
    return [key, Operations[key]];
  })
  .reduce((prev, current) => {
    let [key, value] = current;
    prev[value] = key;
    return prev;
  }, {});

const ArgumentOperations = {
  Start: 1,
  Begin: 2,
  End: 3,
  Stop: 4,
};

ArgumentOperations.__INVERSE__ = Object.keys(ArgumentOperations)
  .map((key) => {
    return [key, ArgumentOperations[key]];
  })
  .reduce((prev, current) => {
    let [key, value] = current;
    prev[value] = key;
    return prev;
  }, {});

const Params = {
  Null: 0,
  Undefined: 1,
  Bool: 2,
  Text8: 3,
  Text16: 4,
  Int8: 5,
  Int16: 6,
  Int32: 7,
  Int64: 8,
  Uint8: 9,
  Uint16: 10,
  Uint32: 11,
  Uint64: 12,
  Float32: 13,
  Float64: 14,
  ExternalReference: 15,
  Uint8ArrayBuffer: 16,
  Uint16ArrayBuffer: 17,
  Uint32ArrayBuffer: 18,
  Uint64ArrayBuffer: 19,
  Int8ArrayBuffer: 20,
  Int16ArrayBuffer: 21,
  Int32ArrayBuffer: 22,
  Int64ArrayBuffer: 23,
  Float32ArrayBuffer: 24,
  Float64ArrayBuffer: 25,
  InternalReference: 26,
  Int128: 27,
  Uint128: 28,
};

Params.__INVERSE__ = Object.keys(Params)
  .map((key) => {
    return [key, Params[key]];
  })
  .reduce((prev, current) => {
    let [key, value] = current;
    prev[value] = key;
    return prev;
  }, {});

function isUndefinedOrNull(value) {
  return NULL_AND_UNDEFINED.indexOf(value) != -1;
}

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

  get(item) {
    return this.get_entry(item);
  }

  update(uid, item) {
    return this.update_entry(uid, item);
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
    LOGGER.debug(`Seeking for entry with id: ${uid} with slot: `, candidate);
    const entry = this.get_entry_at_index(candidate.index);
    if (entry == null) return null;
    return entry.item;
  }

  update_entry(slot_id, item) {
    let candidate = ArenaAllocator.uid_to_entry(slot_id);
    if (candidate.index >= this.items.length) {
      return false;
    }
    slot.item = item;
    slot.active = true;
    return true;
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
    const entry = this.items[candidate.index];
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
  //
  // Returns a BigInt representing the entry.
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
  static SECURE_INSTANCE_OFFSET = 5n;

  constructor() {
    super();

    // add entries for base JS types at the top,
    // and allocate 1-4 for use
    // always for these.
    //
    // 0 is reserved for self
    // 1 is reserved for this (the dom arena)
    // 2 is reserved for window
    // 3 is reserved for document
    // 4 is reserved for document.body
    //
    this.create(self);
    this.create(this);
    this.create(typeof window != "undefined" ? window : null);
    this.create(typeof document != "undefined" ? document : null);
    this.create(
      typeof document != "undefined" && document && document.body
        ? document.body
        : null,
    );
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

  readUint8Buffer(start_pointer, len) {
    const memory = this.get_memory();
    return memory.slice(start_pointer, start_pointer + len);
  }

  readUint8Array(start_pointer, len) {
    return new Uint8Array(this.readUint8Buffer(start_pointer, len));
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
    this.operator = memory_operator;
    this.utf8_encoder = new TextEncoder();
    this.utf8_decoder = new TextDecoder("utf-8");
    this.utf16_decoder = new TextDecoder("utf-16");
  }

  readUTF8FromMemory(start, len) {
    const memory = this.operator.get_memory();
    const data_slice = memory.slice(start, start + len);
    return this.utf8_decoder.decode(data_slice);
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
    const text = this.utf16_decoder.decode(bytes);
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

class ParameterParserV1 {
  constructor(memory_operator, text_codec) {
    if (!(memory_operator instanceof MemoryOperator)) {
      throw new Error("Must be instance of MemoryOperator");
    }
    if (!(text_codec instanceof TextCodec)) {
      throw new Error("Must be instance of TextCodec");
    }

    this.texts = text_codec;
    this.operator = memory_operator;
    this.module = memory_operator.get_module();

    this.parsers = {
      0: this.parseUndefined.bind(this),
      1: this.parseNull.bind(this),
      2: this.parseFloat64.bind(this),
      3: this.parseBigInt64.bind(this),
      4: this.parseString.bind(this),
      5: this.parseExternalReference.bind(this),
      6: this.parseFloat32.bind(this),
      7: this.parseTrue.bind(this),
      8: this.parseFalse.bind(this),
      9: this.parseFloat64Array.bind(this),
      10: this.parseUint32Array.bind(this),
    };
  }

  parse_array(start, length) {
    const parameter_buffer = this.operator.readUint8Array(start, length);
    LOGGER.debug("parse_array:start ", start, length, parameter_buffer);

    const converted_values = [];
    let index = 0;
    while (index < parameter_buffer.length) {
      const parameter_type = parameter_buffer[index];

      // increment index since we read from table
      index += 1;

      const parser = this.get_parser(parameter_type);
      const [move_index_by, should_break] = parser(
        index,
        converted_values,
        parameter_buffer,
      );

      index += move_index_by;
      if (should_break) break;
    }

    LOGGER.debug("parse_array:end: ", converted_values);
    return converted_values;
  }

  get_parser(parameter_type_id) {
    const parser = this.parsers[parameter_type_id];
    if (isUndefinedOrNull(parser)) {
      throw new Error(
        "Invalid parameter_type id provided: ",
        parameter_type_id,
      );
    }
    return parser;
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
    return [Move.MOVE_BY_64_BYTES, false];
  }

  parseBigInt64(index, read_values_list, parameter_buffer) {
    const view = new DataView(parameter_buffer.buffer).getBigInt64(index, true);
    read_values_list.push(view);
    return [Move.MOVE_BY_64_BYTES, false];
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
    start_index += Move.MOVE_BY_32_BYTES;

    const length = new DataView(parameter_buffer.buffer).getInt32(
      start_index,
      true,
    );
    start_index += Move.MOVE_BY_32_BYTES;

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
    return [Move.MOVE_BY_64_BYTES, false];
  }

  parseFloat32(index, read_values_list, parameter_buffer) {
    // 6 = array of Float32 from wasm memory (followed by 32-bit start and size of string in memory)
    let start_index = index;
    const start = new DataView(parameter_buffer.buffer).getInt32(
      start_index,
      true,
    );
    start_index += Move.MOVE_BY_32_BYTES;

    const length = new DataView(parameter_buffer.buffer).getInt32(
      start_index,
      true,
    );
    start_index += Move.MOVE_BY_32_BYTES;

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
    start_index += Move.MOVE_BY_32_BYTES;

    const length = new DataView(parameter_buffer).getInt32(start_index, true);
    start_index += Move.MOVE_BY_32_BYTES;

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
    start_index += Move.MOVE_BY_32_BYTES;

    const length = new DataView(parameter_buffer).getInt32(start_index, true);
    start_index += Move.MOVE_BY_32_BYTES;

    const memory = memory_ops.get_memory();
    const slice = memory.buffer.slice(start, start + length * 4);
    const array = new Uint32Array(slice);

    read_values_list.push(array);
    return [start_index - index, false];
  }
}

class ParameterParserV2 {
  constructor(memory_operator, text_codec) {
    if (!(memory_operator instanceof MemoryOperator)) {
      throw new Error("Must be instance of MemoryOperator");
    }
    if (!(text_codec instanceof TextCodec)) {
      throw new Error("Must be instance of TextCodec");
    }

    this.texts = text_codec;
    this.operator = memory_operator;
    this.module = memory_operator.get_module();
  }

  parse_array(view) {
    if (!(view instanceof DataView)) {
      throw new Error(
        "Argument must be a DataView scoped to the area you want parsed",
      );
    }

    const parameters = [];

    let read_index = 0;

    while (index < view.length) {
      // validate we see begin marker
      if (view.getUint8(read_index) != ArgumentOperations.Begin) {
        throw new Error("Argument did not start with ArgumentOperation.Start");
      }
      read_index += Move.MOVE_BY_1_BYTES;

      const value_type = view.getUint8(read_index);

      let move_by = 0;
      let parameter = null;
      switch (value_type) {
        case Params.Undefined:
          [move_by, parameter] = this.parseUndefined(read_index, view);
          break;
        case Params.Null:
          [move_by, parameter] = this.parseNull(read_index, view);
          break;
        default:
          throw new Error(`ArgumentType with key: ${value_type} not supported`);
      }

      LOGGER.debug("Read parameter type: ", value_type, move_by, parameter);
      parameters.push(parameter);
      read_index += move_by;

      // validate we see end marker
      if (view.getUint8(read_index) != ArgumentOperations.End) {
        throw new Error("Argument did not start with ArgumentOperation.Start");
      }
      read_index += Move.MOVE_BY_1_BYTES;
    }
  }

  parseNull(from_index, view) {
    const value_type = view.getUint8(from_index);
    if (value_type != Params.Undefined) {
      throw new Error(
        `Parameter is not that of Undefined: received ${value_type}`,
      );
    }

    return [Move.MOVE_BY_1_BYTES, [null]];
  }

  parseUndefined(from_index, view) {
    const value_type = view.getUint8(from_index);
    if (value_type != Params.Undefined) {
      throw new Error(
        `Parameter is not that of Undefined: received ${value_type}`,
      );
    }

    return [Move.MOVE_BY_1_BYTES, [undefined]];
  }

  parseExternalPointer(from_index, view) {
    const value_type = view.getUint8(from_index);
    if (value_type != Params.ExternalReference) {
      throw new Error(
        `Parameter is not that of ExternalReference: received ${value_type}`,
      );
    }

    from_index += Move.MOVE_BY_1_BYTES;

    return [from_index, [undefined]];
  }

  parseParam(from_index, view) {
    const value_type = view.getUint8(from_index);
    if (!(value_type in Params.__INVERSE__)) {
      throw new Error(`Params ${value_type} is not known`);
    }

    from_index += Move.MOVE_BY_1_BYTES;

    const optimization_type = view.getUint8(from_index);
    if (!(optimization_type in TypeOptimization.__INVERSE__)) {
      throw new Error(
        `OptimizationType ${optimization_type} is not known for value_type: ${value_type}`,
      );
    }

    switch (value_type) {
      case Null:
        break;
      case Undefined:
        break;
      case Bool:
        break;
      case Text8:
        break;
      case Text16:
        break;
      case Int8:
        break;
      case Int16:
        break;
      case Int32:
        break;
      case Int64:
        break;
      case Uint8:
        break;
      case Uint16:
        break;
      case Uint32:
        break;
      case Uint64:
        break;
      case Float32:
        break;
      case Float64:
        break;
      case ExternalReference:
        break;
      case Uint8ArrayBuffer:
        break;
      case Uint16ArrayBuffer:
        break;
      case Uint32ArrayBuffer:
        break;
      case Uint64ArrayBuffer:
        break;
      case Int8ArrayBuffer:
        break;
      case Int16ArrayBuffer:
        break;
      case Int32ArrayBuffer:
        break;
      case Int64ArrayBuffer:
        break;
      case Float32ArrayBuffer:
        break;
      case Float64ArrayBuffer:
        break;
      case InternalReference:
        break;
      case Int128:
        break;
      case Uint129:
        break;
    }
  }

  parseTypeOptimizatedPtr(from_index, value_type, view) {
    const optimization_type = view.getUint8(from_index);
    if (!(optimization_type in TypeOptimization.__INVERSE__)) {
      throw new Error(
        `OptimizationType ${optimization_type} is not known for value_type: ${value_type}`,
      );
    }

    from_index += Move.MOVE_BY_1_BYTES;

    switch (optimization_type) {
      case TypeOptimization.None:
        return [
          from_index + Move.MOVE_BY_64_BYTES,
          view.getBigUint64(from_index),
        ];
      case TypeOptimization.QuantizedPtrAsU8:
        return [from_index + Move.MOVE_BY_1_BYTES, view.getUint8(from_index)];
      case TypeOptimization.QuantizedPtrAsU16:
        return [from_index + Move.MOVE_BY_16_BYTES, view.getUint16(from_index)];
      case TypeOptimization.QuantizedPtrAsU32:
        return [from_index + Move.MOVE_BY_32_BYTES, view.getUint32(from_index)];
      case TypeOptimization.QuantizedPtrAsU64:
        return [
          from_index + Move.MOVE_BY_64_BYTES,
          view.getBigUint64(from_index),
        ];
    }
  }

  parseTypeOptimizatedNumber16(from_index, value_type, view) {
    const optimization_type = view.getUint8(from_index);
    if (!(optimization_type in TypeOptimization.__INVERSE__)) {
      throw new Error(
        `OptimizationType ${optimization_type} is not known for value_type: ${value_type}`,
      );
    }

    from_index += Move.MOVE_BY_1_BYTES;

    switch (optimization_type) {
      case TypeOptimization.None:
        if (value_type == Params.Int16) {
          return [
            from_index + Move.MOVE_BY_16_BYTES,
            view.getInt16(from_index),
          ];
        }
        return [from_index + Move.MOVE_BY_16_BYTES, view.getUint16(from_index)];
      case TypeOptimization.QuantizedInt16AsI8:
        return [from_index + Move.MOVE_BY_1_BYTES, view.getInt8(from_index)];
      case TypeOptimization.QuantizedUint16AsU8:
        return [from_index + Move.MOVE_BY_1_BYTES, view.getUint8(from_index)];
    }
  }

  parseTypeOptimizatedNumber64(from_index, value_type, view) {
    const optimization_type = view.getUint8(from_index);
    if (!(optimization_type in TypeOptimization.__INVERSE__)) {
      throw new Error(
        `OptimizationType ${optimization_type} is not known for value_type: ${value_type}`,
      );
    }

    from_index += Move.MOVE_BY_1_BYTES;

    switch (optimization_type) {
      case TypeOptimization.None:
        if (value_type == Params.Int64) {
          return [
            from_index + Move.MOVE_BY_64_BYTES,
            view.getBigInt64(from_index),
          ];
        }
        return [
          from_index + Move.MOVE_BY_64_BYTES,
          view.getBigUint64(from_index),
        ];
      case TypeOptimization.QuantizedInt64AsI8:
        return [from_index + Move.MOVE_BY_1_BYTES, view.getInt8(from_index)];
      case TypeOptimization.QuantizedUint64AsU8:
        return [from_index + Move.MOVE_BY_1_BYTES, view.getUint8(from_index)];
      case TypeOptimization.QuantizedUint64AsU16:
        return [from_index + Move.MOVE_BY_16_BYTES, view.getUint16(from_index)];
      case TypeOptimization.QuantizedInt64AsI16:
        return [from_index + Move.MOVE_BY_16_BYTES, view.getInt16(from_index)];
      case TypeOptimization.QuantizedInt64AsI32:
        return [from_index + Move.MOVE_BY_32_BYTES, view.geInt32(from_index)];
      case TypeOptimization.QuantizedUint64AsU32:
        return [from_index + Move.MOVE_BY_32_BYTES, view.getUint32(from_index)];
    }
  }

  parseTypeOptimizatedNumber32(from_index, value_type, view) {
    const optimization_type = view.getUint8(from_index);
    if (!(optimization_type in TypeOptimization.__INVERSE__)) {
      throw new Error(
        `OptimizationType ${optimization_type} is not known for value_type: ${value_type}`,
      );
    }

    from_index += Move.MOVE_BY_1_BYTES;

    switch (optimization_type) {
      case TypeOptimization.None:
        if (value_type == Params.Int32) {
          return [
            from_index + Move.MOVE_BY_32_BYTES,
            view.getInt32(from_index),
          ];
        }
        return [from_index + Move.MOVE_BY_32_BYTES, view.getUint32(from_index)];
      case TypeOptimization.QuantizedInt32AsI8:
        return [from_index + Move.MOVE_BY_1_BYTES, view.getInt8(from_index)];
      case TypeOptimization.QuantizedUint32AsU8:
        return [from_index + Move.MOVE_BY_1_BYTES, view.getUint8(from_index)];
      case TypeOptimization.QuantizedInt32AsI16:
        return [from_index + Move.MOVE_BY_16_BYTES, view.getInt16(from_index)];
      case TypeOptimization.QuantizedUint32AsU16:
        return [from_index + Move.MOVE_BY_16_BYTES, view.getUint16(from_index)];
    }
  }

  parseTypeOptimizatedFloat32(from_index, value_type, view) {
    const optimization_type = view.getUint8(from_index);
    if (!(optimization_type == TypeOptimization.None)) {
      throw new Error(
        `OptimizationType ${optimization_type} is not known for value_type: ${value_type}`,
      );
    }

    from_index += Move.MOVE_BY_1_BYTES;
    return [from_index + Move.MOVE_BY_32_BYTES, view.getFloat32(from_index)];
  }

  parseTypeOptimizatedFloat64(from_index, value_type, view) {
    const optimization_type = view.getUint8(from_index);
    if (!(optimization_type in TypeOptimization.__INVERSE__)) {
      throw new Error(
        `OptimizationType ${optimization_type} is not known for value_type: ${value_type}`,
      );
    }

    from_index += Move.MOVE_BY_1_BYTES;

    switch (optimization_type) {
      case TypeOptimization.None:
        return [
          from_index + Move.MOVE_BY_64_BYTES,
          view.getFloat64(from_index),
        ];
      case TypeOptimization.QuantizedF64AsF32:
        return [
          from_index + Move.MOVE_BY_32_BYTES,
          view.getFloat32(from_index),
        ];
    }
  }

  parseTypeOptimizatedNumber128(from_index, value_type, view) {
    const optimization_type = view.getUint8(from_index);
    if (!(optimization_type in TypeOptimization.__INVERSE__)) {
      throw new Error(
        `OptimizationType ${optimization_type} is not known for value_type: ${value_type}`,
      );
    }

    from_index += Move.MOVE_BY_1_BYTES;

    switch (optimization_type) {
      case TypeOptimization.None:
        if (value_type == Params.Int128) {
          const value_msb = view.getInt64(from_index);
          from_index += Move.MOVE_BY_64_BYTES;

          const value_lsb = view.getInt64(from_index);
          from_index += Move.MOVE_BY_64_BYTES;

          let sent_value = BigInt(value_msb) << BigInt(64);
          sent_value = sent_value | BigInt(value_lsb);

          return [from_index, sent_value];
        }

        const value_msb = view.getUint64(from_index);
        from_index += Move.MOVE_BY_64_BYTES;

        const value_lsb = view.getUint64(from_index);
        from_index += Move.MOVE_BY_64_BYTES;

        let sent_value = BigInt(value_msb) << BigInt(64);
        sent_value = sent_value | BigInt(value_lsb);

        return [from_index, sent_value];
      case TypeOptimization.QuantizedInt128AsI8:
        return [from_index + Move.MOVE_BY_1_BYTES, view.getInt8(from_index)];
      case TypeOptimization.QuantizedUint128AsU8:
        return [from_index + Move.MOVE_BY_1_BYTES, view.getUint8(from_index)];
      case TypeOptimization.QuantizedInt128AsI16:
        return [from_index + Move.MOVE_BY_16_BYTES, view.getInt16(from_index)];
      case TypeOptimization.QuantizedUint128AsU16:
        return [from_index + Move.MOVE_BY_16_BYTES, view.getUint16(from_index)];
      case TypeOptimization.QuantizedInt128AsI32:
        return [from_index + Move.MOVE_BY_32_BYTES, view.getInt32(from_index)];
      case TypeOptimization.QuantizedUint128AsU32:
        return [from_index + Move.MOVE_BY_32_BYTES, view.getUint32(from_index)];
      case TypeOptimization.QuantizedInt128AsI64:
        return [from_index + Move.MOVE_BY_64_BYTES, view.getInt64(from_index)];
      case TypeOptimization.QuantizedUint128AsU64:
        return [from_index + Move.MOVE_BY_64_BYTES, view.getUint64(from_index)];
    }
  }

  // parseTypeOptimizatedU64(from_index, view) {
  //   const optimization_type = view.getUint8(from_index);
  //   if (!(optimization_type in TypeOptimization.__INVERSE__)) {
  //     throw new Error(`OptimizationType ${optimization_type} is not known`);
  //   }
  //
  //   switch (optimization_type) {
  //     case TypeOptimization.None:
  //       break;
  //     case TypeOptimization.QuantizedInt16AsI8:
  //       break;
  //     case TypeOptimization.QuantizedInt32AsI8:
  //       break;
  //     case TypeOptimization.QuantizedInt32AsI16:
  //       break;
  //     case TypeOptimization.QuantizedInt64AsI8:
  //       break;
  //     case TypeOptimization.QuantizedInt64AsI16:
  //       break;
  //     case TypeOptimization.QuantizedUint16AsU8:
  //       break;
  //     case TypeOptimization.QuantizedUint32AsU8:
  //       break;
  //     case TypeOptimization.QuantizedUint32AsU16:
  //       break;
  //     case TypeOptimization.QuantizedUint64AsU8:
  //       break;
  //     case TypeOptimization.QuantizedUint64AsU16:
  //       break;
  //     case TypeOptimization.QuantizedF64AsF32:
  //       break;
  //     case TypeOptimization.QuantizedF128AsF32:
  //       break;
  //     case TypeOptimization.QuantizedF128AsF64:
  //       break;
  //     case TypeOptimization.QuantizedInt128AsI8:
  //       break;
  //     case TypeOptimization.QuantizedInt128AsI16:
  //       break;
  //     case TypeOptimization.QuantizedInt128AsI32:
  //       break;
  //     case TypeOptimization.QuantizedInt128AsI64:
  //       break;
  //     case TypeOptimization.QuantizedUint128AsU8:
  //       break;
  //     case TypeOptimization.QuantizedUint128AsU16:
  //       break;
  //     case TypeOptimization.QuantizedUint128AsU32:
  //       break;
  //     case TypeOptimization.QuantizedUint128AsU64:
  //       break;
  //     case TypeOptimization.QuantizedPtrAsU8:
  //       break;
  //     case TypeOptimization.QuantizedPtrAsU16:
  //       break;
  //     case TypeOptimization.QuantizedPtrAsU32:
  //       break;
  //     case TypeOptimization.QuantizedPtrAsU64:
  //       break;
  //   }
  // }
}

class BatchInstrructions {
  constructor(memory_operator, text_codec) {
    if (!(memory_operator instanceof MemoryOperator)) {
      throw new Error("Must be instance of MemoryOperator");
    }
    if (!(text_codec instanceof TextCodec)) {
      throw new Error("Must be instance of TextCodec");
    }

    this.texts = text_codec;
    this.operator = memory_operator;
    this.module = memory_operator.get_module();

    // v2 function parameters handling
    this.parameter_v2 = new ParameterParserV2(this.operator, this.texts);
  }

  parse_instructions(ops_pointer, ops_length, text_pointer, text_length) {
    LOGGER.debug(
      "Received instructions: ",
      ops_pointer,
      ops_length,
      text_pointer,
      text_length,
    );

    const operations_buffer = this.operator.readUint8Buffer(
      Number(ops_pointer),
      Number(ops_length),
    );
    LOGGER.debug(
      "BatchInstructions::parse_instructions -> processing ops: ",
      operations_buffer,
    );

    const text_buffer = this.operator.readUint8Buffer(
      Number(text_pointer),
      Number(text_length),
    );
    LOGGER.debug(
      "BatchInstructions::parse_instructions -> processing text: ",
      text_buffer,
    );

    const batches = [];

    const operations_view = new DataView(operations_buffer);
    const texts_view = new DataView(text_buffer);

    let readIndex = 0;
    while (true) {
      let [batch, consumedIndex] = this.parse_batch(
        readIndex,
        operations_view,
        texts_view,
      );
      LOGGER.debug("Extracted batch: ", batch, consumedIndex);
      readIndex = consumedIndex;
      batches.push(batch);
      if (!batch) break;
    }

    LOGGER.debug("Extracted batches: ", batches);

    return batches;
  }

  parse_batch(read_index, operations, texts) {
    const batch = [];
    if (operations.getUint8(read_index) != Operations.Begin) {
      throw new Error(
        `Argument did not end with Operation.Begin instead got: ${operations.getUint8(read_index)}`,
      );
    }

    read_index += Move.MOVE_BY_1_BYTES;

    switch (operations.getUint8(read_index)) {
      case Operations.MakeFunction:
        batch.push(this.parse_make_function(read_index, operations, texts));
        break;
      case Operations.InvokeNoReturnFunction:
        break;
      case Operations.InvokeReturningFunction:
        break;
      case Operations.InvokeCallbackFunction:
        break;
    }

    if (operations.getUint8(read_index) != Operations.Stop) {
      throw new Error(
        `Argument did not end with Operation.Stop instead got: ${operations.getUint8(read_index)}`,
      );
    }

    read_index += Move.MOVE_BY_1_BYTES;

    return [batch, read_index];
  }

  parse_make_function(read_index, operations, texts) {
    if (operations.getUint8(read_index) != Operations.Begin) {
      throw new Error(
        `Argument did not end with Operation.MakeFunction instead got: ${operations.getUint8(read_index)}`,
      );
    }

    read_index += Move.MOVE_BY_1_BYTES;
  }
}

class MegatronMiddleware {
  constructor() {
    this.module = null;

    // heap for DOM objects
    this.dom_heap = null;

    // function heap
    this.function_heap = null;

    // object_heap
    this.object_heap = null;

    // text decoders and memory ops
    this.texts = null;
    this.operator = null;

    // function parameters
    this.parameter_v1 = null;
  }

  init(wasm_module) {
    this.module = wasm_module;

    // DOM object heap for registering DOM objects.
    this.dom_heap = new DOMArena();

    // object heap for registering generated or borrowed objects.
    this.object_heap = new ArenaAllocator();

    // function heap for registering function objects.
    this.function_heap = new ArenaAllocator();

    // memory operations.
    this.operator = new MemoryOperator(this.module);

    // text codec for text handling
    this.texts = new TextCodec(this.operator);

    // v1 function parameters handling
    this.parameter_v1 = new ParameterParserV1(this.operator, this.texts);

    // v2 batch instruction handling
    this.batch_parser = new BatchInstrructions(this.operator, this.texts);
  }

  get v2_mappings() {
    return {
      apply_instructions: this.apply_instructions.bind(this),
      dom_allocate_external_pointer:
        this.dom_allocate_external_pointer.bind(this),
      object_allocate_external_pointer:
        this.object_allocate_external_pointer.bind(this),
      function_allocate_external_pointer:
        this.function_allocate_external_pointer.bind(this),
    };
  }

  get v1_mappings() {
    return {
      abort: this.abort.bind(this),
      drop_external_reference: this.drop_external_reference.bind(this),
      js_register_function: this.js_register_function.bind(this),
      js_invoke_function: this.js_invoke_function.bind(this),
      js_invoke_function_and_return_object:
        this.js_invoke_function_and_return_object.bind(this),
      js_invoke_function_and_return_bool:
        this.js_invoke_function_and_return_bool.bind(this),
      js_invoke_function_and_return_bigint:
        this.js_invoke_function_and_return_bigint.bind(this),
      js_invoke_function_and_return_string:
        this.js_invoke_function_and_return_string.bind(this),
    };
  }

  abort() {
    throw new Error("WasmInstance calls abort");
  }

  apply_instructions(ops_pointer, ops_length, text_pointer, text_length) {
    const instructions = this.batch_parser.parse_instructions(
      ops_pointer,
      ops_length,
      text_pointer,
      text_length,
    );
    LOGGER.debug("Received instructions in batch: ", instructions);
  }

  drop_external_reference(uid) {
    return this.dom_heap.destroy(uid);
  }

  object_allocate_external_pointer() {
    LOGGER.debug("Creating pointer for ");
    return this.object_heap.create(null);
  }

  function_allocate_external_pointer() {
    LOGGER.debug("Creating pointer for ");
    return this.function_heap.create(null);
  }

  dom_allocate_external_pointer() {
    LOGGER.debug("Creating pointer for ");
    return this.dom_heap.create(null);
  }

  js_register_function(start, length, utf_indicator) {
    LOGGER.debug(
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

    start = Number(start);
    length = Number(length);

    if (utf_indicator === 16) {
      function_body = this.texts.readUTF16FromMemory(start, length);
    }
    if (utf_indicator === 8) {
      function_body = this.texts.readUTF8FromMemory(start, length);
    }
    if (!function_body) throw new Error("Function body must be supplied");

    const registered_func = Function(
      `"use strict"; return(${function_body})`,
    )();

    return this.function_heap.create(registered_func);
  }

  js_invoke_function(handle, parameter_start, parameter_length) {
    // read parameters and invoke function via handle.
    const parameters = this.parameter_v1.parse_array(
      parameter_start,
      parameter_length,
    );
    if (!parameters && parameter_length > 0)
      throw new Error("No parameters returned though we expect some");

    const func = this.function_heap.get(handle);

    const response = func.call(this, ...parameters);
    if (isUndefinedOrNull(response)) {
      return BigInt(0);
    }
    if (typeof response === "BigInt") {
      return response;
    }
    return BigInt(response);
  }

  js_invoke_function_and_return_object(
    handle,
    parameter_start,
    parameter_length,
  ) {
    // read parameters and invoke function via handle.
    const parameters = this.parameter_v1.parse_array(
      parameter_start,
      parameter_length,
    );

    const func = this.functions[handle];
    const result = func.call(runtime, ...parameters);
    if (result === undefined || result === null) {
      throw new Error(
        "function returned undefined or null while trying to return an object",
      );
    }

    return this.dom_heap.create(result);
  }

  js_invoke_function_and_return_bool(
    handle,
    parameter_start,
    parameter_length,
  ) {
    // read parameters and invoke function via handle.
    const parameters = this.parameter_v1.parse_array(
      parameter_start,
      parameter_length,
    );
    const func = this.functions[handle];
    const result = func.call(runtime, ...parameters);
    return result ? 1 : 0;
  }

  js_invoke_function_and_return_bigint(
    handle,
    parameter_start,
    parameter_length,
  ) {
    // read parameters and invoke function via handle.
    const parameters = this.parameter_v1.parse_array(
      parameter_start,
      parameter_length,
    );
    const func = this.functions[handle];
    return func.call(runtime, ...parameters);
  }

  js_invoke_function_and_return_string(
    handle,
    parameter_start,
    parameter_length,
  ) {
    // read parameters and invoke function via handle.
    const parameters = this.parameter_v1.parse_array(
      parameter_start,
      parameter_length,
    );
    const func = this.functions[handle];
    const result = func.call(runtime, ...parameters);
    if (result === undefined || result === null) {
      throw new Error(
        "function returned undefined or null while trying to return an object",
      );
    }
    return this.operator.writeUint8Array(result);
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

    const scripts = WASMLoader.getScriptsForApplicationWASM();
    for (let index in scripts) {
      let script = scripts[index];
      if (script.url) {
        const loader = new WASMLoader(
          initial_memory,
          maximum_memory,
          environment,
          compileOptions,
        );

        instantiated.push(loader.loadURL(script.url));
      } else {
        console.error("Properly must have 'url' property.", script);
      }
    }

    return await Promise.all(instantiated);
  }

  constructor(initial_memory, maximum_memory, environment, compileOptions) {
    if (!environment) environment = {};
    if (!initial_memory) initial_memory = 10;
    if (!maximum_memory) maximum_memory = 200;

    this.module = null;
    this.initial_memory = initial_memory;
    this.maximum_memory = maximum_memory;
    this.compiled_options = compileOptions;

    this.parser = new MegatronMiddleware();
    this.memory = new WebAssembly.Memory({
      initial: initial_memory,
      maximum: maximum_memory,
    });

    this.env = {
      v1: this.parser.v1_mappings,
      v2: this.parser.v2_mappings,
      ...environment,
    };
  }

  run() {
    if (!this.module) throw new Error("No wasm module loaded");
    if (!this.module.instance.exports.main)
      throw new Error("wasm module has no exported main function");
    return this.module.instance.exports.main();
  }

  #setup_module(module) {
    if (!(module.instance instanceof WebAssembly.Instance)) {
      throw new Error("Module must be an instance of WebAssembly.Instance");
    }

    this.module = module;
    this.parser.init(module);
  }

  async loadURL(wasm_url) {
    await WASMLoader.loadWASMURL(
      wasm_url,
      this.memory,
      this.env,
      this.compiled_options,
    ).then((module) => {
      this.#setup_module(module);
    });
    return this;
  }

  async loadBytes(wasm_bytes) {
    await WASMLoader.loadWASMBytes(
      wasm_bytes,
      this.memory,
      this.env,
      this.compiled_options,
    ).then((module) => {
      this.#setup_module(module);
    });
    return this;
  }

  static async loadWASMBytes(wasm_bytes, memory, environment, compileOptions) {
    return await WASMLoader.loadWASMModule(
      false,
      wasm_bytes,
      compileOptions,
      environment,
      memory,
    );
  }

  static async loadWASMURL(wasmURL, memory, environment, compileOptions) {
    const wasm_response = await WASMLoader.loadURL(wasmURL);
    return await WASMLoader.loadWASMModule(
      true,
      wasm_response,
      compileOptions,
      environment,
      memory,
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
      LOGGER.debug("Loaded Modules: ", modules);
      modules.forEach((instance) => {
        instance.run();
      });
    });
  }
}

WasmWebScripts.default = function (env) {
  return new WasmWebScripts(10, 200, env, WASMLoader.configCompiledOptions({}));
};

if (module) {
  module.exports = {
    LOGGER,
    LEVELS,
    Move,
    Params,
    Operations,
    ArgumentOperations,
    TypeOptimization,
    DOMArena,
    TextCodec,
    WASMLoader,
    ArenaAllocator,
    MemoryOperator,
    WasmWebScripts,
    ParameterParserV1,
    ParameterParserV2,
    BatchInstrructions,
    MegatronMiddleware,
  };
}
