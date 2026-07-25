// foundation-wasm.js — core WASM↔JS ABI runtime (clean rewrite of megatron.js).
//
// WHY: megatron.js is one 7000-line IIFE that tangles the pure ABI (memory,
// messaging, callbacks, timers) with DOM concerns. This file is the ABI half only —
// no DOM, no window — matching the `foundation_wasm` crate split (decision 015).
//
// WHAT — ONE self-contained file (no internal imports, browser-loadable without a
// bundler: `<script type="module">` for the ESM exports, or the
// `globalThis.FoundationWasmRuntime` mirror for classic scripts), in three sections:
//   1. Function-call ABI codec: ParameterParser (flat/V1), ReturnHintParser,
//      ReplyEncoder, FunctionRegistry, ExternalHeap
//   2. V2 quantized batch codec: Operations, TypeOptimization, BatchParameterParser,
//      BatchInstructions (the custom protocol backbone)
//   3. Core runtime: WasmEnvelope (14-byte [protocol][version][memory_id][length]),
//      ProtocolDispatcher, MemoryAllocations, TimerRegistry, CallbackRegistry,
//      StringCache, AnimationDriver, FoundationWasm (bridge + `web_abi` + `init()`)
//
// HOW: bootstrap mirrors the proven integration pattern —
//   const rt = new FoundationWasm();
//   const mod = await WebAssembly.instantiate(bytes, { abi: rt.web_abi });
//   rt.init(mod);                  // memory comes from instance.exports.memory
// The WASM module EXPORTS its own linear memory; the host imports close over a
// lazily-populated `bridge`, so they work even though the instance doesn't exist at
// the time the import object is built.
//
// Protocol bytes (decision 014/022, F27): 0 = Custom Binary, 1 = Arrow, 2 = JSON,
//   3 = Capability Trigger (host→wasm), 4 = IPC Trigger (host→wasm).


// ════════════════════════════════════════════════════════════════════════════════
// SECTION 1: function-call ABI codec (formerly function-registry.js)
// ════════════════════════════════════════════════════════════════════════════════

// Function-call ABI codec — WASM↔JS function-call ABI codec (ported from megatron's
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

/**
 * A pre-typed return slot: `{type: ReturnType, value}`. Registered functions build
 * these via the context `as*` helpers; the encoder passes them through untouched
 * instead of inferring the type from the hint (megatron ReplyContainer parity).
 */
export class ReplyContainer {
  constructor(type, value) { this.type = type; this.value = value; }
}

/** Minimal DOM-node stand-in for non-DOM hosts (megatron FakeNode parity). */
export class FakeNode {
  constructor(tag) { this.tag = tag; }
}

