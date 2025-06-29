"strict mode";

const Megatron = (function () {
  const NULL_AND_UNDEFINED = [null, undefined];
  const MAX_ITERATION = 5000000;
  const FOUR_GIG_BYTES = 4294967296;

  const CONTEXT = {};

  const LEVELS = {
    INFO: 1,
    ERROR: 2,
    WARNINGS: 3,
    DEBUG: 4,
  };

  const LOGGER = {
    mode: LEVELS.ERROR,
    namespace: null,
  };

  LOGGER.scoped = function (namespace) {
    return {
      info: function () {
        const args = [namespace, " "];
        args.push.apply(args, Array.from(arguments));
        console.log.apply(console, args);
      },

      trace: function () {
        const args = [namespace, " "];
        args.push.apply(args, Array.from(arguments));
        console.trace.apply(console, args);
      },

      warning: function () {
        if (LOGGER.mode < LEVELS.WARNINGS) return;
        const args = [namespace, " "];
        args.push.apply(args, Array.from(arguments));
        console.warn.apply(console, args);
      },

      error: function () {
        if (LOGGER.mode < LEVELS.ERROR) return;
        const args = [namespace, " "];
        args.push.apply(args, Array.from(arguments));
        console.error.apply(console, args);
      },

      debug: function () {
        if (LOGGER.mode < LEVELS.DEBUG) return;
        const args = [namespace, " "];
        args.push.apply(args, Array.from(arguments));
        console.debug.apply(console, args);
      },
    };
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

  const ThreeStateId = {
    // Where we can only ever have 1 state
    One: 70,

    // Where we can have two different states.
    Two: 80,

    // Where we can have three different states.
    Three: 90,
  };

  ThreeStateId.__INVERSE__ = Object.keys(ThreeStateId)
    .map((key) => {
      return [key, ThreeStateId[key]];
    })
    .reduce((prev, current) => {
      let [key, value] = current;
      prev[value] = key;
      return prev;
    }, {});

  Object.freeze(ThreeStateId);

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

  const GroupReturnHintMarker = {
    Start: 111,
    Stop: 222,
  };

  GroupReturnHintMarker.__INVERSE__ = Object.keys(GroupReturnHintMarker)
    .map((key) => {
      return [key, GroupReturnHintMarker[key]];
    })
    .reduce((prev, current) => {
      let [key, value] = current;
      prev[value] = key;
      return prev;
    }, {});

  Object.freeze(GroupReturnHintMarker);

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

  /// [`ReturnIds`] represent the type indicating the underlying returned
  /// value for an operation.
  const ReturnIds = {
    None: 0,
    One: 1,
    Multi: 2,
    List: 3,
  };

  ReturnIds.__INVERSE__ = Object.keys(ReturnIds)
    .map((key) => {
      return [key, ReturnIds[key]];
    })
    .reduce((prev, current) => {
      let [key, value] = current;
      prev[value] = key;
      return prev;
    }, {});

  Object.freeze(ReturnIds);

  /// [`ReturnTypeId`] represent the type indicating the underlying returned
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
    None: 30,
    ErrorCode: 31,
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

    /// Invoke represents the desire to call a
    /// function across boundary that may or may not return any value
    /// in response to being called.
    ///
    /// It has two layout formats:
    ///
    /// A. with no argument: Begin, 3, FunctionHandle(u64), End
    ///
    /// B. with arguments: Begin, 3, FunctionHandle(u64), FunctionArguments, {Arguments}, End
    Invoke: 2,

    /// `Operations::InvokeAsync` aka InvokeAsyncCallback represents the desire to call a
    /// function across boundary that takes a callback internal reference
    /// which it will use to supply appropriate response when ready via the
    /// returned promise or future the host uses to represent the operation.
    ///
    /// The idea is the result is not immediate and hence will not have a
    /// returned value but instead the internal callback reference will be
    /// used to deliver the result based on type hints.
    ///
    /// The return value to the callback function must always be of the type: [`Returns`].
    ///
    /// Layout format: Begin, 3, FunctionHandle(u64), ArgStart, ArgBegin, ExternReference, ArgEnd, ArgStop,
    ///  End
    InvokeAsync: 3,

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

  function isBigInt(value) {
    if (typeof value == "bigint" || value instanceof BigInt) return true;
    return false;
  }

  function isBigIntOrNumber(value) {
    if (typeof value == "bigint" || value instanceof BigInt) return true;
    if (typeof value == "number") return true;
    return false;
  }

  function isNumber(value) {
    if (typeof value !== "bigint" && typeof value == "number") return true;
    return false;
  }

  class ReplyError extends Error {
    constructor(code, options) {
      if (!(code instanceof Number) && !isNumber(code)) {
        throw new Error(
          "Only numbers allowed to represent the code to be sent",
        );
      }
      super("Reply failed with error code: " + code, options);
      this.code = code;
    }
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

    // TODO(alex.ewetumo): Decide if this
    // should ever be exposed, the whole idea of the cache
    // is to save those frequently access content in a way
    // that over the lifetime of the instance be amortized
    // without needing to reuse it over and over.
    //
    // MainPoint: And i do not reason we ever want to delete
    // that ever actively via an API, if the brower restarts
    // and that is not stored somewhere then its fine to rebuild.
    //
    //
    // destroy_id(id) {
    //   if (typeof item !== "string") {
    //     throw new Error("Only strings are allowed");
    //   }
    //
    //   if (!this.has_id(id)) {
    //     return false;
    //   }
    //
    //   this.id_to_text.delete(id);
    //   this.text_to_id.delete(item);
    //
    //   if (this.text_to_id.size == 0) {
    //     this.count = 0;
    //   }
    // }
    //
    // destroy(item) {
    //   if (typeof item !== "string") {
    //     throw new Error("Only strings are allowed");
    //   }
    //
    //   if (!this.has_text(item)) {
    //     return false;
    //   }
    //
    //   const id = this.get_id(item);
    //   this.destroy_id(id);
    // }

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
      // 0 is reserved for this.(if `this. exists else is also `this`).
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
      const logger = LOGGER.scoped("MemoryOperation.writeUint8Buffer:");

      // create an allocation within the wasm instance by
      // calling its create_allocation exported function
      // as we expect.
      logger.debug(
        "allocate_memory: allocating memory location with size: ",
        size,
      );
      const allocation_id = this.instance.exports.create_allocation(
        BigInt(size),
      );
      const allocation_entry = ArenaAllocator.uid_to_entry(allocation_id);
      logger.debug(
        "Created allocation_id: ",
        allocation_id,
        " specifically at:",
        allocation_entry,
      );

      const allocation_start_pointer =
        this.instance.exports.allocation_start_pointer(allocation_id);
      logger.debug(
        `Retrieved allocation start ptr: ${allocation_start_pointer} for id: ${allocation_id}`,
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
      const logger = LOGGER.scoped("MemoryOperation.writeUint8Buffer:");
      const buffer_type = typeof buffer;
      const len = buffer.length || buffer.byteLength;
      logger.debug(
        `Writing buffer to memory(type=${buffer_type}): len=${len} from buffer=${buffer}`,
      );
      const [id, start] = this.allocate_memory(len);

      const memory = new Uint8Array(this.get_memory());
      memory.set(buffer, start);

      return id;
    }

    writeUint8Array(uint8_buffer) {
      const logger = LOGGER.scoped("MemoryOperation.writeUint8Array:");
      logger.debug("Writing Uint8Array from: ", uint8_buffer);
      if (!(uint8_buffer instanceof Uint8Array))
        throw new Error("Please supply a Uint8Array to this method");
      return this.writeUint8Buffer(uint8_buffer);
    }

    static copyUint8Data(from, to, fromIndex) {
      LOGGER.debug("Copying data from buffer: ", from, " to buffer2: ", to);

      if (!(from instanceof Uint8Array))
        throw new Error("from variable must be a Uint8Array");
      if (!(to instanceof Uint8Array))
        throw new Error("to variable must be a Uint8Array");

      to.set(from, fromIndex);
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

    writeUTF8ToView(text, to_view, fromIndex) {
      if (!(view instanceof Uint8Array))
        throw new Error("View argument must be Uint8Array");

      const from_source = this.utf8_encoder.encode(text);
      const fromByteLength = from_source.byteLength;
      const targetByteLength = to_view.byteLength;
      const remainingSpace = targetByteLength - fromIndex;

      if (remainingSpace < fromByteLength) {
        throw new Error("Not enough space in target for copying");
      }

      to_view.set(from_source, fromIndex);

      return fromIndex + fromByteLength;
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

  class FakeNode {
    constructor(tag) {
      this.tag = tag;
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

  class ReplyContainer {
    constructor(value_type, value) {
      if (!(value_type in ReturnTypeId.__INVERSE__)) {
        throw new Error(
          `ReturnValueTypes ${return_type} is not known for value_type: ${return_type}`,
        );
      }

      this.type = value_type;
      this.value = value;
    }
  }

  class ThreeState {
    constructor(state_type, options) {
      if (!(options instanceof Array)) {
        throw new Error("type options should an Array of int");
      }
      this.state_type = state_type;
      this.options = options;
    }
  }

  class ReturnHint {
    constructor(return_type, states) {
      if (!(states instanceof Array)) {
        throw new Error("states should an Array of ThreeState");
      }

      if (states.length == 0 && return_type != ReturnIds.None) {
        throw new Error(
          "states cant be an empty array for return type: " + return_type,
        );
      }

      const logger = LOGGER.scoped("ReturnHint:");

      logger.debug(
        "Creating ReturnHint for return_type=",
        return_type,
        " and states=",
        states,
      );

      for (let index in states) {
        if (!(states[index] instanceof ThreeState))
          throw new Error(
            "state definition at index: " +
              index +
              " with state: " +
              states[index],
          );
      }

      this.return_type = return_type;
      this.states = states;
      this.type_validators = {};
      this.type_validators[ReturnTypeId.Bool] = this.validateBool;
      this.type_validators[ReturnTypeId.Text8] = this.validateText8;
      this.type_validators[ReturnTypeId.Uint16] = this.validateInt;
      this.type_validators[ReturnTypeId.ErrorCode] = this.validateInt;
      this.type_validators[ReturnTypeId.Uint32] = this.validateInt;
      this.type_validators[ReturnTypeId.Uint64] = this.validateInt64;
      this.type_validators[ReturnTypeId.Int8] = this.validateInt;
      this.type_validators[ReturnTypeId.Int16] = this.validateInt;
      this.type_validators[ReturnTypeId.Int32] = this.validateInt;
      this.type_validators[ReturnTypeId.Int64] = this.validateInt64;
      this.type_validators[ReturnTypeId.Float32] = this.validateFloat;
      this.type_validators[ReturnTypeId.Float64] = this.validateFloat;
    }

    validate(input) {
      if (input instanceof Array) {
        for (let index in input) {
          let item = input[index];
          if (!(item instanceof ReplyContainer)) {
            throw new Error(
              "Input[" +
                index +
                "] with value: '" +
                item +
                "' should be wrapped in ReplyContainer",
            );
          }
        }
      } else if (!(input instanceof ReplyContainer)) {
        throw new Error("Input should be wrapped in ReplyContainer");
      }

      const logger = LOGGER.scoped("ReturnHint.validate:");
      logger.info(
        "Incoming input",
        input,
        "against (states=",
        this.states,
        "})",
      );

      for (let state_index in this.states) {
        let states = this.states[state_index].options;
        if (states.indexOf(input.type) === -1) {
          throw new Error(
            `Input type: ${input.type} not in any expected types: ` + states,
          );
        }
      }
    }

    asValue(value) {
      if (this.return_type == ReturnIds.None) return null;
      return new ReturnHintValue(this, value);
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

  class ReturnHintValue {
    constructor(hint, value) {
      if (!(hint instanceof ReturnHint)) {
        throw new Error("Must be instance of MemoryOperator");
      }
      this.hint_type = hint.return_type;
      this.states = hint.states;
      this.value = value;
    }
  }

  class NoReturn extends ReturnHint {
    constructor() {
      super(ReturnIds.None, []);
    }

    validate(input) {
      if (isUndefinedOrNull(input)) return;
      if (input instanceof ReplyContainer) {
        if (input.type == ReturnTypeId.None) {
          return;
        }
      }
      throw new Error(
        "NoReturn cant be anything other than None/Null/Undefined",
      );
    }
  }

  class SingleReturn extends ReturnHint {
    constructor(state) {
      super(ReturnIds.One, [state]);
    }
  }

  class ListReturn extends ReturnHint {
    constructor(state) {
      super(ReturnIds.List, [state]);
    }
  }

  class MultiReturn extends ReturnHint {
    constructor(states) {
      super(ReturnIds.Multi, states);
    }

    validate(inputs) {
      if (inputs instanceof Array) {
        for (let index in inputs) {
          let item = inputs[index];
          if (!(item instanceof ReplyContainer)) {
            throw new Error(
              "Input[" +
                index +
                "] with value: '" +
                item +
                "' should be wrapped in ReplyContainer",
            );
          }
        }
      }

      const logger = LOGGER.scoped("ReturnHint.validate:");
      logger.info(
        "Incoming input",
        inputs,
        "against (states=",
        this.states,
        "})",
      );

      for (let state_index in this.states) {
        let input = inputs[state_index];
        let states = this.states[state_index].options;

        if (states.indexOf(input.type) === -1) {
          throw new Error(
            `Input type: ${input.type} not in any expected types: ` + states,
          );
        }
      }
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
    constructor(memory_operator) {
      if (!(memory_operator instanceof MemoryOperator)) {
        throw new Error("Must be instance of MemoryOperator");
      }

      this.operator = memory_operator;
      this.module = memory_operator.get_module();

      this.parsers = {};
      this.parsers[ReturnIds.One] = this.parse_one.bind(this);
      this.parsers[ReturnIds.None] = this.parse_none.bind(this);
      this.parsers[ReturnIds.List] = this.parse_list.bind(this);
      this.parsers[ReturnIds.Multi] = this.parse_multiple.bind(this);
    }

    parse_hint(start, length) {
      const hint_buffer = this.operator.readUint8Buffer(
        Number(start),
        Number(length),
      );
      LOGGER.debug("parse_hint:start ", start, length, hint_buffer);

      const hint_view = new DataView(hint_buffer);
      return this.parse_from(0, hint_view);
    }

    parse_from(moved_by, view) {
      // validate we see begin marker
      const id = view.getUint8(moved_by, true);

      if (id != ReturnHintMarker.Start) {
        throw new Error("Buffer did not start with ReturnHintMarker.Start");
      }
      moved_by += Move.MOVE_BY_1_BYTES;

      // get the type of hint
      const hint_type = view.getUint8(moved_by, true);
      if (!(hint_type in this.parsers)) {
        throw new Error(`ReturnHintType ${hint_type} not found`);
      }

      moved_by += Move.MOVE_BY_1_BYTES;

      LOGGER.debug("Fetching handler for return_type hint: ", hint_type);

      let return_validator;
      const parser = this.parsers[hint_type];
      [moved_by, return_validator] = parser(moved_by, view);

      const next_token = view.getUint8(moved_by, true);
      if (next_token != ReturnHintMarker.Stop) {
        throw new Error(
          "Buffer did not end with ReturnHintMarker.End instead got: " +
            next_token,
        );
      }

      moved_by += Move.MOVE_BY_1_BYTES;

      return [moved_by, return_validator];
    }

    parse_none(offset, view) {
      return [offset, new NoReturn()];
    }

    parse_state(moved_by, view) {
      const state_type = view.getUint8(moved_by, true);
      if (!(state_type in ThreeStateId.__INVERSE__)) {
        throw new Error(`ThreeStateId ${state_type} is not known`);
      }

      LOGGER.debug("Extracted ThreeStateId value item: ", state_type);

      const states = [];

      moved_by += Move.MOVE_BY_1_BYTES;

      let p1, p2, p3;

      switch (state_type) {
        case ThreeStateId.One:
          p1 = view.getUint8(moved_by, true);
          if (!(p1 in ReturnTypeId.__INVERSE__)) {
            throw new Error(`ReturnTypeId ${p1} is not known`);
          }

          moved_by += Move.MOVE_BY_1_BYTES;
          states.push(new ThreeState(state_type, [p1]));
          break;
        case ThreeStateId.Two:
          p1 = view.getUint8(moved_by, true);
          moved_by += Move.MOVE_BY_1_BYTES;

          if (!(p1 in ReturnTypeId.__INVERSE__)) {
            throw new Error(`ReturnTypeId ${p1} is not known`);
          }

          p2 = view.getUint8(moved_by, true);
          moved_by += Move.MOVE_BY_1_BYTES;

          if (!(p2 in ReturnTypeId.__INVERSE__)) {
            throw new Error(`ReturnTypeId ${p2} is not known`);
          }

          states.push(new ThreeState(state_type, [p1, p2]));
          break;
        case ThreeStateId.Three:
          p1 = view.getUint8(moved_by, true);
          moved_by += Move.MOVE_BY_1_BYTES;

          if (!(p1 in ReturnTypeId.__INVERSE__)) {
            throw new Error(`ReturnTypeId ${p1} is not known`);
          }

          p2 = view.getUint8(moved_by, true);
          moved_by += Move.MOVE_BY_1_BYTES;

          if (!(p2 in ReturnTypeId.__INVERSE__)) {
            throw new Error(`ReturnTypeId ${p2} is not known`);
          }

          p3 = view.getUint8(moved_by, true);
          moved_by += Move.MOVE_BY_1_BYTES;

          if (!(p3 in ReturnTypeId.__INVERSE__)) {
            throw new Error(`ReturnTypeId ${p3} is not known`);
          }

          states.push(new ThreeState(state_type, [p1, p2, p3]));
          break;
      }

      return [moved_by, states];
    }

    parse_one(moved_by, view) {
      let states;
      [moved_by, states] = this.parse_state(moved_by, view);

      if (states.length != 1) {
        throw new Error("States  should only be 1");
      }

      LOGGER.debug("Extracted ParseOne value item: ", states);

      return [moved_by, new SingleReturn(states[0])];
    }

    parse_list(moved_by, view) {
      let states;
      [moved_by, states] = this.parse_state(moved_by, view);

      if (states.length != 1) {
        throw new Error("States should only be 1");
      }

      LOGGER.debug("Extracted ParseList value item: ", states);

      moved_by += Move.MOVE_BY_1_BYTES;
      return [moved_by, new ListReturn(value_type)];
    }

    parse_multiple(moved_by, view) {
      const multi_values = [];
      while (true) {
        let value_type = view.getUint8(moved_by, true);
        if (value_type == ReturnHintMarker.Stop) {
          break;
        }

        let states;
        [moved_by, states] = this.parse_state(moved_by, view);

        if (states.length == 0) {
          throw new Error("Atleast 1 States should be provided");
        }

        LOGGER.debug("Extracted multiple state item: ", states);

        multi_values.push.apply(multi_values, states);
      }

      return [moved_by, new MultiReturn(multi_values)];
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
      const logger = LOGGER.scoped("ParameterParserV2.parseParams:");

      logger.debug("Props", moved_by, view, typeof text_string_array);
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
      const logger = LOGGER.scoped("ParameterParserV2.parseParamType:");
      const value_type_str = Params.__INVERSE__[value_type];
      logger.debug(`Received value_type: ${value_type} (${value_type_str})`);

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

  class Reply {
    constructor(
      memory_operator,
      text_codec,
      text_cache,
      function_heap,
      object_heap,
      dom_heap,
    ) {
      if (!(memory_operator instanceof MemoryOperator)) {
        throw new Error("Must be instance of MemoryOperator");
      }
      if (!(text_codec instanceof TextCodec)) {
        throw new Error("Must be instance of TextCodec");
      }
      if (!(text_cache instanceof SimpleStringCache)) {
        throw new Error("Must be instance of SimpleStringCache");
      }
      if (!(function_heap instanceof ArenaAllocator)) {
        throw new Error("Must be instance of ArenaAllocator");
      }
      if (!(object_heap instanceof ArenaAllocator)) {
        throw new Error("Must be instance of ArenaAllocator");
      }
      if (!(dom_heap instanceof DOMArena)) {
        throw new Error("Must be instance of DomArena");
      }

      this.texts = text_codec;
      this.dom_heap = dom_heap;
      this.text_cache = text_cache;
      this.object_heap = object_heap;
      this.operator = memory_operator;
      this.function_heap = function_heap;
      this.module = memory_operator.get_module();
      this.instance = this.module.instance;

      this.return_naked = [
        ReturnTypeId.Bool,
        ReturnTypeId.Uint8,
        ReturnTypeId.Uint16,
        ReturnTypeId.Uint32,
        ReturnTypeId.Uint64,
        ReturnTypeId.Int8,
        ReturnTypeId.Int16,
        ReturnTypeId.Int32,
        ReturnTypeId.Int64,
        ReturnTypeId.Float32,
        ReturnTypeId.Float64,
        ReturnTypeId.Object,
        ReturnTypeId.DOMObject,
        ReturnTypeId.ErrorCode,
        ReturnTypeId.MemorySlice,
        ReturnTypeId.InternalReference,
        ReturnTypeId.ExternalReference,
      ];

      this.reply_types = {};
      this.reply_types[ReturnTypeId.None] = this.encodeNone.bind(this);
      this.reply_types[ReturnTypeId.Bool] = this.encodeBool.bind(this);
      this.reply_types[ReturnTypeId.Uint8] = this.encodeUint8.bind(this);
      this.reply_types[ReturnTypeId.Uint16] = this.encodeInt16.bind(this);
      this.reply_types[ReturnTypeId.Uint32] = this.encodeInt32.bind(this);
      this.reply_types[ReturnTypeId.Uint64] = this.encodeBigInt64.bind(this);
      this.reply_types[ReturnTypeId.Text8] = this.encodeText8.bind(this);
      this.reply_types[ReturnTypeId.Int8] = this.encodeInt8.bind(this);
      this.reply_types[ReturnTypeId.Int16] = this.encodeInt16.bind(this);
      this.reply_types[ReturnTypeId.ErrorCode] =
        this.encodeErrorCode.bind(this);
      this.reply_types[ReturnTypeId.Int32] = this.encodeInt32.bind(this);
      this.reply_types[ReturnTypeId.Int64] = this.encodeBigInt64.bind(this);
      this.reply_types[ReturnTypeId.Float32] = this.encodeFloat32.bind(this);
      this.reply_types[ReturnTypeId.Float64] = this.encodeFloat64.bind(this);
      this.reply_types[ReturnTypeId.Object] = this.encodeObject.bind(this);
      this.reply_types[ReturnTypeId.DOMObject] =
        this.encodeDOMObject.bind(this);
      this.reply_types[ReturnTypeId.ExternalReference] =
        this.encodeExternalReference.bind(this);
      this.reply_types[ReturnTypeId.InternalReference] =
        this.encodeInternalReference.bind(this);
      this.reply_types[ReturnTypeId.MemorySlice] =
        this.encodeMemorySlice.bind(this);
      this.reply_types[ReturnTypeId.Uint8ArrayBuffer] =
        this.encodeUint8ArrayBuffer.bind(this);
      this.reply_types[ReturnTypeId.Uint16ArrayBuffer] =
        this.encodeUint16ArrayBuffer.bind(this);
      this.reply_types[ReturnTypeId.Uint32ArrayBuffer] =
        this.encodeUint32ArrayBuffer.bind(this);
      this.reply_types[ReturnTypeId.Uint64ArrayBuffer] =
        this.encodeUint64ArrayBuffer.bind(this);
      this.reply_types[ReturnTypeId.Int8ArrayBuffer] =
        this.encodeInt8ArrayBuffer.bind(this);
      this.reply_types[ReturnTypeId.Int16ArrayBuffer] =
        this.encodeInt16ArrayBuffer.bind(this);
      this.reply_types[ReturnTypeId.Int32ArrayBuffer] =
        this.encodeInt32ArrayBuffer.bind(this);
      this.reply_types[ReturnTypeId.Int64ArrayBuffer] =
        this.encodeInt64ArrayBuffer.bind(this);
      this.reply_types[ReturnTypeId.Float32ArrayBuffer] =
        this.encodeFloat32ArrayBuffer.bind(this);
      this.reply_types[ReturnTypeId.Float64ArrayBuffer] =
        this.encodeFloat64ArrayBuffer.bind(this);
    }

    encode_into_memory(items) {
      const logger = LOGGER.scoped("Reply.encode_into_memory:");

      const encoded_buffer = new Uint8Array(
        this.encode(items instanceof Array ? items : [items]),
      );
      logger.debug(`encoded value: `, items, " -> ", encoded_buffer);

      const reply_id = BigInt(this.operator.writeUint8Array(encoded_buffer));
      logger.debug("written return values to: ", reply_id);

      return reply_id;
    }

    immediate(return_hint, values) {
      if (!(return_hint instanceof ReturnHint)) {
        throw new Error("argument must be a type of ReturnHint");
      }

      const logger = LOGGER.scoped("Reply.callback: ");

      logger.debug(`received: "`, values, `" with return hint: `, return_hint);

      if (return_hint instanceof NoReturn && !isUndefinedOrNull(values)) {
        throw new Error(`Expected NoReturn value but got ${values}`);
      }

      if (return_hint instanceof NoReturn && isUndefinedOrNull(values)) {
        return BigInt(-1);
      }

      const transformed = this.transform(
        values instanceof Array ? values : [values],
        return_hint,
      );

      if (return_hint instanceof SingleReturn) {
        const target = transformed[0];

        logger.debug(`single return value: `, target);

        return_hint.validate(target);
        if (this.return_naked.indexOf(target.type) !== -1) {
          logger.debug(
            "returning value as is/naked:",
            target.value,
            " from ",
            target,
          );
          return target.value;
        }
      } else {
        logger.debug("validate list/multi-return: ", transformed);
        return_hint.validate(transformed);
      }

      return this.encode_into_memory(transformed);
    }

    callback_failure(internal_pointer, value) {
      const logger = LOGGER.scoped("Reply.callback_failure:");

      logger.debug("Check it is a InternalPointer");
      if (!(internal_pointer instanceof InternalPointer)) {
        throw new Error(
          "internal_pointer: must be a type of InternalPointer: " +
            internal_pointer,
        );
      }

      logger.debug("Check it is a ReplyError");
      if (value instanceof ReplyError) {
        value = Reply.asErrorCode(value);
      }

      logger.debug("Check it is a ReplyContainer");
      if (value instanceof ReplyContainer) {
        if (value.type != ReturnTypeId.ErrorCode) {
          throw new Error(
            "Reply.ErrorCode is used to communicate failure and hence ",
          );
        }
      }

      logger.debug("Sending value to wasm app: ", value);

      const mem_id = this.encode_into_memory(value);
      logger.debug(
        "Calling callback handle: ",
        internal_pointer,
        mem_id,
        this.module,
        this.module.exports,
      );

      try {
        this.instance.exports.invoke_callback(
          BigInt(internal_pointer.value),
          mem_id,
        );
      } catch (e) {
        logger.error("Failed to deliver response to callback due to: ", e);
      }
    }

    callback_success(internal_pointer, return_hint, values) {
      if (!(internal_pointer instanceof InternalPointer)) {
        throw new Error(
          "internal_pointer: must be a type of InternalPointer: " +
            internal_pointer,
        );
      }

      if (!(return_hint instanceof ReturnHint)) {
        throw new Error(
          "argument must be a type of ReturnHint: " + return_hint,
        );
      }

      const logger = LOGGER.scoped("Reply.callback: ");

      logger.debug(`received callback result: "${values}"`);

      if (return_hint instanceof NoReturn && !isUndefinedOrNull(values)) {
        throw new Error(`Expected NoReturn value but got ${values}`);
      }

      if (return_hint instanceof NoReturn && isUndefinedOrNull(values)) {
        return new Big(-1);
      }

      const transformed = this.transform(
        values instanceof Array ? values : [values],
        return_hint,
      );

      if (return_hint instanceof SingleReturn) {
        const target = transformed[0];
        logger.debug(`validate single return value: ${target}`);
        return_hint.validate(target);
      } else {
        logger.debug(`validate list/multi-return: ${transformed}`);
        return_hint.validate(transformed);
      }

      const mem_id = this.encode_into_memory(transformed);
      logger.info(
        "Calling callback handle: ",
        internal_pointer,
        mem_id,
        this.module,
        this.module.exports,
      );

      try {
        this.instance.exports.invoke_callback(
          BigInt(internal_pointer.value),
          mem_id,
        );
      } catch (e) {
        logger.error("Failed to deliver response to callback due to: ", e);
      }
    }

    write_group_return(results) {
      const logger = LOGGER.scoped("MegatronMiddleware.write_out_returns: ");

      if (results.length == 0) {
        return -1;
      }

      // create our content byte buffer
      // x 64bit numbers + 2 int8 numbers for header & trailer + x int8 numbers indicate
      // return type.
      //
      // Where x is the total number of return items,.
      let size = Move.MOVE_BY_64_BYTES * results.length + 2 + results.length;
      let size_at_70 = Math.floor(size * 0.7);
      logger.debug(
        "Creating ArrayBuffer of size=",
        size,
        " with 70% mark at: ",
        size_at_70,
      );

      const content = new ArrayBuffer(size, {
        maxByteLength: FOUR_GIG_BYTES,
      });

      // create our view for setting up values correctly.
      let view = new DataView(content);

      let offset = 0;

      view.setUint8(offset, GroupReturnHintMarker.Start, true);
      offset += Move.MOVE_BY_1_BYTES;

      for (let index = 0; index < results.length; index++) {
        // if we are close to 70% full, then increase size by size.
        if (
          offset >= size_at_70 ||
          offset + Move.MOVE_BY_64_BYTES > size_at_70
        ) {
          size = size * 2;
          size_at_70 = Math.floor(size * 0.7);

          content.resize(size);
          view = new DataView(content);
          logger.debug(
            "Resizing buffer to new size=",
            size,
            " with 70% mark at: ",
            size_at_70,
          );
        }

        const items = results[index];
        const container = items[0];
        const value = items[1];

        if (!isBigInt(value)) {
          throw new Error("Value must be a BigInt");
        }

        logger.debug(
          "returning_instructions: Getting encoder for value: ",
          value,
          container,
          container.states,
          index,
        );

        view.setUint8(offset, container.hint_type, true);
        offset += Move.MOVE_BY_1_BYTES;

        if (container.hint_type == ReturnIds.Multi) {
          view.setUint16(offset, container.states.length, true);
          offset += Move.MOVE_BY_16_BYTES;
        }

        for (let state_index in container.states) {
          let current_state = container.states[state_index];
          let state_type = current_state.state_type;
          view.setUint8(offset, state_type, true);
          offset += Move.MOVE_BY_1_BYTES;

          for (let state_type_index in current_state.options) {
            let index_type = current_state.options[state_type_index];
            view.setUint8(offset, index_type, true);
            offset += Move.MOVE_BY_1_BYTES;
          }
        }

        view.setBigUint64(offset, value, true);
        offset += Move.MOVE_BY_64_BYTES;
      }

      // if we are close to 70% full, then increase size by size.
      if (offset >= size_at_70) {
        logger.debug(
          "Resizing due to offset at: ",
          offset,
          " with size of buffer: ",
          content.byteLength,
        );
        size = size * 2;
        size_at_70 = Math.floor(size * 0.7);

        content.resize(size);
        view = new DataView(content);
        logger.debug(
          "Resizing buffer to new size=",
          size,
          " with 70% mark at: ",
          size_at_70,
        );
      }

      view.setUint8(offset, GroupReturnHintMarker.Stop, true);
      offset += Move.MOVE_BY_1_BYTES;

      // shrink content to fit final offset
      if (offset < content.byteLength) {
        logger.debug(
          "Shrinking size of buffer with offset at: ",
          offset,
          " with size of buffer: ",
          content.byteLength,
        );
        content.resize(offset);
      }

      const encoded_buffer = new Uint8Array(content);
      const reply_id = BigInt(this.operator.writeUint8Array(encoded_buffer));
      logger.debug(
        "returning_instructions:  written return values to: ",
        reply_id,
      );

      return reply_id;
    }

    transform(values, hint) {
      const logger = LOGGER.scoped("Reply.transform");
      logger.debug(
        "Received value: ",
        values,
        " with hint: ",
        hint,
        " with state: ",
        hint.states,
      );

      const transformed = [];
      for (let index in values) {
        const item = values[index];
        transformed.push(
          Reply.transform_return_type(
            this.function_heap,
            this.dom_heap,
            this.object_heap,
            item,
            hint,
          ),
        );
      }

      logger.debug(
        "Generated transformed values: ",
        transformed,
        " from values: ",
        values,
        " with hint: ",
        hint,
      );
      return transformed;
    }

    encode(values) {
      if (!(values instanceof Array)) {
        throw new Error("values must be a list/array");
      }

      const logger = LOGGER.scoped("Reply.encode");

      // create our content byte buffer
      let size = 80;
      let size_at_70 = Math.floor(size * 0.7) - Move.MOVE_BY_64_BYTES;
      const content = new ArrayBuffer(size, {
        maxByteLength: FOUR_GIG_BYTES,
      });

      // create our view for setting up values correctly.
      let view = new DataView(content);

      let offset = 0;

      view.setUint8(offset, ReturnValueMarker.Begin, true);
      offset += Move.MOVE_BY_1_BYTES;

      for (let index = 0; index < values.length; index++) {
        // if we are close to 70% full, then increase size by size.
        if (
          offset >= size_at_70 ||
          offset + Move.MOVE_BY_64_BYTES > size_at_70
        ) {
          size = size * 2;
          size_at_70 = Math.floor(size * 0.7);

          content.resize(size);
          view = new DataView(content);
          logger.debug(
            "Resizing buffer to new size=",
            size,
            " with 70% mark at: ",
            size_at_70,
          );
        }

        const value = values[index];
        if (isUndefinedOrNull(value.type)) {
          throw new Error("Reply types must have a type id");
        }

        LOGGER.debug("Getting encoder for value: ", value, index);
        const encoder = this.getEncoder(value);
        LOGGER.debug(
          "Received encoder for value: ",
          value,
          index,
          " encoder: ",
          encoder,
        );
        offset = encoder(offset, value, view);
      }

      // if we are close to 70% full, then increase size by size.
      if (offset >= size_at_70) {
        logger.debug(
          "Resizing due to offset at: ",
          offset,
          " with size of buffer: ",
          content.byteLength,
        );
        size = size * 2;
        size_at_70 = Math.floor(size * 0.7);

        content.resize(size);
        view = new DataView(content);
        logger.debug(
          "Resizing buffer to new size=",
          size,
          " with 70% mark at: ",
          size_at_70,
        );
      }

      view.setUint8(offset, ReturnValueMarker.End, true);
      offset += Move.MOVE_BY_1_BYTES;

      const encodedBufferSize = content.byteLength;
      LOGGER.debug(
        `Finished encoding data with initial_size=${size} and encoded_size=${offset} with byteLength=`,
        encodedBufferSize,
      );
      if (offset < content.byteLength) {
        content.resize(offset);
      }

      return content;
    }

    getEncoder(directive) {
      if (!(directive.type in ReturnTypeId.__INVERSE__)) {
        throw new Error(`Unknown Reply encode type id: ${directive.type}`);
      }
      return this.reply_types[directive.type];
    }

    encodeFloat64ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Float64) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      const content = Uint8Array(directive.value.buffer);
      const allocation_id = this.operator.writeUint8Buffer(content);
      view.setBigUint64(offset, allocation_id, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeFloat32ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Float32) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      const content = Uint8Array(directive.value.buffer);
      const allocation_id = this.operator.writeUint8Buffer(content);
      view.setBigUint64(offset, allocation_id, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeInt64ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Int64) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      const content = Uint8Array(directive.value.buffer);
      const allocation_id = this.operator.writeUint8Buffer(content);
      view.setBigUint64(offset, allocation_id, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeInt32ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Int32) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      const content = Uint8Array(directive.value.buffer);
      const allocation_id = this.operator.writeUint8Buffer(content);
      view.setBigUint64(offset, allocation_id, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeInt16ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Int16) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      const content = Uint8Array(directive.value.buffer);
      const allocation_id = this.operator.writeUint8Buffer(content);
      view.setBigUint64(offset, allocation_id, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeInt8ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Int8) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      const content = Uint8Array(directive.value.buffer);
      const allocation_id = this.operator.writeUint8Buffer(content);
      view.setBigUint64(offset, allocation_id, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeUint64ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Uint64) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      const content = Uint8Array(directive.value.buffer);
      const allocation_id = this.operator.writeUint8Buffer(content);
      view.setBigUint64(offset, allocation_id, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeUint32ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Uint32) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      const content = Uint8Array(directive.value.buffer);
      const allocation_id = this.operator.writeUint8Buffer(content);
      view.setBigUint64(offset, allocation_id, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeUint16ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Uint16) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      const content = Uint8Array(directive.value.buffer);
      const allocation_id = this.operator.writeUint8Buffer(content);
      view.setBigUint64(offset, allocation_id, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeUint8ArrayBuffer(offset, directive, view) {
      if (directive.type != ReturnTypeId.Uint8) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      const allocation_id = this.operator.writeUint8Buffer(directive.value);
      view.setBigUint64(offset, allocation_id, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeInternalReference(offset, directive, view) {
      if (directive.type != ReturnTypeId.InternalReference) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setBigUint64(offset, directive.value, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeObject(offset, directive, view) {
      if (directive.type != ReturnTypeId.Object) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setBigUint64(offset, directive.value, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeDOMObject(offset, directive, view) {
      if (directive.type != ReturnTypeId.DOMObject) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setBigUint64(offset, directive.value, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeExternalReference(offset, directive, view) {
      if (directive.type != ReturnTypeId.ExternalReference) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setBigUint64(offset, directive.value, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeMemorySlice(offset, directive, view) {
      if (directive.type != ReturnTypeId.MemorySlice) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setBigUint64(offset, directive.value, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeInt128(offset, directive, view) {
      if (directive.type != ReturnTypeId.Int128) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setBigInt64(offset, directive.value.value_msb, true);
      offset += Move.MOVE_BY_64_BYTES;

      view.setBigInt64(offset, directive.value.value_lsb, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeUint128(offset, directive, view) {
      if (directive.type != ReturnTypeId.Uint128) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setBigUint64(offset, directive.value.value_msb, true);
      offset += Move.MOVE_BY_64_BYTES;

      view.setBigUint64(offset, directive.value.value_lsb, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeFloat32(offset, directive, view) {
      if (directive.type != ReturnTypeId.Float32) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setFloat32(offset, directive.value, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    // implement encoding of a float64 using the same as the encodeBool method
    encodeFloat64(offset, directive, view) {
      if (directive.type != ReturnTypeId.Float64) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setFloat64(offset, directive.value, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeBigUint64(offset, directive, view) {
      if (directive.type != ReturnTypeId.Uint64) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setBigUint64(offset, directive.value, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeUint32(offset, directive, view) {
      if (directive.type != ReturnTypeId.Uint32) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setUint32(offset, directive.value, true);
      offset += Move.MOVE_BY_32_BYTES;

      return offset;
    }

    encodeUint16(offset, directive, view) {
      if (directive.type != ReturnTypeId.Uint16) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setUint16(offset, directive.value, true);
      offset += Move.MOVE_BY_16_BYTES;

      return offset;
    }

    encodeUint8(offset, directive, view) {
      if (directive.type != ReturnTypeId.Uint8) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setUint8(offset, directive.value, true);
      offset += Move.MOVE_BY_1_BYTES;

      return offset;
    }

    encodeBigInt64(offset, directive, view) {
      if (directive.type != ReturnTypeId.Int64) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setUint8(offset, directive.value, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeInt32(offset, directive, view) {
      if (directive.type != ReturnTypeId.Int32) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setInt32(offset, directive.value, true);
      offset += Move.MOVE_BY_32_BYTES;

      return offset;
    }

    encodeErrorCode(offset, directive, view) {
      if (directive.type != ReturnTypeId.ErrorCode) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setInt16(offset, directive.value, true);
      offset += Move.MOVE_BY_16_BYTES;

      return offset;
    }

    encodeInt16(offset, directive, view) {
      if (directive.type != ReturnTypeId.Int16) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setInt16(offset, directive.value, true);
      offset += Move.MOVE_BY_16_BYTES;

      return offset;
    }

    encodeInt8(offset, directive, view) {
      if (directive.type != ReturnTypeId.Int8) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setInt8(offset, directive.value, true);
      offset += Move.MOVE_BY_1_BYTES;

      return offset;
    }

    encodeText8(offset, directive, view) {
      if (directive.type != ReturnTypeId.Text8) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      const allocation_id = this.texts.writeUTF8ToMemory(directive.value);
      LOGGER.debug(
        "Reply::encodeText8: written text: ",
        directive,
        " into location_id: ",
        allocation_id,
      );
      view.setBigUint64(offset, allocation_id, true);
      offset += Move.MOVE_BY_64_BYTES;

      return offset;
    }

    encodeNone(offset, directive, view) {
      if (directive.type != ReturnTypeId.None) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      return offset;
    }

    encodeBool(offset, directive, view) {
      if (directive.type != ReturnTypeId.Bool) {
        throw new Error(`Reply type with id ${value.type} is not known`);
      }

      view.setUint8(offset, directive.type, true);
      offset += Move.MOVE_BY_1_BYTES;

      view.setUint8(offset, directive.value == true ? 1 : 0, true);
      offset += Move.MOVE_BY_1_BYTES;

      return offset;
    }

    static check_for_type(item, value_type_id) {
      const logger = LOGGER.scoped("Reply.check_for_type");
      logger.debug(
        "check value: ",
        item,
        " with value type id: ",
        value_type_id,
      );

      if (isUndefinedOrNull(item) && value_type_id == ReturnTypeId.None) {
        return value_type_id;
      }

      if (
        item instanceof Uint8Array &&
        value_type_id == ReturnTypeId.Uint8ArrayBuffer
      ) {
        return value_type_id;
      }

      if (
        item instanceof Uint16Array &&
        value_type_id == ReturnTypeId.Uint16ArrayBuffer
      ) {
        return value_type_id;
      }

      if (
        item instanceof Uint32Array &&
        value_type_id == ReturnTypeId.Uint32ArrayBuffer
      ) {
        return value_type_id;
      }

      if (
        item instanceof BigUint64Array &&
        value_type_id == ReturnTypeId.Uint64ArrayBuffer
      ) {
        return value_type_id;
      }

      if (
        item instanceof Int8Array &&
        value_type_id == ReturnTypeId.Int8ArrayBuffer
      ) {
        return value_type_id;
      }

      if (
        item instanceof ReplyError &&
        value_type_id == ReturnTypeId.ErrorCode
      ) {
        return value_type_id;
      }

      if (
        item instanceof Int16Array &&
        value_type_id == ReturnTypeId.Int16ArrayBuffer
      ) {
        return value_type_id;
      }

      if (
        item instanceof Int32Array &&
        value_type_id == ReturnTypeId.Int32ArrayBuffer
      ) {
        return value_type_id;
      }

      if (
        item instanceof BigInt64Array &&
        value_type_id == ReturnTypeId.Int64ArrayBuffer
      ) {
        return value_type_id;
      }

      if (typeof Document !== "undefined") {
        if (
          item instanceof Document &&
          value_type_id == ReturnTypeId.ExternalReference
        ) {
          return value_type_id;
        }
      }

      if (typeof Window !== "undefined") {
        if (
          item instanceof Window &&
          value_type_id == ReturnTypeId.ExternalReference
        ) {
          return value_type_id;
        }
      }

      if (typeof Node !== "undefined") {
        if (
          item instanceof Node &&
          value_type_id == ReturnTypeId.ExternalReference
        ) {
          return value_type_id;
        }
      }

      if (typeof Element !== "undefined") {
        if (
          item instanceof Element &&
          value_type_id == ReturnTypeId.ExternalReference
        ) {
          return value_type_id;
        }
      }

      const value_type = typeof item;
      switch (true) {
        case (value_type == "function" &&
          value_type_id == ReturnTypeId.ExternalReference) ||
          value_type_id == ReturnTypeId.InternalReference:
          return value_type_id;
        case value_type == "symbol" && value_type_id == ReturnTypeId.Object:
          return value_type_id;
        case value_type == "object" && value_type_id == ReturnTypeId.Object:
          return value_type_id;
        case value_type == "string" && value_type_id == ReturnTypeId.Text8:
          return value_type_id;
        case value_type == "boolean" && value_type_id == ReturnTypeId.Bool:
          return value_type_id;
        case value_type == "bigint" || value_type == "number":
          switch (true) {
            case value_type_id == ReturnTypeId.Uint8 &&
              item <= 255 &&
              item >= 0:
              return value_type_id;
            case value_type_id == ReturnTypeId.Uint16 &&
              item <= 65535 &&
              item >= 0:
              return value_type_id;
            case value_type_id == ReturnTypeId.Uint32 &&
              item <= 4294967295 &&
              item >= 0:
              return value_type_id;
            case value_type_id == ReturnTypeId.Int8 &&
              item <= 128 &&
              item >= -128:
              return value_type_id;
            case value_type_id == ReturnTypeId.Int16 &&
              item <= 127 &&
              item >= -128:
              return value_type_id;
            case value_type_id == ReturnTypeId.Int32 &&
              item <= -2147483648 &&
              item >= 2147483647:
              return value_type_id;
            case value_type_id == ReturnTypeId.ErrorCode:
              return value_type_id;
            case value_type_id == ReturnTypeId.Uint64:
              return value_type_id;
            case value_type_id == ReturnTypeId.Int64:
              return value_type_id;
            case value_type_id == ReturnTypeId.Float32:
              return value_type_id;
            case value_type_id == ReturnTypeId.Float64:
              return value_type_id;
          }
      }

      logger.debug("Unable to transform provided type: ", item);
      return null;
    }

    static transform_return_type(
      function_heap,
      dom_heap,
      object_heap,
      item,
      hint,
    ) {
      const logger = LOGGER.scoped("Reply.transform_return_type:");

      if (hint instanceof ListReturn || hint instanceof MultiReturn) {
        if (!(item instanceof Array)) {
          throw new Error(
            "item should be a List/Array for ListReturn/MultiReturn hints",
          );
        }

        const transformed = [];
        for (let index in item) {
          let value = Reply.transform_from_hint(
            function_heap,
            dom_heap,
            object_heap,
            item[index],
            hint,
            index,
          );
          transformed.push(value);
        }

        logger.debug(
          "Transformed singular input: ",
          item,
          " into: ",
          transformed,
        );
        return transformed;
      }

      const transformed_item = Reply.transform_from_hint(
        function_heap,
        dom_heap,
        object_heap,
        item,
        hint,
        0,
      );

      logger.debug("Transformed input: ", item, " into: ", transformed_item);
      return transformed_item;
    }

    static transform_from_hint(
      function_heap,
      dom_heap,
      object_heap,
      item,
      hint,
      index,
    ) {
      const logger = LOGGER.scoped("Reply.transform_from_hint");
      logger.debug(
        "Received value: ",
        item,
        " with hint: ",
        hint,
        " and states list: ",
        hint.states,
        "at index: ",
        index,
      );

      if (isUndefinedOrNull(item)) {
        return Reply.asNone();
      }

      if (item instanceof ReplyContainer) {
        return item;
      }

      if (item instanceof Function) {
        return Reply.asExternalReference(function_heap.create(item));
      }

      let value_type_id;

      // get the state for that index.
      const state_handle = hint.states[index];
      const states = state_handle.options;

      logger.debug(
        "State at index: ",
        index,
        " having states: ",
        states,
        " from handle type: ",
        state_handle.state_type,
      );

      for (let state_item_index in states) {
        let candidate = states[state_item_index];
        logger.debug(
          "Checking item: ",
          item,
          " against type=",
          candidate,
          " from states: ",
          states,
        );

        let result = this.check_for_type(item, candidate);
        logger.debug(
          "Type checking result for ",
          item,
          " against type=",
          candidate,
          " and confirm type to be: ",
          result,
        );
        if (isUndefinedOrNull(result)) continue;
        value_type_id = result;
        break;
      }

      logger.debug(
        "Extractd value_type_id: ",
        value_type_id,
        " for value:",
        item,
      );

      switch (value_type_id) {
        case ReturnTypeId.None:
          if (!IsUndefinedOrNull(item))
            throw new Error("Value must either be null or undefined");
          return Reply.asNone();
        case ReturnTypeId.ErrorCode:
          return Reply.asErrorCode(item);
        case ReturnTypeId.Bool:
          return Reply.asBool(item);
        case ReturnTypeId.Text8:
          return Reply.asText8(item);
        case ReturnTypeId.Int128:
          return Reply.asInt128(item.value_lsb, item.value_msb);
        case ReturnTypeId.Uint128:
          return Reply.asUint128(item.value_lsb, item.value_msb);
        case ReturnTypeId.Uint8:
          return Reply.asUint8(item);
        case ReturnTypeId.Uint16:
          return Reply.asUint16(item);
        case ReturnTypeId.Uint32:
          return Reply.asUint32(item);
        case ReturnTypeId.Uint64:
          return Reply.asUint64(item);
        case ReturnTypeId.Int8:
          return Reply.asInt8(item);
        case ReturnTypeId.Int16:
          return Reply.asInt16(item);
        case ReturnTypeId.Int32:
          return Reply.asInt32(item);
        case ReturnTypeId.Int64:
          return Reply.asInt64(item);
        case ReturnTypeId.Float32:
          return Reply.asFloat32(item);
        case ReturnTypeId.Float64:
          return Reply.asFloat64(item);
        case Reply.Uint8ArrayBuffer:
          return Reply.asUint8Array(item);
        case Reply.Uint16ArrayBuffer:
          return Reply.asUint8Array(item);
        case Reply.Uint32ArrayBuffer:
          return Reply.asUint8Array(item);
        case Reply.Uint64ArrayBuffer:
          return Reply.asUint8Array(item);
        case Reply.Int8ArrayBuffer:
          return Reply.asInt8Array(item);
        case Reply.Int16ArrayBuffer:
          return Reply.asInt16Array(item);
        case Reply.Int32ArrayBuffer:
          return Reply.asInt32Array(item);
        case Reply.Int64ArrayBuffer:
          return Reply.asInt64Array(item);
        case Reply.Float32ArrayBuffer:
          return Reply.asFloat32Array(item);
        case Reply.Float64ArrayBuffer:
          return Reply.asFloat64Array(item);
        case ReturnTypeId.MemorySlice:
          return Reply.asMemorySlice(item);
        case ReturnTypeId.InternalReference:
          return Reply.asInternalReference(item);
        case ReturnTypeId.ExternalReference:
          return Reply.asExternalReference(item);
        case ReturnTypeId.DOMObject:
          if (typeof Node !== "undefined") {
            if (!(item instanceof Node) && !(item instanceof FakeNode)) {
              throw new Error("Expected value to a Node or FakeNode instance");
            }
          }
          return Reply.asDOMObject(dom_heap.create(item));
        case ReturnTypeId.Object:
          return Reply.asObject(object_heap.create(item));
        default:
          throw new Error(
            "Transformation for type_hint: " +
              hint +
              " failed for value: " +
              item,
          );
      }
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
      if (!isBigIntOrNumber(value)) {
        throw new Error("Value must be bigint");
      }
      return Reply.asValue(
        ReturnTypeId.MemorySlice,
        isBigInt(value) ? value : BigInt(value),
      );
    }

    static asInternalReference(value) {
      if (!(value instanceof InternalPointer) && typeof value !== "bigint") {
        throw new Error("Value must be bigint/InternalPointer");
      }
      if (value instanceof InternalPointer) {
        return Reply.asValue(ReturnTypeId.InternalReference, value.value);
      }
      return Reply.asValue(ReturnTypeId.InternalReference, value);
    }

    static asExternalReference(value) {
      if (!(value instanceof ExternalPointer) && typeof value !== "bigint") {
        throw new Error("Value must be bigint/ExternalPointer");
      }
      if (value instanceof ExternalPointer) {
        return Reply.asValue(ReturnTypeId.ExternalReference, value.value);
      }
      return Reply.asValue(ReturnTypeId.ExternalReference, value);
    }

    static asNone() {
      return Reply.asValue(ReturnTypeId.None, null);
    }

    static asObject(value) {
      if (!(value instanceof ExternalPointer) && typeof value !== "bigint") {
        throw new Error("Value must be bigint/ExternalPointer");
      }
      if (value instanceof ExternalPointer) {
        return Reply.asValue(ReturnTypeId.Object, value.value);
      }
      return Reply.asValue(ReturnTypeId.Object, value);
    }

    static asDOMObject(value) {
      if (!(value instanceof ExternalPointer) && typeof value !== "bigint") {
        throw new Error("Value must be bigint/ExternalPointer");
      }
      if (value instanceof ExternalPointer) {
        return Reply.asValue(ReturnTypeId.DOMObject, value.value);
      }
      return Reply.asValue(ReturnTypeId.DOMObject, value);
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
        throw new Error("Value must be uint32");
      }
      return Reply.asValue(ReturnTypeId.Uint32, value);
    }

    static asErrorCode(value) {
      if (!isBigIntOrNumber(value) && !(value instanceof ReplyError)) {
        throw new Error("Value must be ReplyError or U16");
      }
      if (value instanceof ReplyError) {
        return Reply.asValue(ReturnTypeId.ErrorCode, value.code);
      }
      return Reply.asValue(ReturnTypeId.ErrorCode, value);
    }

    static asUint16(value) {
      if (typeof value !== "number") {
        throw new Error("Value must be uint16");
      }
      return Reply.asValue(ReturnTypeId.Uint16, value);
    }

    static asUint8(value) {
      if (typeof value !== "number") {
        throw new Error("Value must be uint8");
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
      return Reply.asValue(ReturnTypeId.Bool, value ? 1 : 0);
    }

    // asValue provides a clear indicator of what type the giving return value is
    // which provides an important metadata for encoding.
    static asValue(return_type, value) {
      if (!(return_type in ReturnTypeId.__INVERSE__)) {
        throw new Error(
          `ReturnValueTypes ${return_type} is not known for value_type: ${return_type}`,
        );
      }
      return new ReplyContainer(return_type, value);
    }
  }

  const MAKE_FUNCTION_HINT = new SingleReturn(
    new ThreeState(ThreeStateId.One, [ReturnTypeId.ExternalReference]),
  );

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

        return new ReturnHintValue(
          MAKE_FUNCTION_HINT,
          Reply.asExternalReference(external_id),
        );
      };

      return [move_by_next, register_function];
    },
  );

  const INVOKE = new BatchOperation(
    Operations.Invoke,
    (instance, operation_id, moved_by, operations, texts) => {
      if (operation_id != Operations.Invoke) {
        throw new Error(
          `Argument should be Operation.Invoke instead got: ${operation_id}`,
        );
      }

      return instance.parse_and_invoke(
        moved_by,
        operations,
        texts,
        function (result) {
          LOGGER.debug("invoke_no_return: ", result);
        },
      );
    },
  );

  const INVOKE_ASYNC = new BatchOperation(
    Operations.InvokeAsync,
    (instance, operation_id, moved_by, operations, texts) => {
      if (operation_id != Operations.InvokeAsync) {
        throw new Error(
          `Argument should be Operation.InvokeAsync instead got: ${operation_id}`,
        );
      }

      return instance.parse_and_invoke_async(
        moved_by,
        operations,
        texts,
        function (result) {
          LOGGER.debug("invoke_no_return: ", result);
        },
      );
    },
  );

  class AsyncTaskCollector {
    constructor(collect) {
      this.tasks = [];
      this.collect = collect || false;
    }

    disable() {
      this.collect = false;
    }

    enable() {
      this.collect = true;
    }

    add(item) {
      if (!(item instanceof Promise)) {
        throw new Error("Item must be a Promise");
      }
      if (!this.collect) return;
      this.tasks.push(item);
    }

    clear() {
      this.tasks.clear();
    }

    await_all() {
      return Promise.all(this.tasks);
    }
  }

  class BatchInstructions {
    constructor(
      memory_operator,
      text_codec,
      text_cache,
      reply_parser,
      async_tasks,
    ) {
      if (!(memory_operator instanceof MemoryOperator)) {
        throw new Error("Must be instance of MemoryOperator");
      }
      if (!(text_codec instanceof TextCodec)) {
        throw new Error("Must be instance of TextCodec");
      }
      if (!(text_cache instanceof SimpleStringCache)) {
        throw new Error("Must be instance of SimpleStringCache");
      }
      if (!(reply_parser instanceof Reply)) {
        throw new Error("Must be instance of Reply");
      }
      if (!(async_tasks instanceof AsyncTaskCollector)) {
        throw new Error("Must be instance of AsyncTaskCollector");
      }

      this.texts = text_codec;
      this.text_cache = text_cache;
      this.async_tasks = async_tasks;
      this.operator = memory_operator;
      this.reply_parser = reply_parser;
      this.module = memory_operator.get_module();

      // parser for return hints.
      this.return_hints = new ReturnHintParser(this.operator);

      // v2 function parameters handling
      this.parameter_v2 = new ParameterParserV2(
        this.operator,
        this.texts,
        this.text_cache,
      );

      this.operations = [MAKE_FUNCTION, INVOKE, INVOKE_ASYNC];
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
      const logger = LOGGER.scoped(
        "BatchInstructions.BatchParser::parse_instructions:",
      );

      logger.debug(
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

      logger.debug("extracted text8 from buffer -> ", text_utf8);

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
          logger.debug("Found Operations.Stop identification");
          break;
        }
      }

      logger.debug("Extracted batches: ", batches);
      return batches;
    }

    parse_parts(moved_by, operations, texts, has_callback_handle) {
      const logger = LOGGER.scoped("BatchInstructions.parse_parts:");

      const next_value_type = operations.getUint8(moved_by, true);
      moved_by += Move.MOVE_BY_1_BYTES;

      // read the external pointer we want registered
      let external_id = null;
      [moved_by, external_id] = this.parameter_v2.parseExternalPointer(
        moved_by,
        next_value_type,
        operations,
      );

      // read the internal pointer we want registered
      let callback_id = null;
      if (has_callback_handle) {
        const id_value_type = operations.getUint8(moved_by, true);
        moved_by += Move.MOVE_BY_1_BYTES;

        [moved_by, callback_id] = this.parameter_v2.parseInternalPointer(
          moved_by,
          id_value_type,
          operations,
        );

        logger.debug(
          "InternalPointer: ",
          callback_id,
          " with now index: ",
          moved_by,
        );
      }

      logger.debug(
        "ExternalPointer: ",
        external_id,
        " with now index: ",
        moved_by,
      );

      const [moved_after_return, return_hints] = this.parse_return_hints(
        moved_by,
        operations,
        texts,
      );
      logger.debug(
        "Extracted return type hint: ",
        return_hints,
        moved_after_return,
      );

      const [moved_after_parameters, parameters] = this.parse_arguments(
        moved_after_return,
        operations,
        texts,
      );

      moved_by = moved_after_parameters;

      logger.debug(
        "extracted arguments for call: ",
        moved_by,
        external_id,
        parameters,
      );

      if (has_callback_handle) {
        return [moved_by, external_id, callback_id, return_hints, parameters];
      }

      return [moved_by, external_id, return_hints, parameters];
    }

    parse_and_invoke(moved_by, operations, texts, result_callback) {
      const logger = LOGGER.scoped("BatchInstructions.parse_and_invoke:");

      let external_id, return_hints, parameters;
      [moved_by, external_id, return_hints, parameters] = this.parse_parts(
        moved_by,
        operations,
        texts,
        false,
      );

      logger.debug(
        "Extracted Arguments: ",
        moved_by,
        external_id,
        return_hints,
        parameters,
      );

      const callable = (instance) => {
        const callable = instance.function_heap.get(external_id.value);

        const result = callable.apply(instance, parameters);

        LOGGER.debug("Function returned: ", result);

        if (return_hints instanceof NoReturn) {
          return null;
        }

        if (!isUndefinedOrNull(result) && !isUndefinedOrNull(result_callback)) {
          result_callback(result);
        }

        const encoded_result = Reply.transform_return_type(
          instance.function_heap,
          instance.dom_heap,
          instance.object_heap,
          result,
          return_hints,
        );

        logger.debug(
          "result=",
          result,
          " with encoded_result=",
          encoded_result,
        );

        return return_hints.asValue(encoded_result);
      };

      return [moved_by, callable];
    }

    parse_and_invoke_async(moved_by, operations, texts, result_callback) {
      const logger = LOGGER.scoped("BatchInstructions.parse_and_invoke_async:");

      let external_id, callback_id, return_hints, parameters;
      [moved_by, external_id, callback_id, return_hints, parameters] =
        this.parse_parts(moved_by, operations, texts, true);

      logger.debug(
        "Extracted Arguments: ",
        moved_by,
        external_id,
        callback_id,
        return_hints,
        parameters,
      );

      const callbable = (instance) => {
        const callable = instance.function_heap.get(external_id.value);

        const promise = callable.apply(instance, parameters);
        if (!(promise instanceof Promise)) {
          throw new Error("Result of function call must be a Promise");
        }

        // if no return is expected then we can leave this and not
        // check the promise
        if (return_hints instanceof NoReturn) {
          return null;
        }

        const success = (result) => {
          if (
            !isUndefinedOrNull(result) &&
            !isUndefinedOrNull(result_callback)
          ) {
            result_callback(result);
          }

          logger.debug("Promise resolved to result=", result);

          this.reply_parser.callback_success(callback_id, return_hints, result);
        };

        const failure = (error) => {
          logger.debug("Promise rejected with error: ", error);

          if (!(error instanceof ReplyError)) {
            throw new Error("Errors must be an instance of ReplyError");
          }

          this.reply_parser.callback_failure(
            callback_id,
            this.asErrorCode(error),
          );

          return 0;
        };

        promise
          .then(success.bind(this))
          .catch(failure.bind(this))
          .then((v) => logger.debug("ending operation with: ", v));

        this.async_tasks.add(promise);
      };

      return [moved_by, callbable];
    }

    parse_return_hints(moved_by, operations, texts) {
      let return_hints = null;
      [moved_by, return_hints] = this.return_hints.parse_from(
        moved_by,
        operations,
      );
      if (!(return_hints instanceof ReturnHint)) {
        throw new Error("value should be a type of ReturnHint validator");
      }
      return [moved_by, return_hints];
    }

    parse_arguments(moved_by, operations, texts) {
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

      return [moved_by, parameters];
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

      // return hints instruction parser
      this.return_hints = null;

      // reply instruction parser
      this.reply_parser = null;

      // collector for async tasks, enabled by default.
      this.async_tasks = new AsyncTaskCollector(false);
    }

    init(wasm_module) {
      // the ctx (context) allowing access
      // to all classes and systems
      // the megatron module has.
      this.LIBRARY = CONTEXT;

      // the wasm module
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

      // parser for return hints.
      this.return_hints = new ReturnHintParser(this.operator);

      // text codec for text handling (encoding & decoding)
      this.texts = new TextCodec(this.operator);

      // v1 function parameters handling
      this.parameter_v1 = new ParameterParserV1(
        this.operator,
        this.texts,
        this.string_cache,
      );

      // reply parameter parser
      this.reply_parser = new Reply(
        this.operator,
        this.texts,
        this.string_cache,
        this.function_heap,
        this.object_heap,
        this.dom_heap,
      );

      // v2 batch instruction handling
      this.batch_parser = new BatchInstructions(
        this.operator,
        this.texts,
        this.string_cache,
        this.reply_parser,
        this.async_tasks,
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
        host_batch_apply: this.no_return_instructions.bind(this),
        host_batch_returning_apply: this.returning_instructions.bind(this),

        // string caching and interning
        host_cache_string: this.host_cache_string.bind(this),

        // registration binding
        host_register_function: this.host_register_function.bind(this),

        // invocation bindings
        //
        // 1. Generic Binding
        host_invoke_function: this.host_invoke_function.bind(this),

        // 2. Callback Binding
        host_invoke_async_function: this.host_invoke_async_function.bind(this),

        // host_invoke_function_as_dom:
        //   this.host_invoke_function_as_dom.bind(this),
        // host_invoke_function_as_object:
        //   this.host_invoke_function_as_object.bind(this),

        // 3. Specific Bindings
        host_invoke_function_as_bool:
          this.host_invoke_function_as_bool.bind(this),
        host_invoke_function_as_f64:
          this.host_invoke_function_as_float.bind(this),
        host_invoke_function_as_f32:
          this.host_invoke_function_as_float.bind(this),
        host_invoke_function_as_u8: this.host_invoke_function_as_int.bind(this),
        host_invoke_function_as_u16:
          this.host_invoke_function_as_int.bind(this),
        host_invoke_function_as_u32:
          this.host_invoke_function_as_int.bind(this),
        host_invoke_function_as_u64:
          this.host_invoke_function_as_bigint.bind(this),
        host_invoke_function_as_i8: this.host_invoke_function_as_int.bind(this),
        host_invoke_function_as_i16:
          this.host_invoke_function_as_int.bind(this),
        host_invoke_function_as_i32:
          this.host_invoke_function_as_int.bind(this),
        host_invoke_function_as_i64:
          this.host_invoke_function_as_bigint.bind(this),
        host_invoke_function_as_str:
          this.host_invoke_function_as_string.bind(this),
      };
    }

    await_tasks() {
      return this.async_tasks.await_all();
    }

    collect_async_tasks() {
      this.async_tasks.enable();
    }

    abort() {
      throw new Error("WasmInstance calls abort");
    }

    no_return_instructions(ops_pointer, ops_length, text_pointer, text_length) {
      const logger = LOGGER.scoped(
        "MegatronMiddleware.returning_instructions: ",
      );
      const instructions = this.batch_parser.parse_instructions(
        ops_pointer,
        ops_length,
        text_pointer,
        text_length,
      );

      logger.debug("Received instructions for batch: ", instructions);

      for (let index in instructions) {
        const instruction = instructions[index];
        instruction.call(this, this);
      }
    }

    returning_instructions(ops_pointer, ops_length, text_pointer, text_length) {
      const logger = LOGGER.scoped(
        "MegatronMiddleware.returning_instructions:",
      );

      const instructions = this.batch_parser.parse_instructions(
        ops_pointer,
        ops_length,
        text_pointer,
        text_length,
      );

      logger.debug("Received instructions for batch:", instructions);

      const results = [];
      for (let index in instructions) {
        const instruction = instructions[index];
        let result = instruction.call(this, this);
        if (isUndefinedOrNull(result)) continue;

        logger.debug(
          "Received result value:",
          result,
          " to be written into memory",
        );

        const alloc_id = this.reply_parser.encode_into_memory(result.value);
        logger.debug(
          "Written result into memory: mem_id:",
          alloc_id,
          " with result:",
          result,
        );

        results.push([result, alloc_id]);
      }

      logger.debug("All results memory id:", results);

      return this.reply_parser.write_group_return(results);
    }

    get_pending_async() {
      return this.pending_promises;
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
      const logger = LOGGER.scoped(
        "MegatronMiddlewarte.object_allocate_external_pointer",
      );
      const slot_id = this.object_heap.create(null);
      logger.debug("Creating new object pointer for pre-allocation: ", slot_id);
      return slot_id;
    }

    function_allocate_external_pointer() {
      const logger = LOGGER.scoped(
        "MegatronMiddlewarte.function_allocate_external_pointer",
      );
      const slot_id = this.function_heap.create(null);
      logger.debug(
        "Creating new function pointer for pre-allocation: ",
        slot_id,
      );
      return slot_id;
    }

    dom_allocate_external_pointer() {
      const logger = LOGGER.scoped(
        "MegatronMiddlewarte.dom_allocate_external_pointer",
      );
      const slot_id = this.dom_heap.create(null);
      logger.debug("Creating new DOM pointer for pre-allocation: ", slot_id);
      return slot_id;
    }

    host_cache_string(start, length, utf_indicator) {
      const logger = LOGGER.scoped(
        "MegatronMiddlewarte.host_register_function",
      );
      logger.debug(
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

      logger.debug(`Generated cache string: ${target_string}`);

      if (!target_string) throw new Error("No valid string was provided");

      return BigInt(this.string_cache.create(target_string));
    }

    host_register_function(start, length, utf_indicator) {
      const logger = LOGGER.scoped(
        "MegatronMiddlewarte.host_register_function",
      );
      logger.debug(
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

    host_invoke_function_for_return(
      handle,
      parameter_start,
      parameter_length,
      return_hints,
    ) {
      if (isUndefinedOrNull(handle)) {
        throw new Error("handle: Must provide function handle id");
      }
      const logger = LOGGER.scoped(
        "MegatronMiddlewarte.host_invoke_function_for_return:",
      );
      // read parameters and invoke function via handle.
      const parameters = this.parameter_v1.parse_array(
        parameter_start,
        parameter_length,
      );

      if (!parameters && parameter_length > 0)
        throw new Error("No parameters returned though we expect some");

      logger.debug(
        `Parameters=${parameters} (type=${typeof parameters})`,
        parameters,
      );

      const func = this.function_heap.get(handle);

      const response = func.call(this, ...parameters);
      logger.debug("result=", response);

      return this.reply_parser.immediate(return_hints, response);
    }

    host_invoke_function_with_return(
      handle,
      parameter_start,
      parameter_length,
      returns_start,
      returns_length,
    ) {
      if (isUndefinedOrNull(handle)) {
        throw new Error("handle: Must provide function handle id");
      }
      const logger = LOGGER.scoped(
        "MegatronMiddlewarte.host_invoke_function_for_return:",
      );

      // read parameters and invoke function via handle.
      const parameters = this.parameter_v1.parse_array(
        parameter_start,
        parameter_length,
      );

      if (!parameters && parameter_length > 0)
        throw new Error("No parameters returned though we expect some");

      logger.debug(
        `Parameters=${parameters} (type=${typeof parameters})`,
        parameters,
      );

      const [read, return_hints] = this.return_hints.parse_hint(
        returns_start,
        returns_length,
      );
      logger.debug(`Return hints(read=${read}): ${return_hints}`);

      const func = this.function_heap.get(handle);

      const response = func.call(this, ...parameters);
      logger.debug("result=", response);

      return this.reply_parser.immediate(return_hints, response);
    }

    host_invoke_async_function(
      handle,
      callback_handle,
      parameter_start,
      parameter_length,
      returns_start,
      returns_length,
    ) {
      if (isUndefinedOrNull(handle)) {
        throw new Error("handle: Must provide function handle id");
      }

      if (isUndefinedOrNull(callback_handle)) {
        throw new Error("callback_handle: Must provide function handle id");
      }

      const logger = LOGGER.scoped(
        "MegatronMiddlewarte.host_invoke_async_function: ",
      );

      // read parameters and invoke function via handle.
      const parameters = this.parameter_v1.parse_array(
        parameter_start,
        parameter_length,
      );

      if (!parameters && parameter_length > 0)
        throw new Error("No parameters returned though we expect some");

      const [read, return_hints] = this.return_hints.parse_hint(
        returns_start,
        returns_length,
      );
      logger.debug(`Return hints(read=${read}): ${return_hints}`);

      const func = this.function_heap.get(handle);
      const promise = func.call(this, ...parameters);
      if (!(promise instanceof Promise)) {
        throw new Error("Result of function call must be a Promise");
      }

      // if no return is expected then we can leave this and not
      // check the promise
      if (return_hints instanceof NoReturn) {
        return;
      }

      const callback_id = new InternalPointer(BigInt(callback_handle));

      const success = (result) => {
        logger.debug("Promise resolved to result=", result);
        this.reply_parser.callback_success(callback_id, return_hints, result);
      };

      const failure = (error) => {
        logger.debug("Promise rejected with error: (", error, ")");

        if (!(error instanceof ReplyError)) {
          throw new Error("Errors must be an instance of ReplyError");
        }

        const error_container = this.asErrorCode(error);
        logger.debug(
          "Wrapping returned error=",
          error,
          " in container: ",
          error_container,
        );

        this.reply_parser.callback_failure(callback_id, error_container);

        logger.debug("Sent error to callback");

        return 0;
      };

      promise
        .then(success.bind(this))
        .catch(failure.bind(this))
        .then((v) => logger.debug("ending operation with: ", v));

      this.async_tasks.add(promise);
    }

    host_invoke_function(
      handle,
      parameter_start,
      parameter_length,
      returns_start,
      returns_length,
    ) {
      const logger = LOGGER.scoped("MegatronMiddlewarte.host_invoke_function");

      logger.debug("Arguments: ", {
        handle,
        parameter_start,
        parameter_length,
        returns_start,
        returns_length,
      });

      const result = this.host_invoke_function_with_return(
        handle,
        parameter_start,
        parameter_length,
        returns_start,
        returns_length,
      );

      logger.debug("Call returned: ", result);

      if (isUndefinedOrNull(result)) return BigInt(-1);
      if (!isBigIntOrNumber(result)) throw new Error("Not a BigInt or Number");
      if (isNumber(result)) return BigInt(result);
      return result;
    }

    host_invoke_function_as_dom(handle, parameter_start, parameter_length) {
      return this.host_invoke_function_for_return(
        handle,
        parameter_start,
        parameter_length,
        new SingleReturn(
          new ThreeState(ThreeStateId.One, [ReturnTypeId.DOMObject]),
        ),
      );
    }

    host_invoke_function_as_object(handle, parameter_start, parameter_length) {
      return this.host_invoke_function_for_return(
        handle,
        parameter_start,
        parameter_length,
        new SingleReturn(
          new ThreeState(ThreeStateId.One, [ReturnTypeId.Object]),
        ),
      );
    }

    host_invoke_function_as_bool(handle, parameter_start, parameter_length) {
      const response = this.host_invoke_function_for_return(
        handle,
        parameter_start,
        parameter_length,
        new SingleReturn(new ThreeState(ThreeStateId.One, [ReturnTypeId.Bool])),
      );
      return response ? 1 : 0;
    }

    host_invoke_function_as_float(handle, parameter_start, parameter_length) {
      const response = this.host_invoke_function_for_return(
        handle,
        parameter_start,
        parameter_length,
        new SingleReturn(
          new ThreeState(ThreeStateId.One, [ReturnTypeId.Float64]),
        ),
      );
      if (typeof response != "number") {
        throw new Error(`Response ${response} is not a number`);
      }
      return response;
    }

    host_invoke_function_as_int(handle, parameter_start, parameter_length) {
      const response = this.host_invoke_function_for_return(
        handle,
        parameter_start,
        parameter_length,
        new SingleReturn(
          new ThreeState(ThreeStateId.One, [ReturnTypeId.Int64]),
        ),
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
      const response = this.host_invoke_function_for_return(
        handle,
        parameter_start,
        parameter_length,
        new SingleReturn(
          new ThreeState(ThreeStateId.One, [ReturnTypeId.Uint64]),
        ),
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

    asNone() {
      return Reply.asNone();
    }

    asObject(value) {
      if (typeof value !== "object") {
        throw new Error("Value must be a JS Object/object");
      }

      const id = this.object_heap.create(value);
      return Reply.asObject(id);
    }

    asDOMObject(value) {
      if (!(value instanceof FakeNode) && !(value instanceof Node)) {
        throw new Error("Value must be a DOM Node/FakeNode");
      }

      const dom_id = this.dom_heap.create(value);
      return Reply.asDOMObject(dom_id);
    }

    asFakeNode(tag) {
      if (typeof tag !== "string") {
        throw new Error("Value must be a JS string");
      }
      return this.asDOMObject(new FakeNode(tag));
    }

    asReplyError(value) {
      return new ReplyError(value);
    }

    raiseErrorCode(value) {
      throw new ReplyError(value);
    }

    asErrorCode(value) {
      return Reply.asErrorCode(value);
    }

    asFloat64(value) {
      return Reply.asFloat64(value);
    }

    asFloat32(value) {
      return Reply.asFloat32(value);
    }

    asInt128(value_lsb, value_msb) {
      return Reply.asInt128(value_lsb, value_msb);
    }

    asUint128(value_lsb, value_msb) {
      return Reply.asUint128(value_lsb, value_msb);
    }

    asUint64(value) {
      return Reply.asUint64(value);
    }

    asUint32(value) {
      return Reply.asUint32(value);
    }

    asErrorCode(value) {
      return Reply.asErrorCode(value);
    }

    asUint16(value) {
      return Reply.asUint16(value);
    }

    asUint8(value) {
      return Reply.asUint8(value);
    }

    asInt64(value) {
      return Reply.asInt64(value);
    }

    asInt32(value) {
      return Reply.asInt32(value);
    }

    asInt16(value) {
      return Reply.asInt16(value);
    }

    asInt8(value) {
      return Reply.asInt8(value);
    }

    asText8(value) {
      return Reply.asText8(value);
    }

    asBool(value) {
      return Reply.asBool(value);
    }

    asFloat64Array(value) {
      return Reply.asFloat64Array(value);
    }

    asFloat32Array(value) {
      return Reply.asFloat32Array(value);
    }

    asInt64Array(value) {
      return Reply.asInt64Array(value);
    }

    asInt32Array(value) {
      return Reply.asInt32Array(value);
    }

    asInt16Array(value) {
      return Reply.asInt16Array(value);
    }

    asInt8Array(value) {
      return Reply.asInt8Array(value);
    }

    asUint64Array(value) {
      return Reply.asUint64Array(value);
    }

    asUint32Array(value) {
      return Reply.asUint32Array(value);
    }

    asUint16Array(value) {
      return Reply.asUint16Array(value);
    }

    asUint8Array(value) {
      return Reply.asUint8Array(value);
    }

    asMemorySlice(value) {
      return Reply.asMemorySlice(value);
    }

    asInternalReference(value) {
      return Reply.asInternalReference(value);
    }

    asExternalReference(value) {
      return Reply.asExternalReference(value);
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

  // Returns and Result replies
  CONTEXT.Reply = Reply;

  // Base loggers and types
  CONTEXT.LOGGER = LOGGER;
  CONTEXT.LEVELS = LEVELS;

  // Core types
  CONTEXT.Move = Move;
  CONTEXT.Params = Params;
  CONTEXT.Operations = Operations;
  CONTEXT.ReplyError = ReplyError;
  CONTEXT.TypedSlice = TypedSlice;
  CONTEXT.ReturnIds = ReturnIds;
  CONTEXT.ReturnTypeId = ReturnTypeId;
  CONTEXT.TypeOptimization = TypeOptimization;
  CONTEXT.ArgumentOperations = ArgumentOperations;

  // Support classes and functions
  CONTEXT.FakeNode = FakeNode;
  CONTEXT.DOMArena = DOMArena;
  CONTEXT.TextCodec = TextCodec;
  CONTEXT.WASMLoader = WASMLoader;
  CONTEXT.CachePointer = CachePointer;
  CONTEXT.ArenaAllocator = ArenaAllocator;
  CONTEXT.MemoryOperator = MemoryOperator;
  CONTEXT.ExternalPointer = ExternalPointer;
  CONTEXT.InternalPointer = InternalPointer;
  CONTEXT.TypedArraySlice = TypedArraySlice;

  // Arguments parsers (immediate and batch)
  CONTEXT.BatchInstructions = BatchInstructions;
  CONTEXT.ParameterParserV1 = ParameterParserV1;
  CONTEXT.ParameterParserV2 = ParameterParserV2;

  // Core classes and managers
  CONTEXT.WasmWebScripts = WasmWebScripts;
  CONTEXT.MegatronMiddleware = MegatronMiddleware;

  return {
    LEVELS,
    LOGGER,
    WasmWebScripts,
    MegatronMiddleware,
  };
})();

if (typeof module !== "undefined") {
  module.exports = Megatron;
}
