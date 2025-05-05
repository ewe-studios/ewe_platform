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

  read_uint8_buffer(start_pointer, len) {
    const memory = this.get_memory();
    const slice = memory.slice(start_pointer, start_pointer + len);
    return new Uint8Array(slice);
  }

  write_int8_buffer(uint8_buffer) {
    const len = uint8_buffer.length;
    const [id, start] = this.allocate_memory(len);

    const memory = this.get_memory();
    memory.set(uint8_buffer, start);

    return id;
  }

  write_array_buffer(array_buffer) {
    const uint8_buffer = new Uint8Array(array_buffer);
    return this.write_int8_buffer(uint8_buffer);
  }
}
