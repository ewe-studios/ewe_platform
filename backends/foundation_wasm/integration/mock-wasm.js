// A minimal in-JS stand-in for a real WASM instance, so the foundation-wasm.js core
// classes can be unit-tested without compiling a .wasm module.
//
// It models the arena as a bump allocator over a real ArrayBuffer (the "linear
// memory") and records calls to the export functions tests care about.

/**
 * @param {number} [memoryBytes]
 * @returns {{ memory: { buffer: ArrayBuffer }, exports: object, calls: object }}
 */
export function makeMockWasm(memoryBytes = 64 * 1024) {
  const memory = { buffer: new ArrayBuffer(memoryBytes) };

  // arena: id -> { ptr, len }
  const slots = new Map();
  let nextId = 1n;
  let bump = 16; // leave a little headroom at offset 0

  const calls = {
    run_scheduled_callback: [],
    run_interval_callback: [],
    invoke_callback: [],
    unregister_callback: [],
    dispose_allocation: [],
    clear_allocation: [],
  };

  // Lets a test make the next interval tick ask to stop (return 0).
  let intervalStopAt = new Map(); // id(string) -> remaining ticks before STOP

  const exports = {
    create_allocation(size) {
      const len = Number(size);
      const ptr = bump;
      bump += Math.max(len, 1);
      const id = nextId++;
      slots.set(id, { ptr, len });
      // zero-fill, mirroring the Rust arena (vec![0; size]).
      new Uint8Array(memory.buffer, ptr, len).fill(0);
      return id; // u64 -> bigint
    },
    allocation_start_pointer(id) {
      const slot = slots.get(BigInt(id));
      if (!slot) throw new Error(`invalid allocation id: ${id}`);
      return slot.ptr; // *const u8 -> Number (wasm32)
    },
    allocation_length(id) {
      const slot = slots.get(BigInt(id));
      if (!slot) throw new Error(`invalid allocation id: ${id}`);
      return BigInt(slot.len); // u64 -> bigint
    },
    dispose_allocation(id) {
      calls.dispose_allocation.push(BigInt(id));
      slots.delete(BigInt(id));
    },
    clear_allocation(id) {
      calls.clear_allocation.push(BigInt(id));
      const slot = slots.get(BigInt(id));
      if (slot) new Uint8Array(memory.buffer, slot.ptr, slot.len).fill(0);
    },
    run_scheduled_callback(id) {
      calls.run_scheduled_callback.push(BigInt(id));
    },
    run_interval_callback(id) {
      calls.run_interval_callback.push(BigInt(id));
      const key = String(id);
      if (intervalStopAt.has(key)) {
        const left = intervalStopAt.get(key) - 1;
        intervalStopAt.set(key, left);
        if (left <= 0) return 0; // STOP
      }
      return 1; // REQUEUE
    },
    invoke_callback(internalPointer, allocationId) {
      calls.invoke_callback.push([BigInt(internalPointer), BigInt(allocationId)]);
    },
    unregister_callback(addr) {
      calls.unregister_callback.push(BigInt(addr));
    },
    // test helper, not part of the real ABI
    __stopIntervalAfter(id, ticks) {
      intervalStopAt.set(String(id), ticks);
    },
    __slotCount() {
      return slots.size;
    },
  };

  return { memory, exports, calls };
}
