"strict mode";

const Megatron = (function () {
  const NULL_AND_UNDEFINED = [null, undefined];
  const MAX_ITERATION = 5000000;

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

  Object.freeze(Move);

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

  Object.freeze(UtfEncoding);

  const ALLOWED_UTF8_INDICATOR = [UtfEncoding.UTF8, UtfEncoding.UTF16];

  const TypedSlice = {
    Int8: 1,
    Int16: 2,
    Int32: 3,
    Int64: 4,
    Uint8: 5,
    Uint16: 6,
    Uint32: 7,
    Uint64: 8,
    Float32: 9,
    Float64: 10,
  };

  TypedSlice.__INVERSE__ = Object.keys(TypedSlice)
    .map((key) => {
      return [key, TypedSlice[key]];
    })
    .reduce((prev, current) => {
      let [key, value] = current;
      prev[value] = key;
      return prev;
    }, {});

  Object.freeze(TypedSlice);

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

  Object.freeze(TypeOptimization);

  const ReturnHintMarker = {
    Start: 200,
    Stop: 201,
  };

  ReturnHintMarker.__INVERSE__ = Object.keys(ReturnHintMarker)
    .map((key) => {
      return [key, ReturnHintMarker[key]];
    })
    .reduce((prev, current) => {
      let [key, value] = current;
      prev[value] = key;
      return prev;
    }, {});

  Object.freeze(ReturnHintMarker);

  ReturnValueMarker = {
    Begin: 100,
    End: 101,
  };

  ReturnValueMarker.__INVERSE__ = Object.keys(ReturnValueMarker)
    .map((key) => {
      return [key, ReturnValueMarker[key]];
    })
    .reduce((prev, current) => {
      let [key, value] = current;
      prev[value] = key;
      return prev;
    }, {});

  Object.freeze(ReturnValueMarker);

  /// [`ReturnTypes`] represent the type indicating the underlying returned
  /// value for an operation.
  const ReturnTypes = {
    None: 0,
    One: 1,
    Multi: 2,
    List: 3,
  };

  ReturnTypes.__INVERSE__ = Object.keys(ReturnTypes)
    .map((key) => {
      return [key, ReturnTypes[key]];
    })
    .reduce((prev, current) => {
      let [key, value] = current;
      prev[value] = key;
      return prev;
    }, {});

  Object.freeze(ReturnTypes);

  /// [`ReturnValueTypes`] represent the type indicating the underlying returned
  /// value for an operation.
  const ReturnTypeId = {
    Bool: 1,
    Text8: 2,
    Int8: 3,
    Int16: 4,
    Int32: 5,
    Int64: 6,
    Uint8: 7,
    Uint16: 8,
    Uint32: 9,
    Uint64: 10,
    Float32: 11,
    Float64: 12,
    Int128: 13,
    Uint128: 14,
    MemorySlice: 15,
    ExternalReference: 16,
    InternalReference: 17,
    Uint8ArrayBuffer: 18,
    Uint16ArrayBuffer: 19,
    Uint32ArrayBuffer: 20,
    Uint64ArrayBuffer: 21,
    Int8ArrayBuffer: 22,
    Int16ArrayBuffer: 23,
    Int32ArrayBuffer: 24,
    Int64ArrayBuffer: 25,
    Float32ArrayBuffer: 26,
    Float64ArrayBuffer: 27,
    Object: 28,
    DOMObject: 29,
  };

  ReturnTypeId.__INVERSE__ = Object.keys(ReturnTypeId)
    .map((key) => {
      return [key, ReturnTypeId[key]];
    })
    .reduce((prev, current) => {
      let [key, value] = current;
      prev[value] = key;
      return prev;
    }, {});

  Object.freeze(ReturnTypeId);

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
    /// function across boundary that takes a callback internal reference
    /// which it will use to supply appropriate response when ready (say async call)
    /// as response to being called.
    ///
    /// The return value to the callback function must always be of the type: [`Returns`].
    ///
    /// Layout format: Begin, 3, FunctionHandle(u64), ArgStart, ArgBegin, ExternReference, ArgEnd, ArgStop,
    ///  End
    InvokeCallbackFunction: 4,

    /// InvokeAsyncCallbackFunction represents the desire to call a
    /// async function across boundary that takes a callback internal reference
    /// which it will use to supply appropriate response when ready.
    ///
    /// The return value to the callback function must always be of the type: [`Returns`].
    ///
    /// Layout format: Begin, 3, FunctionHandle(u64), ArgStart, ArgBegin, ExternReference, ArgEnd, ArgStop,
    ///  End
    InvokeAsyncCallbackFunction: 5,

    /// End - indicates the end of a portion of a instruction set.
    /// Since an instruction memory array can contain multiple instructions
    /// batched together, then each instruction must have a end marker indicating
    /// one portion is over.
    End: 254,

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

    // All these will generally use `TypedArray.slice`
    // to get the actual TypedArray's content which is a
    // shallow copy but a copy none-theless.
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
    CachedText: 29,

    // TypedArraySlice provides a non-copy option where you
    // can intentionally send a slice you clearly indicate
    // points to the actual portion in wasm memory and
    // so you get the raw Uint8ArrayBuffer which you must
    // use or copy to avoid corruption of data.
    TypedArraySlice: 30,
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

  class SimpleStringCache {
    constructor() {
      this.text_to_id = new Map();
      this.id_to_text = new Map();
      this.count = 0;
    }

    clear() {
      this.text_to_id = new Map();
      this.id_to_text = new Map();
      this.count = 0;
    }

    destroy_id(id) {
      if (typeof item !== "string") {
        throw new Error("Only strings are allowed");
      }

      if (!this.has_id(id)) {
        return false;
      }

      this.id_to_text.delete(id);
      this.text_to_id.delete(item);

      if (this.text_to_id.size == 0) {
        this.count = 0;
      }
    }

    destroy(item) {
      if (typeof item !== "string") {
        throw new Error("Only strings are allowed");
      }

      if (!this.has_text(item)) {
        return false;
      }

      const id = this.get_id(item);
      this.destroy_id(id);
    }

    create(item) {
      if (typeof item !== "string") {
        throw new Error("Only strings are allowed");
      }

      if (this.has_text(item)) {
        return this.text_to_id.get(item);
      }

      const new_id = this.count + 1;
      this.new_count += 1;

      this.text_to_id.set(item, new_id);
      this.id_to_text.set(new_id, item);

      return new_id;
    }

    has_id(id) {
      if (typeof id !== "number") {
        throw new Error("Only Number allowed");
      }
      return this.id_to_text.has(id);
    }

    has_text(text) {
      if (typeof text !== "string") {
        throw new Error("Only string allowed");
      }
      return this.text_to_id.has(text);
    }

    get_text(id) {
      if (typeof id !== "number") {
        throw new Error("Only Number allowed");
      }
      return this.id_to_text.get(id);
    }

    get_id(text) {
      if (typeof text !== "string") {
        throw new Error("Only string allowed");
      }
      return this.text_to_id.get(text);
    }
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

    length() {
      return this.items.length;
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
      LOGGER.debug(
        `ArenaAllocator::get_entry for entry with id: ${uid} with slot: `,
        candidate,
      );
      const entry = this.get_entry_at_index(candidate.index);
      if (entry == null) return null;
      return entry.item;
    }

    update_entry(slot_id, item) {
      let candidate = ArenaAllocator.uid_to_entry(slot_id);
      if (candidate.index >= this.items.length) {
        return false;
      }
      const slot = this.get_entry_at_index(candidate.index);
      if (slot.generation != candidate.generation) {
        LOGGER.debug(
          `Slot for ${slot_id} with (index=${candidate.index}, gen=${candidate.generation}) failed generation match against ${slot.generation}`,
        );
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
      const index_as_32bit = BigInt(entry.index) & ArenaAllocator.BIT_MASK;
      const generation_as_32bit =
        BigInt(entry.generation) & ArenaAllocator.BIT_MASK;

      // shift the index to the MSB(Most Significant Bits, starting from the leftmost bit).
      // others say: shifting to the upper bits (so leftmost)
      // Then: shift the generation uid to the rightmost bit (Least Significant bit).
      // others say: shifting to the lower bits (so rightmost)
      let packed_uid =
        (index_as_32bit << ArenaAllocator.BIT_SIZE) | generation_as_32bit;
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
      // 0 is reserved for self (if `self` exists else is also `this`).
      // 1 is reserved for this (the DOM arena)
      // 2 is reserved for window
      // 3 is reserved for document
      // 4 is reserved for document.body
      //
      if (typeof self != "undefined") {
        this.create(self);
      } else {
        this.create(this);
      }
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
      this.instance = this.module.instance;
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
      LOGGER.debug(
        "allocate_memory: allocating memory location with size: ",
        size,
      );
      const allocation_id = this.instance.exports.create_allocation(
        BigInt(size),
      );
      LOGGER.debug(
        "Created allocation_id: ",
        allocation_id,
        typeof allocation_id,
      );
      const allocation_start_pointer =
        this.instance.exports.allocation_start_pointer(allocation_id);
      LOGGER.debug(
        `Retrieved allocation ptr: ${allocation_start_pointer} for id: ${allocation_id}`,
      );

      return [allocation_id, allocation_start_pointer];
    }

    readUint8Buffer(start_pointer, len) {
      const memory = this.get_memory();
      return memory.slice(start_pointer, start_pointer + len);
    }

    readUint8Array(start_pointer, len) {
      return new Uint8Array(this.readUint8Buffer(start_pointer, len));
    }

    writeUint8Buffer(buffer) {
      const buffer_type = typeof buffer;
      LOGGER.debug(
        `writeUint8Buffer: Writing buffer to memory(type=${buffer_type}): len=${buffer.length} from buffer=${buffer}`,
      );
      const len = buffer.length;
      const [id, start] = this.allocate_memory(len);

      const memory = new Uint8Array(this.get_memory());
      memory.set(buffer, start);

      return id;
    }

    writeUint8Array(int8_buffer) {
      LOGGER.debug("Writing Uint8Array from: ", int8_buffer);
      let uint8_buffer =
        int8_buffer instanceof Uint8Array
          ? int8_buffer
          : new Uint8Array(int8_buffer);
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

    readShortUTF8FromMemory(start, len) {
      const memory = new Uint8Array(this.operator.get_memory());
      const data_slice = memory.subarray(start, start + len);
      const text = TextCodec.utf8ArrayToStr(data_slice);
      LOGGER.debug("TextDecoder:utf8ArrayToStr -> ", text);
      return text;
    }

    readShortUTF16FromMemory(start, len) {
      const memory = new Uint8Array(this.operator.get_memory());
      const bytes = memory.subarray(start, start + len);
      const text = TextCodec.utf8ArrayToStr(bytes);
      LOGGER.debug("TextDecoder:utf8ArrayToStr -> ", text);
      return text;
    }

    readUTF8FromMemory(start, len) {
      const memory = new Uint8Array(this.operator.get_memory());
      const data_slice = memory.subarray(start, start + len);
      return this.utf8_decoder.decode(data_slice);
    }

    readUTF16FromMemory(start, len) {
      const memory = new Uint8Array(this.operator.get_memory());
      const bytes = memory.subarray(start, start + len);
      const text = this.utf16_decoder.decode(bytes);
      return text;
    }

    readUTF8FromView(view) {
      return this.utf8_decoder.decode(view);
    }

    readUTF16FromView(view) {
      return this.utf16_decoder.decode(view);
    }

    writeUTF8ToMemory(text) {
      const bytes = this.utf8_encoder.encode(text);
      return this.operator.writeUint8Array(bytes);
    }

    // *******************************************************
    // Below are all the sample uint8/uint16 conversion to string
    // i found, I kept these here for both testing and inspiration.
    // *******************************************************

    // [utf8ArrayToStr] converts your array to UTF-16 using
    // the decodeURIComponent.
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

    // Extracted from benchmarks: https://jsbench.me/4vl97c05lb/5
    //
    // Roughly 59.3% slower with large data list.
    static decodeUTF8Slice(byte_array, start, _end) {
      let pos = start;
      const end = _end;
      let out = "";
      while (pos < end) {
        const byte1 = byte_array[pos++];
        if ((byte1 & 0x80) === 0) {
          // 1 byte
          out += String.fromCharCode(byte1);
        } else if ((byte1 & 0xe0) === 0xc0) {
          // 2 bytes
          const byte2 = u8arr[pos++] & 0x3f;
          out += String.fromCharCode(((byte1 & 0x1f) << 6) | byte2);
        } else if ((byte1 & 0xf0) === 0xe0) {
          // 3 bytes
          const byte2 = u8arr[pos++] & 0x3f;
          const byte3 = u8arr[pos++] & 0x3f;
          out += String.fromCharCode(
            ((byte1 & 0x1f) << 12) | (byte2 << 6) | byte3,
          );
        } else if ((byte1 & 0xf8) === 0xf0) {
          // 4 bytes
          const byte2 = u8arr[pos++] & 0x3f;
          const byte3 = u8arr[pos++] & 0x3f;
          const byte4 = u8arr[pos++] & 0x3f;
          let unit =
            ((byte1 & 0x07) << 0x12) |
            (byte2 << 0x0c) |
            (byte3 << 0x06) |
            byte4;
          if (unit > 0xffff) {
            unit -= 0x10000;
            out += String.fromCharCode(((unit >>> 10) & 0x3ff) | 0xd800);
            unit = 0xdc00 | (unit & 0x3ff);
          }
          out += String.fromCharCode(unit);
        } else {
          out += String.fromCharCode(byte1);
        }
      }

      return out;
    }

    static decodeAsciiDecode(byte_array, start, _en) {
      let pos = start;
      const end = _end;
      let out = "";
      while (pos < end) {
        out += String.fromCharCode(byte_array[pos++]);
      }
      return out;
    }

    // See https://github.com/emscripten-core/emscripten/blob/main/test/host_optimizer/applyImportAndExportNameChanges2-output.js#L30-L64
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
              ((c & 0x0f) << 12) |
                ((char2 & 0x3f) << 6) |
                ((char3 & 0x3f) << 0),
            );
            break;
        }
      }

      return out;
    }
  }

  class RefPointer {
    constructor(value) {
      this.id = value;
    }

    get value() {
      return this.id;
    }
  }

  class ReturnHint {
    constructor(return_type, value_type) {
      this.value_type = value_type;
      this.return_type = return_type;
      this.type_validators = {};
      this.type_validators[ReturnTypeId.Bool] = this.validateBool;
      this.type_validators[ReturnTypeId.Text8] = this.validateText8;
      this.type_validators[ReturnTypeId.Uint16] = this.validateInt;
      this.type_validators[ReturnTypeId.Uint32] = this.validateInt;
      this.type_validators[ReturnTypeId.Uint64] = this.validateInt64;
      this.type_validators[ReturnTypeId.Int8] = this.validateInt;
      this.type_validators[ReturnTypeId.Int16] = this.validateInt;
      this.type_validators[ReturnTypeId.Int32] = this.validateInt;
      this.type_validators[ReturnTypeId.Int64] = this.validateInt64;
      this.type_validators[ReturnTypeId.Float32] = this.validateFloat;
      this.type_validators[ReturnTypeId.Float64] = this.validateFloat;
    }

    get return_valuetype() {
      return this.value_type;
    }

    get_validator(input_type) {
      const validator = this.type_validators[input_type];
      if (isUndefinedOrNull(validator)) {
        throw new Error(`ReturnTypeId ${value_type} is not known`);
      }
      return validator;
    }

    validateBool(value) {
      return typeof value != "boolean";
    }

    validateText8(value) {
      return typeof value != "string";
    }

    validateFloat(value) {
      return typeof value != "number";
    }

    validateInt(value) {
      return typeof value != "number";
    }

    validateInt64(value) {
      return typeof value != "bigint";
    }

    validateUint8Array(value) {
      return value instanceof Uint8Array;
    }

    validateUint16Array(value) {
      return value instanceof Uint16Array;
    }

    validateUint32Array(value) {
      return value instanceof Uint32Array;
    }

    validateUint64Array(value) {
      return value instanceof BigUint64Array;
    }

    validateInt8Array(value) {
      return value instanceof Int8Array;
    }

    validateInt16Array(value) {
      return value instanceof Int16Array;
    }

    validateInt32Array(value) {
      return value instanceof Int32Array;
    }

    validateInt64Array(value) {
      return value instanceof BigInt64Array;
    }

    validateFloat32Array(value) {
      return value instanceof Float32Array;
    }

    validateFloat64Array(value) {
      return value instanceof Float32Array;
    }

    validateMemorySlice(value) {
      if (typeof value == "bigint") {
        return true;
      }
      return false;
    }

    validateReference(value) {
      if (value instanceof ExternalPointer) {
        return true;
      }
      if (value instanceof InternalPointer) {
        return true;
      }
      if (typeof value == "bigint") {
        return true;
      }
      return false;
    }
  }

  class NoReturn extends ReturnHint {
    constructor() {
      super(ReturnTypes.None, -1);
    }

    validate(input) {
      return isUndefinedOrNull(input);
    }
  }

  class SingleReturn extends ReturnHint {
    constructor(value_type) {
      super(ReturnTypes.One, value_type);
    }

    validate(input) {
      return false;
    }
  }

  class ListReturn extends ReturnHint {
    constructor(value_types) {
      super(ReturnTypes.List, value_types);
    }
  }

  class MultiReturn extends ReturnHint {
    constructor(value_type) {
      super(ReturnTypes.Multi, value_type);
    }
  }

  class ExternalPointer extends RefPointer {}

  class InternalPointer extends RefPointer {}

  class CachePointer extends RefPointer {}

  class TypedArraySlice {
    constructor(slice_type, content) {
      if (!(slice_type in TypedSlice.__INVERSE__)) {
        throw new Error(
          `TypedSlice ${slice_type} is not known for TypedSlice: ${slice_type}`,
        );
      }

      this.slice_type = slice_type;
      this.slice_content = content;
    }

    equals(other) {
      return (
        this.content_type == other.content_type && this.content == other.content
      );
    }

    get content() {
      return this.slice_content;
    }

    get content_type() {
      return this.slice_type;
    }
  }

  class ReturnHintParser {
    constructor() {
      this.parsers = {};
      this.parsers[ReturnTypes.One] = this.parse_one;
      this.parsers[ReturnTypes.None] = this.parse_none;
      this.parsers[ReturnTypes.List] = this.parse_list;
      this.parsers[ReturnTypes.Multi] = this.parse_multiple;
    }

    parse(moved_by, view) {
      // validate we see begin marker
      const id = view.getUint8(moved_by, true);

      if (id != ReturnHintMarker.Start) {
        throw new Error("Argument did not start with ArgumentOperation.Start");
      }
      moved_by += Move.MOVE_BY_1_BYTES;

      // get the type of hint
      const hint_type = view.getUint8(moved_by, true);
      if (!(hint_type in this.parsers)) {
        throw new Error(`ReturnHintType ${hint_type} not found`);
      }

      let return_validator;
      const parser = this.parsers[hint_type];
      [moved_by, return_validator] = parser(moved_by, view);

      if (view.getUint8(moved_by, true) != ReturnHintMarker.Stop) {
        throw new Error("Argument did not end with ArgumentOperation.End");
      }

      moved_by += Move.MOVE_BY_1_BYTES;

      return [moved_by, return_validator];
    }

    parse_none(offset, view) {
      return [offset, new NoReturn()];
    }

    parse_one(offset, view) {
      const value_type = view.getUint8(moved_by, true);
      if (!(value_type in ReturnTypeId.__INVERSE__)) {
        throw new Error(`ReturnTypeId ${value_type} is not known`);
      }

      moved_by += Move.MOVE_BY_1_BYTES;
      return [offset, new SingleReturn(value_type)];
    }

    parse_multiple(offset, view) {
      const multi_values = [];
      while (true) {
        let value_type = view.getUint8(moved_by, true);
        if (value_type == ReturnHintMarker.Stop) {
          break;
        }

        if (!(value_type in ReturnTypeId.__INVERSE__)) {
          throw new Error(`ReturnTypeId ${value_type} is not known`);
        }

        multi_values.push(value_type);
      }

      return [offset, new MultiReturn(multi_values)];
    }

    parse_list(offset, view) {
      const value_type = view.getUint8(moved_by, true);
      if (!(value_type in ReturnTypeId.__INVERSE__)) {
        throw new Error(`ReturnTypeId ${value_type} is not known`);
      }

      moved_by += Move.MOVE_BY_1_BYTES;
      return [offset, new ListReturn(value_type)];
    }
  }

  class ParameterParserV1 {
    constructor(memory_operator, text_codec, text_cache) {
      if (!(memory_operator instanceof MemoryOperator)) {
        throw new Error("Must be instance of MemoryOperator");
      }
      if (!(text_codec instanceof TextCodec)) {
        throw new Error("Must be instance of TextCodec");
      }
      if (!(text_cache instanceof SimpleStringCache)) {
        throw new Error("text_cache Must be instance of SimpleStringCache");
      }

      this.texts = text_codec;
      this.text_cache = text_cache;
      this.operator = memory_operator;
      this.module = memory_operator.get_module();

      this.parsers = {
        0: this.parseNull.bind(this),
        1: this.parseUndefined.bind(this),
        2: this.parseBool.bind(this),
        3: this.parseText8.bind(this),
        4: this.parseText16.bind(this),
        5: this.parseInt8.bind(this),
        6: this.parseInt16.bind(this),
        7: this.parseInt32.bind(this),
        8: this.parseBigInt64.bind(this),
        9: this.parseUint8.bind(this),
        10: this.parseUint16.bind(this),
        11: this.parseUint32.bind(this),
        12: this.parseBigUint64.bind(this),
        13: this.parseFloat32.bind(this),
        14: this.parseFloat64.bind(this),
        15: this.parseExternalReference.bind(this),
        16: this.parseUint8Array.bind(this),
        17: this.parseUint16Array.bind(this),
        18: this.parseUint32Array.bind(this),
        19: this.parseUint64Array.bind(this),
        20: this.parseInt8Array.bind(this),
        21: this.parseInt16Array.bind(this),
        22: this.parseInt32Array.bind(this),
        23: this.parseInt64Array.bind(this),
        24: this.parseFloat32Array.bind(this),
        25: this.parseFloat64Array.bind(this),
        26: this.parseInternalReference.bind(this),
        27: this.parseBigInt128.bind(this),
        28: this.parseBigUint128.bind(this),
        29: this.parseCachedText.bind(this),
        30: this.parseTypedArraySlice.bind(this),
      };
    }

    get_parser(parameter_type_id) {
      const parser = this.parsers[parameter_type_id];
      LOGGER.debug(
        "Retrieved parser for parameter typeId: ",
        parameter_type_id,
        " with parser: ",
        parser,
      );
      if (isUndefinedOrNull(parser)) {
        throw new Error(
          "Invalid parameter_type id provided: ",
          parameter_type_id,
        );
      }
      return parser;
    }

    parse_array(start, length) {
      const parameter_buffer = this.operator.readUint8Buffer(
        Number(start),
        Number(length),
      );
      LOGGER.debug("parse_array:start ", start, length, parameter_buffer);

      const parameter_view = new DataView(parameter_buffer);

      const converted_values = [];

      let index = 0;
      while (index < parameter_buffer.byteLength) {
        const parameter_type = parameter_view.getUint8(index);

        // increment index since we read from table
        index += Move.MOVE_BY_1_BYTES;

        LOGGER.debug("Getting parameter type: ", index, parameter_type);

        const parser = this.get_parser(parameter_type);
        const [move_by, should_break] = parser(
          index,
          converted_values,
          parameter_view,
        );

        index = move_by;
        if (should_break) break;
      }

      LOGGER.debug("parse_array:end: ", converted_values);
      return converted_values;
    }

    parseUndefined(index, read_values_list, parameter_buffer) {
      read_values_list.push(undefined);
      return [index, false];
    }

    parseNull(index, read_values_list, parameter_buffer) {
      read_values_list.push(null);
      return [index, false];
    }

    parseExternalReference(index, read_values_list, parameter_buffer) {
      // 5 = extern ref
      const handle_uid = parameter_buffer.getBigInt64(index, true);
      read_values_list.push(new ExternalPointer(handle_uid));
      return [index + Move.MOVE_BY_64_BYTES, false];
    }

    parseInternalReference(index, read_values_list, parameter_buffer) {
      // 5 = extern ref
      const handle_uid = parameter_buffer.getBigInt64(index, true);
      read_values_list.push(new InternalPointer(handle_uid));
      return [index + Move.MOVE_BY_64_BYTES, false];
    }

    parseInt8(index, read_values_list, parameter_buffer) {
      const value = parameter_buffer.getInt8(index, true);
      read_values_list.push(value);
      return [index + Move.MOVE_BY_1_BYTES, false];
    }

    parseUint8(index, read_values_list, parameter_buffer) {
      const value = parameter_buffer.getUint8(index, true);
      read_values_list.push(value);
      return [index + Move.MOVE_BY_1_BYTES, false];
    }

    parseUint32(index, read_values_list, parameter_buffer) {
      const value = parameter_buffer.getUint32(index, true);
      read_values_list.push(value);
      return [index + Move.MOVE_BY_32_BYTES, false];
    }

    parseInt32(index, read_values_list, parameter_buffer) {
      const value = parameter_buffer.getInt32(index, true);
      read_values_list.push(value);
      return [index + Move.MOVE_BY_32_BYTES, false];
    }

    parseUint16(index, read_values_list, parameter_buffer) {
      const value = parameter_buffer.getUint16(index, true);
      read_values_list.push(value);
      return [index + Move.MOVE_BY_16_BYTES, false];
    }

    parseInt16(index, read_values_list, parameter_buffer) {
      const value = parameter_buffer.getInt16(index, true);
      read_values_list.push(value);
      return [index + Move.MOVE_BY_16_BYTES, false];
    }

    parseFloat32(index, read_values_list, parameter_buffer) {
      const value = parameter_buffer.getFloat32(index, true);
      read_values_list.push(value);
      return [index + Move.MOVE_BY_32_BYTES, false];
    }

    parseFloat64(index, read_values_list, parameter_buffer) {
      const value = parameter_buffer.getFloat64(index, true);
      read_values_list.push(value);
      return [index + Move.MOVE_BY_64_BYTES, false];
    }

    parseBigInt64(index, read_values_list, parameter_buffer) {
      const value = parameter_buffer.getBigInt64(index, true);
      LOGGER.debug("parsedBigInt64: ", value, parameter_buffer.buffer);
      read_values_list.push(value);
      return [index + Move.MOVE_BY_64_BYTES, false];
    }

    parseBigUint128(start_index, read_values_list, parameter_buffer) {
      const view = parameter_buffer;

      const value_msb = view.getBigUint64(start_index, true);
      start_index += Move.MOVE_BY_64_BYTES;

      const value_lsb = view.getBigUint64(start_index, true);
      start_index += Move.MOVE_BY_64_BYTES;

      let sent_value = value_msb << BigInt(64);
      sent_value = sent_value | value_lsb;

      read_values_list.push(sent_value);
      return [start_index, false];
    }

    parseBigInt128(start_index, read_values_list, parameter_buffer) {
      const view = parameter_buffer;

      const value_msb = view.getBigInt64(start_index, true);
      start_index += Move.MOVE_BY_64_BYTES;

      const value_lsb = view.getBigInt64(start_index, true);
      start_index += Move.MOVE_BY_64_BYTES;

      let sent_value = value_msb << BigInt(64);
      sent_value = sent_value | value_lsb;

      read_values_list.push(sent_value);
      return [start_index, false];
    }

    parseBigUint64(index, read_values_list, parameter_buffer) {
      const value = parameter_buffer.getBigInt64(index, true);

      read_values_list.push(value);
      return [index + Move.MOVE_BY_64_BYTES, false];
    }

    parseCachedText(index, read_values_list, parameter_buffer) {
      const value = parameter_buffer.getBigInt64(index, true);

      // pull the actual text that should already be registered in the cache
      // if not found (i.e Null or Undefined) throw an error.
      const cache_id = Number(value);
      const cached_text = this.text_cache.get_text(cache_id);
      if (isUndefinedOrNull(cached_text)) {
        throw new Error(
          `Expected text to have been cached with cache id: '${cache_id}'`,
        );
      }

      read_values_list.push(cached_text);
      return [index + Move.MOVE_BY_64_BYTES, false];
    }

    parseBool(index, read_values_list, parameter_buffer) {
      const view = parameter_buffer;
      const value = view.getUint8(index, true);
      read_values_list.push(value == 1 ? true : false);
      return [index + Move.MOVE_BY_1_BYTES, false];
    }

    // cloneBufferArray creates a new DataView for the region of the
    // wasm Memory to save the cost of the copy operation but also be
    // aware this means that area of memory must be quickly consumed to
    // avoid potential corruption or over-written of the data.
    //
    // It's wise to use this when you will immediately consume the contents
    // and generate your derived value else use copyBufferArray instead
    // to get a unique copy of the content.
    cloneBufferArrayAdjusted(start_index, view, adjusterMultiplier) {
      let start = Number(view.getBigUint64(start_index, true));
      start_index += Move.MOVE_BY_64_BYTES;

      let length = Number(view.getBigUint64(start_index, true));
      start_index += Move.MOVE_BY_64_BYTES;

      const end = start + length * adjusterMultiplier;

      // for more efficient usage, as slice copies, we can use:
      const memory = this.operator.get_memory();
      const uint_array = new Uint8Array(memory);
      const slice_view = uint_array.subarray(start, end);

      LOGGER.debug(
        `clonedBufferArray: selecting start=${start}, length=${length}, end=${end} -> ${slice_view}`,
      );

      return [start_index, slice_view];
    }

    cloneBufferArray1Byte(from_index, view) {
      return this.cloneBufferArrayAdjusted(
        from_index,
        view,
        Move.MOVE_BY_1_BYTES,
      );
    }

    cloneBufferArray16Bytes(from_index, view) {
      return this.cloneBufferArrayAdjusted(
        from_index,
        view,
        Move.MOVE_BY_16_BYTES,
      );
    }

    cloneBufferArray32Bytes(from_index, view) {
      return this.cloneBufferArrayAdjusted(
        from_index,
        view,
        Move.MOVE_BY_32_BYTES,
      );
    }

    cloneBufferArray64Bytes(from_index, view) {
      return this.cloneBufferArrayAdjusted(
        from_index,
        view,
        Move.MOVE_BY_64_BYTES,
      );
    }

    /// [copyBufferArray] creates a unique copy of the contents of
    // the memory location pointing to the start and length of the
    // expected content.
    copyBufferArrayAdjusted(start_index, view, adjusterMultiplier) {
      let start = Number(view.getBigUint64(start_index, true));
      start_index += Move.MOVE_BY_64_BYTES;

      let length = Number(view.getBigUint64(start_index, true));
      start_index += Move.MOVE_BY_64_BYTES;

      const end = start + length * adjusterMultiplier;

      const memory = this.operator.get_memory();
      const slice = memory.slice(start, end);

      LOGGER.debug(
        `copyBufferArray: selecting start=${start}, length=${length}, end=${end} -> ${slice}`,
      );

      return [start_index, slice];
    }

    copyBufferArray1Byte(from_index, view) {
      return this.copyBufferArrayAdjusted(
        from_index,
        view,
        Move.MOVE_BY_1_BYTES,
      );
    }

    copyBufferArray16Bytes(from_index, view) {
      return this.copyBufferArrayAdjusted(
        from_index,
        view,
        Move.MOVE_BY_16_BYTES,
      );
    }

    copyBufferArray32Bytes(from_index, view) {
      return this.copyBufferArrayAdjusted(
        from_index,
        view,
        Move.MOVE_BY_32_BYTES,
      );
    }

    copyBufferArray64Bytes(from_index, view) {
      return this.copyBufferArrayAdjusted(
        from_index,
        view,
        Move.MOVE_BY_64_BYTES,
      );
    }

    parseText16(index, read_values_list, parameter_buffer) {
      const [moved_by, slice] = this.cloneBufferArray16Bytes(
        index,
        parameter_buffer,
      );
      const data = this.texts.readUTF16FromView(slice);
      read_values_list.push(data);
      return [moved_by, false];
    }

    parseText8(index, read_values_list, parameter_buffer) {
      const [moved_by, slice] = this.cloneBufferArray1Byte(
        index,
        parameter_buffer,
      );
      const data = this.texts.readUTF8FromView(slice);
      read_values_list.push(data);
      return [moved_by, false];
    }

    parseFloat32Array(index, read_values_list, parameter_buffer) {
      const [moved_by, slice] = this.copyBufferArray32Bytes(
        index,
        parameter_buffer,
      );
      const array = new Float32Array(slice);
      read_values_list.push(array);
      return [moved_by, false];
    }

    parseFloat64Array(index, read_values_list, parameter_buffer) {
      const [moved_by, slice] = this.copyBufferArray64Bytes(
        index,
        parameter_buffer,
      );
      const array = new Float64Array(slice);
      read_values_list.push(array);
      return [moved_by, false];
    }

    parseInt8Array(index, read_values_list, parameter_buffer) {
      const [moved_by, slice] = this.copyBufferArray1Byte(
        index,
        parameter_buffer,
      );
      const array = new Int8Array(slice);
      read_values_list.push(array);
      return [moved_by, false];
    }

    parseInt16Array(index, read_values_list, parameter_buffer) {
      // 10 = array of Uint32 from wasm memory (followed by 32-bit start and size of string in memory)
      const [moved_by, slice] = this.copyBufferArray16Bytes(
        index,
        parameter_buffer,
      );
      const array = new Int16Array(slice);
      read_values_list.push(array);
      return [moved_by, false];
    }

    parseInt32Array(index, read_values_list, parameter_buffer) {
      // 10 = array of Uint32 from wasm memory (followed by 32-bit start and size of string in memory)
      const [moved_by, slice] = this.copyBufferArray32Bytes(
        index,
        parameter_buffer,
      );
      const array = new Int32Array(slice);
      read_values_list.push(array);
      return [moved_by, false];
    }

    parseInt64Array(index, read_values_list, parameter_buffer) {
      // 10 = array of Uint64 from wasm memory (followed by 32-bit start and size of string in memory)
      const [moved_by, slice] = this.copyBufferArray64Bytes(
        index,
        parameter_buffer,
      );
      const array = new BigInt64Array(slice);
      read_values_list.push(array);
      return [moved_by, false];
    }

    // WARNING: This tries to be efficient and avoids copying the contents of the
    // memory location in the wasm memory instance, so ensure to copy the provided
    // data buffer to avoid data corruption if that memory gets overwritten.
    parseTypedArraySlice(start_index, read_values_list, view) {
      // read out the type of the typed slice array
      const slice_type = Number(view.getUint8(start_index, true));
      start_index += Move.MOVE_BY_1_BYTES;

      const [moved_by, slice] = this.cloneBufferArray1Byte(start_index, view);

      read_values_list.push(new TypedArraySlice(slice_type, slice));
      return [moved_by, false];
    }

    parseUint8Array(index, read_values_list, parameter_buffer) {
      const [moved_by, slice] = this.copyBufferArray1Byte(
        index,
        parameter_buffer,
      );
      const array = new Uint8Array(slice);
      read_values_list.push(array);
      return [moved_by, false];
    }

    parseUint16Array(index, read_values_list, parameter_buffer) {
      const [moved_by, slice] = this.copyBufferArray16Bytes(
        index,
        parameter_buffer,
      );
      const array = new Uint16Array(slice);
      read_values_list.push(array);
      return [moved_by, false];
    }

    parseUint32Array(index, read_values_list, parameter_buffer) {
      const [moved_by, slice] = this.copyBufferArray32Bytes(
        index,
        parameter_buffer,
      );
      const array = new Uint32Array(slice);
      read_values_list.push(array);
      return [moved_by, false];
    }

    parseUint64Array(index, read_values_list, parameter_buffer) {
      // 10 = array of Uint64 from wasm memory (followed by 32-bit start and size of string in memory)
      const [moved_by, slice] = this.copyBufferArray64Bytes(
        index,
        parameter_buffer,
      );
      const array = new BigUint64Array(slice);
      read_values_list.push(array);
      return [moved_by, false];
    }
  }

  class ParameterParserV2 {
    constructor(memory_operator, text_codec, text_cache) {
      if (!(memory_operator instanceof MemoryOperator)) {
        throw new Error("Must be instance of MemoryOperator");
      }
      if (!(text_codec instanceof TextCodec)) {
        throw new Error("Must be instance of TextCodec");
      }
      if (!(text_cache instanceof SimpleStringCache)) {
        throw new Error("text_cache Must be instance of SimpleStringCache");
      }

      this.texts = text_codec;
      this.text_cache = text_cache;
      this.operator = memory_operator;
      this.module = memory_operator.get_module();
    }

    parseParams(moved_by, view, text_string_array) {
      LOGGER.debug("Props", moved_by, view, typeof text_string_array);
      if (!(view instanceof DataView)) {
        throw new Error(
          "Argument must be a DataView scoped to the area you want parsed",
        );
      }
      if (!(typeof text_string_array === "string")) {
        throw new Error(
          "Argument must be a DataView scoped to the area you want parsed",
        );
      }

      const parameters = [];

      for (let i = 0; i < MAX_ITERATION; i++) {
        if (view.getUint8(moved_by, true) == ArgumentOperations.Stop) {
          break;
        }

        // validate we see begin marker
        const id = view.getUint8(moved_by, true);

        if (id != ArgumentOperations.Begin) {
          throw new Error(
            "Argument did not start with ArgumentOperation.Start",
          );
        }
        moved_by += Move.MOVE_BY_1_BYTES;

        let param;
        [moved_by, param] = this.parseParam(moved_by, view, text_string_array);

        parameters.push(param);

        LOGGER.debug("Read parameter type: ", moved_by, param);

        // validate we see end marker
        if (view.getUint8(moved_by, true) != ArgumentOperations.End) {
          throw new Error("Argument did not end with ArgumentOperation.End");
        }

        moved_by += Move.MOVE_BY_1_BYTES;
      }

      if (view.getUint8(moved_by, true) != ArgumentOperations.Stop) {
        throw new Error("Argument did not start with ArgumentOperation.End");
      }
      moved_by += Move.MOVE_BY_1_BYTES;

      return [moved_by, parameters];
    }

    parseParam(from_index, view, text_string_array) {
      const value_type = view.getUint8(from_index, true);
      if (!(value_type in Params.__INVERSE__)) {
        throw new Error(`Params ${value_type} is not known`);
      }

      from_index += Move.MOVE_BY_1_BYTES;

      return this.parseParamType(
        from_index,
        value_type,
        view,
        text_string_array,
      );
    }

    parseParamType(from_index, value_type, view, text_string_array) {
      const value_type_str = Params.__INVERSE__[value_type];
      LOGGER.debug(`Received value_type: ${value_type} (${value_type_str})`);

      switch (value_type) {
        case Params.Null:
          return this.parseNull(from_index, value_type, view);
        case Params.Undefined:
          return this.parseUndefined(from_index, value_type, view);
        case Params.Bool:
          return this.parseBoolean(from_index, value_type, view);
        case Params.Int8:
          return this.parseNumber8(from_index, value_type, view);
        case Params.Int16:
          return this.parseNumber16(from_index, value_type, view);
        case Params.Int32:
          return this.parseNumber32(from_index, value_type, view);
        case Params.Int64:
          return this.parseNumber64(from_index, value_type, view);
        case Params.Uint8:
          return this.parseNumber8(from_index, value_type, view);
        case Params.Uint16:
          return this.parseNumber16(from_index, value_type, view);
        case Params.Uint32:
          return this.parseNumber32(from_index, value_type, view);
        case Params.Uint64:
          return this.parseNumber64(from_index, value_type, view);
        case Params.Int128:
          return this.parseNumber128(from_index, value_type, view);
        case Params.Uint128:
          return this.parseNumber128(from_index, value_type, view);
        case Params.Float32:
          return this.parseFloat32(from_index, value_type, view);
        case Params.Float64:
          return this.parseFloat64(from_index, value_type, view);
        case Params.InternalReference:
          return this.parseInternalPointer(from_index, value_type, view);
        case Params.ExternalReference:
          return this.parseExternalPointer(from_index, value_type, view);
        case Params.Uint8ArrayBuffer:
          return this.parseUint8ArrayBuffer(from_index, value_type, view);
        case Params.Uint16ArrayBuffer:
          return this.parseUint16ArrayBuffer(from_index, value_type, view);
        case Params.Uint32ArrayBuffer:
          return this.parseUint32ArrayBuffer(from_index, value_type, view);
        case Params.Uint64ArrayBuffer:
          return this.parseUint64ArrayBuffer(from_index, value_type, view);
        case Params.Int8ArrayBuffer:
          return this.parseInt8ArrayBuffer(from_index, value_type, view);
        case Params.Int16ArrayBuffer:
          return this.parseInt16ArrayBuffer(from_index, value_type, view);
        case Params.Int32ArrayBuffer:
          return this.parseInt32ArrayBuffer(from_index, value_type, view);
        case Params.Int64ArrayBuffer:
          return this.parseInt64ArrayBuffer(from_index, value_type, view);
        case Params.Float32ArrayBuffer:
          return this.parseFloat32ArrayBuffer(from_index, value_type, view);
        case Params.Float64ArrayBuffer:
          return this.parseFloat64ArrayBuffer(from_index, value_type, view);
        case Params.TypedArraySlice:
          return this.parseTypedArraySlice(from_index, value_type, view);
        case Params.CachedText:
          return this.parseCachedText(
            from_index,
            value_type,
            view,
            text_string_array,
          );
        case Params.Text8:
          return this.parseText8(
            from_index,
            value_type,
            view,
            text_string_array,
          );
        case Params.Text16:
          return this.parseText16(
            from_index,
            value_type,
            view,
            text_string_array,
          );
        default:
          throw new Error(
            `Params type ${value_type} (type=${values_type_str}) is not supported`,
          );
      }
    }

    // cloneBufferArray creates a new DataView for the region of the
    // wasm Memory to save the cost of the copy operation but also be
    // aware this means that area of memory must be quickly consumed to
    // avoid potential corruption or over-written of the data.
    //
    // It's wise to use this when you will immediately consume the contents
    // and generate your derived value else use copyBufferArray instead
    // to get a unique copy of the content.
    cloneBufferArrayAdjusted(from_index, view, adjusterMultiplier) {
      // get the quantized pointer that points us to the location of the data.
      let [moved_by, pointer] = this.parsePtr(from_index, Params.Uint64, view);

      // get the length of the
      let [moved_by_again, pointer_length] = this.parseNumber64(
        moved_by,
        Params.Uint64,
        view,
      );

      const start = Number(pointer);
      const length = Number(pointer_length);
      const end = start + length * adjusterMultiplier;

      // for more efficient usage, as slice copies, we can use:
      const memory = this.operator.get_memory();
      const uint_array = new Uint8Array(memory);
      const slice_view = uint_array.subarray(start, end);

      LOGGER.debug(
        `clonedBufferArray: selecting start=${start}, length=${length}, end=${end} -> slice_view=${slice_view}`,
      );
      LOGGER.debug(
        `View: ${slice_view} -> ${slice_view.length} --> ${slice_view.buffer} -> ${slice_view.buffer.byteLength}`,
      );

      return [moved_by_again, slice_view];
    }

    cloneBufferArray1Byte(from_index, view) {
      return this.cloneBufferArrayAdjusted(
        from_index,
        view,
        Move.MOVE_BY_1_BYTES,
      );
    }

    cloneBufferArray16Bytes(from_index, view) {
      return this.cloneBufferArrayAdjusted(
        from_index,
        view,
        Move.MOVE_BY_16_BYTES,
      );
    }

    cloneBufferArray32Bytes(from_index, view) {
      return this.cloneBufferArrayAdjusted(
        from_index,
        view,
        Move.MOVE_BY_32_BYTES,
      );
    }

    cloneBufferArray64Bytes(from_index, view) {
      return this.cloneBufferArrayAdjusted(
        from_index,
        view,
        Move.MOVE_BY_64_BYTES,
      );
    }

    /// [copyBufferArray] creates a unique copy of the contents of
    // the memory location pointing to the start and length of the
    // expected content.
    copyBufferArrayAdjusted(from_index, view, adjusterMultiplier) {
      // get the quantized pointer that points us to the location of the data.
      let [moved_by, pointer] = this.parsePtr(from_index, Params.Uint64, view);

      // get the length of the
      let [moved_by_again, pointer_length] = this.parseNumber64(
        moved_by,
        Params.Uint64,
        view,
      );

      const start = Number(pointer);
      const length = Number(pointer_length);
      const end = start + length * adjusterMultiplier;

      // copy the memory which does a copy operation and is more costly.
      const memory = this.operator.get_memory();
      const slice = memory.slice(start, end);

      LOGGER.debug(
        `copyBufferArray: selecting start=${start}, length=${length}, end=${end} -> slice=${slice}`,
      );

      return [moved_by_again, slice];
    }

    copyBufferArray1Byte(from_index, view) {
      return this.copyBufferArrayAdjusted(
        from_index,
        view,
        Move.MOVE_BY_1_BYTES,
      );
    }

    copyBufferArray16Bytes(from_index, view) {
      return this.copyBufferArrayAdjusted(
        from_index,
        view,
        Move.MOVE_BY_16_BYTES,
      );
    }

    copyBufferArray32Bytes(from_index, view) {
      return this.copyBufferArrayAdjusted(
        from_index,
        view,
        Move.MOVE_BY_32_BYTES,
      );
    }

    copyBufferArray64Bytes(from_index, view) {
      return this.copyBufferArrayAdjusted(
        from_index,
        view,
        Move.MOVE_BY_64_BYTES,
      );
    }

    parseFloat64ArrayBuffer(from_index, value_type, view) {
      if (value_type != Params.Float64ArrayBuffer) {
        throw new Error(
          `Parameter is not that of Undefined: received ${value_type}`,
        );
      }

      const [moved_by, buffer] = this.copyBufferArray64Bytes(from_index, view);
      return [moved_by, new Float64Array(buffer)];
    }

    parseFloat32ArrayBuffer(from_index, value_type, view) {
      if (value_type != Params.Float32ArrayBuffer) {
        throw new Error(
          `Parameter is not that of Undefined: received ${value_type}`,
        );
      }

      const [moved_by, buffer] = this.copyBufferArray32Bytes(from_index, view);
      LOGGER.debug("Float32Array: ", moved_by, buffer);
      return [moved_by, new Float32Array(buffer)];
    }

    parseInt64ArrayBuffer(from_index, value_type, view) {
      if (value_type != Params.Int64ArrayBuffer) {
        throw new Error(
          `Parameter is not that of Undefined: received ${value_type}`,
        );
      }

      const [moved_by, buffer] = this.copyBufferArray64Bytes(from_index, view);
      return [moved_by, new BigInt64Array(buffer)];
    }

    parseInt32ArrayBuffer(from_index, value_type, view) {
      if (value_type != Params.Int32ArrayBuffer) {
        throw new Error(
          `Parameter is not that of Undefined: received ${value_type}`,
        );
      }

      const [moved_by, buffer] = this.copyBufferArray32Bytes(from_index, view);
      LOGGER.debug(
        `Int32Array moved_by ${moved_by} bytes with buffer: ${buffer} : ${typeof buffer}`,
      );
      return [moved_by, new Int32Array(buffer)];
    }

    parseInt16ArrayBuffer(from_index, value_type, view) {
      if (value_type != Params.Int16ArrayBuffer) {
        throw new Error(
          `Parameter is not that of Undefined: received ${value_type}`,
        );
      }

      const [moved_by, buffer] = this.copyBufferArray16Bytes(from_index, view);
      LOGGER.debug(
        `Int16Array moved_by ${moved_by} bytes with buffer: ${buffer} : ${typeof buffer}`,
      );
      return [moved_by, new Int16Array(buffer)];
    }

    parseInt8ArrayBuffer(from_index, value_type, view) {
      if (value_type != Params.Int8ArrayBuffer) {
        throw new Error(
          `Parameter is not that of Undefined: received ${value_type}`,
        );
      }

      const [moved_by, buffer] = this.copyBufferArray1Byte(from_index, view);
      LOGGER.debug(
        `Int64Array moved_by ${moved_by} bytes with buffer: ${buffer}`,
      );
      return [moved_by, new Int8Array(buffer)];
    }

    parseUint64ArrayBuffer(from_index, value_type, view) {
      if (value_type != Params.Uint64ArrayBuffer) {
        throw new Error(
          `Parameter is not that of Undefined: received ${value_type}`,
        );
      }

      const [moved_by, buffer] = this.copyBufferArray64Bytes(from_index, view);
      return [moved_by, new BigUint64Array(buffer)];
    }

    parseUint32ArrayBuffer(from_index, value_type, view) {
      if (value_type != Params.Uint32ArrayBuffer) {
        throw new Error(
          `Parameter is not that of Undefined: received ${value_type}`,
        );
      }

      const [moved_by, buffer] = this.copyBufferArray32Bytes(from_index, view);
      LOGGER.debug(
        `Uint32Array moved_by ${moved_by} bytes with buffer: ${buffer}`,
      );
      return [moved_by, new Uint32Array(buffer)];
    }

    parseUint16ArrayBuffer(from_index, value_type, view) {
      if (value_type != Params.Uint16ArrayBuffer) {
        throw new Error(
          `Parameter is not that of Undefined: received ${value_type}`,
        );
      }

      const [moved_by, buffer] = this.copyBufferArray16Bytes(from_index, view);
      LOGGER.debug(
        `Uint16Array moved_by ${moved_by} bytes with buffer: ${buffer}`,
      );

      return [moved_by, new Uint16Array(buffer)];
    }

    parseUint8ArrayBuffer(from_index, value_type, view) {
      if (value_type != Params.Uint8ArrayBuffer) {
        throw new Error(
          `Parameter is not that of Params.Uint8Array: received ${value_type}`,
        );
      }

      const [moved_by, buffer] = this.copyBufferArray1Byte(from_index, view);
      LOGGER.debug(
        `Uint8Array moved_by ${moved_by} bytes with buffer: ${buffer}`,
      );
      return [moved_by, new Uint8Array(buffer)];
    }

    // WARNING: This tries to be efficient and avoids copying the contents of the
    // memory location in the wasm memory instance, so ensure to clone the provided
    // data buffer to avoid data corruption if that memory gets overwritten.
    parseTypedArraySlice(from_index, value_type, view) {
      if (value_type != Params.TypedArraySlice) {
        throw new Error(
          `Parameter is not that of Params.Uint8Array: received ${value_type}`,
        );
      }

      const slice_type = view.getUint8(from_index);
      from_index += Move.MOVE_BY_1_BYTES;

      const [moved_by, buffer] = this.cloneBufferArray1Byte(from_index, view);
      LOGGER.debug(
        `TypedArraySlice moved_by ${moved_by} bytes with buffer: ${buffer} and slice_type: ${slice_type}`,
      );

      return [moved_by, new TypedArraySlice(slice_type, buffer)];
    }

    parseNull(from_index, value_type, view) {
      if (value_type != Params.Undefined) {
        throw new Error(
          `Parameter is not that of Undefined: received ${value_type}`,
        );
      }

      return [from_index, null];
    }

    parseUndefined(from_index, value_type, view) {
      if (value_type != Params.Undefined) {
        throw new Error(
          `Parameter is not that of Undefined: received ${value_type}`,
        );
      }

      return [from_index, undefined];
    }

    parseNumber8(from_index, value_type, view) {
      if ([Params.Int8, Params.Uint8].indexOf(value_type) == -1) {
        throw new Error(
          `Parameter is not that of Number8(Int/uint): received ${value_type}`,
        );
      }

      if (value_type == Params.Int8) {
        return [
          from_index + Move.MOVE_BY_1_BYTES,
          view.getInt8(from_index, true),
        ];
      }

      return [
        from_index + Move.MOVE_BY_1_BYTES,
        view.getUint8(from_index, true),
      ];
    }

    parseBoolean(from_index, value_type, view) {
      if (value_type != Params.Bool) {
        throw new Error(
          `Parameter is not that of Boolean: received ${value_type}`,
        );
      }

      const value_int = view.getUint8(from_index, true);
      const value = value_int == 1;

      from_index += Move.MOVE_BY_1_BYTES;

      return [from_index, value];
    }

    parseStringLocation(from_index, view) {
      const str_index = view.getBigUint64(from_index, true);
      from_index += Move.MOVE_BY_64_BYTES;

      const str_length = view.getBigUint64(from_index, true);
      from_index += Move.MOVE_BY_64_BYTES;

      LOGGER.debug(
        `StringLocation::(index=${str_index}, length=${str_length})`,
      );

      return [from_index, [Number(str_index), Number(str_length)]];
    }

    parseStringLocationAsOptimized(from_index, view) {
      const [moved_by, location_index] = this.parseNumber64(
        from_index,
        Params.Uint64,
        view,
      );
      const [moved_by_next, location_length] = this.parseNumber64(
        moved_by,
        Params.Uint64,
        view,
      );
      LOGGER.debug(
        `parseStringLocationAsOptimized::(index=${location_index}, length=${location_length})`,
      );
      return [moved_by_next, [Number(location_index), Number(location_length)]];
    }

    parseText16(from_index, view) {
      if (value_type != Params.Text16) {
        throw new Error(
          `Parameter is not that of Undefined: received ${value_type}`,
        );
      }

      const [moved_by, buffer] = this.cloneBufferArray16Bytes(from_index, view);
      LOGGER.debug(
        `Int16Array moved_by ${moved_by} bytes with buffer: ${buffer} : ${typeof buffer}`,
      );
    }

    parseCachedText(from_index, value_type, view, text_string_array) {
      if (value_type != Params.CachedText) {
        throw new Error(
          `Parameter is not that of CachedText: received ${value_type}`,
        );
      }

      // read out a 64 bit number representing the external reference.
      const [move_by, cache_id] = this.parseNumber64(
        from_index,
        Params.Uint64,
        view,
      );

      const target_text = this.text_cache.get_text(cache_id);
      LOGGER.debug(
        `parseCachedText: cache_id=${cache_id}, text=${target_text}`,
      );

      if (isUndefinedOrNull(target_text)) {
        throw new Error(`Missing Cached text at cache_id: ${cache_id}`);
      }

      return [move_by, target_text];
    }

    parseText8(from_index, value_type, view, text_string_array) {
      if (value_type != Params.Text8) {
        throw new Error(
          `Parameter is not that of Text8: received ${value_type}`,
        );
      }

      let content_location;
      [from_index, content_location] = this.parseStringLocationAsOptimized(
        from_index,
        view,
      );

      const target_text = text_string_array.substr.apply(
        text_string_array,
        content_location,
      );

      LOGGER.debug(
        `parseText8: index=${content_location[0]}, length=${content_location[1]}, text=${target_text}`,
      );

      return [from_index, target_text];
    }

    parseDOMPointer(from_index, value_type, view) {
      if (value_type != Params.InternalReference) {
        throw new Error(
          `Parameter is not that of InternalReference: received ${value_type}`,
        );
      }

      // read out a 64 bit number representing the external reference.
      const [move_by, external_id] = this.parseNumber64(
        from_index,
        Params.Uint64,
        view,
      );

      LOGGER.debug("InternalIdExtraction: ", move_by, external_id);

      return [move_by, new InternalPointer(external_id)];
    }

    parseInternalPointer(from_index, value_type, view) {
      if (value_type != Params.InternalReference) {
        throw new Error(
          `Parameter is not that of InternalReference: received ${value_type}`,
        );
      }

      // read out a 64 bit number representing the external reference.
      const [move_by, external_id] = this.parseNumber64(
        from_index,
        Params.Uint64,
        view,
      );

      LOGGER.debug("InternalIdExtraction: ", move_by, external_id);

      return [move_by, new InternalPointer(external_id)];
    }

    parseExternalPointer(from_index, value_type, view) {
      if (value_type != Params.ExternalReference) {
        throw new Error(
          `Parameter is not that of ExternalReference: received ${value_type}`,
        );
      }

      // read out a 64 bit number representing the external reference.
      const [move_by, external_id] = this.parseNumber64(
        from_index,
        Params.Uint64,
        view,
      );

      LOGGER.debug("ExternalIdExtraction: ", move_by, external_id);

      return [move_by, new ExternalPointer(external_id)];
    }

    parsePtr(from_index, value_type, view) {
      const optimization_type = view.getUint8(from_index, true);
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
          return [
            from_index + Move.MOVE_BY_1_BYTES,
            view.getUint8(from_index, true),
          ];
        case TypeOptimization.QuantizedPtrAsU16:
          return [
            from_index + Move.MOVE_BY_16_BYTES,
            view.getUint16(from_index, true),
          ];
        case TypeOptimization.QuantizedPtrAsU32:
          return [
            from_index + Move.MOVE_BY_32_BYTES,
            view.getUint32(from_index, true),
          ];
        case TypeOptimization.QuantizedPtrAsU64:
          return [
            from_index + Move.MOVE_BY_64_BYTES,
            view.getBigUint64(from_index),
          ];
      }
    }

    parseNumber16(from_index, value_type, view) {
      if ([Params.Int16, Params.Uint16].indexOf(value_type) == -1) {
        throw new Error(
          `Parameter is not that of Number16(Int/Uint): received ${value_type}`,
        );
      }

      const optimization_type = view.getUint8(from_index, true);
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
          return [
            from_index + Move.MOVE_BY_16_BYTES,
            view.getUint16(from_index, true),
          ];
        case TypeOptimization.QuantizedInt16AsI8:
          return [
            from_index + Move.MOVE_BY_1_BYTES,
            view.getInt8(from_index, true),
          ];
        case TypeOptimization.QuantizedUint16AsU8:
          return [
            from_index + Move.MOVE_BY_1_BYTES,
            view.getUint8(from_index, true),
          ];
      }
    }

    parseNumber64(from_index, value_type, view) {
      if ([Params.Int64, Params.Uint64].indexOf(value_type) == -1) {
        throw new Error(
          `Parameter is not that of Number64(Int/Uint): received ${value_type}`,
        );
      }

      const optimization_type = view.getUint8(from_index, true);
      if (!(optimization_type in TypeOptimization.__INVERSE__)) {
        throw new Error(
          `OptimizationType ${optimization_type} is not known for value_type: ${value_type}`,
        );
      }

      LOGGER.debug(
        "parseNumber64 saw optimization: ",
        optimization_type,
        TypeOptimization.__INVERSE__[optimization_type],
        Params.__INVERSE__[value_type],
      );

      from_index += Move.MOVE_BY_1_BYTES;

      switch (optimization_type) {
        case TypeOptimization.None:
          if (value_type == Params.Int64) {
            return [
              from_index + Move.MOVE_BY_64_BYTES,
              view.getBigInt64(from_index, true),
            ];
          }
          return [
            from_index + Move.MOVE_BY_64_BYTES,
            view.getBigUint64(from_index, true),
          ];
        case TypeOptimization.QuantizedInt64AsI8:
          return [
            from_index + Move.MOVE_BY_1_BYTES,
            view.getInt8(from_index, true),
          ];
        case TypeOptimization.QuantizedUint64AsU8:
          return [
            from_index + Move.MOVE_BY_1_BYTES,
            view.getUint8(from_index, true),
          ];
        case TypeOptimization.QuantizedUint64AsU16:
          return [
            from_index + Move.MOVE_BY_16_BYTES,
            view.getUint16(from_index, true),
          ];
        case TypeOptimization.QuantizedInt64AsI16:
          return [
            from_index + Move.MOVE_BY_16_BYTES,
            view.getInt16(from_index, true),
          ];
        case TypeOptimization.QuantizedInt64AsI32:
          return [from_index + Move.MOVE_BY_32_BYTES, view.geInt32(from_index)];
        case TypeOptimization.QuantizedUint64AsU32:
          return [
            from_index + Move.MOVE_BY_32_BYTES,
            view.getUint32(from_index, true),
          ];
      }
    }

    parseNumber32(from_index, value_type, view) {
      if ([Params.Int32, Params.Uint32].indexOf(value_type) == -1) {
        throw new Error(
          `Parameter is not that of Number64(Int/Uint): received ${value_type}`,
        );
      }

      const optimization_type = view.getUint8(from_index, true);
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
          return [
            from_index + Move.MOVE_BY_32_BYTES,
            view.getUint32(from_index, true),
          ];
        case TypeOptimization.QuantizedInt32AsI8:
          return [
            from_index + Move.MOVE_BY_1_BYTES,
            view.getInt8(from_index, true),
          ];
        case TypeOptimization.QuantizedUint32AsU8:
          return [
            from_index + Move.MOVE_BY_1_BYTES,
            view.getUint8(from_index, true),
          ];
        case TypeOptimization.QuantizedInt32AsI16:
          return [
            from_index + Move.MOVE_BY_16_BYTES,
            view.getInt16(from_index, true),
          ];
        case TypeOptimization.QuantizedUint32AsU16:
          return [
            from_index + Move.MOVE_BY_16_BYTES,
            view.getUint16(from_index, true),
          ];
      }
    }

    parseFloat32(from_index, value_type, view) {
      if (value_type != Params.Float32) {
        throw new Error(
          `Parameter is not that of Float32: received ${value_type}`,
        );
      }

      return [
        from_index + Move.MOVE_BY_32_BYTES,
        view.getFloat32(from_index, true),
      ];
    }

    parseFloat64(from_index, value_type, view) {
      if (value_type != Params.Float64) {
        throw new Error(
          `Parameter is not that of Float64: received ${value_type}`,
        );
      }

      const optimization_type = view.getUint8(from_index, true);
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
            view.getFloat64(from_index, true),
          ];
        case TypeOptimization.QuantizedF64AsF32:
          return [
            from_index + Move.MOVE_BY_32_BYTES,
            view.getFloat32(from_index, true),
          ];
      }
    }

    parseNumber128(from_index, value_type, view) {
      const optimization_type = view.getUint8(from_index, true);
      if (!(optimization_type in TypeOptimization.__INVERSE__)) {
        throw new Error(
          `OptimizationType ${optimization_type} is not known for value_type: ${value_type}`,
        );
      }

      from_index += Move.MOVE_BY_1_BYTES;

      switch (optimization_type) {
        case TypeOptimization.None:
          if (value_type == Params.Int128) {
            const value_msb = view.getBigInt64(from_index, true);
            from_index += Move.MOVE_BY_64_BYTES;

            const value_lsb = view.getBigInt64(from_index, true);
            from_index += Move.MOVE_BY_64_BYTES;

            let sent_value = value_msb << BigInt(64);
            sent_value = sent_value | value_lsb;

            return [from_index, sent_value];
          }

          const value_msb = view.getBigUint64(from_index, true);
          from_index += Move.MOVE_BY_64_BYTES;

          const value_lsb = view.getBigUint64(from_index, true);
          from_index += Move.MOVE_BY_64_BYTES;

          let sent_value = value_msb << BigInt(64);
          sent_value = sent_value | value_lsb;

          return [from_index, sent_value];
        case TypeOptimization.QuantizedInt128AsI8:
          return [
            from_index + Move.MOVE_BY_1_BYTES,
            view.getInt8(from_index, true),
          ];
        case TypeOptimization.QuantizedUint128AsU8:
          return [
            from_index + Move.MOVE_BY_1_BYTES,
            view.getUint8(from_index, true),
          ];
        case TypeOptimization.QuantizedInt128AsI16:
          return [
            from_index + Move.MOVE_BY_16_BYTES,
            view.getInt16(from_index, true),
          ];
        case TypeOptimization.QuantizedUint128AsU16:
          return [
            from_index + Move.MOVE_BY_16_BYTES,
            view.getUint16(from_index, true),
          ];
        case TypeOptimization.QuantizedInt128AsI32:
          return [
            from_index + Move.MOVE_BY_32_BYTES,
            view.getInt32(from_index, true),
          ];
        case TypeOptimization.QuantizedUint128AsU32:
          return [
            from_index + Move.MOVE_BY_32_BYTES,
            view.getUint32(from_index, true),
          ];
        case TypeOptimization.QuantizedInt128AsI64:
          return [
            from_index + Move.MOVE_BY_64_BYTES,
            view.getInt64(from_index, true),
          ];
        case TypeOptimization.QuantizedUint128AsU64:
          return [
            from_index + Move.MOVE_BY_64_BYTES,
            view.getUint64(from_index, true),
          ];
      }
    }
  }

  class BatchOperation {
    constructor(operation_id, operation_handler) {
      this.operation_id = operation_id;
      this.handler = operation_handler;
    }

    is_operation(operation_id) {
      return this.operation_id == operation_id;
    }

    perform(batch_instructions, operations_id, read_index, operations, texts) {
      return this.handler(
        batch_instructions,
        operations_id,
        read_index,
        operations,
        texts,
      );
    }
  }

  const MAKE_FUNCTION = new BatchOperation(
    Operations.MakeFunction,
    (instance, operation_id, read_index, operations, texts) => {
      if (operation_id != Operations.MakeFunction) {
        throw new Error(
          `Argument should be Operation.MakeFunction instead got: ${operation_id}`,
        );
      }

      const next_value_type = operations.getUint8(read_index, true);
      read_index += Move.MOVE_BY_1_BYTES;

      // read the external pointer we want registered
      let [move_by, external_id] = instance.parameter_v2.parseExternalPointer(
        read_index,
        next_value_type,
        operations,
      );

      LOGGER.debug("ExternalPointer: ", read_index, move_by, external_id);

      if (operations.getUint8(move_by) != Params.Text8) {
        throw new Error(
          `Argument should be Params.Text8 instead got: ${operations.getUint8(
            move_by,
          )}`,
        );
      }

      move_by += Move.MOVE_BY_1_BYTES;

      let [move_by_next, string_location] =
        instance.parameter_v2.parseStringLocation(move_by, operations);

      if (operations.getUint8(move_by_next) != Operations.End) {
        throw new Error(
          `Operation should be Params.End instead got: ${operations.getUint8(
            move_by_next,
          )}`,
        );
      }

      move_by_next += Move.MOVE_BY_1_BYTES;

      // read the location of the text.
      const function_definition = texts.substr.apply(texts, string_location);

      LOGGER.debug(
        "FunctionDefinition: ",
        move_by_next,
        string_location,
        function_definition,
      );

      const register_function = (instance) => {
        // define the function as a callable.
        const function_invoker = Function(
          `"use strict"; return(${function_definition})`,
        )();

        LOGGER.debug(
          "Register function in heap: ",
          external_id,
          external_id.value,
          function_invoker,
        );

        // update the reference to now be the reference
        // for calling the function.
        instance.function_heap.update(external_id.value, function_invoker);
        return external_id;
      };

      return [move_by_next, register_function];
    },
  );

  const INVOKE_NO_RETURN = new BatchOperation(
    Operations.InvokeNoReturnFunction,
    (instance, operation_id, moved_by, operations, texts) => {
      if (operation_id != Operations.InvokeNoReturnFunction) {
        throw new Error(
          `Argument should be Operation.InvokeNoReturnFunction instead got: ${operation_id}`,
        );
      }

      return instance.parse_and_invoke(
        moved_by,
        operations,
        texts,
        function (result) {
          LOGGER.debug("parse_invoke_no_return_function result: ", result);
        },
      );
    },
  );

  // Add comments to this class. Keep your thinking short and simple, do not overthink. AI!
  class Reply {
    constructor(memory_operator, text_codec, text_cache) {
      if (!(memory_operator instanceof MemoryOperator)) {
        throw new Error("Must be instance of MemoryOperator");
      }
      if (!(text_codec instanceof TextCodec)) {
        throw new Error("Must be instance of TextCodec");
      }
      if (!(text_cache instanceof SimpleStringCache)) {
        throw new Error("Must be instance of SimpleStringCache");
      }

      this.texts = text_codec;
      this.text_cache = text_cache;
      this.operator = memory_operator;
      this.module = memory_operator.get_module();

      this.reply_types = {};
      this.reply_types[ReturnTypeId.Bool] = Reply.encodeBool;
      this.reply_types[ReturnTypeId.Uint8] = Reply.encodeUint8;
      this.reply_types[ReturnTypeId.Uint16] = Reply.encodeInt16;
      this.reply_types[ReturnTypeId.Uint32] = Reply.encodeInt32;
      this.reply_types[ReturnTypeId.Uint64] = Reply.encodeBigInt64;
      this.reply_types[ReturnTypeId.Int8] = Reply.encodeInt8;
      this.reply_types[ReturnTypeId.Int16] = Reply.encodeInt16;
      this.reply_types[ReturnTypeId.Int32] = Reply.encodeInt32;
      this.reply_types[ReturnTypeId.Int64] = Reply.encodeBigInt64;
      this.reply_types[ReturnTypeId.Float32] = Reply.encodeFloat32;
      this.reply_types[ReturnTypeId.Float64] = Reply.encodeFloat64;
      this.reply_types[ReturnTypeId.Object] = Reply.encodeObject;
      this.reply_types[ReturnTypeId.DOMObject] = Reply.encodeDOMObject;
      this.reply_types[ReturnTypeId.ExternalReference] =
        Reply.encodeExternalReference;
      this.reply_types[ReturnTypeId.InternalReference] =
        Reply.encodeInternalReference;
      this.reply_types[ReturnTypeId.MemorySlice] = Reply.encodeMemorySlice;
      this.reply_types[ReturnTypeId.Uint8ArrayBuffer] =
        Reply.encodeUint8ArrayBuffer;
      this.reply_types[ReturnTypeId.Uint16ArrayBuffer] =
        Reply.encodeUint16ArrayBuffer;
      this.reply_types[ReturnTypeId.Uint32ArrayBuffer] =
        Reply.encodeUint32ArrayBuffer;
      this.reply_types[ReturnTypeId.Uint64ArrayBuffer] =
        Reply.encodeUint64ArrayBuffer;
      this.reply_types[ReturnTypeId.Int8ArrayBuffer] =
        Reply.encodeInt8ArrayBuffer;
      this.reply_types[ReturnTypeId.Int16ArrayBuffer] =
        Reply.encodeInt16ArrayBuffer;
      this.reply_types[ReturnTypeId.Int32ArrayBuffer] =
        Reply.encodeInt32ArrayBuffer;
      this.reply_types[ReturnTypeId.Int64ArrayBuffer] =
        Reply.encodeInt64ArrayBuffer;
      this.reply_types[ReturnTypeId.Float32ArrayBuffer] =
        Reply.encodeFloat32ArrayBuffer;
      this.reply_types[ReturnTypeId.Float64ArrayBuffer] =
        Reply.encodeFloat64ArrayBuffer;
    }

    send(internal_pointer, values) {
      // send these to wasm targeting the callback handler
      LOGGER.debug(
        `Sending values: ${values} to callback pointer: ${internal_pointer}`,
      );
    }

    encode(values) {
      if (!(values instanceof Array)) {
        throw new Error("values must be a list/array");
      }

      // create our content byte buffer
      const initial_bytes_size = 80;
      const content = new ArrayBuffer(initial_bytes_size);

      // create our view for setting up values correctly.
      const view = new DataView(content);

      let offset = 0;

      view.setUint8(offset, ReturnValueMarker.Begin);
      offset += Move.MOVE_BY_1_BYTES;

      for (let index = 0; i < values.length; i++) {
        const value = values[index];
        if (isUndefinedOrNull(value.type)) {
          throw new Error("Reply types must have a type id");
        }

        const encoder = this.getEncoder(value);
        offset = encoder(offset, value, view);
      }

      view.setUint8(offset, ReturnValueMarker.End);
      offset += Move.MOVE_BY_1_BYTES;

      LOGGER.debug(
        `Finished encoding data with initial_size=${initial_bytes_size} and encoded_size=${offset}`,
      );
      if (offset < initial_bytes_size) {
        content.resize(offset);
      }

      return content;
    }

    getEncoder(directive) {
      if (!(directive.type in ReturnTypeId.__INVERSE__)) {
        throw new Error(`Unknown Reply encode type id: ${directive.type}`);
      }
      return ReturnTypeId.__INVERSE__[directive.type];
    }

    static encodeFloat64ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Float64) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      const content = Uint8Array(directive.value.buffer);
      const allocation_id = this.operations.writeUint8Buffer(content);
      view.getBigUint64(offset, allocation_id);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeFloat32ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Float32) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      const content = Uint8Array(directive.value.buffer);
      const allocation_id = this.operations.writeUint8Buffer(content);
      view.getBigUint64(offset, allocation_id);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeInt64ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Int64) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      const content = Uint8Array(directive.value.buffer);
      const allocation_id = this.operations.writeUint8Buffer(content);
      view.getBigUint64(offset, allocation_id);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeInt32ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Int32) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      const content = Uint8Array(directive.value.buffer);
      const allocation_id = this.operations.writeUint8Buffer(content);
      view.getBigUint64(offset, allocation_id);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeInt16ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Int16) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      const content = Uint8Array(directive.value.buffer);
      const allocation_id = this.operations.writeUint8Buffer(content);
      view.getBigUint64(offset, allocation_id);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeInt8ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Int8) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      const content = Uint8Array(directive.value.buffer);
      const allocation_id = this.operations.writeUint8Buffer(content);
      view.getBigUint64(offset, allocation_id);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeUint64ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Uint64) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      const content = Uint8Array(directive.value.buffer);
      const allocation_id = this.operations.writeUint8Buffer(content);
      view.getBigUint64(offset, allocation_id);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeUint32ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Uint32) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      const content = Uint8Array(directive.value.buffer);
      const allocation_id = this.operations.writeUint8Buffer(content);
      view.getBigUint64(offset, allocation_id);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeUint16ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Uint16) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      const content = Uint8Array(directive.value.buffer);
      const allocation_id = this.operations.writeUint8Buffer(content);
      view.getBigUint64(offset, allocation_id);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeUint8ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Uint8) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      const allocation_id = this.operations.writeUint8Buffer(directive.value);
      view.getBigUint64(offset, allocation_id);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeInternalReference(offset, directive, view) {
      if (directive.type != ReturnTypeId.InternalReference) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      view.setBigUint64(offset, directive.value);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeObject(offset, directive, view) {
      if (directive.type != ReturnTypeId.Object) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      view.setBigUint64(offset, directive.value);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeDOMObject(offset, directive, view) {
      if (directive.type != ReturnTypeId.DOMObject) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      view.setBigUint64(offset, directive.value);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeExternalReference(offset, directive, view) {
      if (directive.type != ReturnTypeId.ExternalReference) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      view.setBigUint64(offset, directive.value);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeMemorySlice(offset, directive, view) {
      if (directive.type != ReturnTypeId.MemorySlice) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      view.setBigUint64(offset, directive.value);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeInt128(offset, directive, view) {
      if (directive.type != ReturnTypeId.Int128) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      view.setBigInt64(offset, directive.value.value_msb);
      offset += Move.MOVE_BY_64_BYTES;

      view.setBigInt64(offset, directive.value.value_lsb);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeUint128(offset, directive, view) {
      if (directive.type != ReturnTypeId.Uint128) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      view.setBigUint64(offset, directive.value.value_msb);
      offset += Move.MOVE_BY_64_BYTES;

      view.setBigUint64(offset, directive.value.value_lsb);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeFloat32(offset, directive, view) {
      if (directive.type != ReturnTypeId.Float32) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      view.setFloat32(offset, directive.value);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    // implement encoding of a float64 using the same as the encodeBool method
    static encodeFloat64(offset, directive, view) {
      if (directive.type != ReturnTypeId.Float64) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      view.setFloat64(offset, directive.value);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeBigUint64(offset, directive, view) {
      if (directive.type != ReturnTypeId.Uint64) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      view.setBigUint64(offset, directive.value);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeUint32(offset, directive, view) {
      if (directive.type != ReturnTypeId.Uint32) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      view.setUint32(offset, directive.value);
      offset += Move.MOVE_BY_32_BYTES;

      return offset;
    }

    static encodeUint16(offset, directive, view) {
      if (directive.type != ReturnTypeId.Uint16) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      view.setUint16(offset, directive.value);
      offset += Move.MOVE_BY_16_BYTES;

      return offset;
    }

    static encodeUint8(offset, directive, view) {
      if (directive.type != ReturnTypeId.Bool) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      view.setUint8(offset, directive.value);
      offset += Move.MOVE_BY_1_BYTES;

      return offset;
    }

    static encodeBigInt64(offset, directive, view) {
      if (directive.type != ReturnTypeId.Bool) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      view.setUint8(offset, directive.value);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeInt32(offset, directive, view) {
      if (directive.type != ReturnTypeId.Int32) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      view.setInt32(offset, directive.value);
      offset += Move.MOVE_BY_32_BYTES;

      return offset;
    }

    static encodeInt16(offset, directive, view) {
      if (directive.type != ReturnTypeId.Int16) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      view.setInt16(offset, directive.value);
      offset += Move.MOVE_BY_16_BYTES;

      return offset;
    }

    static encodeInt8(offset, directive, view) {
      if (directive.type != ReturnTypeId.Int8) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      view.setInt8(offset, directive.value);
      offset += Move.MOVE_BY_1_BYTES;

      return offset;
    }

    static encodeText8(offset, directive, view) {
      if (directive.type != ReturnTypeId.Text8) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      const allocation_id = this.texts.writeUTF8ToMemory(directive.value);
      view.getBigUint64(offset, allocation_id);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    static encodeBool(offset, directive, view) {
      if (directive.type != ReturnTypeId.Bool) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type);
      offset += Move.MOVE_BY_1_BYTES;

      view.setUint8(offset, directive.value == true ? 1 : 0);
      offset += Move.MOVE_BY_1_BYTES;

      return offset;
    }

    static asFloat64Array(value) {
      if (value instanceof Float64Array) {
        throw new Error("Value must be Float64Array");
      }
      return Reply.asValue(ReturnTypeId.Float64ArrayBuffer, value);
    }

    static asFloat32Array(value) {
      if (value instanceof Float32Array) {
        throw new Error("Value must be Float32Array");
      }
      return Reply.asValue(ReturnTypeId.Float32ArrayBuffer, value);
    }

    static asInt64Array(value) {
      if (value instanceof Int64Array) {
        throw new Error("Value must be Int64Array");
      }
      return Reply.asValue(ReturnTypeId.Int64ArrayBuffer, value);
    }

    static asInt32Array(value) {
      if (value instanceof Int32Array) {
        throw new Error("Value must be Int32Array");
      }
      return Reply.asValue(ReturnTypeId.Int32ArrayBuffer, value);
    }

    static asInt16Array(value) {
      if (value instanceof Int16Array) {
        throw new Error("Value must be Int16Array");
      }
      return Reply.asValue(ReturnTypeId.Int16ArrayBuffer, value);
    }

    static asInt8Array(value) {
      if (value instanceof Int8Array) {
        throw new Error("Value must be Int8Array");
      }
      return Reply.asValue(ReturnTypeId.Int8ArrayBuffer, value);
    }

    static asUint64Array(value) {
      if (value instanceof Uint64Array) {
        throw new Error("Value must be Uint64Array");
      }
      return Reply.asValue(ReturnTypeId.Uint64ArrayBuffer, value);
    }

    static asUint32Array(value) {
      if (value instanceof Uint32Array) {
        throw new Error("Value must be Uint32Array");
      }
      return Reply.asValue(ReturnTypeId.Uint32ArrayBuffer, value);
    }

    static asUint16Array(value) {
      if (value instanceof Uint16Array) {
        throw new Error("Value must be Uint16Array");
      }
      return Reply.asValue(ReturnTypeId.Uint16ArrayBuffer, value);
    }

    static asUint8Array(value) {
      if (value instanceof Uint8Array) {
        throw new Error("Value must be Uint8Array");
      }
      return Reply.asValue(ReturnTypeId.Uint8ArrayBuffer, value);
    }

    static asMemorySlice(value) {
      if (typeof value !== "bigint") {
        throw new Error("Value must be bigint");
      }
      return Reply.asValue(ReturnTypeId.MemorySlice, value);
    }

    static asInternalReference(value) {
      if (value instanceof InternalPointer && typeof value !== "bigint") {
        throw new Error("Value must be bigint/InternalPointer");
      }
      if (value instanceof InternalPointer) {
        return Reply.asValue(ReturnTypeId.InternalReference, value.value);
      }
      return Reply.asValue(ReturnTypeId.InternalReference, value);
    }

    static asExternalReference(value) {
      if (value instanceof ExternalPointer && typeof value !== "bigint") {
        throw new Error("Value must be bigint/ExternalPointer");
      }
      if (value instanceof ExternalPointer) {
        return Reply.asValue(ReturnTypeId.ExternalReference, value.value);
      }
      return Reply.asValue(ReturnTypeId.ExternalReference, value);
    }

    static asObject(value) {
      if (value instanceof ExternalPointer && typeof value !== "bigint") {
        throw new Error("Value must be bigint/ExternalPointer");
      }
      if (value instanceof ExternalPointer) {
        return Reply.asValue(ReturnTypeId.Object, value.value);
      }
      return Reply.asValue(ReturnTypeId.Object, value);
    }

    static asDOMObject(value) {
      if (value instanceof ExternalPointer && typeof value !== "bigint") {
        throw new Error("Value must be bigint/ExternalPointer");
      }
      if (value instanceof ExternalPointer) {
        return Reply.asValue(ReturnTypeId.DOMObject, value.value);
      }
      return Reply.asValue(ReturnTypeId.DOMObject, value);
    }

    static asMemorySlice(value) {
      if (typeof value !== "bigint") {
        throw new Error("Value must be bigint");
      }
      return Reply.asValue(ReturnTypeId.MemorySlice, value);
    }

    static asFloat64(value) {
      if (typeof value !== "number") {
        throw new Error("Value must be float64");
      }
      return Reply.asValue(ReturnTypeId.Float64, value);
    }

    static asFloat32(value) {
      if (typeof value !== "number") {
        throw new Error("Value must be float32");
      }
      return Reply.asValue(ReturnTypeId.Float32, value);
    }

    static asInt128(value_lsb, value_msb) {
      if (typeof value !== "bigint") {
        throw new Error("Value must be bigint for 128bit number");
      }
      return Reply.asValue(ReturnTypeId.Int128, {
        value_lsb,
        value_msb,
      });
    }

    static asUint128(value_lsb, value_msb) {
      if (typeof value !== "bigint") {
        throw new Error("Value must be bigint for 128bit number");
      }
      return Reply.asValue(ReturnTypeId.Uint128, {
        value_lsb,
        value_msb,
      });
    }

    static asUint64(value) {
      if (typeof value !== "number" && typeof value !== "bigint") {
        throw new Error("Value must be int64/bigint");
      }
      return Reply.asValue(ReturnTypeId.Uint64, value);
    }

    static asUint32(value) {
      if (typeof value !== "number") {
        throw new Error("Value must be int32");
      }
      return Reply.asValue(ReturnTypeId.Uint32, value);
    }

    static asUint16(value) {
      if (typeof value !== "number") {
        throw new Error("Value must be int16");
      }
      return Reply.asValue(ReturnTypeId.Uint16, value);
    }

    static asUint8(value) {
      if (typeof value !== "number") {
        throw new Error("Value must be int8");
      }
      return Reply.asValue(ReturnTypeId.Uint8, value);
    }

    static asInt64(value) {
      if (typeof value !== "number") {
        throw new Error("Value must be int64");
      }
      return Reply.asValue(ReturnTypeId.Int64, value);
    }

    static asInt32(value) {
      if (typeof value !== "number") {
        throw new Error("Value must be int32");
      }
      return Reply.asValue(ReturnTypeId.Int32, value);
    }

    static asInt16(value) {
      if (typeof value !== "number") {
        throw new Error("Value must be int16");
      }
      return Reply.asValue(ReturnTypeId.Int16, value);
    }

    static asInt8(value) {
      if (typeof value !== "number") {
        throw new Error("Value must be int8");
      }
      return Reply.asValue(ReturnTypeId.Int8, value);
    }

    static asText8(value) {
      if (typeof value !== "string") {
        throw new Error("Value must be string");
      }
      return Reply.asValue(ReturnTypeId.Text8, value);
    }

    static asBool(value) {
      if (typeof value !== "boolean") {
        throw new Error("Value must be bool/boolean");
      }
      return Reply.asValue(ReturnTypeId.Bool, value);
    }

    // asValue provides a clear indicator of what type the giving return value is
    // which provides an important metadata for encoding.
    static asValue(return_type, value) {
      if (!(return_type in ReturnTypeId.__INVERSE__)) {
        throw new Error(
          `ReturnValueTypes ${return_type} is not known for value_type: ${return_type}`,
        );
      }

      return { type: return_type, value };
    }
  }

  class BatchInstructions {
    constructor(memory_operator, text_codec, text_cache) {
      if (!(memory_operator instanceof MemoryOperator)) {
        throw new Error("Must be instance of MemoryOperator");
      }
      if (!(text_codec instanceof TextCodec)) {
        throw new Error("Must be instance of TextCodec");
      }
      if (!(text_cache instanceof SimpleStringCache)) {
        throw new Error("Must be instance of SimpleStringCache");
      }

      this.texts = text_codec;
      this.text_cache = text_cache;
      this.operator = memory_operator;
      this.module = memory_operator.get_module();

      // parser for return hints.
      this.returns = ReturnHintParser();

      // v2 function parameters handling
      this.parameter_v2 = new ParameterParserV2(
        this.operator,
        this.texts,
        this.text_cache,
      );

      this.operations = [MAKE_FUNCTION, INVOKE_NO_RETURN];
    }

    register_operation_handler(operation) {
      if (!(operation instanceof BatchOperation)) {
        throw new Error(`operation: ${operation} should be a BatchOperation`);
      }
      this.operations.push(operation);
    }

    parse_one_batch(read_index, operations, texts) {
      const operation_id = operations.getUint8(read_index);
      const operations_name = Operations.__INVERSE__[operation_id];
      LOGGER.debug(
        `Received operation id=${operation_id}, name=${operations_name} at read_index: ${read_index}`,
        operations,
      );

      read_index += Move.MOVE_BY_1_BYTES;

      LOGGER.debug(`Registered Operations: ${this.operations.length}`);
      for (let index = 0; index < this.operations.length; index++) {
        const operation = this.operations[index];
        LOGGER.debug(
          `Checking for operation=${operation_id} against ${operation} at index=${index}`,
        );
        if (!operation.is_operation(operation_id)) continue;

        return operation.perform(
          this,
          operation_id,
          read_index,
          operations,
          texts,
        );
      }

      throw new Error(`operation: ${operation_id} could not be handled`);
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

      const text_buffer = this.operator.readUint8Buffer(
        Number(text_pointer),
        Number(text_length),
      );

      const text_utf8 = this.texts.readUTF8FromView(text_buffer);

      LOGGER.debug(
        "BatchInstructions::parse_instructions -> ",
        "\n",
        "ops:\n",
        operations_buffer,
        "\n",
        "text:\n",
        text_buffer,
        "\n",
        "text_decoded:\n",
        text_utf8,
      );

      let moved_by = 0;
      const batches = [];

      const operations_view = new DataView(operations_buffer);

      const starter_id = operations_view.getUint8(moved_by);
      if (starter_id != Operations.Begin) {
        throw new Error(
          `Argument did not end with Operation.Begin instead got: ${starter_id}`,
        );
      }

      moved_by += Move.MOVE_BY_1_BYTES;

      for (let i = 0; i < MAX_ITERATION; i++) {
        let batch = null;
        [moved_by, batch] = this.parse_one_batch(
          moved_by,
          operations_view,
          text_utf8,
        );

        batches.push(batch);

        const stop_op = operations_view.getUint8(moved_by);
        if (stop_op == Operations.Stop) {
          LOGGER.debug("Found Operations.Stop identification");
          break;
        }
      }

      LOGGER.debug("Extracted batches: ", batches);
      return batches;
    }

    parse_and_invoke(moved_by, operations, texts, result_callback) {
      const next_value_type = operations.getUint8(moved_by, true);
      moved_by += Move.MOVE_BY_1_BYTES;

      // read the external pointer we want registered
      let external_id = null;
      [moved_by, external_id] = this.parameter_v2.parseExternalPointer(
        moved_by,
        next_value_type,
        operations,
      );

      LOGGER.debug(
        "ExternalPointer: ",
        external_id,
        " with now index: ",
        moved_by,
      );

      const next_token = operations.getUint8(moved_by, true);
      if (next_token != ArgumentOperations.Start) {
        const type_name =
          ArgumentOperations.__INVERSE__[ArgumentOperations.Start];
        throw new Error(
          `Next token should be (id=${ArgumentOperations.Start}, name=${type_name}) instead got: ${next_token}`,
        );
      }

      moved_by += Move.MOVE_BY_1_BYTES;

      let parameters;
      [moved_by, parameters] = this.parameter_v2.parseParams(
        moved_by,
        operations,
        texts,
      );

      if (operations.getUint8(moved_by) != Operations.End) {
        throw new Error(
          `Operation should be Params.End instead got: ${operations.getUint8(
            moved_by_next,
          )}`,
        );
      }

      moved_by += Move.MOVE_BY_1_BYTES;

      LOGGER.debug("Params: ", moved_by, external_id, parameters);
      const callbable = (instance) => {
        const callable = instance.function_heap.get(external_id.value);
        LOGGER.debug(
          `Retrieved callable: ${callbable} from external_id=${external_id}`,
        );

        const result = callable.apply(instance, parameters);
        if (!isUndefinedOrNull(result)) {
          result_callback(result);
        }
      };

      return [moved_by, callbable];
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

      // string_cache
      this.string_cache = null;

      // function parameters
      this.parameter_v1 = null;

      // text decoders and encoder handler
      this.texts = null;

      // memory operation handler
      this.operator = null;

      // batch instruction parser
      this.batch_parser = null;

      // reply instruction parser
      this.reply_parser = null;
    }

    init(wasm_module) {
      this.module = wasm_module;

      // DOM object heap for registering DOM objects.
      this.dom_heap = new DOMArena();

      // string cache provides a clear cache for registering reusable.
      // dynamic strings that can will be re-used by we can definitely
      // save on the overall conversion by caching them for re-use on both
      // sides, similar to how wasm-bindgen supports interning strings
      //
      // See https://github.com/rustwasm/wasm-bindgen/blob/f28cfc26fe28f5c87b32387415aedc52eea14cb8/src/cache/intern.rs
      this.string_cache = new SimpleStringCache();

      // object heap for registering generated or borrowed objects.
      this.object_heap = new ArenaAllocator();

      // function heap for registering function objects.
      this.function_heap = new ArenaAllocator();

      // memory operations.
      this.operator = new MemoryOperator(this.module);

      // text codec for text handling (encoding & decoding)
      this.texts = new TextCodec(this.operator);

      // v1 function parameters handling
      this.parameter_v1 = new ParameterParserV1(
        this.operator,
        this.texts,
        this.string_cache,
      );

      this.reply_parser = new Reply(
        this.operator,
        this.texts,
        this.string_cache,
      );

      // v2 batch instruction handling
      this.batch_parser = new BatchInstructions(
        this.operator,
        this.texts,
        this.string_cache,
      );
    }

    get web_abi() {
      return {
        // aborting host call/execution, basicalling panicing on the host.
        host_abort: this.abort.bind(this),

        // extern references
        //
        // external reference/pointer bindings
        //
        // 1. allocate pointer for dom node/object.
        dom_allocate_external_pointer:
          this.dom_allocate_external_pointer.bind(this),

        // 2. allocate pointer for general js object.
        object_allocate_external_pointer:
          this.object_allocate_external_pointer.bind(this),

        // 2. allocate pointer for function
        function_allocate_external_pointer:
          this.function_allocate_external_pointer.bind(this),

        // external reference/pointer de-registration
        //
        // 1. Drop string external reference
        host_string_cache_drop_external_pointer:
          this.string_cache_drop_external_pointer.bind(this),

        // 2. Drop function external reference
        host_function_drop_external_pointer:
          this.function_drop_external_pointer.bind(this),

        // 3. Drop dom external reference
        host_dom_drop_external_pointer:
          this.dom_drop_external_pointer.bind(this),

        // 4. Drop object external reference
        host_object_drop_external_pointer:
          this.object_drop_external_pointer.bind(this),

        // Batch bindings
        //
        host_batch_apply: this.apply_instructions.bind(this),

        // string caching and interning
        host_cache_string: this.host_cache_string.bind(this),

        // registration binding
        host_register_function: this.host_register_function.bind(this),

        // invocation bindings
        //
        // 1. Generic Binding
        host_invoke_function: this.host_invoke_function.bind(this),

        // 2. Callback Binding
        host_invoke_callback_function:
          this.host_invoke_callback_function.bind(this),

        // host_invoke_function_as_dom:
        //   this.host_invoke_function_as_dom.bind(this),
        // host_invoke_function_as_object:
        //   this.host_invoke_function_as_object.bind(this),

        // 3. Specific Bindings
        host_invoke_function_as_bool:
          this.host_invoke_function_as_bool.bind(this),
        host_invoke_function_as_float64:
          this.host_invoke_function_as_float.bind(this),
        host_invoke_function_as_float32:
          this.host_invoke_function_as_float.bind(this),
        host_invoke_function_as_uint8:
          this.host_invoke_function_as_int.bind(this),
        host_invoke_function_as_uint16:
          this.host_invoke_function_as_int.bind(this),
        host_invoke_function_as_uint32:
          this.host_invoke_function_as_int.bind(this),
        host_invoke_function_as_uint64:
          this.host_invoke_function_as_bigint.bind(this),
        host_invoke_function_as_int8:
          this.host_invoke_function_as_int.bind(this),
        host_invoke_function_as_int16:
          this.host_invoke_function_as_int.bind(this),
        host_invoke_function_as_int32:
          this.host_invoke_function_as_int.bind(this),
        host_invoke_function_as_int64:
          this.host_invoke_function_as_bigint.bind(this),
        host_invoke_function_as_string:
          this.host_invoke_function_as_string.bind(this),
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
      LOGGER.debug("Received instructions for batch: ", instructions);

      for (let index in instructions) {
        const instruction = instructions[index];
        LOGGER.debug("Executing instruction: ", instruction);
        instruction.call(this, this);
      }
    }

    function_drop_external_pointer(uid) {
      this.function_heap.destroy(uid);
    }

    dom_drop_external_pointer(uid) {
      this.dom_heap.destroy(uid);
    }

    object_drop_external_pointer(uid) {
      this.object_heap.destroy(uid);
    }

    string_cache_drop_external_pointer(uid) {
      this.string_cache.destroy(uid);
    }

    drop_external_reference(uid) {
      return this.dom_heap.destroy(uid);
    }

    object_allocate_external_pointer() {
      const slot_id = this.object_heap.create(null);
      LOGGER.debug("Creating new object pointer for pre-allocation: ", slot_id);
      return slot_id;
    }

    function_allocate_external_pointer() {
      const slot_id = this.function_heap.create(null);
      LOGGER.debug(
        "Creating new function pointer for pre-allocation: ",
        slot_id,
      );
      return slot_id;
    }

    dom_allocate_external_pointer() {
      const slot_id = this.dom_heap.create(null);
      LOGGER.debug("Creating new DOM pointer for pre-allocation: ", slot_id);
      return slot_id;
    }

    host_cache_string(start, length, utf_indicator) {
      LOGGER.debug(
        "Register string for cache/interning: ",
        start,
        length,
        " UTF8: ",
        utf_indicator,
      );

      if (ALLOWED_UTF8_INDICATOR.indexOf(utf_indicator) === -1) {
        throw new Error("Unsupported UTF indicator (only 8 or 16)");
      }

      start = Number(start);
      length = Number(length);

      let target_string;
      if (utf_indicator === 16) {
        target_string = this.texts.readShortUTF16FromMemory(start, length);
      }
      if (utf_indicator === 8) {
        target_string = this.texts.readShortUTF8FromMemory(start, length);
      }

      LOGGER.debug(`Generated cache string: ${target_string}`);

      if (!target_string) throw new Error("No valid string was provided");

      return BigInt(this.string_cache.create(target_string));
    }

    host_register_function(start, length, utf_indicator) {
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

    host_invoke_function_with_return(
      handle,
      parameter_start,
      parameter_length,
      returns_start,
      returns_length,
    ) {
      // read parameters and invoke function via handle.
      const parameters = this.parameter_v1.parse_array(
        parameter_start,
        parameter_length,
      );

      if (!parameters && parameter_length > 0)
        throw new Error("No parameters returned though we expect some");

      const func = this.function_heap.get(handle);

      const response = func.call(this, ...parameters);
      LOGGER.debug("host_invoke_function_with_return: ", response);
      return response;
    }

    host_invoke_callback_function(
      handle,
      callback_handle,
      parameter_start,
      parameter_length,
    ) {
      // this.host_invoke_function_with_return(
      //   handle,
      //   parameter_start,
      //   parameter_length,
      // );
      return BigInt(0);
    }

    host_invoke_function(handle, parameter_start, parameter_length) {
      // this.host_invoke_function_with_return(
      //   handle,
      //   parameter_start,
      //   parameter_length,
      // );
      return BigInt(0);
    }

    host_invoke_function(
      handle,
      parameter_start,
      parameter_length,
      returns_start,
      returns_length,
    ) {
      this.host_invoke_function_with_return(
        handle,
        parameter_start,
        parameter_length,
        returns_start,
        returns_length,
      );
    }

    host_invoke_function_as_dom(handle, parameter_start, parameter_length) {
      const response = this.host_invoke_function_with_return(
        handle,
        parameter_start,
        parameter_length,
      );
      return this.dom_heap.create(response);
    }

    host_invoke_function_as_object(handle, parameter_start, parameter_length) {
      const response = this.host_invoke_function_with_return(
        handle,
        parameter_start,
        parameter_length,
      );
      return this.object_heap.create(response);
    }

    host_invoke_function_as_bool(handle, parameter_start, parameter_length) {
      const response = this.host_invoke_function_with_return(
        handle,
        parameter_start,
        parameter_length,
      );
      return response ? true : false;
    }

    host_invoke_function_as_float(handle, parameter_start, parameter_length) {
      const response = this.host_invoke_function_with_return(
        handle,
        parameter_start,
        parameter_length,
      );
      if (typeof response != "number") {
        throw new Error(`Response ${response} is not a number`);
      }
      return response;
    }

    host_invoke_function_as_int(handle, parameter_start, parameter_length) {
      const response = this.host_invoke_function_with_return(
        handle,
        parameter_start,
        parameter_length,
      );
      if (typeof response != "number" && typeof response != "bigint") {
        throw new Error(`Response ${response} is not a number`);
      }
      if (response instanceof BigInt || typeof response == "bigint") {
        return Number(response);
      }

      return response;
    }

    host_invoke_function_as_bigint(handle, parameter_start, parameter_length) {
      const response = this.host_invoke_function_with_return(
        handle,
        parameter_start,
        parameter_length,
      );
      if (response instanceof BigInt) {
        return response;
      }
      if (typeof response == "bigint") {
        return response;
      }
      return BigInt(response);
    }

    host_invoke_function_as_string(handle, parameter_start, parameter_length) {
      const response = this.host_invoke_function_with_return(
        handle,
        parameter_start,
        parameter_length,
      );
      return this.texts.writeUTF8ToMemory(response);
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
          LOGGER.error("Properly must have 'url' property.", script);
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
        abi: this.parser.web_abi,
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

    static async loadWASMBytes(
      wasm_bytes,
      memory,
      environment,
      compileOptions,
    ) {
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
            ...env,
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
    return new WasmWebScripts(
      10,
      200,
      env,
      WASMLoader.configCompiledOptions({}),
    );
  };

  return {
    // Base loggers and types
    LOGGER,
    LEVELS,
    Move,
    Params,
    Operations,
    ReturnTypes,
    ReturnValueTypes: ReturnTypeId,
    TypedSlice,
    ArgumentOperations,
    TypeOptimization,

    // Support classes and functions
    CachePointer,
    ExternalPointer,
    InternalPointer,
    TypedArraySlice,
    DOMArena,
    TextCodec,
    WASMLoader,
    ArenaAllocator,
    MemoryOperator,

    // Arguments parsers (immediate and batch)
    ParameterParserV1,
    ParameterParserV2,
    BatchInstructions,

    // Returns and Result replies
    Reply,

    // Core classes and managers
    MegatronMiddleware,
    WasmWebScripts,
  };
})();

if (typeof module !== "undefined") {
  module.exports = Megatron;
}
