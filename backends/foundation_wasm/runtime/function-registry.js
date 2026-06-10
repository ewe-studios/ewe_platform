// function-registry.js — WASM↔JS function-call ABI codec (ported from megatron's
// ParameterParserV1 + ReturnHintParser + Reply, per feature 17 research docs).
//
// FAITHFULNESS: this is the FLAT invoke encoding — `[ParamTypeId:u8][value]` per param,
// concatenated (Rust `Params::to_binary` ↔ this `ParameterParser`). The marker/quantized
// batch encoding (V2) is a SEPARATE codec. Discriminants and byte layouts mirror
// `foundation_wasm/src/{base.rs, ops.rs, protocol.rs}` exactly — see research-core-types.md.

// Move-by widths (megatron's MOVE_BY_N_BYTES counts BITS for 16/32/64): bytes consumed.
const B1 = 1, B2 = 2, B4 = 4, B8 = 8;

// ─── Reference wrappers (RefPointer family) ─────────────────────────────────────
export class RefPointer {
  constructor(id) { this.id = id; }
  get value() { return this.id; }
}
export class ExternalPointer extends RefPointer {}
export class InternalPointer extends RefPointer {}
export class CachePointer extends RefPointer {}
export class ErrorCodeValue { constructor(code) { this.code = code; } }
export class TypedArraySliceValue {
  constructor(sliceType, content) { this.sliceType = sliceType; this.content = content; }
}

// ─── Shared discriminants (the cross-language contract) ─────────────────────────
export const ParamType = Object.freeze({
  Null: 0, Undefined: 1, Bool: 2, Text8: 3, Text16: 4, Int8: 5, Int16: 6, Int32: 7,
  Int64: 8, Uint8: 9, Uint16: 10, Uint32: 11, Uint64: 12, Float32: 13, Float64: 14,
  ExternalReference: 15, Uint8Array: 16, Uint16Array: 17, Uint32Array: 18, Uint64Array: 19,
  Int8Array: 20, Int16Array: 21, Int32Array: 22, Int64Array: 23, Float32Array: 24,
  Float64Array: 25, InternalReference: 26, Int128: 27, Uint128: 28, CachedText: 29,
  TypedArraySlice: 30, ErrorCode: 31,
});

export const ReturnType = Object.freeze({
  Bool: 1, Text8: 2, Int8: 3, Int16: 4, Int32: 5, Int64: 6, Uint8: 7, Uint16: 8, Uint32: 9,
  Uint64: 10, Float32: 11, Float64: 12, Int128: 13, Uint128: 14, MemorySlice: 15,
  ExternalReference: 16, InternalReference: 17, Object: 28, DOMObject: 29, None: 30,
  ErrorCode: 31, TypedArraySlice: 32,
});

const RETURN_NAKED = new Set([
  ReturnType.Bool, ReturnType.Uint8, ReturnType.Uint16, ReturnType.Uint32, ReturnType.Uint64,
  ReturnType.Int8, ReturnType.Int16, ReturnType.Int32, ReturnType.Int64, ReturnType.Float32,
  ReturnType.Float64, ReturnType.Object, ReturnType.DOMObject, ReturnType.ErrorCode,
  ReturnType.MemorySlice, ReturnType.InternalReference, ReturnType.ExternalReference,
]);

export const ReturnIds = Object.freeze({ None: 0, One: 1, Multi: 2, List: 3 });
export const ThreeStateId = Object.freeze({ One: 70, Two: 80, Three: 90 });
export const ReturnHintMarker = Object.freeze({ Start: 200, Stop: 201 });
// The reply ReturnValues binary is framed Begin..End (Rust `FromBinary for ReturnTypeHints`).
export const ReturnValueMarker = Object.freeze({ Begin: 100, End: 101 });
export const TypedSliceArray = {
  1: Int8Array, 2: Int16Array, 3: Int32Array, 4: BigInt64Array, 5: Uint8Array,
  6: Uint16Array, 7: Uint32Array, 8: BigUint64Array, 9: Float32Array, 10: Float64Array,
};

// ─── ExternalHeap (generation-based arena, = megatron ArenaAllocator) ───────────