/** A failure a registered fn raises to reach the WASM callback as an ErrorCode. */
export class ReplyError extends Error {
  constructor(code, options) {
    if (!Number.isInteger(code)) {
      throw new Error("Only numbers allowed to represent the code to be sent");
    }
    super(`Reply failed with error code: ${code}`, options);
    this.code = code;
  }
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
 * Normalize a typed-slice return value: a `TypedArraySliceValue` keeps its declared
 * slice type; a bare TypedArray maps to its `TypedSlice` id (megatron check_for_type
 * accepted both). Returns the slice id + a byte view over the content.
 */
function asTypedSlice(value) {
  if (value instanceof TypedArraySliceValue) {
    const content = value.content;
    return {
      sliceType: value.sliceType,
      u8: new Uint8Array(content.buffer, content.byteOffset, content.byteLength),
    };
  }
  for (const [id, Ctor] of Object.entries(TypedSliceArray)) {
    if (value instanceof Ctor && value.constructor === Ctor) {
      return {
        sliceType: Number(id),
        u8: new Uint8Array(value.buffer, value.byteOffset, value.byteLength),
      };
    }
  }
  throw new Error("TypedArraySlice return: expected a TypedArray or TypedArraySliceValue");
}

/**
 * The u16 code behind any error representation: an `ErrorCodeValue` (echoed param),
 * a `ReplyError`/Error carrying `.code`, or a bare integer. Unknown shapes → 1.
 */
function errorCodeOf(value) {
  if (value instanceof ErrorCodeValue) return value.code;
  if (value && Number.isInteger(value.code)) return value.code;
  if (Number.isInteger(value)) return value;
  if (typeof value === "bigint") return Number(value);
  return 1;
}

/**
 * Does `value`'s JS runtime type satisfy the candidate `ReturnType`? Used to resolve
 * a union ThreeState (Two/Three): candidates are tried IN DECLARED ORDER and the
 * first match wins (megatron `Reply.check_for_type` parity — e.g. `1` against
 * `Three(Bool, Int8, Uint8)` picks Int8, not Bool). Non-union states never consult
 * this; the Rust `ReturnValueParserIter` validates the final choice.
 */
function matchesReturnType(value, type) {
  switch (type) {
    case ReturnType.None: return value === undefined || value === null;
    case ReturnType.Bool: return typeof value === "boolean";
    case ReturnType.Text8: return typeof value === "string";
    case ReturnType.Int8:
    case ReturnType.Int16:
    case ReturnType.Int32:
    case ReturnType.Uint8:
    case ReturnType.Uint16:
    case ReturnType.Uint32:
      return typeof value === "number" && Number.isInteger(value);
    case ReturnType.Int64:
    case ReturnType.Uint64:
    case ReturnType.Int128:
    case ReturnType.Uint128:
      return typeof value === "bigint" || (typeof value === "number" && Number.isInteger(value));
    case ReturnType.Float32:
    case ReturnType.Float64:
      return typeof value === "number";
    case ReturnType.ErrorCode:
      return value instanceof ErrorCodeValue || value instanceof ReplyError ||
        (typeof value === "number" && Number.isInteger(value));
    case ReturnType.ExternalReference: return value instanceof ExternalPointer;
    case ReturnType.InternalReference: return value instanceof InternalPointer;
    case ReturnType.TypedArraySlice:
      return value instanceof TypedArraySliceValue || ArrayBuffer.isView(value);
    case ReturnType.MemorySlice:
      return typeof value === "bigint" || typeof value === "number" || ArrayBuffer.isView(value);
    case ReturnType.DOMObject:
      return value instanceof FakeNode ||
        (typeof Node !== "undefined" && value instanceof Node) ||
        (typeof value === "object" && value !== null);
    case ReturnType.Object: return typeof value === "object" && value !== null;
    default: return false;
  }
}

// ─── ParameterParser (FLAT, = ParameterParserV1) ────────────────────────────────

/**
 * Decode `host_invoke_function` params: `[ParamType:u8][value]` repeated until the
 * buffer is consumed. Text/arrays carry `[ptr:u64][len:u64]` into WASM memory.
 */
export class ParameterParser {
  /** @param {{memory:WebAssembly.Memory}} bridge @param {StringCache} strings */
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
  /** @param {MemoryAllocations} memory */
  constructor(memory) {
    this.memory = memory;
    // Heaps for reference-returning types; the runtime wires `objects`, the DOM layer
    // wires `dom` (megatron threaded object_heap/dom_heap into Reply.transform).
    this.objects = null; // ExternalHeap | null
    this.dom = null; // ExternalHeap | null
  }

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
      return this.#naked(containers[0]); // typed fast-path — value crosses raw
    }
    return this.encodeIntoMemory(containers);
  }

  /**
   * The naked form of one container (megatron transforms BEFORE the naked check):
   * Object/DOMObject intern into their heap and the HANDLE crosses; reference
   * pointers cross as their raw id; scalars cross as-is.
   */
  #naked({ type, value }) {
    switch (type) {
      case ReturnType.Object: {
        if (!this.objects) throw new Error("ReplyEncoder: no object heap wired");
        return value instanceof RefPointer ? value.value : this.objects.create(value);
      }
      case ReturnType.DOMObject: {
        if (!this.dom) throw new Error("ReplyEncoder: no DOM heap wired");
        return value instanceof RefPointer ? value.value : this.dom.create(value);
      }
      case ReturnType.ExternalReference:
      case ReturnType.InternalReference:
        return value instanceof RefPointer ? value.value : value;
      case ReturnType.ErrorCode:
        return errorCodeOf(value);
      default:
        return value;
    }
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
    // A ReplyContainer is already typed (context as* helpers) — pass it through
    // untouched (megatron transform_from_hint checks this before any inference).
    const resolve = (state, v) =>
      v instanceof ReplyContainer
        ? { type: v.type, value: v.value }
        : { type: this.#pickType(state, v), value: v };
    switch (hint.id) {
      case ReturnIds.One:
        return [resolve(hint.states[0], value)];
      case ReturnIds.List: {
        const state = hint.states[0];
        return Array.from(value).map((v) => resolve(state, v));
      }
      case ReturnIds.Multi: {
        const arr = Array.from(value);
        return hint.states.map((state, k) => resolve(state, arr[k]));
      }
      default:
        return [];
    }
  }

  /**
   * Resolve a single declared ThreeState to the concrete ReturnType for `value`.
   * Non-union (`One(t)`) states use their one type directly; union states try each
   * candidate IN DECLARED ORDER and take the first whose runtime type matches
   * (megatron check_for_type order — Rust validates the final choice).
   */
  #pickType(state, value) {
    const { types } = state;
    if (types.length === 1) return types[0];
    for (const candidate of types) {
      if (matchesReturnType(value, candidate)) return candidate;
    }
    throw new Error(`return value ${value} matches none of the hinted types [${types}]`);
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
   * @param {CallbackRegistry} callbacks
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
   * @param {CallbackRegistry} callbacks
   * @param {bigint} callbackId
   */
  callbackFailure(callbacks, callbackId, error) {
    const bytes = this.encode([{ type: ReturnType.ErrorCode, value: errorCodeOf(error) }]);
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
        case ReturnType.Uint16: case ReturnType.Int16: push(value, 2); break;
        // ErrorCode values may arrive wrapped (an echoed ErrorCodeValue param or a
        // ReplyError) — the u16 code is what crosses.
        case ReturnType.ErrorCode: push(errorCodeOf(value), 2); break;
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
        // MemorySlice / array buffers: a numeric value IS an existing slot id (the
        // asMemorySlice contract); raw bytes get written into a fresh slot first.
        case ReturnType.MemorySlice: {
          if (typeof value === "bigint" || typeof value === "number") {
            push(BigInt(value), 8);
            break;
          }
          const u8 = value instanceof Uint8Array ? value : new Uint8Array(value.buffer ?? value);
          const slot = this.memory.create(u8.length);
          this.memory.write(slot, u8);
          push(slot, 8);
          break;
        }
        // Object/DOMObject: intern in the matching host heap, send [type][heap_handle:u64]
        // (megatron Reply.asObject/asDOMObject). A RefPointer passes its existing id through.
        case ReturnType.Object: {
          if (!this.objects) throw new Error("ReplyEncoder: no object heap wired");
          push(value instanceof RefPointer ? value.value : this.objects.create(value), 8);
          break;
        }
        case ReturnType.DOMObject: {
          if (!this.dom) throw new Error("ReplyEncoder: no DOM heap wired");
          push(value instanceof RefPointer ? value.value : this.dom.create(value), 8);
          break;
        }
        // TypedArraySlice: stage the bytes in a slot and send the slot's LIVE address —
        // [type][slice_type:u8][ptr:u64][len:u64] (Rust ReturnValues::TypedArraySlice
        // carries a raw MemoryLocation; consume it before the next allocation).
        case ReturnType.TypedArraySlice: {
          const { sliceType, u8 } = asTypedSlice(value);
          const slot = this.memory.create(u8.length);
          this.memory.write(slot, u8);
          out.push(sliceType);
          push(this.memory.get(slot).ptr, 8);
          push(u8.length, 8);
          break;
        }
        default: throw new Error(`ReplyEncoder: unsupported return type ${type}`);
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
   * @param {MemoryAllocations} memory
   * @param {StringCache} strings
   * @param {CallbackRegistry} callbacks  delivers async replies
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
    // immediate() owns the no-value convention: only a None HINT yields -1n. An
    // undefined result under a One(None) hint still encodes a framed None reply —
    // the module dereferences the returned slot id (megatron with_return parity).
    const reply = this.reply.immediate(hint, result, true);
    return typeof reply === "bigint" ? reply : BigInt(reply);
  }

  /**
   * Naked typed invoke: call the fn and return the value raw per `returnTypeId`
   * (Object/DOMObject intern into their heap and the handle crosses). Public so the
   * DOM layer can wire `host_invoke_function_as_dom` (ReturnType.DOMObject).
   */
  invokeNakedAs(handle, pPtr, pLen, returnTypeId) {
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
    const settled = Promise.resolve(result).then(
      (value) => this.reply.callbackSuccess(this.callbacks, callbackId, hint, value),
      (error) => this.reply.callbackFailure(this.callbacks, callbackId, error),
    );
    if (this.tasks) this.tasks.add(settled);
  }

  invokeAsBool(handle, pPtr, pLen) { return this.invokeNakedAs(handle, pPtr, pLen, ReturnType.Bool) ? 1 : 0; }
  invokeAsFloat(handle, pPtr, pLen) { return Number(this.invokeNakedAs(handle, pPtr, pLen, ReturnType.Float64)); }
  invokeAsInt(handle, pPtr, pLen) { const v = this.invokeNakedAs(handle, pPtr, pLen, ReturnType.Int64); return typeof v === "bigint" ? Number(v) : v; }
  invokeAsBigInt(handle, pPtr, pLen) { const v = this.invokeNakedAs(handle, pPtr, pLen, ReturnType.Uint64); return typeof v === "bigint" ? v : BigInt(v); }
  /** Object fast-path: the result interns into the object heap; its handle crosses naked. */
  invokeAsObject(handle, pPtr, pLen) { const v = this.invokeNakedAs(handle, pPtr, pLen, ReturnType.Object); return typeof v === "bigint" ? v : BigInt(v); }

  /**
   * String fast-path (megatron `as_str`): the result string's RAW UTF-8 bytes go into
   * a fresh arena slot (no reply framing) and the slot id crosses.
   */
  invokeAsString(handle, pPtr, pLen) {
    const bytes = new TextEncoder().encode(String(this.#call(handle, pPtr, pLen)));
    const slot = this.reply.memory.create(bytes.length);
    this.reply.memory.write(slot, bytes);
    return slot;
  }

  // ── Context `as*` helpers (megatron middleware parity) ──────────────────────────
  // Registered functions run with `this` = the context (this registry by default) and
  // build PRE-TYPED return slots with these, e.g. `return this.asUint8(10)`. Each
  // yields a ReplyContainer the encoder passes through without hint inference.

  asNone() { return new ReplyContainer(ReturnType.None, undefined); }
  asBool(v) { return new ReplyContainer(ReturnType.Bool, Boolean(v)); }
  asText8(v) { return new ReplyContainer(ReturnType.Text8, String(v)); }
  asInt8(v) { return new ReplyContainer(ReturnType.Int8, v); }
  asInt16(v) { return new ReplyContainer(ReturnType.Int16, v); }
  asInt32(v) { return new ReplyContainer(ReturnType.Int32, v); }
  asInt64(v) { return new ReplyContainer(ReturnType.Int64, BigInt(v)); }
  asUint8(v) { return new ReplyContainer(ReturnType.Uint8, v); }
  asUint16(v) { return new ReplyContainer(ReturnType.Uint16, v); }
  asUint32(v) { return new ReplyContainer(ReturnType.Uint32, v); }
  asUint64(v) { return new ReplyContainer(ReturnType.Uint64, BigInt(v)); }
  asFloat32(v) { return new ReplyContainer(ReturnType.Float32, v); }
  asFloat64(v) { return new ReplyContainer(ReturnType.Float64, v); }
  asInt128(lsb, msb = 0) {
    return new ReplyContainer(
      ReturnType.Int128,
      (BigInt(msb) << 64n) | (BigInt(lsb) & 0xffffffffffffffffn),
    );
  }
  asUint128(lsb, msb = 0) {
    return new ReplyContainer(
      ReturnType.Uint128,
      (BigInt(msb) << 64n) | (BigInt(lsb) & 0xffffffffffffffffn),
    );
  }
  asErrorCode(v) {
    return new ReplyContainer(ReturnType.ErrorCode, v instanceof ErrorCodeValue ? v.code : v);
  }
  asReplyError(code) { return new ReplyError(code); }

  /** Intern a JS object in the host object heap → pre-typed Object container. */
  asObject(value) {
    if (typeof value !== "object" || value === null) {
      throw new Error("Value must be a JS Object/object");
    }
    if (!this.reply.objects) throw new Error("asObject: no object heap wired");
    return new ReplyContainer(ReturnType.Object, new ExternalPointer(this.reply.objects.create(value)));
  }

  /** Intern a DOM node in the host DOM heap → pre-typed DOMObject container. */
  asDOMObject(value) {
    if (!this.reply.dom) throw new Error("asDOMObject: no DOM heap wired");
    return new ReplyContainer(ReturnType.DOMObject, new InternalPointer(this.reply.dom.create(value)));
  }

  /** A FakeNode (non-DOM host stand-in) interned as a DOMObject. */
  asFakeNode(tag) {
    if (typeof tag !== "string") throw new Error("Value must be a JS string");
    return this.asDOMObject(new FakeNode(tag));
  }

  /** An existing arena-slot id (or raw bytes) as a MemorySlice return. */
  asMemorySlice(value) {
    const id = typeof value === "bigint" || typeof value === "number" ? BigInt(value) : value;
    return new ReplyContainer(ReturnType.MemorySlice, id);
  }

  asTypedArraySlice(sliceType, content) {
    return new ReplyContainer(ReturnType.TypedArraySlice, new TypedArraySliceValue(sliceType, content));
  }

  asInternalReference(v) {
    return new ReplyContainer(
      ReturnType.InternalReference,
      v instanceof RefPointer ? v : new InternalPointer(BigInt(v)),
    );
  }

  asExternalReference(v) {
    return new ReplyContainer(
      ReturnType.ExternalReference,
      v instanceof RefPointer ? v : new ExternalPointer(BigInt(v)),
    );
  }

  // Typed-array returns ride MemorySlice (raw bytes) under the current contract.
  #asArray(Ctor, v) {
    if (!(v instanceof Ctor)) throw new Error(`Value must be a ${Ctor.name}`);
    return new ReplyContainer(ReturnType.MemorySlice, new Uint8Array(v.buffer, v.byteOffset, v.byteLength));
  }
  asUint8Array(v) { return this.#asArray(Uint8Array, v); }
  asUint16Array(v) { return this.#asArray(Uint16Array, v); }
  asUint32Array(v) { return this.#asArray(Uint32Array, v); }
  asUint64Array(v) { return this.#asArray(BigUint64Array, v); }
  asInt8Array(v) { return this.#asArray(Int8Array, v); }
  asInt16Array(v) { return this.#asArray(Int16Array, v); }
  asInt32Array(v) { return this.#asArray(Int32Array, v); }
  asInt64Array(v) { return this.#asArray(BigInt64Array, v); }
  asFloat32Array(v) { return this.#asArray(Float32Array, v); }
  asFloat64Array(v) { return this.#asArray(Float64Array, v); }
}

// ════════════════════════════════════════════════════════════════════════════════
// SECTION 2: V2 quantized batch codec (formerly batch-instructions.js)
// ════════════════════════════════════════════════════════════════════════════════

// V2 batch codec — the V2 quantized batch codec (ported from megatron's
// ParameterParserV2 + BatchInstructions + BatchOperation, per feature 17 research docs).
//
// This is the backbone of the custom binary protocol: WASM builds an instruction batch
// (Rust `Instructions`, ops.rs) in TWO arena slots — an OPS buffer of opcodes/markers
// and a TEXTS buffer of raw UTF-8 — and ships both via `host_batch_apply` (no results)
// or `host_batch_returning_apply` (group-return slot id). DISTINCT from the flat invoke
// codec (V1, section 1): batch params are marker-framed AND quantized.
//
// Wire layout (Rust `Batchable` impls are the source of truth):
//   ops    = [Operations.Begin=0] (op…)* [Operations.Stop=255]
//   op     = [opId:u8] [payload…] [Operations.End=254]
//   MakeFunction payload = [ParamType.ExternalReference=15][TQ][handle]
//                          [ParamType.Text8=3][index:u64 raw][len:u64 raw]  (into TEXTS)
//   Invoke payload       = [15][TQ][handle] [return-hint frame 200..201] [args]
//   InvokeAsync payload  = [15][TQ][handle] [26][TQ][callback] [hint] [args]
//   args   = [ArgStart=1] ([ArgBegin=2][ParamType][TQ?][value][ArgEnd=3])* [ArgStop=4]
// Quantized values carry a TypeOptimization byte; Bool/Int8/Uint8/Float32 never do.



// ─── Shared discriminants (cross-language contract, base.rs) ────────────────────
export const Operations = Object.freeze({
  Begin: 0, MakeFunction: 1, Invoke: 2, InvokeAsync: 3, End: 254, Stop: 255,
});

export const ArgumentOperations = Object.freeze({ Start: 1, Begin: 2, End: 3, Stop: 4 });

export const TypeOptimization = Object.freeze({
  None: 0,
  QuantizedInt16AsI8: 1, QuantizedInt32AsI8: 2, QuantizedInt32AsI16: 3,
  QuantizedInt64AsI8: 4, QuantizedInt64AsI16: 5, QuantizedInt64AsI32: 6,
  QuantizedUint16AsU8: 7, QuantizedUint32AsU8: 8, QuantizedUint32AsU16: 9,
  QuantizedUint64AsU8: 10, QuantizedUint64AsU16: 11, QuantizedUint64AsU32: 12,
  QuantizedF64AsF32: 13, QuantizedF128AsF32: 14, QuantizedF128AsF64: 15,
  QuantizedInt128AsI8: 16, QuantizedInt128AsI16: 17, QuantizedInt128AsI32: 18,
  QuantizedInt128AsI64: 19, QuantizedUint128AsU8: 20, QuantizedUint128AsU16: 21,
  QuantizedUint128AsU32: 22, QuantizedUint128AsU64: 23,
  QuantizedPtrAsU8: 24, QuantizedPtrAsU16: 25, QuantizedPtrAsU32: 26, QuantizedPtrAsU64: 27,
});

// Group-return frame (Rust GroupReturnHintMarker; protocol.rs GroupReturnTypeHints).
const GroupReturnHintMarker = Object.freeze({ Start: 111, Stop: 222 });

/**
 * Read one TQ-prefixed value: `[TypeOptimization:u8][bytes…]`. `none` reads the value
 * at full width when no quantization was applied; every Quantized* case reads the
 * narrowed width (the Rust `value_quantitization::q*` inverse). → `[newIndex, value]`.
 */
function readQuantized(view, i, none) {
  const tq = view.getUint8(i);
  i += B1;
  switch (tq) {
    case TypeOptimization.None:
      return none(view, i);
    case TypeOptimization.QuantizedInt16AsI8:
    case TypeOptimization.QuantizedInt32AsI8:
    case TypeOptimization.QuantizedInt64AsI8:
    case TypeOptimization.QuantizedInt128AsI8:
      return [i + B1, view.getInt8(i)];
    case TypeOptimization.QuantizedInt32AsI16:
    case TypeOptimization.QuantizedInt64AsI16:
    case TypeOptimization.QuantizedInt128AsI16:
      return [i + B2, view.getInt16(i, true)];
    case TypeOptimization.QuantizedInt64AsI32:
    case TypeOptimization.QuantizedInt128AsI32:
      return [i + B4, view.getInt32(i, true)];
    case TypeOptimization.QuantizedInt128AsI64:
      return [i + B8, view.getBigInt64(i, true)];
    case TypeOptimization.QuantizedUint16AsU8:
    case TypeOptimization.QuantizedUint32AsU8:
    case TypeOptimization.QuantizedUint64AsU8:
    case TypeOptimization.QuantizedUint128AsU8:
    case TypeOptimization.QuantizedPtrAsU8:
      return [i + B1, view.getUint8(i)];
    case TypeOptimization.QuantizedUint32AsU16:
    case TypeOptimization.QuantizedUint64AsU16:
    case TypeOptimization.QuantizedUint128AsU16:
    case TypeOptimization.QuantizedPtrAsU16:
      return [i + B2, view.getUint16(i, true)];
    case TypeOptimization.QuantizedUint64AsU32:
    case TypeOptimization.QuantizedUint128AsU32:
    case TypeOptimization.QuantizedPtrAsU32:
      return [i + B4, view.getUint32(i, true)];
    case TypeOptimization.QuantizedUint128AsU64:
    case TypeOptimization.QuantizedPtrAsU64:
      return [i + B8, view.getBigUint64(i, true)];
    case TypeOptimization.QuantizedF64AsF32:
    case TypeOptimization.QuantizedF128AsF32:
      return [i + B4, view.getFloat32(i, true)];
    case TypeOptimization.QuantizedF128AsF64:
      return [i + B8, view.getFloat64(i, true)];
    default:
      throw new Error(`readQuantized: unknown TypeOptimization ${tq}`);
  }
}

// Full-width readers for the TypeOptimization.None case, one per declared type.
const noneI16 = (v, i) => [i + B2, v.getInt16(i, true)];
const noneU16 = (v, i) => [i + B2, v.getUint16(i, true)];
const noneI32 = (v, i) => [i + B4, v.getInt32(i, true)];
const noneU32 = (v, i) => [i + B4, v.getUint32(i, true)];
const noneI64 = (v, i) => [i + B8, v.getBigInt64(i, true)];
const noneU64 = (v, i) => [i + B8, v.getBigUint64(i, true)];
const noneF64 = (v, i) => [i + B8, v.getFloat64(i, true)];
// 128-bit: [msb:8][lsb:8] little-endian halves, msb first (ops.rs Int128/Uint128 arms).
const noneI128 = (v, i) => [i + 16, (v.getBigInt64(i, true) << 64n) | v.getBigUint64(i + B8, true)];
const noneU128 = (v, i) => [i + 16, (v.getBigUint64(i, true) << 64n) | v.getBigUint64(i + B8, true)];

// ─── BatchParameterParser (= ParameterParserV2) ─────────────────────────────────

/**
 * Decode the marker-framed, quantized argument list of a batch Invoke/InvokeAsync.
 * Reads from the OPS view; Text8 indexes the TEXTS string; pointer-carrying params
 * (Text16 / TypedArraySlice / *ArrayBuffer) dereference WASM linear memory.
 */
export class BatchParameterParser {
  /** @param {{memory:WebAssembly.Memory}} bridge @param {StringCache} strings */
  constructor(bridge, strings) {
    this.bridge = bridge;
    this.strings = strings;
  }

  /**
   * Parse `[ArgBegin][param][ArgEnd]…[ArgStop]` starting AFTER ArgStart.
   * @returns {[number, any[]]} `[indexAfterStop, args]`
   */
  parseParams(view, i, texts) {
    const args = [];
    while (view.getUint8(i) !== ArgumentOperations.Stop) {
      if (view.getUint8(i) !== ArgumentOperations.Begin) {
        throw new Error(`batch args: expected Begin marker, got ${view.getUint8(i)}`);
      }
      i += B1;
      let value;
      [i, value] = this.parseParam(view, i, texts);
      args.push(value);
      if (view.getUint8(i) !== ArgumentOperations.End) {
        throw new Error(`batch args: expected End marker, got ${view.getUint8(i)}`);
      }
      i += B1;
    }
    i += B1; // consume ArgStop
    return [i, args];
  }

  /** Parse one `[ParamType][TQ?][value]` (after the Begin marker). */
  parseParam(view, i, texts) {
    const type = view.getUint8(i);
    i += B1;
    switch (type) {
      case ParamType.Null: return [i, null];
      case ParamType.Undefined: return [i, undefined];
      case ParamType.Bool: return [i + B1, view.getUint8(i) === 1];
      case ParamType.Int8: return [i + B1, view.getInt8(i)];
      case ParamType.Uint8: return [i + B1, view.getUint8(i)];
      case ParamType.Float32: return [i + B4, view.getFloat32(i, true)];
      case ParamType.Int16: return readQuantized(view, i, noneI16);
      case ParamType.Uint16: return readQuantized(view, i, noneU16);
      case ParamType.Int32: return readQuantized(view, i, noneI32);
      case ParamType.Uint32: return readQuantized(view, i, noneU32);
      case ParamType.Int64: return readQuantized(view, i, noneI64);
      case ParamType.Uint64: return readQuantized(view, i, noneU64);
      case ParamType.Float64: return readQuantized(view, i, noneF64);
      case ParamType.Int128: return readQuantized(view, i, noneI128);
      case ParamType.Uint128: return readQuantized(view, i, noneU128);
      case ParamType.ErrorCode: {
        // Rust encodes via qu16 ([TQ][u8|u16]); megatron's parseErrorCode misread this
        // as a u64 — we decode per the Rust encoder (see LEARNINGS).
        const [ni, code] = readQuantized(view, i, noneU16);
        return [ni, new ErrorCodeValue(Number(code))];
      }
      case ParamType.CachedText: {
        const [ni, handle] = readQuantized(view, i, noneU64);
        const text = this.strings.get(BigInt(handle));
        if (text === undefined) throw new Error(`batch CachedText: no string for handle ${handle}`);
        return [ni, text];
      }
      case ParamType.ExternalReference: {
        const [ni, id] = readQuantized(view, i, noneU64);
        return [ni, new ExternalPointer(BigInt(id))];
      }
      case ParamType.InternalReference: {
        const [ni, id] = readQuantized(view, i, noneU64);
        return [ni, new InternalPointer(BigInt(id))];
      }
      case ParamType.Text8: {
        // Two TQ'd u64s: (index, length) into the TEXTS string (NOT WASM memory).
        let index, length;
        [i, index] = readQuantized(view, i, noneU64);
        [i, length] = readQuantized(view, i, noneU64);
        return [i, texts.substr(Number(index), Number(length))];
      }
      case ParamType.Text16: {
        // TQ'd pointer into WASM memory + TQ'd u16-unit count (megatron ×2 multiplier).
        let ptr, length;
        [i, ptr] = readQuantized(view, i, noneU64);
        [i, length] = readQuantized(view, i, noneU64);
        const bytes = new Uint8Array(this.bridge.memory.buffer, Number(ptr), Number(length) * 2);
        return [i, new TextDecoder("utf-16le").decode(bytes)];
      }
      case ParamType.TypedArraySlice: {
        const sliceType = view.getUint8(i);
        i += B1;
        let ptr, length; // length is in BYTES (Rust passes a byte slice)
        [i, ptr] = readQuantized(view, i, noneU64);
        [i, length] = readQuantized(view, i, noneU64);
        const copy = this.bridge.memory.buffer.slice(Number(ptr), Number(ptr) + Number(length));
        const Ctor = TypedSliceArray[sliceType] ?? Uint8Array;
        return [i, new TypedArraySliceValue(sliceType, new Ctor(copy))];
      }
      default: {
        // *ArrayBuffer (16–25): TQ'd pointer + TQ'd ELEMENT count into WASM memory.
        const Ctor = BATCH_ARRAY_CTORS[type];
        if (!Ctor) throw new Error(`batch param: unknown ParamType ${type}`);
        let ptr, length;
        [i, ptr] = readQuantized(view, i, noneU64);
        [i, length] = readQuantized(view, i, noneU64);
        const start = Number(ptr);
        const copy = this.bridge.memory.buffer.slice(start, start + Number(length) * Ctor.BYTES_PER_ELEMENT);
        return [i, new Ctor(copy)];
      }
    }
  }
}

const BATCH_ARRAY_CTORS = {
  [ParamType.Uint8Array]: Uint8Array, [ParamType.Uint16Array]: Uint16Array,
  [ParamType.Uint32Array]: Uint32Array, [ParamType.Uint64Array]: BigUint64Array,
  [ParamType.Int8Array]: Int8Array, [ParamType.Int16Array]: Int16Array,
  [ParamType.Int32Array]: Int32Array, [ParamType.Int64Array]: BigInt64Array,
  [ParamType.Float32Array]: Float32Array, [ParamType.Float64Array]: Float64Array,
};

// ─── BatchInstructions ───────────────────────────────────────────────────────────

const MAKE_FUNCTION_HINT = Object.freeze({
  id: ReturnIds.One,
  states: [{ stateId: ThreeStateId.One, types: [ReturnType.ExternalReference] }],
});

/**
 * Parses + executes instruction batches (the `host_batch_apply` /
 * `host_batch_returning_apply` imports). Mirrors megatron's two-phase design:
 * a parse pass turns each op into a thunk, then the thunks run in order — so a
 * MakeFunction earlier in the batch is visible to an Invoke later in it.
 *
 * Extension point: `registerOperation(opId, handler)` adds custom opcodes (the DOM
 * layer registers its own). A handler is `(batch, opId, i, view, texts) => [newIndex,
 * thunk|null]`; a thunk is `(batch) => ({hint, value} | null)`.
 */
export class BatchInstructions {
  /**
   * @param {{exports:object, memory:WebAssembly.Memory}} bridge
   * @param {MemoryAllocations} memory
   * @param {StringCache} strings
   * @param {FunctionRegistry} functions
   * @param {CallbackRegistry} callbacks
   */
  constructor(bridge, memory, strings, functions, callbacks) {
    this.bridge = bridge;
    this.memory = memory;
    this.functions = functions;
    this.callbacks = callbacks;
    this.reply = functions.reply; // share the ReturnValues encoder (one contract)
    this.params = new BatchParameterParser(bridge, strings);
    this.operations = new Map([
      [Operations.MakeFunction, BatchInstructions.makeFunction],
      [Operations.Invoke, BatchInstructions.invoke],
      [Operations.InvokeAsync, BatchInstructions.invokeAsync],
    ]);
  }

  registerOperation(opId, handler) {
    this.operations.set(opId, handler);
    return this;
  }

  /** host_batch_apply: parse + run every op; results are discarded. */
  applyNoReturn(opsPtr, opsLen, textPtr, textLen) {
    for (const thunk of this.#parse(opsPtr, opsLen, textPtr, textLen)) {
      thunk(this);
    }
  }

  /**
   * host_batch_returning_apply: run every op, encode each non-null result into its own
   * framed ReturnValues slot, then write the group-return frame
   * `[Start=111]([ReturnIds][Multi: count:u16][ThreeStates…][slot_id:u64])*[Stop=222]`
   * into a fresh slot → its id (or -1n when nothing returned). Decoded by Rust
   * `GroupReturnTypeHints::from_binary`.
   */
  applyReturning(opsPtr, opsLen, textPtr, textLen) {
    const results = [];
    for (const thunk of this.#parse(opsPtr, opsLen, textPtr, textLen)) {
      const result = thunk(this);
      if (result === null || result === undefined) continue;
      const memId = this.reply.immediate(result.hint, result.value, true);
      results.push({ hint: result.hint, memId: BigInt(memId) });
    }
    if (results.length === 0) return -1n;

    const out = [GroupReturnHintMarker.Start];
    for (const { hint, memId } of results) {
      out.push(hint.id);
      if (hint.id === ReturnIds.Multi) {
        out.push(Number(hint.states.length & 0xff), Number((hint.states.length >> 8) & 0xff));
      }
      for (const state of hint.states) {
        out.push(state.stateId, ...state.types);
      }
      for (let k = 0; k < 8; k++) out.push(Number((memId >> BigInt(8 * k)) & 0xffn));
    }
    out.push(GroupReturnHintMarker.Stop);

    const bytes = Uint8Array.from(out);
    const slot = this.memory.create(bytes.length);
    this.memory.write(slot, bytes);
    return slot;
  }

  /** Read both buffers (ops as a detached copy — thunks may grow WASM memory), parse all ops. */
  #parse(opsPtr, opsLen, textPtr, textLen) {
    const opsStart = Number(opsPtr);
    const ops = this.bridge.memory.buffer.slice(opsStart, opsStart + Number(opsLen));
    const textStart = Number(textPtr);
    const textBytes = new Uint8Array(this.bridge.memory.buffer, textStart, Number(textLen));
    const texts = new TextDecoder().decode(textBytes);

    const view = new DataView(ops);
    let i = 0;
    if (view.getUint8(i) !== Operations.Begin) {
      throw new Error(`batch: expected Operations.Begin, got ${view.getUint8(i)}`);
    }
    i += B1;

    const thunks = [];
    while (i < view.byteLength && view.getUint8(i) !== Operations.Stop) {
      const opId = view.getUint8(i);
      i += B1;
      const handler = this.operations.get(opId);
      if (!handler) throw new Error(`batch: unhandled operation ${opId}`);
      let thunk;
      [i, thunk] = handler(this, opId, i, view, texts);
      if (thunk) thunks.push(thunk);
    }
    return thunks;
  }

  /** Read `[ParamType.ExternalReference][TQ][handle]` → bigint handle. */
  static readExternalHandle(view, i) {
    const vt = view.getUint8(i);
    if (vt !== ParamType.ExternalReference) {
      throw new Error(`batch: expected ExternalReference param, got ${vt}`);
    }
    const [ni, raw] = readQuantized(view, i + B1, noneU64);
    return [ni, BigInt(raw)];
  }

  /** MakeFunction: compile TEXTS[index..len] and bind it at the pre-allocated handle. */
  static makeFunction(batch, _opId, i, view, texts) {
    let handle;
    [i, handle] = BatchInstructions.readExternalHandle(view, i);

    if (view.getUint8(i) !== ParamType.Text8) {
      throw new Error(`batch MakeFunction: expected Text8, got ${view.getUint8(i)}`);
    }
    i += B1;
    // Raw (unquantized) u64 pair — register_function writes plain to_le_bytes.
    const index = Number(view.getBigUint64(i, true));
    i += B8;
    const length = Number(view.getBigUint64(i, true));
    i += B8;

    if (view.getUint8(i) !== Operations.End) {
      throw new Error(`batch MakeFunction: expected Operations.End, got ${view.getUint8(i)}`);
    }
    i += B1;

    const source = texts.substr(index, length);
    const thunk = (b) => {
      const fn = b.functions.constructor.compile(source);
      b.functions.heap.update(handle, fn);
      return { hint: MAKE_FUNCTION_HINT, value: new ExternalPointer(handle) };
    };
    return [i, thunk];
  }

  /** Shared head of Invoke/InvokeAsync: handle, (callback), hint, args, End. */
  static parseCallParts(batch, i, view, texts, hasCallback) {
    let handle;
    [i, handle] = BatchInstructions.readExternalHandle(view, i);

    let callbackId = null;
    if (hasCallback) {
      const vt = view.getUint8(i);
      if (vt !== ParamType.InternalReference) {
        throw new Error(`batch: expected InternalReference param, got ${vt}`);
      }
      let raw;
      [i, raw] = readQuantized(view, i + B1, noneU64);
      callbackId = BigInt(raw);
    }

    let hint;
    [i, hint] = ReturnHintParser.parseFrom(view, i);

    if (view.getUint8(i) !== ArgumentOperations.Start) {
      throw new Error(`batch args: expected Start marker, got ${view.getUint8(i)}`);
    }
    i += B1;
    let args;
    [i, args] = batch.params.parseParams(view, i, texts);

    if (view.getUint8(i) !== Operations.End) {
      throw new Error(`batch: expected Operations.End, got ${view.getUint8(i)}`);
    }
    i += B1;

    return [i, handle, callbackId, hint, args];
  }

  static invoke(batch, _opId, i, view, texts) {
    let handle, hint, args;
    [i, handle, , hint, args] = BatchInstructions.parseCallParts(batch, i, view, texts, false);

    const thunk = (b) => {
      const fn = b.functions.heap.get(handle);
      if (!fn) throw new Error(`batch invoke: no function for handle ${handle}`);
      const result = fn.apply(b.functions.context, args);
      if (hint.id === ReturnIds.None) return null;
      return { hint, value: result };
    };
    return [i, thunk];
  }

  static invokeAsync(batch, _opId, i, view, texts) {
    let handle, callbackId, hint, args;
    [i, handle, callbackId, hint, args] = BatchInstructions.parseCallParts(batch, i, view, texts, true);

    const thunk = (b) => {
      const fn = b.functions.heap.get(handle);
      if (!fn) throw new Error(`batch invokeAsync: no function for handle ${handle}`);
      const result = fn.apply(b.functions.context, args);
      if (hint.id === ReturnIds.None) return null; // fire-and-forget
      const settled = Promise.resolve(result).then(
        (value) => b.reply.callbackSuccess(b.callbacks, callbackId, hint, value),
        (error) => b.reply.callbackFailure(b.callbacks, callbackId, error),
      );
      if (b.functions.tasks) b.functions.tasks.add(settled);
      return null; // delivery happens via invoke_callback, never inline
    };
    return [i, thunk];
  }
}

// ════════════════════════════════════════════════════════════════════════════════
// SECTION 3: core runtime
// ════════════════════════════════════════════════════════════════════════════════

// ─── WasmEnvelope ──────────────────────────────────────────────────────────────

/**
 * The 14-byte WASM message header — mirror of `foundation_wasm::WasmEnvelope`.
 * Layout (little-endian): [protocol:1][version:1][memory_id:8][length:4][payload...]
 */
export class WasmEnvelope {
  static HEADER_LEN = 14;

  /**
   * Frame a payload into a fresh Uint8Array.
   * @param {number} protocol  protocol byte (0/1/2)
   * @param {number} version   protocol version
   * @param {bigint|number} memoryId  arena slot id (u64)
   * @param {Uint8Array} payload
   * @returns {Uint8Array}
   */
  static write(protocol, version, memoryId, payload) {
    const buf = new Uint8Array(WasmEnvelope.HEADER_LEN + payload.length);
    const view = new DataView(buf.buffer);
    view.setUint8(0, protocol);
    view.setUint8(1, version);
    view.setBigUint64(2, BigInt(memoryId), true);
    view.setUint32(10, payload.length, true);
    buf.set(payload, WasmEnvelope.HEADER_LEN);
    return buf;
  }

  /**
   * Parse the header (zero-copy payload view).
   * @param {ArrayBuffer|Uint8Array} bytes
   * @param {number} [byteOffset] offset when `bytes` is an ArrayBuffer
   * @returns {{ protocol:number, version:number, memoryId:bigint, length:number, payload:Uint8Array }}
   */
  static parse(bytes, byteOffset = 0) {
    const u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes, byteOffset);
    if (u8.length < WasmEnvelope.HEADER_LEN) {
      throw new RangeError("WasmEnvelope: buffer shorter than 14-byte header");
    }
    const view = new DataView(u8.buffer, u8.byteOffset, u8.byteLength);
    const protocol = view.getUint8(0);
    const version = view.getUint8(1);
    const memoryId = view.getBigUint64(2, true);
    const length = view.getUint32(10, true);
    const end = WasmEnvelope.HEADER_LEN + length;
    if (u8.length < end) {
      throw new RangeError("WasmEnvelope: payload length exceeds buffer");
    }
    return { protocol, version, memoryId, length, payload: u8.subarray(WasmEnvelope.HEADER_LEN, end) };
  }
}

// ─── ProtocolDispatcher ──────────────────────────────────────────────────────────

/**
 * Reads the protocol byte from an incoming message and routes it to the matching
 * handler `{ apply(memoryId: bigint, payload: Uint8Array) }`.
 *
 * Mirror of `foundation_wasm::dispatch_message` — same demux, same throw-on-unknown.
 */
export class ProtocolDispatcher {
  /** @param {Record<number, {apply:Function}>} handlers keyed by protocol byte */
  constructor(handlers = {}) {
    this.handlers = handlers;
  }

  setHandler(protocol, handler) {
    this.handlers[protocol] = handler;
    return this;
  }

  /** @param {ArrayBuffer|Uint8Array} buffer */
  dispatch(buffer) {
    const { protocol, memoryId, payload } = WasmEnvelope.parse(buffer);
    const handler = this.handlers[protocol];
    if (!handler) {
      throw new Error(`unknown protocol: ${protocol}`);
    }
    return handler.apply(memoryId, payload);
  }
}

// ─── MemoryAllocations ───────────────────────────────────────────────────────────

/**
 * JS view over the WASM arena. Reads the bridge lazily so it keeps working after the
 * instance is wired in `FoundationWasm.init`. Slot ids are u64 (`bigint`);
 * generation-based ids in the Rust arena make stale access fail safely (decision 028).
 */
export class MemoryAllocations {
  /** @param {{exports:object, memory:WebAssembly.Memory}} bridge shared, late-populated */
  constructor(bridge) {
    this.bridge = bridge;
  }

  /** Allocate a slot of `size` bytes; returns its id (bigint). */
  create(size) {
    return this.bridge.exports.create_allocation(BigInt(size));
  }

  /**
   * Resolve a slot id to a live byte view into WASM linear memory (zero-copy).
   * Valid only until the memory grows or the slot is disposed.
   * @returns {{ ptr:number, len:number, bytes:Uint8Array }}
   */
  get(id) {
    const ptr = Number(this.bridge.exports.allocation_start_pointer(BigInt(id)));
    const len = Number(this.bridge.exports.allocation_length(BigInt(id)));
    return { ptr, len, bytes: new Uint8Array(this.bridge.memory.buffer, ptr, len) };
  }

  /** Copy `data` into the slot's memory (slot must be ≥ data.length). */
  write(id, data) {
    this.get(id).bytes.set(data);
  }

  /** ACK/free a slot back to the arena. MUST be called after processing a message. */
  dispose(id) {
    this.bridge.exports.dispose_allocation(BigInt(id));
  }

  /** Reset a slot's contents without freeing it. */
  clear(id) {
    this.bridge.exports.clear_allocation(BigInt(id));
  }
}

// ─── TimerRegistry ───────────────────────────────────────────────────────────────

/**
 * Implements the `schedule_timeout` / `schedule_interval` host imports using the
 * platform timers, firing the matching WASM export when they elapse. WASM owns the
 * callback ids; JS just maps id → live handle so it can cancel.
 */
export class TimerRegistry {
  /**
   * @param {{exports:object}} bridge
   * @param {{setTimeout:Function,clearTimeout:Function,setInterval:Function,clearInterval:Function}} [host]
   */
  constructor(bridge, host = globalThis) {
    this.bridge = bridge;
    this.host = host;
    this.timeouts = new Map();
    this.intervals = new Map();
  }

  scheduleTimeout(callbackId, delayMs) {
    const key = String(callbackId);
    const handle = this.host.setTimeout(() => {
      this.timeouts.delete(key);
      this.bridge.exports.run_scheduled_callback(BigInt(callbackId));
    }, delayMs);
    this.timeouts.set(key, handle);
  }

  cancelTimeout(callbackId) {
    const key = String(callbackId);
    const handle = this.timeouts.get(key);
    if (handle !== undefined) {
      this.host.clearTimeout(handle);
      this.timeouts.delete(key);
    }
  }

  scheduleInterval(callbackId, intervalMs) {
    const key = String(callbackId);
    const handle = this.host.setInterval(() => {
      // run_interval_callback returns 0 (STOP) when the callback asked to stop.
      if (Number(this.bridge.exports.run_interval_callback(BigInt(callbackId))) === 0) {
        this.cancelInterval(callbackId);
      }
    }, intervalMs);
    this.intervals.set(key, handle);
  }

  cancelInterval(callbackId) {
    const key = String(callbackId);
    const handle = this.intervals.get(key);
    if (handle !== undefined) {
      this.host.clearInterval(handle);
      this.intervals.delete(key);
    }
  }
}

// ─── CallbackRegistry ────────────────────────────────────────────────────────────

/**
 * Delivers async JS→WASM responses. WASM hands JS a callback id (its
 * `InternalPointer`); when the JS-side work resolves, JS writes the (already
 * protocol-encoded) result into a fresh arena slot and calls `invoke_callback`.
 * Monotonic ids in the Rust registry make stale invocations drop silently.
 */
export class CallbackRegistry {
  /** @param {{exports:object}} bridge @param {MemoryAllocations} memory */
  constructor(bridge, memory) {
    this.bridge = bridge;
    this.memory = memory;
  }

  /** Deliver `data` (Uint8Array) to WASM callback `id`. */
  invoke(id, data) {
    const slot = this.memory.create(data.length);
    this.memory.write(slot, data);
    this.bridge.exports.invoke_callback(BigInt(id), BigInt(slot));
  }

  /** Permanently retire a WASM callback id (never reused). */
  unregister(id) {
    this.bridge.exports.unregister_callback(BigInt(id));
  }
}

// ─── StringCache ─────────────────────────────────────────────────────────────────

/**
 * Interns UTF-8/UTF-16 strings that WASM caches via the `host_cache_string` import,
 * returning a stable `bigint` handle reused for identical strings. The handle is how
 * `CachedText` params later refer to the string without re-sending the bytes.
 */
export class StringCache {
  /** @param {{memory:WebAssembly.Memory}} bridge */
  constructor(bridge) {
    this.bridge = bridge;
    this.byString = new Map(); // string -> handle (bigint)
    this.byHandle = new Map(); // handle (bigint) -> string
    this.next = 1n;
  }

  /**
   * Read a string from WASM memory at `(ptr, len)` and intern it.
   * @param {bigint|number} ptr
   * @param {bigint|number} len
   * @param {number} encoding 0 = UTF-8, 1 = UTF-16LE (JSEncoding)
   * @returns {bigint} stable handle
   */
  cache(ptr, len, encoding = 0) {
    const bytes = new Uint8Array(this.bridge.memory.buffer, Number(ptr), Number(len));
    const decoder = encoding === 1 ? new TextDecoder("utf-16le") : new TextDecoder();
    const str = decoder.decode(bytes);
    let handle = this.byString.get(str);
    if (handle === undefined) {
      handle = this.next;
      this.next += 1n;
      this.byString.set(str, handle);
      this.byHandle.set(handle, str);
    }
    return handle;
  }

  /** Resolve a handle back to its string (or `undefined`). */
  get(handle) {
    return this.byHandle.get(BigInt(handle));
  }

  /** Evict an interned string (host_string_cache_drop_external_pointer). */
  drop(handle) {
    const key = BigInt(handle);
    const str = this.byHandle.get(key);
    if (str === undefined) return false;
    this.byHandle.delete(key);
    this.byString.delete(str);
    return true;
  }
}

// ─── AnimationDriver ─────────────────────────────────────────────────────────────

/**
 * Drives the WASM animation-frame loop. WASM calls the `hook_up_animation_frames`
 * import when it registers a frame callback; this driver runs a rAF loop that calls
 * the `trigger_animation_callbacks(timestamp)` export each frame and stops once
 * `get_total_animation_callbacks()` returns 0 (decision: exposed_runtime semantics).
 */
export class AnimationDriver {
  /**
   * @param {{exports:object}} bridge
   * @param {{request:(cb:(ts:number)=>void)=>any, cancel:(handle:any)=>void}} [raf]
   */
  constructor(bridge, raf) {
    this.bridge = bridge;
    this.raf = raf ?? defaultRaf();
    this.running = false;
    this.handle = null;
  }

  /** Start the loop. Idempotent — repeated `hook_up` calls keep a single loop. */
  start() {
    if (this.running) return;
    this.running = true;
    this.#schedule();
  }

  #schedule() {
    this.handle = this.raf.request((ts) => this.#frame(ts));
  }

  #frame(ts) {
    this.bridge.exports.trigger_animation_callbacks(ts);
    if (Number(this.bridge.exports.get_total_animation_callbacks()) > 0) {
      this.#schedule(); // more callbacks pending — next frame
    } else {
      this.running = false; // 0 callbacks left — WASM tells JS to stop the loop
      this.handle = null;
    }
  }

  /** Stop the loop early (e.g. teardown). */
  stop() {
    if (this.handle !== null) this.raf.cancel(this.handle);
    this.running = false;
    this.handle = null;
  }
}

/** Browser `requestAnimationFrame` when available; a ~16ms timer fallback otherwise. */
function defaultRaf() {
  if (typeof globalThis.requestAnimationFrame === "function") {
    return {
      request: (cb) => globalThis.requestAnimationFrame(cb),
      cancel: (h) => globalThis.cancelAnimationFrame?.(h),
    };
  }
  const now = () => (globalThis.performance?.now?.() ?? Date.now());
  return {
    request: (cb) => setTimeout(() => cb(now()), 16),
    cancel: (h) => clearTimeout(h),
  };
}

// ─── TestReports (feature 13 — owned #[wasm_test] result protocol) ─────────────────

/**
 * Collects `#[wasm_test]` case outcomes delivered by the `host_report` import:
 * `{ status, message }` with status 0 = pass, 1 = fail, 2 = ignored. A runner
 * resolves async cases by awaiting `next()` — one report per executed case.
 */
export class TestReports {
  static PASS = 0;
  static FAIL = 1;
  static IGNORED = 2;

  constructor() {
    this.reports = [];
    this.waiters = [];
  }

  /** Record a report (called by the host_report import). */
  push(status, message) {
    const report = { status, message };
    const waiter = this.waiters.shift();
    if (waiter) waiter(report);
    else this.reports.push(report);
  }

  /** Resolve with the next report — already-buffered or yet to arrive. */
  next() {
    const buffered = this.reports.shift();
    if (buffered) return Promise.resolve(buffered);
    return new Promise((resolve) => this.waiters.push(resolve));
  }

  /** Number of buffered (unconsumed) reports. */
  get pending() {
    return this.reports.length;
  }
}

// ─── Batch protocol handler (protocol byte 0 = Custom Binary) ──────────────────────

/**
 * Build a ProtocolDispatcher handler for protocol byte 0 — the batch-instructions
 * format (decision 022). The envelope payload is
 * `[texts_off:u32 LE][texts_len:u32 LE][ops stream][texts pool]`; since the payload
 * is a LIVE view over WASM memory, the handler just computes absolute pointers and
 * feeds the existing BatchInstructions machinery (no re-parse, no copy).
 *
 * @param {BatchInstructions} batches
 */
export function batchProtocolHandler(batches) {
  return {
    apply(_memoryId, payload) {
      if (payload.byteLength < 8) throw new Error("batch payload: missing header");
      const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
      const textsOff = view.getUint32(0, true);
      const textsLen = view.getUint32(4, true);
      if (textsOff < 8 || textsOff + textsLen > payload.byteLength) {
        throw new Error("batch payload: header out of range");
      }
      const base = payload.byteOffset; // absolute position in WASM linear memory
      batches.applyNoReturn(base + 8, textsOff - 8, base + textsOff, textsLen);
    },
  };
}

// ─── AsyncTaskCollector ────────────────────────────────────────────────────────────

/**
 * Tracks the Promises behind async invocations so tests/hosts can await settlement
 * (`rt.awaitTasks()`). Collection is OFF by default (megatron parity) — long-running
 * apps shouldn't accumulate promise refs; enable around the window you care about.
 */
export class AsyncTaskCollector {
  constructor(collect = false) {
    this.tasks = [];
    this.collect = collect;
  }

  enable() { this.collect = true; }
  disable() { this.collect = false; }

  add(task) {
    if (!(task instanceof Promise)) throw new Error("Item must be a Promise");
    if (this.collect) this.tasks.push(task);
  }

  clear() { this.tasks.length = 0; }

  awaitAll() { return Promise.all(this.tasks); }
}

// ─── WasmStreamReceiver / WasmStreamSender (F28) ─────────────────────────────────

/**
 * Host→WASM incoming stream. The WASM side writes data; JS receives it.
 *
 * Usage:
 *   var stream = new WasmStreamReceiver({
 *     onData: function(chunk) { ... },    // { data: Uint8Array, sequence: number }
 *     onEnd:  function() { ... },
 *     onError: function(err) { ... }
 *   });
 *
 * The WASM side calls the internal `_push(id, data, seq, isLast)` to deliver
 * chunks. IDs are assigned by the JS side (monotonic counter).
 */
let _receiverIdCounter = 0;
const _receivers = {};

export class WasmStreamReceiver {
  constructor(opts) {
    this._id = ++_receiverIdCounter;
    this._onData = (opts && opts.onData) || function () {};
    this._onEnd = (opts && opts.onEnd) || function () {};
    this._onError = (opts && opts.onError) || function () {};
    _receivers[this._id] = this;
  }

  /** Called by WASM (through host_apply or direct FFI) when data arrives. */
  static _push(id, data, sequence, isLast) {
    var r = _receivers[id];
    if (!r) return;
    try {
      if (isLast) {
        r._onEnd();
        delete _receivers[id];
      } else {
        r._onData({ data: data, sequence: sequence });
      }
    } catch (e) {
      r._onError(e);
    }
  }

  /** The global ID used by WASM to reference this receiver. */
  get receiverId() { return this._id; }
}

/**
 * WASM→host outgoing stream. The WASM side creates a stream and sends chunks;
 * the host receives them through callbacks.
 *
 * Usage:
 *   var stream = new WasmStreamSender(function(chunk) {
 *     // chunk = { data: Uint8Array, sequence: number } — deliver to host
 *   }, function() {
 *     // stream complete
 *   });
 *   // Pass stream.senderId to WASM so it can call host_sender_send / host_sender_end.
 */
let _senderIdCounter = 0;
const _senders = {};

export class WasmStreamSender {
  constructor(onChunk, onEnd) {
    this._id = ++_senderIdCounter;
    this._onChunk = onChunk || function () {};
    this._onEnd = onEnd || function () {};
    _senders[this._id] = this;
  }

  /** Called by WASM when it has a chunk to send. */
  static _send(id, data, sequence) {
    var s = _senders[id];
    if (s) s._onChunk({ data: data, sequence: sequence });
  }

  /** Called by WASM when the stream is complete. */
  static _end(id) {
    var s = _senders[id];
    if (s) { s._onEnd(); delete _senders[id]; }
  }

  /** The global ID used by WASM to reference this sender. */
  get senderId() { return this._id; }
}

// ─── FoundationWasm runtime ──────────────────────────────────────────────────────

/**
 * Ties the core ABI together. Build it, instantiate with its `web_abi` import object,
 * then call `init(module)` to bind the instance.
 */
export class FoundationWasm {
  /** @param {{timerHost?:object}} [opts] */
  constructor(opts = {}) {
    // Shared, late-populated bridge — import closures and registries read it lazily.
    this.bridge = { exports: null, memory: null };
    this.memory = new MemoryAllocations(this.bridge);
    this.timers = new TimerRegistry(this.bridge, opts.timerHost);
    this.callbacks = new CallbackRegistry(this.bridge, this.memory);
    this.animation = new AnimationDriver(this.bridge, opts.rafHost);
    this.strings = new StringCache(this.bridge);
    this.functions = new FunctionRegistry(this.bridge, this.memory, this.strings, this.callbacks);
    // Host-side object heap (object returns + object_allocate_external_pointer).
    this.objects = new ExternalHeap();
    this.functions.reply.objects = this.objects;
    // Pending async-invocation promises (off by default; rt.tasks.enable() to track).
    this.tasks = new AsyncTaskCollector(false);
    this.functions.tasks = this.tasks;
    // #[wasm_test] case outcomes (host_report import — feature 13).
    this.testReports = new TestReports();
    // V2 quantized batch codec (host_batch_apply / host_batch_returning_apply).
    this.batches = new BatchInstructions(
      this.bridge, this.memory, this.strings, this.functions, this.callbacks,
    );
    this.dispatcher = new ProtocolDispatcher();
    // Protocol byte 0 (Custom Binary) IS the batch-instructions format (decision
    // 022) — pre-wire its handler so batch messages route without extra setup.
    this.dispatcher.setHandler(0, batchProtocolHandler(this.batches));
    // F27: Protocol bytes 3 = capability trigger, 4 = IPC trigger.
    // Handlers are set lazily via registerTriggerHandlers() so the WASM app
    // can register callbacks before the dispatcher routes to them.
    this._capTriggerHandler = null;
    this._ipcTriggerHandler = null;
    this.dispatcher.setHandler(3, { apply: (mid, payload) => this._dispatchCapTrigger(mid, payload) });
    this.dispatcher.setHandler(4, { apply: (mid, payload) => this._dispatchIpcTrigger(mid, payload) });
    // F28: Stream registry placeholder — WASM exports register streams here.
    this._streamRegistry = null;
    // F28: Host-side stream registry. Streams are created by WASM via
    // host_stream_create() → returns an ID. Chunks are buffered until
    // callbacks are bound via host_stream_bind(). This decouples creation
    // (WASM gets an ID it can pass as a return value) from consumption
    // (JS binds callbacks later).
    this._streamRegistry = {};
    this._nextStreamId = 1;
    // F41: IPC handler (set via registerIpcHandler by ipc-bridge.js)
    this._ipcHandler = null;
    // F43: async IPC handler (set via registerIpcAsyncHandler)
    this._ipcAsyncHandler = null;
    this._ipcStreamHandler = null;
    this._ipcStreamRegistry = {};
  }

  // ── F41: IPC binary wire format encode/decode (static helpers) ──────────
  //
  // Mirrors Rust `ipc::encode_request` / `encode_response` / `decode_request` /
  // `decode_response`. Wire layout (little-endian):
  //   4 bytes LE u32: ipc name length + N bytes ipc name (UTF-8)
  //   4 bytes LE u32: action length + N bytes action (UTF-8)
  //   1 byte:         content_type (0=Json, 1=Arrow, 2=Binary)
  //   4 bytes LE u32: target length + N bytes target (UTF-8, 0 len = None)
  //   4 bytes LE u32: payload length + N bytes payload

  /** @private — write a length-prefixed UTF-8 string into an array */
  static _ipcWriteStr(arr, str) {
    var enc = new TextEncoder().encode(str);
    var len = enc.length;
    arr.push(len & 0xff, (len >> 8) & 0xff, (len >> 16) & 0xff, (len >> 24) & 0xff);
    for (var i = 0; i < enc.length; i++) arr.push(enc[i]);
  }

  /** @private — encode an IpcRequest to binary wire bytes (Uint8Array) */
  static _ipcEncodeRequest(req) {
    var parts = [];
    FoundationWasm._ipcWriteStr(parts, req.ipc || "");
    FoundationWasm._ipcWriteStr(parts, req.action || "");
    parts.push(req.content_type !== undefined ? req.content_type : 0);
    FoundationWasm._ipcWriteStr(parts, req.target || "");
    var payload = req.payload;
    if (typeof payload === "string") payload = new TextEncoder().encode(payload);
    else if (!(payload instanceof Uint8Array)) payload = new Uint8Array(payload || []);
    // Write payload length (4 bytes LE) + payload bytes directly (no _ipcWriteStr)
    parts.push(payload.length & 0xff, (payload.length >> 8) & 0xff, (payload.length >> 16) & 0xff, (payload.length >> 24) & 0xff);
    for (var i = 0; i < payload.length; i++) parts.push(payload[i]);
    return new Uint8Array(parts);
  }

  /** @private — encode an IpcResponse to binary wire bytes (value-only: ct + payload len + payload) */
  static _ipcEncodeResponse(ct, payload) {
    if (typeof payload === "string") payload = new TextEncoder().encode(payload);
    else if (!(payload instanceof Uint8Array)) payload = new Uint8Array(payload || []);
    var out = new Uint8Array(5 + payload.length);
    out[0] = ct !== undefined ? ct : 0;
    out[1] = payload.length & 0xff;
    out[2] = (payload.length >> 8) & 0xff;
    out[3] = (payload.length >> 16) & 0xff;
    out[4] = (payload.length >> 24) & 0xff;
    out.set(payload, 5);
    return out;
  }

  /** @private — decode binary wire bytes into an IpcResponse { content_type, payload: Uint8Array } */
  static _ipcDecodeResponse(bytes) {
    if (bytes.length < 5) return null;
    var ct = bytes[0];
    var plen = bytes[1] | (bytes[2] << 8) | (bytes[3] << 16) | (bytes[4] << 24);
    if (bytes.length < 5 + plen) return null;
    return { content_type: ct, payload: bytes.slice(5, 5 + plen) };
  }

  /** @private — decode binary wire bytes into an IpcRequest with payload as Uint8Array */
  static _ipcDecodeRequest(bytes) {
    var off = 0;
    function readStr() {
      if (off + 4 > bytes.length) return null;
      var len = bytes[off] | (bytes[off+1] << 8) | (bytes[off+2] << 16) | (bytes[off+3] << 24);
      off += 4;
      if (off + len > bytes.length) return null;
      var s = new TextDecoder().decode(bytes.slice(off, off + len));
      off += len;
      return s;
    }
    var ipc = readStr(); if (ipc === null) return null;
    var action = readStr(); if (action === null) return null;
    if (off >= bytes.length) return null;
    var ct = bytes[off]; off++;
    var target = readStr(); if (target === null) return null;
    if (off + 4 > bytes.length) return null;
    var plen = bytes[off] | (bytes[off+1] << 8) | (bytes[off+2] << 16) | (bytes[off+3] << 24);
    off += 4;
    if (off + plen > bytes.length) return null;
    return { ipc: ipc, action: action, content_type: ct, target: target || null, payload: bytes.slice(off, off + plen) };
  }

  // ── F41: WASM→host IPC dispatch ────────────────────────────────────────

  /**
   * Called by host_ipc_invoke(ptr, len). Decodes binary request, dispatches
   * to registered handler, encodes response, returns allocation ID or 0n.
   */
  _dispatchIpcInvoke(ptr, len) {
    try {
      var bytes = new Uint8Array(this.bridge.memory.buffer, Number(ptr), Number(len));
      var req = FoundationWasm._ipcDecodeRequest(bytes);
      if (!req) return 0n;
      if (!this._ipcHandler) return 0n;
      var resp = this._ipcHandler(req);
      if (!resp) return 0n;
      var respBytes = FoundationWasm._ipcEncodeResponse(resp.content_type, resp.payload);
      var allocId = this.memory.create(respBytes.length);
      this.memory.write(allocId, respBytes);
      return allocId;
    } catch(_) { return 0n; }
  }

  // ── F43: WASM→host IPC dispatch (async) ────────────────────────────────

  /**
   * Called by host_ipc_invoke_async(ptr, len, token). Decodes request,
   * dispatches through the async handler (returns a Promise), then calls
   * ipc_resolve(token, allocId) when the Promise settles.
   */
  _dispatchIpcInvokeAsync(ptr, len, token) {
    try {
      var bytes = new Uint8Array(this.bridge.memory.buffer, Number(ptr), Number(len));
      var req = FoundationWasm._ipcDecodeRequest(bytes);
      if (!req) return;
      if (!this._ipcAsyncHandler) return;
      var self = this;
      var inst = this.bridge.instance;
      Promise.resolve(this._ipcAsyncHandler(req)).then(function(resp) {
        if (!resp) return;
        var respBytes = FoundationWasm._ipcEncodeResponse(resp.content_type, resp.payload);
        var allocId = self.memory.create(respBytes.length);
        self.memory.write(allocId, respBytes);
        inst.exports.ipc_resolve(token, allocId);
      }, function(err) {
        console.error('[WASM-IPC] handler rejected:', err);
        // Encode IpcError::ExecutionFailed (6442) using ReplyEncoder
        var errBytes = self.functions.reply.encode([{ type: 31, value: 6442 }]); // ReturnType.ErrorCode
        var allocId = self.memory.create(errBytes.length);
        self.memory.write(allocId, errBytes);
        inst.exports.ipc_resolve(token, allocId);
      });
    } catch(_) { /* discard */ }
  }

  // ── F41: Host→WASM stream dispatch ─────────────────────────────────────

  _dispatchIpcStreamOpen(ptr, len) {
    try {
      var bytes = new Uint8Array(this.bridge.memory.buffer, Number(ptr), Number(len));
      var req = FoundationWasm._ipcDecodeRequest(bytes);
      if (!req || !this._ipcStreamHandler) return 0n;
      var streamId = this._nextStreamId++;
      var queue = [];
      var closed = false;
      var self = this;
      this._ipcStreamRegistry[streamId] = { queue: queue, closed: false };
      this._ipcStreamHandler(req, {
        write: function(data) { self._ipcStreamRegistry[streamId].queue.push(data); },
        end: function() { self._ipcStreamRegistry[streamId].closed = true; },
      });
      return BigInt(streamId);
    } catch(_) { return 0n; }
  }

  _dispatchIpcStreamRead(streamId) {
    try {
      var s = this._ipcStreamRegistry[Number(streamId)];
      if (!s) return this._ipcErrorAlloc();
      if (s.queue.length === 0) {
        if (s.closed) { delete this._ipcStreamRegistry[Number(streamId)]; return 0n; }
        return 0n; // empty but not yet closed — caller polls
      }
      var chunk = s.queue.shift();
      var data = chunk.payload || chunk;
      var ct = chunk.content_type !== undefined ? chunk.content_type : 0;
      var respBytes = FoundationWasm._ipcEncodeResponse(ct, data);
      var allocId = this.memory.create(respBytes.length);
      this.memory.write(allocId, respBytes);
      return allocId;
    } catch(_) { return 0n; }
  }

  _dispatchIpcStreamClose(streamId) {
    delete this._ipcStreamRegistry[Number(streamId)];
  }

  /**
   * Register the WASM→host IPC handler (sync). Called by ipc-bridge.js.
   * @param {{ onIpc: Function, onIpcStream: Function }} handlers
   */
  registerIpcHandler(handlers) {
    if (handlers.onIpc) this._ipcHandler = handlers.onIpc;
    if (handlers.onIpcStream) this._ipcStreamHandler = handlers.onIpcStream;
  }

  /**
   * F43: Register the WASM→host IPC handler (async). Called by ipc-bridge.js.
   * The handler must return a Promise<{content_type, payload}>.
   * @param {Function} handler  async function(req) → Promise<response>
   */
  registerIpcAsyncHandler(handler) {
    this._ipcAsyncHandler = handler;
  }

  /**
   * F27: Deliver a capability trigger FROM the host INTO the WASM module.
   * The host (browser, Tauri, Deno) calls this to invoke a capability handler
   * registered on the WASM side via TriggerRegistry.
   *
   * @param {{ capability: string, action: string, payload: Uint8Array }} request
   * @returns {Promise<object>} capability response
   */
  triggerCapability(request) {
    if (!this._capTriggerHandler) {
      return Promise.reject(new Error('triggerCapability: no handler registered'));
    }
    return this._capTriggerHandler(request);
  }

  /**
   * F27: Deliver an IPC trigger FROM the host INTO the WASM module.
   *
   * @param {{ ipc: string, action: string, payload: Uint8Array }} request
   * @returns {Promise<object>} IPC response
   */
  triggerIpc(request) {
    if (!this._ipcTriggerHandler) {
      return Promise.reject(new Error('triggerIpc: no handler registered'));
    }
    return this._ipcTriggerHandler(request);
  }

  /**
   * F27: Register trigger handlers for capability and IPC dispatch.
   * Called by foundation_wasm_ui once the WASM app has set up its
   * TriggerRegistry.
   *
   * @param {{ onCapability: Function, onIpc: Function }} handlers
   */
  registerTriggerHandlers(handlers) {
    if (handlers.onCapability) this._capTriggerHandler = handlers.onCapability;
    if (handlers.onIpc) this._ipcTriggerHandler = handlers.onIpc;
  }

  /** @private */
  _dispatchCapTrigger(memoryId, payload) {
    if (!this._capTriggerHandler) return;
    try {
      var req = JSON.parse(new TextDecoder().decode(payload));
      this._capTriggerHandler(req);
    } catch (_) { /* best-effort */ }
    this.memory.dispose(memoryId);
  }

  /** @private */
  _dispatchIpcTrigger(memoryId, payload) {
    if (!this._ipcTriggerHandler) return;
    try {
      var req = JSON.parse(new TextDecoder().decode(payload));
      this._ipcTriggerHandler(req);
    } catch (_) { /* best-effort */ }
    this.memory.dispose(memoryId);
  }

  /**
   * The `{ ...abi imports }` object — pass as `{ abi: rt.web_abi }` to instantiate.
   * The DOM-specific imports (rAF, function invocation, dom_* refs) are layered on by
   * foundation-wasm-ui.js, which can extend this object.
   */
  get web_abi() {
    const self = this;
    const { bridge, memory, dispatcher, timers, animation, strings, functions, objects, batches } = this;
    return {
      // Uniform protocol transport: WASM shipped a message in slot `memId`.
      host_apply(memId, ptr, len) {
        try {
          dispatcher.dispatch(memory.get(memId).bytes);
        } finally {
          memory.dispose(memId); // ACK: always free, even if the handler threw.
        }
      },
      schedule_timeout(timing, callbackId) {
        timers.scheduleTimeout(callbackId, Number(timing));
      },
      unschedule_timeout(callbackId) {
        timers.cancelTimeout(callbackId);
      },
      schedule_interval(timing, callbackId) {
        timers.scheduleInterval(callbackId, Number(timing));
      },
      unschedule_interval(callbackId) {
        timers.cancelInterval(callbackId);
      },
      // WASM registered a frame callback — start (or keep) the rAF loop.
      hook_up_animation_frames() {
        animation.start();
      },
      // Intern a UTF-8/UTF-16 string from WASM memory; returns a stable handle.
      host_cache_string(ptr, len, encoding) {
        return strings.cache(ptr, len, Number(encoding));
      },
      // #[wasm_test] outcome (feature 13): status 0/1/2 + optional UTF-8 message.
      host_report: (status, ptr, len) => {
        const length = Number(len);
        const message = length
          ? new TextDecoder().decode(
              new Uint8Array(this.bridge.memory.buffer, Number(ptr), length),
            )
          : "";
        this.testReports.push(Number(status), message);
      },

      // Function registry: register a JS fn (source string) → handle; invoke it.
      host_register_function(start, len, utf) {
        return functions.register(start, len, utf);
      },
      host_invoke_function(handle, pPtr, pLen, rPtr, rLen) {
        return functions.invoke(handle, pPtr, pLen, rPtr, rLen);
      },
      // Async fn: result Promise → framed reply delivered later via invoke_callback.
      host_invoke_async_function(handle, callbackHandle, pPtr, pLen, rPtr, rLen) {
        functions.invokeAsync(handle, callbackHandle, pPtr, pLen, rPtr, rLen);
      },
      // Typed fast-paths: return the naked scalar directly.
      host_invoke_function_as_bool: (h, p, l) => functions.invokeAsBool(h, p, l),
      host_invoke_function_as_f32: (h, p, l) => functions.invokeAsFloat(h, p, l),
      host_invoke_function_as_f64: (h, p, l) => functions.invokeAsFloat(h, p, l),
      host_invoke_function_as_u8: (h, p, l) => functions.invokeAsInt(h, p, l),
      host_invoke_function_as_u16: (h, p, l) => functions.invokeAsInt(h, p, l),
      host_invoke_function_as_u32: (h, p, l) => functions.invokeAsInt(h, p, l),
      host_invoke_function_as_i8: (h, p, l) => functions.invokeAsInt(h, p, l),
      host_invoke_function_as_i16: (h, p, l) => functions.invokeAsInt(h, p, l),
      host_invoke_function_as_i32: (h, p, l) => functions.invokeAsInt(h, p, l),
      host_invoke_function_as_u64: (h, p, l) => functions.invokeAsBigInt(h, p, l),
      // Object fast-path: interned in the object heap, handle crosses naked.
      host_invoke_function_as_object: (h, p, l) => functions.invokeAsObject(h, p, l),
      host_invoke_function_as_i64: (h, p, l) => functions.invokeAsBigInt(h, p, l),
      host_unregister_function(handle) {
        functions.unregister(handle);
      },
      // megatron-era aliases/extras, kept so megatron-built modules instantiate:
      host_function_drop_external_pointer(handle) {
        functions.unregister(handle);
      },
      // String fast-path: raw UTF-8 into a fresh slot, slot id crosses.
      host_invoke_function_as_str: (h, p, l) => functions.invokeAsString(h, p, l),
      // WASM-side fatal: surface it as a JS exception (megatron parity).
      host_abort() {
        throw new Error("WasmInstance called abort");
      },

      // Pre-allocate empty heap handles for later binding (batch MakeFunction / objects).
      function_allocate_external_pointer: () => functions.allocate(),
      object_allocate_external_pointer: () => objects.create(null),

      // Retire host-heap handles (generation-checked; stale ids no-op).
      host_object_drop_external_pointer(handle) {
        objects.destroy(BigInt(handle));
      },
      host_string_cache_drop_external_pointer(handle) {
        strings.drop(handle);
      },

      // V2 quantized batch transport: ops + texts buffers in WASM memory.
      host_batch_apply(opsPtr, opsLen, textPtr, textLen) {
        batches.applyNoReturn(opsPtr, opsLen, textPtr, textLen);
      },
      host_batch_returning_apply(opsPtr, opsLen, textPtr, textLen) {
        return batches.applyReturning(opsPtr, opsLen, textPtr, textLen);
      },

      // F28: Stream FFI — WASM pushes data through buffered host-side streams.
      //
      // host_stream_create()  → returns stream ID (u64). WASM can pass this
      //                         as a return value. Chunks are buffered until
      //                         callbacks are bound.
      // host_stream_bind(id, onChunk, onEnd) → bind callbacks, drain buffer.
      // host_sender_send(id, ptr, len, seq)  → push chunk (buffer or deliver).
      // host_sender_end(id)                  → signal end (or mark ended).
      //
      // This decouples stream creation from consumption:
      //   1. WASM calls host_stream_create() → gets ID
      //   2. WASM passes ID as return value to JS
      //   3. JS binds callbacks via host_stream_bind(id, onChunk, onEnd)
      //   4. WASM pushes chunks → delivered to onChunk
      //   5. WASM calls host_sender_end → delivered to onEnd
      host_stream_create() {
        var id = self._nextStreamId++;
        self._streamRegistry[id] = {
          chunks: [],
          onChunk: null,
          onEnd: null,
          ended: false,
          bind: function (chunkFn, endFn) {
            this.onChunk = chunkFn || null;
            this.onEnd = endFn || null;
            // Drain buffered chunks
            if (this.onChunk) {
              for (var i = 0; i < this.chunks.length; i++) {
                this.onChunk(this.chunks[i]);
              }
              this.chunks.length = 0;
            }
            if (this.ended && this.onEnd) {
              this.onEnd();
            }
          },
        };
        return BigInt(id);
      },
      host_stream_bind(streamId, cbOnChunk, cbOnEnd) {
        var s = self._streamRegistry[Number(streamId)];
        if (!s) return;
        s.onChunk = typeof cbOnChunk === "function" ? cbOnChunk : null;
        s.onEnd = typeof cbOnEnd === "function" ? cbOnEnd : null;
        // Drain buffered chunks
        if (s.onChunk) {
          for (var i = 0; i < s.chunks.length; i++) {
            s.onChunk(s.chunks[i]);
          }
          s.chunks.length = 0;
        }
        // If already ended, fire onEnd immediately
        if (s.ended && s.onEnd) {
          s.onEnd();
        }
      },
      host_sender_send(streamId, dataPtr, dataLen, seq) {
        var id = Number(streamId);
        var data = new Uint8Array(bridge.memory.buffer, Number(dataPtr), Number(dataLen));
        var s = self._streamRegistry[id];
        if (s) {
          var chunk = { data: data, sequence: Number(seq) };
          if (s.onChunk) {
            s.onChunk(chunk);
          } else {
            s.chunks.push(chunk);
          }
        } else {
          // Fallback: direct WasmStreamSender (pre-F28 API)
          WasmStreamSender._send(id, data, Number(seq));
        }
      },
      host_sender_end(streamId) {
        var id = Number(streamId);
        var s = self._streamRegistry[id];
        if (s) {
          s.ended = true;
          if (s.onEnd) s.onEnd();
          delete self._streamRegistry[id];
        } else {
          // Fallback: direct WasmStreamSender (pre-F28 API)
          WasmStreamSender._end(id);
        }
      },
      // Host → WASM (incoming): call host_receiver_push to deliver chunks.
      host_receiver_push(receiverId, dataPtr, dataLen, seq, isLast) {
        var data = new Uint8Array(bridge.memory.buffer, Number(dataPtr), Number(dataLen));
        WasmStreamReceiver._push(Number(receiverId), data, Number(seq), isLast !== 0);
      },

      // ── F43: IPC host import (wasm → host) ─────────────────────────────
      //
      // WASM calls host_ipc_invoke(ptr, len, callback_id) with a registered
      // callback. The host dispatches asynchronously and calls
      // ipc_resolve(callback_id, allocId) when the response is ready.
      host_ipc_invoke(ptr, len, callback_id) {
        self._dispatchIpcInvokeAsync(ptr, len, callback_id);
      },
      // host_ipc_stream_open(ptr, len) → stream_id: WASM requests a host→WASM
      //   stream. The host creates a queue, returns an ID. WASM polls chunks.
      host_ipc_stream_open(ptr, len) {
        return self._dispatchIpcStreamOpen(ptr, len);
      },
      // host_ipc_stream_read(streamId) → allocation_id: WASM polls next chunk
      //   from a host-created stream. Returns 0 when closed/done.
      host_ipc_stream_read(streamId) {
        return self._dispatchIpcStreamRead(streamId);
      },
      // host_ipc_stream_close(streamId): WASM closes a host-created stream.
      host_ipc_stream_close(streamId) {
        self._dispatchIpcStreamClose(streamId);
      },
    };
  }

  /**
   * Bind the instantiated module. Accepts the `{ instance }` result of
   * `WebAssembly.instantiate` (or a bare instance). Memory is taken from the
   * instance's exported `memory`.
   */
  init(moduleOrInstance) {
    const instance = moduleOrInstance.instance ?? moduleOrInstance;
    this.bridge.exports = instance.exports;
    this.bridge.memory = instance.exports.memory;
    this.bridge.instance = instance;
    return this;
  }

  /** Register a protocol handler `{ apply(memoryId, payload) }` for a protocol byte. */
  setProtocolHandler(protocol, handler) {
    this.dispatcher.setHandler(protocol, handler);
    return this;
  }

  /** Await every tracked async invocation (requires `rt.tasks.enable()` beforehand). */
  awaitTasks() {
    return this.tasks.awaitAll();
  }
}

// ─── WasmLoader / WasmWebScripts (megatron WASMLoader parity) ──────────────────────

/**
 * Convenience loader: owns a FoundationWasm runtime, instantiates a module from a
 * URL (streaming) or raw bytes with the `abi` imports + a `js.mem` memory, and binds
 * the bridge. Mirrors megatron WASMLoader incl. the JS-string-builtins compile
 * options (`builtins: ["js-strings"]`, `importedStringConstants`).
 */
export class WasmLoader {
  /**
   * @param {{initialMemory?:number, maximumMemory?:number, environment?:object,
   *          compileOptions?:object}} [opts] memory sizes are in WASM pages
   */
  constructor(opts = {}) {
    this.runtime = new FoundationWasm(opts);
    this.environment = opts.environment ?? {};
    this.compileOptions = WasmLoader.compileOptionsOf(opts.compileOptions);
    this.memory = new WebAssembly.Memory({
      initial: opts.initialMemory ?? 10,
      maximum: opts.maximumMemory ?? 200,
    });
    this.module = null;
  }

  static compileOptionsOf(compileOptions) {
    return {
      builtins: compileOptions?.builtins ?? ["js-strings"],
      importedStringConstants: compileOptions?.importedStringConstants ?? "imported_strings",
    };
  }

  /** The full import object: the ABI, a host memory under `js.mem`, plus extras. */
  get imports() {
    return { abi: this.runtime.web_abi, js: { mem: this.memory }, ...this.environment };
  }

  /** Instantiate from a URL via instantiateStreaming. */
  async loadURL(url) {
    const module = await WebAssembly.instantiateStreaming(
      fetch(url), this.imports, this.compileOptions,
    );
    this.#bind(module);
    return this;
  }

  /** Instantiate from raw bytes (ArrayBuffer/TypedArray). */
  async loadBytes(bytes) {
    const module = await WebAssembly.instantiate(bytes, this.imports, this.compileOptions);
    this.#bind(module);
    return this;
  }

  #bind(module) {
    this.module = module;
    this.runtime.init(module);
  }

  /** Call the module's exported `main()`. */
  run() {
    if (!this.module) throw new Error("No wasm module loaded");
    const main = this.module.instance.exports.main;
    if (!main) throw new Error("wasm module has no exported main function");
    return main();
  }

  /** Load every `<script type="application/wasm" src=…>` on the page → loaders. */
  static async fromScripts(opts = {}) {
    const scripts = document.querySelectorAll('script[type="application/wasm"]');
    const loading = [];
    for (const script of scripts) {
      if (!script.src) continue;
      loading.push(new WasmLoader(opts).loadURL(script.src));
    }
    return Promise.all(loading);
  }
}

/**
 * Page bootstrap: loads every `application/wasm` script tag and runs each module's
 * `main()` (megatron WasmWebScripts parity).
 */
export class WasmWebScripts {
  constructor(opts = {}) {
    this.modules = WasmLoader.fromScripts(opts);
  }

  static default(environment) {
    return new WasmWebScripts({ environment });
  }

  async runAll() {
    const loaders = await this.modules;
    for (const loader of loaders) loader.run();
    return loaders;
  }
}

// ─── Global registration ──────────────────────────────────────────────────────────
//
// The ESM exports above are canonical (`<script type="module">` / import). This
// mirror lets classic (non-module) scripts on the same page reach the runtime as
// `globalThis.FoundationWasmRuntime` once the module has loaded.
globalThis.FoundationWasmRuntime = Object.freeze({
  FoundationWasm,
  batchProtocolHandler,
  WasmLoader,
  WasmWebScripts,
  AsyncTaskCollector,
  TestReports,
  WasmEnvelope,
  ProtocolDispatcher,
  MemoryAllocations,
  TimerRegistry,
  CallbackRegistry,
  StringCache,
  AnimationDriver,
  FunctionRegistry,
  ParameterParser,
  ReturnHintParser,
  ReplyEncoder,
  ExternalHeap,
  BatchInstructions,
  BatchParameterParser,
  Operations,
  ArgumentOperations,
  TypeOptimization,
  ParamType,
  ReturnType,
  ReturnIds,
  ThreeStateId,
  ExternalPointer,
  InternalPointer,
  CachePointer,
  ErrorCodeValue,
  TypedArraySliceValue,
  TypedSliceArray,
  ReplyContainer,
  FakeNode,
  ReplyError,
  WasmStreamReceiver,
  WasmStreamSender,
});
