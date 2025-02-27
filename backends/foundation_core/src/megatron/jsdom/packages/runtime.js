// Runtime module for the megatron WASM bindgen which provides core functionality
// required to enjoy and interact with the browser and APIs.
//
// It's meant to both be simple but also extensively useful.

const ARENA = function () {
  // Javascript module implements the necessary ArenaList used for
  // representing different javascript objects that are cached and
  // controlled by WebAssembly, though the structure can be used for
  // any usecase where control and optimized usage of a list is important.

  const MAX_SIZE = BigInt(0xFFFFFFF0);
  const BIT_MASK = BigInt(0xFFFFFFFF);
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
      add: add_entry,
      get: get_entry,
      remove: remove_entry,
    };
  }

  let arena_instance = create_arena();
  arena_instance.create = create_entry;
  arena_instance.to_uid = entry_to_uid;
  arena_instance.from_uid = uid_to_entry;

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
  //
  arena_instance.create(undefined);
  arena_instance.create(null);
  arena_instance.create(self);
  arena_instance.create(typeof window != "undefined" ? window : null);
  arena_instance.create(typeof document != "undefined" ? document : null);
  arena_instance.create(typeof document != "undefined" ? document.body : null);

  return arena_instance;
}();