/**
 * Host-side heap for objects referenced across the ABI by `ExternalPointer` ids.
 * Ids pack `(index << 32) | generation` (bigint) — same scheme as the Rust arena —
 * so stale ids fail the generation check instead of resolving to a reused slot.
 * `create(null)` pre-allocates a handle (the `*_allocate_external_pointer` imports);
 * `update` fills it later (e.g. batch MakeFunction).
 */
export class ExternalHeap {
  constructor() {
    this.items = []; // { item, generation, active }
    this.free = [];
  }

  #unpack(uid) {
    const v = BigInt(uid);
    return { index: Number(v >> 32n), generation: v & 0xffffffffn };
  }

  /** Allocate a slot for `item` (may be null) → packed uid (bigint). */
  create(item) {
    let index;
    if (this.free.length > 0) {
      index = this.free.pop();
      const slot = this.items[index];
      slot.generation += 1n;
      slot.active = true;
      slot.item = item;
    } else {
      index = this.items.length;
      this.items.push({ item, generation: 0n, active: true });
    }
    return (BigInt(index) << 32n) | this.items[index].generation;
  }

  /** Resolve a uid → item (undefined when stale/missing). */
  get(uid) {
    const { index, generation } = this.#unpack(uid);
    const slot = this.items[index];
    if (!slot || !slot.active || slot.generation !== generation) return undefined;
    return slot.item;
  }

  /** Replace the item at a live uid (pre-allocated handles). False when stale. */
  update(uid, item) {
    const { index, generation } = this.#unpack(uid);
    const slot = this.items[index];
    if (!slot || slot.generation !== generation) return false;
    slot.item = item;
    slot.active = true;
    return true;
  }

  /** Retire a uid; its slot is recycled with a bumped generation. */
  destroy(uid) {
    const { index, generation } = this.#unpack(uid);
    const slot = this.items[index];
    if (!slot || !slot.active || slot.generation !== generation) return false;
    slot.item = null;
    slot.active = false;
    this.free.push(index);
    return true;
  }
}

/**
 * Best-effort map from a concrete JS value to a `ReturnType`, used only to resolve a
 * union ThreeState (Two/Three) where the hint allows several types. Non-union states
 * never call this. The Rust `ReturnValueParserIter` validates the choice.
 */
function inferReturnType(value) {
  switch (typeof value) {
    case "boolean": return ReturnType.Bool;
    case "string": return ReturnType.Text8;
    case "bigint": return ReturnType.Int64;
    case "number": return Number.isInteger(value) ? ReturnType.Int32 : ReturnType.Float64;
    default:
      if (value instanceof ErrorCodeValue) return ReturnType.ErrorCode;
      if (value instanceof ExternalPointer) return ReturnType.ExternalReference;
      if (value instanceof InternalPointer) return ReturnType.InternalReference;
      return ReturnType.MemorySlice;
  }
}

// ─── ParameterParser (FLAT, = ParameterParserV1) ────────────────────────────────

/**
 * Decode `host_invoke_function` params: `[ParamType:u8][value]` repeated until the
 * buffer is consumed. Text/arrays carry `[ptr:u64][len:u64]` into WASM memory.
 */
export class ParameterParser {
  /** @param {{memory:WebAssembly.Memory}} bridge @param {import('./foundation-wasm.js').StringCache} strings */
  constructor(bridge, strings) {
    this.bridge = bridge;
    this.strings = strings;
  }

  /** @returns {any[]} decoded positional args */
  parse(ptr, len) {
    const start = Number(ptr);
    const total = Number(len);
    const view = new DataView(this.bridge.memory.buffer, start, total);
    const args = [];
    let i = 0;
    while (i < total) {
      const type = view.getUint8(i);
      i += B1;
      i = this.#one(type, i, view, args);
    }
    return args;
  }

  #slice(view, i) {
    // [ptr:u64 LE][len:u64 LE] → (byteStart, byteLen) into WASM memory
    const ptr = Number(view.getBigUint64(i, true));
    i += B8;
    const len = Number(view.getBigUint64(i, true));
    i += B8;
    return [i, ptr, len];
  }

  #one(type, i, view, args) {
    const mem = this.bridge.memory.buffer;
    switch (type) {
      case ParamType.Null: args.push(null); return i;
      case ParamType.Undefined: args.push(undefined); return i;
      case ParamType.Bool: args.push(view.getUint8(i) === 1); return i + B1;
      case ParamType.Int8: args.push(view.getInt8(i)); return i + B1;
      case ParamType.Uint8: args.push(view.getUint8(i)); return i + B1;
      case ParamType.Int16: args.push(view.getInt16(i, true)); return i + B2;
      case ParamType.Uint16: args.push(view.getUint16(i, true)); return i + B2;
      case ParamType.ErrorCode: args.push(new ErrorCodeValue(view.getUint16(i, true))); return i + B2;
      case ParamType.Int32: args.push(view.getInt32(i, true)); return i + B4;
      case ParamType.Uint32: args.push(view.getUint32(i, true)); return i + B4;
      case ParamType.Float32: args.push(view.getFloat32(i, true)); return i + B4;
      case ParamType.Float64: args.push(view.getFloat64(i, true)); return i + B8;
      case ParamType.Int64: args.push(view.getBigInt64(i, true)); return i + B8;
      case ParamType.Uint64: args.push(view.getBigUint64(i, true)); return i + B8;
      case ParamType.ExternalReference: args.push(new ExternalPointer(view.getBigUint64(i, true))); return i + B8;
      case ParamType.InternalReference: args.push(new InternalPointer(view.getBigUint64(i, true))); return i + B8;
      case ParamType.CachedText: {
        const handle = view.getBigUint64(i, true);
        const text = this.strings.get(handle);
        if (text === undefined) throw new Error(`CachedText: no string for handle ${handle}`);
        args.push(text);
        return i + B8;
      }
      case ParamType.Int128: {
        const msb = view.getBigInt64(i, true), lsb = view.getBigUint64(i + B8, true);
        args.push((msb << 64n) | lsb);
        return i + B8 + B8;
      }
      case ParamType.Uint128: {
        const msb = view.getBigUint64(i, true), lsb = view.getBigUint64(i + B8, true);
        args.push((msb << 64n) | lsb);
        return i + B8 + B8;
      }
      case ParamType.Text8: {
        const [ni, p, l] = this.#slice(view, i);
        args.push(new TextDecoder().decode(new Uint8Array(mem, p, l)));
        return ni;
      }
      case ParamType.Text16: {
        const [ni, p, l] = this.#slice(view, i);
        args.push(new TextDecoder("utf-16le").decode(new Uint8Array(mem, p, l)));
        return ni;
      }
      case ParamType.TypedArraySlice: {
        const sliceType = view.getUint8(i);
        const [ni, p, l] = this.#slice(view, i + B1);
        const Ctor = TypedSliceArray[sliceType] ?? Uint8Array;
        args.push(new TypedArraySliceValue(sliceType, new Ctor(mem.slice(p, p + l * Ctor.BYTES_PER_ELEMENT))));
        return ni;
      }
      default: {
        // *ArrayBuffer (16–25): [ptr][len] → a copy in the matching typed array.
        const Ctor = ARRAY_BUFFER_CTORS[type];
        if (!Ctor) throw new Error(`ParameterParser: unknown ParamType ${type}`);
        const [ni, p, l] = this.#slice(view, i);
        args.push(new Ctor(mem.slice(p, p + l * Ctor.BYTES_PER_ELEMENT)));
        return ni;
      }
    }
  }
}

const ARRAY_BUFFER_CTORS = {
  [ParamType.Uint8Array]: Uint8Array, [ParamType.Uint16Array]: Uint16Array,
  [ParamType.Uint32Array]: Uint32Array, [ParamType.Uint64Array]: BigUint64Array,
  [ParamType.Int8Array]: Int8Array, [ParamType.Int16Array]: Int16Array,
  [ParamType.Int32Array]: Int32Array, [ParamType.Int64Array]: BigInt64Array,
  [ParamType.Float32Array]: Float32Array, [ParamType.Float64Array]: Float64Array,
};

// ─── ReturnHintParser ────────────────────────────────────────────────────────────

/** Decode `[Start][ReturnIds][ThreeState…][Stop]` → { id, states:[{ stateId, types:[] }] }. */
export class ReturnHintParser {
  constructor(bridge) { this.bridge = bridge; }

  parse(ptr, len) {
    const view = new DataView(this.bridge.memory.buffer, Number(ptr), Number(len));
    const [, hint] = ReturnHintParser.parseFrom(view, 0);
    return hint;
  }

  /**
   * Offset-based variant shared with the batch codec (hints are embedded mid-stream
   * there). Returns `[indexAfterStop, hint]`.
   */
  static parseFrom(view, i) {
    if (view.getUint8(i) !== ReturnHintMarker.Start) throw new Error("hint: missing Start");
    i += B1;
    const id = view.getUint8(i);
    i += B1;
    const states = [];
    if (id !== ReturnIds.None) {
      // One/List carry exactly one ThreeState; Multi carries several, concatenated.
      // Each ThreeState is [ThreeStateId][ReturnType×n]; read until the Stop marker.
      while (view.getUint8(i) !== ReturnHintMarker.Stop) {
        const stateId = view.getUint8(i);
        i += B1;
        const n = stateId === ThreeStateId.One ? 1 : stateId === ThreeStateId.Two ? 2 : 3;
        const types = [];
        for (let k = 0; k < n; k++) { types.push(view.getUint8(i)); i += B1; }
        states.push({ stateId, types });
      }
    }
    if (view.getUint8(i) !== ReturnHintMarker.Stop) throw new Error("hint: missing Stop");
    i += B1;
    return [i, { id, states }];
  }
}

// ─── ReplyEncoder (= Reply) ─────────────────────────────────────────────────────

/** Encodes a JS return value as `[ReturnType:u8][value]` and (when needed) into an arena slot. */
export class ReplyEncoder {
  /** @param {import('./foundation-wasm.js').MemoryAllocations} memory */
  constructor(memory) { this.memory = memory; }

  /**
   * Mirror of megatron `Reply.immediate`. `hint` is the parsed return-hint
   * ({id, states}); returns either a naked scalar (typed fast-paths, !alwaysEncoded)
   * or a MemoryId (bigint) for an encoded slot, or -1n for None+undefined.
   */
  immediate(hint, value, alwaysEncoded) {
    if (hint.id === ReturnIds.None) {
      if (value === undefined || value === null) return -1n;
      throw new Error(`Expected NoReturn but got ${value}`);
    }
    const containers = this.#containers(hint, value);
    if (
      hint.id === ReturnIds.One && containers.length === 1 &&
      RETURN_NAKED.has(containers[0].type) && !alwaysEncoded
    ) {
      return value; // naked scalar — typed fast-path
    }
    return this.encodeIntoMemory(containers);
  }

  /**
   * Build the `{type, value}` containers for a return hint, mirroring the Rust
   * `ReturnValueParserIter` shape:
   *   One  → exactly 1 value typed by the single ThreeState;
   *   List → N homogeneous values, each typed by the same ThreeState;
   *   Multi → one value per ThreeState (value `k` ↔ `states[k]`).
   * A union ThreeState (Two/Three) is resolved per concrete value (see #pickType).
   */
  #containers(hint, value) {
    switch (hint.id) {
      case ReturnIds.One:
        return [{ type: this.#pickType(hint.states[0], value), value }];
      case ReturnIds.List: {
        const state = hint.states[0];
        return Array.from(value).map((v) => ({ type: this.#pickType(state, v), value: v }));
      }
      case ReturnIds.Multi: {
        const arr = Array.from(value);
        return hint.states.map((state, k) => ({ type: this.#pickType(state, arr[k]), value: arr[k] }));
      }
      default:
        return [];
    }
  }

  /**
   * Resolve a single declared ThreeState to the concrete ReturnType for `value`.
   * Non-union (`One(t)`) states use their one type directly; union states pick the
   * member that matches the JS value's runtime type (Rust validates the match).
   */
  #pickType(state, value) {
    const { types } = state;
    if (types.length === 1) return types[0];
    const inferred = inferReturnType(value);
    return types.includes(inferred) ? inferred : types[0];
  }

  /** Encode containers `{type, value}` as `[type][value]…` into one arena slot → MemoryId. */
  encodeIntoMemory(containers) {
    const bytes = this.encode(containers);
    const id = this.memory.create(bytes.length);
    this.memory.write(id, bytes);
    return id;
  }

  /**
   * Mirror of megatron `Reply.callback_success`: an async JS result resolved — encode it
   * (always framed Begin..End) and deliver it to WASM callback `callbackId` via the
   * `CallbackRegistry` (which writes a slot + calls `invoke_callback`). `None`-hinted
   * async calls are fire-and-forget and never reach here.
   * @param {import('./foundation-wasm.js').CallbackRegistry} callbacks
   * @param {bigint} callbackId  the WASM `InternalPointer` value
   * @param {{id:number, states:Array<{types:number[]}>}} hint
   */
  callbackSuccess(callbacks, callbackId, hint, value) {
    const bytes = this.encode(this.#containers(hint, value));
    callbacks.invoke(callbackId, bytes);
  }

  /**
   * Mirror of megatron `Reply.callback_failure`: a rejected async result — frame it as an
   * `ErrorCode` ReturnValue and deliver it (Rust decodes `ReturnValues::ErrorCode`).
   * @param {import('./foundation-wasm.js').CallbackRegistry} callbacks
   * @param {bigint} callbackId
   */
  callbackFailure(callbacks, callbackId, error) {
    const code = error instanceof ErrorCodeValue
      ? error.code
      : (error && Number.isInteger(error.code) ? error.code : 1);
    const bytes = this.encode([{ type: ReturnType.ErrorCode, value: code }]);
    callbacks.invoke(callbackId, bytes);
  }

  /** Encode containers as `[Begin][ReturnType][value]…[End]` (Rust FromBinary expects the frame). */
  encode(containers) {
    const out = [ReturnValueMarker.Begin];
    const push = (n, bytes) => { for (let k = 0; k < bytes; k++) out.push(Number((BigInt(n) >> BigInt(8 * k)) & 0xffn)); };
    for (const { type, value } of containers) {
      out.push(type);
      switch (type) {
        case ReturnType.None: break;
        case ReturnType.Bool: out.push(value ? 1 : 0); break;
        case ReturnType.Uint8: case ReturnType.Int8: push(value, 1); break;
        case ReturnType.Uint16: case ReturnType.Int16: case ReturnType.ErrorCode: push(value, 2); break;
        case ReturnType.Uint32: case ReturnType.Int32: push(value, 4); break;
        case ReturnType.Uint64: case ReturnType.Int64: push(BigInt(value), 8); break;
        case ReturnType.Float32: { const b = new Uint8Array(4); new DataView(b.buffer).setFloat32(0, value, true); out.push(...b); break; }
        case ReturnType.Float64: { const b = new Uint8Array(8); new DataView(b.buffer).setFloat64(0, value, true); out.push(...b); break; }
        case ReturnType.Int128: case ReturnType.Uint128: { const v = BigInt(value); push((v >> 64n) & 0xffffffffffffffffn, 8); push(v & 0xffffffffffffffffn, 8); break; }
        // Reference returns: the id is sent inline (Rust resolves it host-side).
        case ReturnType.ExternalReference:
        case ReturnType.InternalReference:
          push(BigInt(value instanceof RefPointer ? value.value : value), 8);
          break;
        // Text8: write the UTF-8 bytes into a fresh slot, send [type][slot_id:u64].
        // Rust's ReturnValueParserIter Text8 arm takes + frees that slot.
        case ReturnType.Text8: {
          const bytes = new TextEncoder().encode(String(value));
          const slot = this.memory.create(bytes.length);
          this.memory.write(slot, bytes);
          push(slot, 8);
          break;
        }
        // MemorySlice / array buffers: write the raw bytes into a slot, send [type][slot_id].
        case ReturnType.MemorySlice: {
          const u8 = value instanceof Uint8Array ? value : new Uint8Array(value.buffer ?? value);
          const slot = this.memory.create(u8.length);
          this.memory.write(slot, u8);
          push(slot, 8);
          break;
        }
        default: throw new Error(`ReplyEncoder: unsupported return type ${type} (Object/DOMObject/typed arrays need heaps — extend when needed)`);
      }
    }
    out.push(ReturnValueMarker.End);
    return Uint8Array.from(out);
  }
}

// ─── FunctionRegistry ────────────────────────────────────────────────────────────

/**
 * Implements `host_register_function` + `host_invoke_function(+_as_*)`. Registered JS
 * functions are stored by handle and called with `this` = `context` (so they can use
 * runtime helpers), args spread positionally (mirrors megatron).
 */
export class FunctionRegistry {
  /**
   * @param {{exports:object, memory:WebAssembly.Memory}} bridge
   * @param {import('./foundation-wasm.js').MemoryAllocations} memory
   * @param {import('./foundation-wasm.js').StringCache} strings
   * @param {import('./foundation-wasm.js').CallbackRegistry} callbacks  delivers async replies
   */
  constructor(bridge, memory, strings, callbacks) {
    this.bridge = bridge;
    this.params = new ParameterParser(bridge, strings);
    this.hints = new ReturnHintParser(bridge);
    this.reply = new ReplyEncoder(memory);
    this.callbacks = callbacks;
    // Generation-arena heap shared by ALL function-handle paths: host_register_function,
    // function_allocate_external_pointer pre-allocation, and batch MakeFunction (megatron
    // used one function_heap for all three — handles must be interchangeable).
    this.heap = new ExternalHeap();
    this.context = this; // `this` for registered fns; override to expose helpers
  }

  /** Compile a registered-function source string into a callable. */
  static compile(source) {
    return Function(`"use strict"; return(${source})`)();
  }

  /** host_register_function(start, len, utf) → handle. Evals the source string. */
  register(start, len, utf) {
    const enc = Number(utf) === 16 ? "utf-16le" : "utf-8";
    const bytes = new Uint8Array(this.bridge.memory.buffer, Number(start), Number(len));
    const source = new TextDecoder(enc).decode(bytes);
    return this.heap.create(FunctionRegistry.compile(source));
  }

  /** function_allocate_external_pointer → an empty handle MakeFunction fills later. */
  allocate() {
    return this.heap.create(null);
  }

  /** host_unregister_function. */
  unregister(handle) {
    this.heap.destroy(BigInt(handle));
  }

  #call(handle, pPtr, pLen) {
    const fn = this.heap.get(BigInt(handle));
    if (!fn) throw new Error(`invoke: no function for handle ${handle}`);
    return fn.apply(this.context, this.params.parse(pPtr, pLen));
  }

  /** Generic host_invoke_function → MemoryId of the encoded ReturnValues (or -1n). */
  invoke(handle, pPtr, pLen, rPtr, rLen) {
    const hint = this.hints.parse(rPtr, rLen);
    const result = this.#call(handle, pPtr, pLen);
    const reply = this.reply.immediate(hint, result, true);
    if (result === undefined || result === null) return -1n;
    return typeof reply === "bigint" ? reply : BigInt(reply);
  }

  #invokeNaked(handle, pPtr, pLen, returnTypeId) {
    const hint = { id: ReturnIds.One, states: [{ stateId: ThreeStateId.One, types: [returnTypeId] }] };
    return this.reply.immediate(hint, this.#call(handle, pPtr, pLen), false);
  }

  /**
   * `host_invoke_async_function`: call a fn that returns a Promise. Params + return hint
   * are read from WASM memory SYNCHRONOUSLY (before any await); when the promise settles
   * the framed reply is delivered to WASM via `invoke_callback`. A `None` hint is
   * fire-and-forget (mirror of megatron — the promise result is discarded).
   */
  invokeAsync(handle, callbackHandle, pPtr, pLen, rPtr, rLen) {
    const hint = this.hints.parse(rPtr, rLen);
    const result = this.#call(handle, pPtr, pLen);
    if (hint.id === ReturnIds.None) return; // fire-and-forget
    const callbackId = BigInt(callbackHandle);
    Promise.resolve(result).then(
      (value) => this.reply.callbackSuccess(this.callbacks, callbackId, hint, value),
      (error) => this.reply.callbackFailure(this.callbacks, callbackId, error),
    );
  }

  invokeAsBool(handle, pPtr, pLen) { return this.#invokeNaked(handle, pPtr, pLen, ReturnType.Bool) ? 1 : 0; }
  invokeAsFloat(handle, pPtr, pLen) { return Number(this.#invokeNaked(handle, pPtr, pLen, ReturnType.Float64)); }
  invokeAsInt(handle, pPtr, pLen) { const v = this.#invokeNaked(handle, pPtr, pLen, ReturnType.Int64); return typeof v === "bigint" ? Number(v) : v; }
  invokeAsBigInt(handle, pPtr, pLen) { const v = this.#invokeNaked(handle, pPtr, pLen, ReturnType.Uint64); return typeof v === "bigint" ? v : BigInt(v); }
}
