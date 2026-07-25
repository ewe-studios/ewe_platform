
/* ═════════ foundation-wasm.js ═════════ */
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
class RefPointer {
  constructor(id) { this.id = id; }
  get value() { return this.id; }
}
class ExternalPointer extends RefPointer {}
class InternalPointer extends RefPointer {}
class CachePointer extends RefPointer {}
class ErrorCodeValue { constructor(code) { this.code = code; } }
class TypedArraySliceValue {
  constructor(sliceType, content) { this.sliceType = sliceType; this.content = content; }
}

/**
 * A pre-typed return slot: `{type: ReturnType, value}`. Registered functions build
 * these via the context `as*` helpers; the encoder passes them through untouched
 * instead of inferring the type from the hint (megatron ReplyContainer parity).
 */
class ReplyContainer {
  constructor(type, value) { this.type = type; this.value = value; }
}

/** Minimal DOM-node stand-in for non-DOM hosts (megatron FakeNode parity). */
class FakeNode {
  constructor(tag) { this.tag = tag; }
}

/** A failure a registered fn raises to reach the WASM callback as an ErrorCode. */
class ReplyError extends Error {
  constructor(code, options) {
    if (!Number.isInteger(code)) {
      throw new Error("Only numbers allowed to represent the code to be sent");
    }
    super(`Reply failed with error code: ${code}`, options);
    this.code = code;
  }
}

// ─── Shared discriminants (the cross-language contract) ─────────────────────────
const ParamType = Object.freeze({
  Null: 0, Undefined: 1, Bool: 2, Text8: 3, Text16: 4, Int8: 5, Int16: 6, Int32: 7,
  Int64: 8, Uint8: 9, Uint16: 10, Uint32: 11, Uint64: 12, Float32: 13, Float64: 14,
  ExternalReference: 15, Uint8Array: 16, Uint16Array: 17, Uint32Array: 18, Uint64Array: 19,
  Int8Array: 20, Int16Array: 21, Int32Array: 22, Int64Array: 23, Float32Array: 24,
  Float64Array: 25, InternalReference: 26, Int128: 27, Uint128: 28, CachedText: 29,
  TypedArraySlice: 30, ErrorCode: 31,
});

const ReturnType = Object.freeze({
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

const ReturnIds = Object.freeze({ None: 0, One: 1, Multi: 2, List: 3 });
const ThreeStateId = Object.freeze({ One: 70, Two: 80, Three: 90 });
const ReturnHintMarker = Object.freeze({ Start: 200, Stop: 201 });
// The reply ReturnValues binary is framed Begin..End (Rust `FromBinary for ReturnTypeHints`).
const ReturnValueMarker = Object.freeze({ Begin: 100, End: 101 });
const TypedSliceArray = {
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
class ExternalHeap {
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
class ParameterParser {
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
class ReturnHintParser {
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
class ReplyEncoder {
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
class FunctionRegistry {
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
const Operations = Object.freeze({
  Begin: 0, MakeFunction: 1, Invoke: 2, InvokeAsync: 3, End: 254, Stop: 255,
});

const ArgumentOperations = Object.freeze({ Start: 1, Begin: 2, End: 3, Stop: 4 });

const TypeOptimization = Object.freeze({
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
class BatchParameterParser {
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
class BatchInstructions {
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
class WasmEnvelope {
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
class ProtocolDispatcher {
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
class MemoryAllocations {
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
class TimerRegistry {
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
class CallbackRegistry {
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
class StringCache {
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
class AnimationDriver {
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
class TestReports {
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
function batchProtocolHandler(batches) {
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
class AsyncTaskCollector {
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

class WasmStreamReceiver {
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

class WasmStreamSender {
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
class FoundationWasm {
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
        var errCode = errorCodeOf(self.functions.asReplyError(6442)); // IpcError::ExecutionFailed
        var errBytes = self.functions.reply.encode([{ type: ReturnType.ErrorCode, value: errCode }]);
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
class WasmLoader {
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
class WasmWebScripts {
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

/* ═════════ ipc-bridge.js ═════════ */
// ipc-bridge.js — unified IPC bridge for WASM apps (F41, replaces capability-bridge.js).
//
// WHY: IPC and capabilities are unified under Ipc<Input, Output>. One bridge
// handles WASM→host invoke and host→WASM events across all hosts.
//
// WHAT: global `invokeIpc(name, action, payload)` function. Host detection:
//   - Tauri:   routes through __TAURI_INTERNALS__.invoke('__ewe_ipc', ...)
//   - Deno:    Deno.core.opAsync('op_ipc_invoke', ...)
//   - Browser: direct host_ipc_invoke WASM import (FoundationWasm handles it)
//
// Also registers trigger handlers so the host can push events back to WASM.
//
// HOW: exported as ESM and globalThis.invokeIpc for classic <script>.

;(function () {
  'use strict';

  function isTauri() {
    return typeof window !== 'undefined'
      && window.__TAURI_INTERNALS__
      && typeof window.__TAURI_INTERNALS__.invoke === 'function';
  }

  function isDeno() {
    return typeof Deno !== 'undefined' && Deno.core && typeof Deno.core.opAsync === 'function';
  }

  // ── Tauri transport ────────────────────────────────────────────────────

  async function tauriInvokeIpc(name, action, payload) {
    var jsonPayload = JSON.stringify(payload !== undefined ? payload : {});
    var result = await window.__TAURI_INTERNALS__.invoke('__ewe_ipc', {
      ipc: name,
      action: action,
      payload: Array.from(new TextEncoder().encode(jsonPayload)),
      content_type: 'application/json',
    });
    try { return JSON.parse(new TextDecoder().decode(new Uint8Array(result))); }
    catch (_) { return { content_type: 0, payload: result }; }
  }

  // ── Deno transport ─────────────────────────────────────────────────────

  async function denoInvokeIpc(name, action, payload) {
    try {
      var jsonPayload = JSON.stringify(payload !== undefined ? payload : {});
      var result = await Deno.core.opAsync('op_ipc_invoke', {
        ipc: name,
        action: action,
        payload: Array.from(new TextEncoder().encode(jsonPayload)),
        content_type: 0,
      });
      if (typeof result === 'string') return JSON.parse(result);
      return { content_type: 0, payload: result };
    } catch (e) {
      throw new Error('denoInvokeIpc: ' + (e.message || e));
    }
  }

  // ── Browser transport ──────────────────────────────────────────────────

  function browserInvokeIpc(name, action, payload) {
    var FoundationWasm = globalThis.FoundationWasm;
    if (!FoundationWasm || !FoundationWasm._ipcEncodeRequest) {
      throw new Error('invokeIpc: FoundationWasm runtime not loaded.');
    }
    var jsonPayload = JSON.stringify(payload !== undefined ? payload : {});
    var req = {
      ipc: name,
      action: action,
      content_type: 0,
      target: null,
      payload: new TextEncoder().encode(jsonPayload),
    };
    var encoded = FoundationWasm._ipcEncodeRequest(req);
    // FoundationWasm handles host_ipc_invoke internally via the WASM import
    // For browser, we call directly through FoundationWasm's handler if set
    var rt = FoundationWasm._instance;
    if (!rt || !rt._ipcHandler) {
      throw new Error('invokeIpc: no IPC handler registered for browser transport.');
    }
    var result = rt._ipcHandler(req);
    if (!result) throw new Error('invokeIpc: handler returned null');
    if (result.content_type === 0 && result.payload) {
      try { return JSON.parse(new TextDecoder().decode(result.payload)); }
      catch (_) { return result; }
    }
    return result;
  }

  // ── Public API ─────────────────────────────────────────────────────────

  /**
   * Invoke an IPC handler by name. Returns the deserialized result.
   *
   * @param {string} name   — IPC handler name (e.g. "camera", "echo", "filesystem")
   * @param {string} action — action (e.g. "open", "capture", "pick")
   * @param {object} [payload] — JSON-serializable payload
   * @returns {Promise<any>} the IPC response
   */
  async function invokeIpc(name, action, payload) {
    if (!name || typeof name !== 'string') {
      throw new Error('invokeIpc: name must be a non-empty string');
    }
    if (!action || typeof action !== 'string') {
      throw new Error('invokeIpc: action must be a non-empty string');
    }
    if (isTauri()) return await tauriInvokeIpc(name, action, payload);
    if (isDeno()) return await denoInvokeIpc(name, action, payload);
    // Browser fallback: try FoundationWasm
    return browserInvokeIpc(name, action, payload);
  }

  // ── Trigger handler registration (host→WASM) ──────────────────────────

  /**
   * Register handlers so the host can push events back to WASM.
   * Called after FoundationWasm.init() when trigger handlers are ready.
   *
   * @param {object} rt — FoundationWasm runtime instance
   */
  function registerIpcTriggers(rt) {
    // Host→WASM IPC events (toolbar taps, notification responses, etc.)
    rt.registerIpcHandler({
      onIpc: function (req) {
        if (isTauri()) {
          var payload = new TextDecoder().decode(req.payload || new Uint8Array());
          try { payload = JSON.parse(payload); } catch (_) {}
          return tauriInvokeIpc(req.ipc, req.action, payload);
        }
        if (isDeno()) {
          var payload = new TextDecoder().decode(req.payload || new Uint8Array());
          try { payload = JSON.parse(payload); } catch (_) {}
          return denoInvokeIpc(req.ipc, req.action, payload);
        }
        return null;
      },
    });

    // F43: Register async IPC handler for host_ipc_invoke_async.
    // On Tauri, invokeIpc is async and returns a Promise — the async
    // dispatch path resolves it through ipc_resolve(token, allocId).
    rt.registerIpcAsyncHandler(async function (req) {
      var payload = new TextDecoder().decode(req.payload || new Uint8Array());
      try { payload = JSON.parse(payload); } catch (_) {}
      var result = await invokeIpc(req.ipc, req.action, payload);
      // invokeIpc returns { content_type, payload } or just the payload bytes
      if (result && result.content_type !== undefined) {
        return result;
      }
      return { content_type: 0, payload: result };
    });

    // Register trigger handlers for protocol bytes 3 and 4 (host→WASM via host_apply)
    rt.registerTriggerHandlers({
      onCapability: function (req) {
        // Forward old capability trigger to unified IPC
        if (req.capability && req.action) {
          invokeIpc(req.capability, req.action, req.payload || {});
        }
      },
      onIpc: function (req) {
        // Forward old IPC trigger to unified IPC
        if (req.ipc && req.action) {
          invokeIpc(req.ipc, req.action, req.payload || {});
        }
      },
    });
  }

  // ── Exports ────────────────────────────────────────────────────────────

  if (typeof globalThis !== 'undefined') {
    globalThis.invokeIpc = invokeIpc;
    globalThis.registerIpcTriggers = registerIpcTriggers;
  }

  return { invokeIpc, registerIpcTriggers };
})();

/* ═════════ foundation-wasm-ui.js ═════════ */
// foundation-wasm-ui.js — DOM layer of the runtime (built on foundation-wasm.js).
//
// WHY: foundation-wasm.js is the pure ABI (memory, transport, timers). The DOM half
// lives here so the ABI stays reusable by non-DOM hosts — mirroring the
// foundation_wasm / foundation_wasm_ui crate split.
//
// WHAT: the first DOM increment —
//   - ColumnarParser        : decodes the Arrow columnar payload (the exact layout
//                          `foundation_ui_traits::ColumnarEncoder` produces)
//   - NodeRegistry       : primal-id (u32) → DOM node
//   - DomOpApplicator : applies a parsed batch to a DOM `document`
//   - columnarHandler()     : a ProtocolDispatcher handler that ties parse → apply
//
// HOW: register `columnarHandler(...)` on a FoundationWasm dispatcher for protocol byte
// 1; when WASM ships an Arrow batch via host_apply, it's parsed and applied here.
//
// The op codes match decision 010 (shared with the Rust encoders).

// ─── Operation codes (decision 010) ─────────────────────────────────────────────

const Op = Object.freeze({
  CREATE_ELEMENT: 0,
  CREATE_TEXT_NODE: 1,
  SET_TEXT_CONTENT: 2,
  SET_ATTRIBUTE: 3,
  REMOVE_ATTRIBUTE: 4,
  SET_PROPERTY: 5,
  ADD_EVENT_LISTENER: 6,
  REMOVE_EVENT_LISTENER: 7,
  APPEND_CHILD: 8,
  REMOVE_CHILD: 9,
  REMOVE_NODE: 10,
  INSERT_BEFORE: 11,
  REPLACE_NODE: 12,
  SET_STYLE: 13,
  ADD_CLASS: 14,
  REMOVE_CLASS: 15,
  MORPH_NODE: 16,
  REGISTER_NODE: 17,
  UNREGISTER_NODE: 18,
});

// Reserved node ids (gap inventory G1): 0=<head>, 1=<body>, 2=<html>. Macro ids ≥ 1000.
const RESERVED = Object.freeze({ HEAD: 0, BODY: 1, HTML: 2 });

// ─── Known tag/attribute id tables (decision G12/G13/G14) ───────────────────
//
// EXACT mirrors of `foundation_ui_traits::html::TAG_NAMES` / `ATTR_NAMES`:
// index + 1 == wire id, `"id:<n>"` strings in the columns resolve here.
// Append-only — ORDER IS ABI. Regenerate from html.rs when extending.

const TAG_NAMES = Object.freeze([
  "div", "span", "input", "button", "html", "head", "body", "title", "base",
  "link", "meta", "style", "script", "noscript", "template", "slot", "main", "section",
  "nav", "article", "aside", "header", "footer", "address", "h1", "h2", "h3",
  "h4", "h5", "h6", "hgroup", "p", "hr", "pre", "blockquote", "ol",
  "ul", "menu", "li", "dl", "dt", "dd", "figure", "figcaption", "search",
  "a", "em", "strong", "small", "s", "cite", "q", "dfn", "abbr",
  "ruby", "rt", "rp", "data", "time", "code", "var", "samp", "kbd",
  "sub", "sup", "i", "b", "u", "mark", "bdi", "bdo", "br",
  "wbr", "ins", "del", "picture", "source", "img", "iframe", "embed", "object",
  "video", "audio", "track", "map", "area", "svg", "math", "canvas", "table",
  "caption", "colgroup", "col", "tbody", "thead", "tfoot", "tr", "td", "th",
  "form", "label", "select", "datalist", "optgroup", "option", "textarea", "output", "progress",
  "meter", "fieldset", "legend", "details", "summary", "dialog",
]);

const ATTR_NAMES = Object.freeze([
  "class", "id", "style", "value", "title", "lang", "dir", "hidden", "tabindex",
  "accesskey", "draggable", "contenteditable", "spellcheck", "translate", "role", "slot", "part", "is",
  "href", "src", "srcset", "sizes", "alt", "rel", "target", "download", "referrerpolicy",
  "crossorigin", "integrity", "loading", "media", "type", "name", "placeholder", "disabled", "readonly",
  "required", "checked", "selected", "multiple", "min", "max", "step", "pattern", "minlength",
  "maxlength", "autocomplete", "autofocus", "for", "form", "action", "method", "enctype", "novalidate",
  "accept", "rows", "cols", "wrap", "list", "size", "colspan", "rowspan", "headers",
  "scope", "width", "height", "controls", "autoplay", "loop", "muted", "preload", "poster",
  "playsinline", "charset", "content", "http-equiv", "open", "label", "datetime", "cite", "data",
]);

/**
 * Resolve a decision-010 string-column name: `"id:<n>"` looks up `table`
 * (1-based), anything else is already the literal name.
 * @param {string} wire @param {readonly string[]} table
 */
function resolveWireName(wire, table) {
  if (wire.startsWith("id:")) {
    const id = Number(wire.slice(3));
    if (Number.isInteger(id) && id >= 1 && id <= table.length) return table[id - 1];
  }
  return wire;
}


// ─── ColumnarParser (compact columnar payload — protocol 1, wire v1) ─────────────

/**
 * Decode the COMPACT COLUMNAR payload v1.1 (protocol byte 1, wire VERSION 1 —
 * the owned layout; wire version 2 is real Arrow IPC, server-side). Layout
 * (little-endian), mirroring `foundation_ui_traits::ColumnarBatch::serialize`:
 *
 *   [pad_len:u8][0x00 × pad_len]      // alignment shim
 *   [row_count:u32][flags:u32]        // 8-byte header — 8-ALIGNED by contract
 *   [op_id:    u32 × N]
 *   [node_id:  u32 × N]
 *   [operation:u8  × N][pad to 4]
 *   [attribute column][value column][text_val column]
 *
 * A column is `[(N+1) offsets:u32][data_len:u32][utf8 bytes][pad to 4]`.
 *
 * ZERO-COPY (feature 19): every producer pads so the header lands 8-aligned —
 * the pure encoder relative to the payload, the wasm framing layer against the
 * ABSOLUTE arena address. When that holds here (`byteOffset` math), the u32
 * columns become TRUE TypedArray views sharing the payload's buffer
 * (`zeroCopy: true`); otherwise we fall back to copying (correct everywhere).
 */
class ColumnarParser {
  /**
   * @param {Uint8Array} payload
   * @returns {{ count:number, opIds:Uint32Array, nodeIds:Uint32Array,
   *            operations:Uint8Array, attribute:string[], value:string[],
   *            textVal:string[], zeroCopy:boolean }}
   */
  static parse(payload) {
    const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
    const padLen = view.getUint8(0);
    const headerAt = 1 + padLen;
    // Column alignment is relative to the header; the producers' shim makes it
    // absolute. Verify — views need absolute 4-alignment.
    const aligned = (payload.byteOffset + headerAt) % 4 === 0;

    let off = headerAt;
    const u32 = () => {
      const v = view.getUint32(off, true);
      off += 4;
      return v;
    };
    const pad4 = () => {
      off += (4 - ((off - headerAt) % 4)) % 4;
    };

    const count = u32();
    u32(); // flags (reserved — future cached-string demux)

    const u32Column = () => {
      let column;
      if (aligned) {
        // TRUE view — shares the payload's buffer, zero copies.
        column = new Uint32Array(payload.buffer, payload.byteOffset + off, count);
      } else {
        column = new Uint32Array(count);
        for (let i = 0; i < count; i++) column[i] = view.getUint32(off + i * 4, true);
      }
      off += count * 4;
      return column;
    };

    const opIds = u32Column();
    const nodeIds = u32Column();
    // u8 views have no alignment requirement — always zero-copy.
    const operations = new Uint8Array(payload.buffer, payload.byteOffset + off, count);
    off += count;
    pad4();

    const decoder = new TextDecoder();
    const readStringColumn = () => {
      const offsets = u32Column.call(null);
      // (count+1) offsets — u32Column read `count`; read the extra one.
      const last = u32();
      const dataLen = u32();
      const base = payload.byteOffset + off;
      const out = new Array(count);
      for (let i = 0; i < count; i++) {
        const end = i + 1 < count ? offsets[i + 1] : last;
        out[i] = decoder.decode(
          new Uint8Array(payload.buffer, base + offsets[i], end - offsets[i]),
        );
      }
      off += dataLen;
      pad4();
      return out;
    };

    const attribute = readStringColumn();
    const value = readStringColumn();
    const textVal = readStringColumn();

    return { count, opIds, nodeIds, operations, attribute, value, textVal, zeroCopy: aligned };
  }
}

// ─── NodeRegistry ────────────────────────────────────────────────────────────

/**
 * Maps primal-ids (u32) to DOM nodes. CreateElement/Text register; RemoveNode and
 * ReplaceNode unregister. Reserved ids 0-2 map to head/body/html when seeded.
 */
class NodeRegistry {
  constructor() {
    this.nodes = new Map();
    // Created-but-not-yet-registered nodes (CreateElement/CreateTextNode do NOT
    // auto-register — feature 01): REGISTER_NODE promotes them into `nodes`.
    this.pending = new Map();
  }

  /** Seed the reserved ambient nodes from a document (browser) — optional in tests. */
  seedDocument(doc) {
    if (doc.head) this.nodes.set(RESERVED.HEAD, doc.head);
    if (doc.body) this.nodes.set(RESERVED.BODY, doc.body);
    if (doc.documentElement) this.nodes.set(RESERVED.HTML, doc.documentElement);
    return this;
  }

  register(id, node) {
    this.nodes.set(id, node);
    return node;
  }

  get(id) {
    return this.nodes.get(id);
  }

  /** Look up `id`, throwing a clear error if it isn't registered. */
  expect(id) {
    const node = this.nodes.get(id);
    if (node === undefined) throw new Error(`NodeRegistry: unknown node id ${id}`);
    return node;
  }

  unregister(id) {
    this.nodes.delete(id);
  }

  /** Park a freshly created node until an explicit REGISTER_NODE promotes it. */
  stage(id, node) {
    this.pending.set(id, node);
    return node;
  }

  /**
   * REGISTER_NODE semantics: promote a staged node, keep an existing
   * registration (no-op), or find a pre-existing element in the document via
   * `[primal-id="<id>"]`. Throws if the id is nowhere to be found.
   */
  applyRegister(id, document) {
    if (this.nodes.has(id)) return this.nodes.get(id); // re-register = no-op
    const staged = this.pending.get(id);
    if (staged !== undefined) {
      this.pending.delete(id);
      return this.register(id, staged);
    }
    const found = document.querySelector?.(`[primal-id="${id}"]`);
    if (found) return this.register(id, found);
    throw new Error(`NodeRegistry: REGISTER_NODE ${id} matches no staged node or [primal-id]`);
  }

  /**
   * Resolve an id that is allowed to still be staged — REPLACE_NODE's `new_id`
   * (its implicit registration) is the one consumer. Promotes staged nodes.
   */
  expectOrStaged(id) {
    const node = this.nodes.get(id);
    if (node !== undefined) return node;
    const staged = this.pending.get(id);
    if (staged !== undefined) {
      this.pending.delete(id);
      return staged;
    }
    throw new Error(`NodeRegistry: unknown node id ${id}`);
  }

  get size() {
    return this.nodes.size;
  }
}

// ─── MorphDom (feature 07 — decision 027, Datastar-style morphing) ───────────

/**
 * WHY: Server/WASM HTML patches (MORPH_NODE, op 16) must update a live subtree
 * WITHOUT destroying user state — focus, form values, CSS animations, element
 * identity. Hard replacement (op 12) loses all of it; morphing reconciles.
 *
 * WHAT: A morphdom/idiomorph-style reconciler: persistent-ID tracking with
 * tag-mismatch and duplicate exclusion, bottom-up ID maps, best-match scanning
 * with pantry retrieval and equality-lookahead anti-churn, a pantry for parked
 * nodes (retrievable within the same morph), form-state preservation, script
 * re-execution with a WeakSet guard, and the `data-ignore-morph` /
 * `data-preserve-attr` escape hatches.
 *
 * HOW: All per-morph state lives on a fresh MorphContext (G31 — re-entrant
 * morphs can't corrupt each other); `cleanup()` runs in `finally` (G33 — the
 * pantry never leaks). Node access is duck-typed (tagName/tag,
 * children/childNodes, getAttribute…) so the algorithm runs identically on
 * the real DOM and the test mock.
 */

const morphIsElement = (n) => {
  if (!n) return false;
  if (n.nodeType !== undefined) return n.nodeType === 1; // real DOM
  // Mocks: text nodes carry tag "#text" but still expose attribute methods.
  return typeof n.getAttributeNames === "function" && (n.tag ?? "") !== "#text";
};
const morphTag = (n) => (n.tagName ?? n.tag ?? "").toUpperCase();
const morphKids = (n) => Array.from(n.childNodes ?? n.children ?? []);
const morphText = (n) => (n.nodeValue !== undefined && n.nodeValue !== null ? n.nodeValue : n.textContent);
const morphSetText = (n, v) => {
  if (n.nodeValue !== undefined && n.nodeValue !== null) n.nodeValue = v;
  else n.textContent = v;
};
const morphAttr = (n, name) => (n.getAttribute ? n.getAttribute(name) : null);
// Morph identity: `id` wins, else `primal-id` (spec-42 feature 00 §4 — slot
// spans and template nodes carry primal-id; server re-renders of the same
// template have STABLE primal-ids, so they anchor preservation exactly like
// user ids do).
const morphIdentity = (n) => morphAttr(n, "id") ?? morphAttr(n, "primal-id");

/** Walk every element in a subtree (root included), depth-first. */
function morphWalk(root, fn) {
  if (morphIsElement(root)) fn(root);
  for (const child of morphKids(root)) morphWalk(child, fn);
}

/**
 * `moveBefore` keeps focus/animations/lifecycle when the platform has it
 * (G32 — Chromium 115+, Firefox 125+, Safari TP 185); the fallback is
 * structurally correct but loses that state.
 */
function moveBefore(parent, node, ref) {
  if (typeof parent.moveBefore === "function") parent.moveBefore(node, ref);
  else {
    node.parentNode?.removeChild?.(node);
    parent.insertBefore(node, ref);
  }
}

class MorphContext {
  constructor(doc) {
    this.doc = doc;
    this.idMap = new Map(); // Node -> Set<string> (persistent ids in subtree)
    this.persistentIds = new Set();
    this.oldIdTagMap = new Map(); // id -> tagName (old tree)
    this.duplicates = new Set();
    this.pantry = null; // created lazily on first park
  }

  // Phase 1 — ids that exist in BOTH trees with the SAME tag, no duplicates.
  computePersistentIds(oldRoot, newRoot) {
    morphWalk(oldRoot, (el) => {
      const id = morphIdentity(el);
      if (!id) return;
      if (this.oldIdTagMap.has(id)) this.duplicates.add(id);
      else this.oldIdTagMap.set(id, morphTag(el));
    });
    morphWalk(newRoot, (el) => {
      const id = morphIdentity(el);
      if (!id || this.duplicates.has(id)) return;
      if (this.oldIdTagMap.get(id) === morphTag(el)) this.persistentIds.add(id);
    });
  }

  // Phase 2 — bottom-up: each node -> the persistent ids inside its subtree.
  populateIdMap(root) {
    const build = (node) => {
      const ids = new Set();
      const own = morphIsElement(node) ? morphIdentity(node) : null;
      if (own && this.persistentIds.has(own)) ids.add(own);
      for (const child of morphKids(node)) {
        for (const id of build(child)) ids.add(id);
      }
      if (ids.size > 0) this.idMap.set(node, ids);
      return ids;
    };
    build(root);
  }

  hasConflictingId(node) {
    const id = morphIsElement(node) ? morphIdentity(node) : null;
    return !!id && !this.persistentIds.has(id);
  }

  // §3 — priority 1: ID-set intersection. The scan is UNBOUNDED over the
  // remaining siblings AND the pantry: an id match is an anchor, and anchors
  // are always worth moving for (the spec's own reorder test demands it; its
  // displacement-limit example is the newIds-EMPTY case, which never enters
  // this scan at all). Priority 2: soft match guarded by anti-churn.
  findBestMatch(oldCursor, newChild) {
    if (!morphIsElement(newChild)) {
      // Text/comment: soft-match a same-kind node at the cursor.
      return oldCursor && !morphIsElement(oldCursor) ? oldCursor : null;
    }
    const newIds = this.idMap.get(newChild) ?? new Set();

    if (newIds.size > 0) {
      const intersects = (candidate) => {
        if (
          !morphIsElement(candidate) ||
          morphTag(candidate) !== morphTag(newChild) ||
          !this.idMap.has(candidate)
        ) {
          return false;
        }
        const ids = this.idMap.get(candidate);
        for (const id of newIds) {
          if (ids.has(id)) return true;
        }
        return false;
      };
      for (let candidate = oldCursor; candidate; candidate = candidate.nextSibling) {
        if (intersects(candidate)) return candidate;
      }
      // Parked earlier in THIS morph — retrievable (§4).
      if (this.pantry) {
        for (const candidate of morphKids(this.pantry)) {
          if (intersects(candidate)) return candidate;
        }
      }
    }

    if (
      oldCursor &&
      morphIsElement(oldCursor) === morphIsElement(newChild) &&
      morphTag(oldCursor) === morphTag(newChild) &&
      !this.hasConflictingId(oldCursor)
    ) {
      // Anti-churn: the spec's future-sibling counter blocks EVERY element of
      // a homogeneous list (its own narrative contradicts it). The rule that
      // satisfies both spec examples is an equality lookahead: if the NEXT new
      // sibling is structurally equal to the cursor, `newChild` is an
      // INSERTION before it — create fresh instead of morphing the cursor
      // into its successor (prepend churn) or dragging anchors (displacement).
      if (typeof oldCursor.isEqualNode === "function") {
        if (oldCursor.isEqualNode(newChild)) return oldCursor;
        const nextNew = newChild.nextSibling ?? null;
        if (nextNew && oldCursor.isEqualNode(nextNew)) return null;
      }
      return oldCursor;
    }
    return null;
  }

  // §4 — park id-bearing nodes (retrievable this morph), drop the rest.
  removeNode(node) {
    if (this.idMap.has(node)) {
      if (!this.pantry) this.pantry = this.doc.createElement("div");
      moveBefore(this.pantry, node, null);
    } else {
      node.parentNode?.removeChild?.(node);
    }
  }

  morphChildren(oldParent, newParent) {
    let oldCursor = oldParent.firstChild ?? null;
    for (const newChild of morphKids(newParent)) {
      // Escape hatch: BOTH sides carry data-ignore-morph -> leave untouched.
      if (
        oldCursor &&
        morphAttr(oldCursor, "data-ignore-morph") !== null &&
        morphAttr(newChild, "data-ignore-morph") !== null
      ) {
        oldCursor = oldCursor.nextSibling;
        continue;
      }

      const match = this.findBestMatch(oldCursor, newChild);
      if (match) {
        // Park/remove everything between the cursor and the match.
        while (oldCursor && oldCursor !== match) {
          const next = oldCursor.nextSibling;
          this.removeNode(oldCursor);
          oldCursor = next;
        }
        if (match !== oldCursor) moveBefore(oldParent, match, oldCursor);
        this.morphNode(match, newChild);
        oldCursor = match.nextSibling;
      } else {
        const clone = cloneNode(this.doc, newChild);
        oldParent.insertBefore(clone, oldCursor);
      }
    }
    while (oldCursor) {
      const next = oldCursor.nextSibling;
      this.removeNode(oldCursor);
      oldCursor = next;
    }
  }

  morphNode(oldNode, newNode) {
    if (!morphIsElement(oldNode) || !morphIsElement(newNode)) {
      if (morphText(oldNode) !== morphText(newNode)) {
        morphSetText(oldNode, morphText(newNode));
      }
      return;
    }
    if (typeof oldNode.isEqualNode === "function" && oldNode.isEqualNode(newNode)) {
      return; // identical subtree — skip entirely
    }
    this.syncAttributes(oldNode, newNode);
    preserveFormState(oldNode, newNode);
    const tag = morphTag(oldNode);
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return; // leaves
    this.morphChildren(oldNode, newNode);
  }

  syncAttributes(oldEl, newEl) {
    const preserved = (morphAttr(oldEl, "data-preserve-attr") || "")
      .split(",")
      .map((sliver) => sliver.trim())
      .filter(Boolean);
    for (const name of newEl.getAttributeNames()) {
      if (!preserved.includes(name)) oldEl.setAttribute(name, newEl.getAttribute(name));
    }
    for (const name of oldEl.getAttributeNames()) {
      if (!preserved.includes(name) && newEl.getAttribute(name) === null) {
        oldEl.removeAttribute(name);
      }
    }
  }

  cleanup() {
    if (this.pantry) {
      while (this.pantry.firstChild) this.pantry.removeChild(this.pantry.firstChild);
    }
    this.idMap.clear();
    this.persistentIds.clear();
    this.oldIdTagMap.clear();
    this.duplicates.clear();
  }
}

/** §5 — keep what the USER did to form controls across the morph. */
function preserveFormState(oldEl, newEl) {
  if (morphTag(oldEl) !== morphTag(newEl)) return;
  switch (morphTag(oldEl)) {
    case "INPUT": {
      const type = morphAttr(oldEl, "type");
      if (type === "checkbox" || type === "radio") newEl.checked = oldEl.checked;
      else if (type !== "file") newEl.value = oldEl.value;
      break;
    }
    case "TEXTAREA":
      newEl.value = oldEl.value;
      break;
    case "SELECT":
      newEl.selectedIndex = oldEl.selectedIndex;
      break;
    default:
  }
}

/** Deep clone for newly created content (mock-aware). */
function cloneNode(doc, node) {
  if (typeof node.cloneNode === "function") return node.cloneNode(true);
  if (!morphIsElement(node)) return doc.createTextNode(morphText(node) ?? "");
  const el = doc.createElement(node.tag ?? node.tagName);
  for (const name of node.getAttributeNames()) el.setAttribute(name, node.getAttribute(name));
  if (node.value != null) el.value = node.value;
  if (node.checked != null) el.checked = node.checked;
  for (const child of morphKids(node)) el.appendChild(cloneNode(doc, child));
  return el;
}

class MorphDom {
  /** Executed scripts — never re-run across morphs (browser concern). */
  static scripts = new WeakSet();

  /**
   * Morph `target`'s children to mirror `newContent`'s children.
   * @param {Element} target  live element
   * @param {Element|DocumentFragment} newContent  desired tree (its CHILDREN)
   * @param {Document} [doc]  owning document (defaults to target's)
   */
  static morph(
    target,
    newContent,
    doc = target.ownerDocument ?? (typeof document === "undefined" ? null : document),
  ) {
    const ctx = new MorphContext(doc);
    try {
      ctx.computePersistentIds(target, newContent);
      ctx.populateIdMap(target);
      ctx.populateIdMap(newContent);
      ctx.morphChildren(target, newContent);
      MorphDom.executeNewScripts(target, doc);
    } finally {
      ctx.cleanup(); // G33 — pantry never leaks, even on throw
    }
  }

  /** §7 — re-create injected <script>s so the browser executes them, once. */
  static executeNewScripts(root, doc) {
    if (!doc || typeof root.querySelectorAll !== "function") return; // browser-only
    for (const script of root.querySelectorAll("script")) {
      if (MorphDom.scripts.has(script)) continue;
      const clone = doc.createElement("script");
      for (const name of script.getAttributeNames()) {
        clone.setAttribute(name, script.getAttribute(name));
      }
      clone.textContent = script.textContent;
      script.parentNode?.replaceChild?.(clone, script);
      MorphDom.scripts.add(clone);
    }
  }
}

// ─── DomOpApplicator ──────────────────────────────────────────────────────

/**
 * Applies a parsed Arrow batch to a DOM. `document` must provide `createElement`,
 * `createTextNode`; nodes must provide the usual mutation API. Designed so a minimal
 * stub (tests) or a real `document` both work.
 */
class DomOpApplicator {
  /**
   * @param {NodeRegistry} registry @param {Document} document
   * @param {(eventName:string, nodeId:number, event:Event, el:Element)=>void} [onEvent]
   *   dispatch hook for ADD_EVENT_LISTENER-bound listeners (feature 08 wires WASM).
   */
  constructor(registry, document, onEvent) {
    this.registry = registry;
    this.document = document;
    this.onEvent = onEvent;
    this.listeners = new Map(); // `${nodeId}:${event}` -> bound handler
  }

  /** Apply a full batch (output of {@link ColumnarParser.parse}). */
  apply(batch) {
    const { count, nodeIds, operations, attribute, value, textVal } = batch;
    for (let i = 0; i < count; i++) {
      this.applyOne(operations[i], nodeIds[i], attribute[i], value[i], textVal[i]);
    }
  }

  applyOne(op, nodeId, attribute, value, textVal) {
    const reg = this.registry;
    switch (op) {
      case Op.CREATE_ELEMENT: {
        // attribute = tag (id: form or literal), value = class.
        const el = this.document.createElement(resolveWireName(attribute, TAG_NAMES));
        if (value) el.className = value;
        reg.stage(nodeId, el); // NO auto-register — REGISTER_NODE promotes.
        break;
      }
      case Op.CREATE_TEXT_NODE:
        reg.stage(nodeId, this.document.createTextNode(textVal));
        break;
      case Op.SET_TEXT_CONTENT:
        reg.expect(nodeId).textContent = textVal;
        break;
      case Op.SET_ATTRIBUTE:
        reg.expect(nodeId).setAttribute(resolveWireName(attribute, ATTR_NAMES), value);
        break;
      case Op.REMOVE_ATTRIBUTE:
        reg.expect(nodeId).removeAttribute(resolveWireName(attribute, ATTR_NAMES));
        break;
      case Op.SET_PROPERTY: {
        // value is the SERIALIZED property value; JSON covers the primitive
        // cases (numbers, booleans, quoted strings) with raw-string fallback.
        let parsed = value;
        try {
          parsed = JSON.parse(value);
        } catch {
          /* raw string property */
        }
        reg.expect(nodeId)[resolveWireName(attribute, ATTR_NAMES)] = parsed;
        break;
      }
      case Op.ADD_EVENT_LISTENER: {
        // value = event name. The bound listener forwards into the runtime's
        // event dispatch hook (feature 08 wires the WASM callback path).
        const eventName = resolveWireName(value, ATTR_NAMES);
        const el = reg.expect(nodeId);
        const key = `${nodeId}:${eventName}`;
        if (!this.listeners.has(key)) {
          const handler = (event) => this.onEvent?.(eventName, nodeId, event, el);
          this.listeners.set(key, handler);
          el.addEventListener(eventName, handler);
        }
        break;
      }
      case Op.REMOVE_EVENT_LISTENER: {
        const eventName = resolveWireName(value, ATTR_NAMES);
        const el = reg.expect(nodeId);
        const key = `${nodeId}:${eventName}`;
        const handler = this.listeners.get(key);
        if (handler) {
          el.removeEventListener(eventName, handler);
          this.listeners.delete(key);
        }
        break;
      }
      case Op.APPEND_CHILD:
        reg.expect(nodeId).appendChild(reg.expect(Number(attribute))); // attribute = child id
        break;
      case Op.REMOVE_CHILD:
        reg.expect(nodeId).removeChild(reg.expect(Number(attribute)));
        break;
      case Op.INSERT_BEFORE:
        reg
          .expect(nodeId)
          .insertBefore(reg.expect(Number(attribute)), reg.expect(Number(textVal))); // text_val = ref id
        break;
      case Op.REMOVE_NODE: {
        const node = reg.expect(nodeId);
        node.remove();
        reg.unregister(nodeId); // implicit unregister
        break;
      }
      case Op.REPLACE_NODE: {
        // attribute = new id. Implicitly unregisters old, registers new — the
        // replacement may still be staged (no REGISTER_NODE needed for it).
        const newId = Number(attribute);
        const oldNode = reg.expect(nodeId);
        const newNode = reg.expectOrStaged(newId);
        oldNode.replaceWith(newNode);
        reg.unregister(nodeId);
        reg.register(newId, newNode);
        break;
      }
      case Op.SET_STYLE:
        reg.expect(nodeId).style[resolveWireName(attribute, ATTR_NAMES)] = value;
        break;
      case Op.ADD_CLASS:
        reg.expect(nodeId).classList.add(value); // value = class
        break;
      case Op.REMOVE_CLASS:
        reg.expect(nodeId).classList.remove(value);
        break;
      case Op.MORPH_NODE:
        this.applyMorph(nodeId, attribute, textVal);
        break;
      case Op.REGISTER_NODE:
        reg.applyRegister(nodeId, this.document);
        break;
      case Op.UNREGISTER_NODE:
        reg.unregister(nodeId); // registry only — DOM untouched
        break;
      default:
        throw new Error(`DomOpApplicator: unknown operation ${op}`);
    }
  }

  /**
   * MORPH_NODE (op 16): `packed` is `"<action>:<kind>:<selector>"` (split only
   * the first two colons — CSS queries contain `:`); `nodeId` carries the
   * target for kind 0. Minimal application — full Datastar-style morphing with
   * state preservation is feature 07 (decision 027).
   */
  applyMorph(nodeId, packed, content) {
    const first = packed.indexOf(":");
    const second = packed.indexOf(":", first + 1);
    if (first < 0 || second < 0) {
      throw new Error(`DomOpApplicator: malformed morph packing \`${packed}\``);
    }
    const action = Number(packed.slice(0, first));
    const kind = packed.slice(first + 1, second);
    const selector = packed.slice(second + 1);

    let target;
    if (kind === "0") target = this.registry.expect(nodeId);
    else if (kind === "1") target = this.document.querySelector(`#${selector}`);
    else if (kind === "2") target = this.document.querySelector(`.${selector}`);
    else if (kind === "3") target = this.document.querySelector(selector);
    else throw new Error(`DomOpApplicator: unknown morph selector kind ${kind}`);
    if (!target) throw new Error(`DomOpApplicator: morph target not found (${kind}:${selector})`);

    switch (action) {
      case 0: // ReplaceChildren — full morph (decision 027) when the document
        // can parse HTML; innerHTML fallback otherwise (test mocks).
        if (typeof this.document.createRange === "function") {
          const fragment = this.document.createRange().createContextualFragment(content);
          MorphDom.morph(target, fragment, this.document);
        } else {
          target.innerHTML = content;
        }
        break;
      case 1: // ReplaceElement
        target.insertAdjacentHTML("afterend", content);
        target.remove();
        break;
      case 2: // InsertBefore
        target.insertAdjacentHTML("beforebegin", content);
        break;
      case 3: // InsertAfter
        target.insertAdjacentHTML("afterend", content);
        break;
      case 4: // AppendSibling — last child of the target's parent
        (target.parent ?? target.parentNode)?.insertAdjacentHTML("beforeend", content);
        break;
      default:
        throw new Error(`DomOpApplicator: unknown morph action ${action}`);
    }
  }
}

// ─── Protocol handler factory ────────────────────────────────────────────────

/**
 * Build a ProtocolDispatcher handler (`{ apply(memoryId, payload) }`) for
 * protocol byte 1 wire v1 (compact columnar): parse + apply to the DOM.
 * @param {DomOpApplicator} applicator
 */
function columnarHandler(applicator) {
  return {
    apply(_memoryId, payload) {
      applicator.apply(ColumnarParser.parse(payload));
    },
  };
}

// ─── EventDispatcher (primal:on* → WASM callbacks) ───────────────────────────────

const PRIMAL_ON = "primal:on";

/**
 * Build the EventData object handed to a WASM callback (decision 018 / G4):
 * type, the element's primal-id, current value/checked, key code, and modifier keys.
 */
function buildEventData(eventType, event, el) {
  return {
    type: eventType,
    primalId: el.getAttribute ? el.getAttribute("primal-id") : null,
    value: el.value ?? null,
    checked: el.checked ?? null,
    keyCode: event.keyCode ?? null,
    modifiers: {
      alt: !!event.altKey,
      ctrl: !!event.ctrlKey,
      shift: !!event.shiftKey,
      meta: !!event.metaKey,
    },
  };
}

/**
 * Parse a `primal:on*` attribute value into a WASM callback id.
 * Accepts `"7"` or `"callback-7"`. Returns `null` for non-callback refs (e.g. a
 * stimulus-style `"controller.delete"`), which a later increment will resolve as a
 * JS function ref.
 */
function parseCallbackId(ref) {
  if (ref == null) return null;
  const s = String(ref).trim();
  const body = s.startsWith("callback-") ? s.slice("callback-".length) : s;
  return /^\d+$/.test(body) ? Number(body) : null;
}

/**
 * WHY: DOM events have to reach BOTH worlds: WASM (signal setters via
 * `primal:setter`, registry callbacks via `callback-N`) and plain JS handlers
 * (dot-path refs like `"controller.delete"`). Decision 018: direct binding is
 * the default (works for non-bubbling events); delegation is opt-in per
 * attribute.
 *
 * WHAT: The feature-08 event runtime — scan/wire/unwire with idempotent
 * rewiring (G2), dot-path resolution, opt-in delegation
 * (`primal:onclick:delegate="#container"`), MutationObserver auto-wiring with
 * the island boundary rule, microtask-batched removal cleanup, and the
 * programmatic `on`/`off`/`on<event>` API.
 *
 * HOW: `deliver(callbackId, eventData)` ships registry-callback events;
 * `deliverSignal(setterId, eventData)` ships signal-setter events (two id
 * NAMESPACES — see {@link callbackDeliver} / {@link signalDeliver}).
 * Handler resolution order per element/event:
 *   1. `handlerRef` is `"N"`/`"callback-N"`  → registry callback bridge
 *   2. element carries `primal:setter="N"`   → signal bridge (two-way binding)
 *   3. `handlerRef` is a dot-path             → JS function from `scope`
 *   4. otherwise                              → console.warn, no listener
 */
class EventDispatcher {
  /**
   * @param {(callbackId:number, eventData:object) => void} deliver
   * @param {{ deliverSignal?:(setterId:number, eventData:object)=>void,
   *           scope?:object }} [options]
   */
  constructor(deliver, options = {}) {
    this.deliver = deliver;
    this.deliverSignal = options.deliverSignal ?? deliver;
    this.scope = options.scope ?? globalThis;
    // element -> Map<key, listenerFn>; keys are "click" (direct) or
    // "click:delegate:<primal-id>" (delegated, stored on the TARGET element).
    this.listeners = new WeakMap();
    // trackRemoved microtask batching (feature 08 §9).
    this.cleanupQueue = [];
    this.cleanupScheduled = false;
    this.observer = null;
  }

  /** Wire every `primal:on*` attribute on `root` and its descendants. */
  scanAndWire(root) {
    visit(root, (el) => {
      for (const name of el.getAttributeNames()) {
        if (!name.startsWith(PRIMAL_ON)) continue;
        const rest = name.slice(PRIMAL_ON.length); // "click" | "click:delegate"
        const [eventType, mode] = rest.split(":");
        if (!eventType) continue;
        if (mode === "delegate") {
          this.wireDelegated(el, eventType, el.getAttribute(name));
        } else if (mode === undefined) {
          this.wire(el, eventType, el.getAttribute(name));
        }
      }
    });
  }

  /**
   * Wire one direct `eventType` on `el` to `handlerRef` (resolution order in
   * the class docs). Idempotent (G2): the prior listener for that event is
   * removed first, so re-scans never stack duplicates.
   */
  wire(el, eventType, handlerRef) {
    const listener = this.#buildListener(el, eventType, handlerRef);
    if (!listener) {
      console.warn(`EventDispatcher: unresolvable handler "${handlerRef}" for ${eventType}`);
      return;
    }
    this.off(el, eventType);
    el.addEventListener(eventType, listener);
    this.#listenerMap(el).set(eventType, listener);
  }

  #buildListener(el, eventType, handlerRef) {
    const callbackId = parseCallbackId(handlerRef);
    if (callbackId !== null) {
      return (event) => this.deliver(callbackId, buildEventData(eventType, event, el));
    }
    const setterId = parseCallbackId(el.getAttribute?.("primal:setter"));
    if (setterId !== null) {
      return (event) => this.deliverSignal(setterId, buildEventData(eventType, event, el));
    }
    const fn = resolveFunctionRef(this.scope, handlerRef);
    if (fn) return fn.bind(el); // clean `this` = the attributed element
    return null;
  }

  /**
   * Opt-in delegation (feature 08 §6): attach a listener on the element named
   * by `selector` that stamps `event.delegateTarget = el` whenever the event
   * originated inside `el`. Keys include `el`'s primal-id so many elements can
   * delegate the same event type to one container without colliding.
   */
  wireDelegated(el, eventType, selector) {
    const target = resolveDelegateTarget(el, selector, this.documentOf(el));
    if (!target) {
      console.warn(`EventDispatcher: delegate target "${selector}" not found`);
      return;
    }
    const listener = (event) => {
      if (el === event.target || (el.contains && el.contains(event.target))) {
        event.delegateTarget = el;
      }
    };
    const key = `${eventType}:delegate:${el.getAttribute?.("primal-id") || el.id || ""}`;
    const map = this.#listenerMap(target);
    const prior = map.get(key);
    if (prior) target.removeEventListener(eventType, prior);
    map.set(key, listener);
    target.addEventListener(eventType, listener);
  }

  /** The document an element belongs to (overridable for tests/mocks). */
  documentOf(el) {
    return el.ownerDocument ?? (typeof document === "undefined" ? null : document);
  }

  #listenerMap(el) {
    let map = this.listeners.get(el);
    if (!map) {
      map = new Map();
      this.listeners.set(el, map);
    }
    return map;
  }

  /**
   * Remove listeners on `el`: a specific event type (matching its direct key
   * AND any delegated compound keys), or — with no `eventType` — everything.
   */
  off(el, eventType) {
    const map = this.listeners.get(el);
    if (!map) return;
    if (eventType !== undefined) {
      for (const [key, listener] of map) {
        if (key === eventType || key.startsWith(`${eventType}:`)) {
          el.removeEventListener(eventType, listener);
          map.delete(key);
        }
      }
      return;
    }
    for (const [key, listener] of map) {
      el.removeEventListener(key.split(":")[0], listener);
    }
    map.clear();
    this.listeners.delete(el);
  }

  /** Remove all listeners on `el` and its descendants (node-removal cleanup). */
  removeListeners(root) {
    visit(root, (el) => this.off(el));
  }

  /**
   * Queue a removed node for cleanup; one microtask drains the whole batch
   * (feature 08 §9 — removing 50 nodes costs one pass, before paint).
   */
  trackRemoved(node) {
    this.cleanupQueue.push(node);
    if (this.cleanupScheduled) return;
    this.cleanupScheduled = true;
    queueMicrotask(() => {
      const batch = this.cleanupQueue.splice(0);
      this.cleanupScheduled = false;
      for (const item of batch) this.removeListeners(item);
    });
  }

  /**
   * Process MutationObserver-style records (feature 08 §7): wire added
   * subtrees, clean removed ones — SKIPPING anything inside an `<island>`
   * (the island custom element owns its own lifecycle, F06).
   */
  handleMutations(mutations) {
    for (const mutation of mutations) {
      if (mutation.type !== "childList") continue;
      for (const node of mutation.addedNodes) {
        if (!isElement(node) || insideIsland(node)) continue;
        this.scanAndWire(node);
        // Hydrate scoped styles/scripts on runtime-inserted subtrees (machinery
        // delivery, spec-42 feature 05 §M-delivery). Idempotent: hydrate runs
        // each scoped script once then removes it. Islands self-hydrate via
        // connectedCallback, so they're skipped above.
        Hydrator.hydrate(node);
      }
      for (const node of mutation.removedNodes) {
        if (!isElement(node) || insideIsland(node)) continue;
        this.removeListeners(node);
      }
    }
  }

  /** Start the document-level observer (browser only; no-op without one). */
  observe(doc) {
    if (typeof MutationObserver === "undefined" || this.observer) return;
    this.observer = new MutationObserver((mutations) => this.handleMutations(mutations));
    this.observer.observe(doc, { subtree: true, childList: true });
  }

  /**
   * Programmatic wiring (feature 08 §11): `on(el, "click", "ctrl.fn")` or
   * `on(el, "click", null, { delegate: "#box" })`.
   */
  on(el, eventType, handlerRef, options = {}) {
    if (options.delegate) this.wireDelegated(el, eventType, options.delegate);
    else this.wire(el, eventType, handlerRef);
  }
}

/** Events that get `dispatcher.on<event>(el, ref, opts)` convenience methods. */
const CONVENIENCE_EVENTS = [
  "click", "change", "submit", "keydown", "keyup",
  "focus", "blur", "scroll", "input", "mousedown", "mouseup",
];
for (const evt of CONVENIENCE_EVENTS) {
  EventDispatcher.prototype[`on${evt}`] = function (el, handlerRef, options) {
    this.on(el, evt, handlerRef, options);
  };
}

/** Depth-first walk over attribute-bearing nodes (root first). */
function visit(node, fn) {
  if (typeof node.getAttributeNames === "function") fn(node);
  for (const child of node.children || []) visit(child, fn);
}

function isElement(node) {
  return !!node && typeof node.getAttributeNames === "function";
}

/** The island boundary rule: nearest `<island>` ancestor-or-self opts out. */
function insideIsland(node) {
  if (typeof node.closest === "function") return node.closest("island") !== null;
  // Mock fallback: walk parents by tag.
  for (let cur = node; cur; cur = cur.parent ?? cur.parentElement ?? null) {
    if ((cur.tag ?? cur.tagName ?? "").toLowerCase() === "island") return true;
  }
  return false;
}

/**
 * Dot-path resolution against `scope` (feature 08 §4): `"controller.delete"`
 * walks `scope.controller.delete`; missing segments or non-functions → null.
 */
function resolveFunctionRef(scope, handlerRef) {
  if (typeof handlerRef !== "string" || handlerRef.length === 0) return null;
  let current = scope;
  for (const part of handlerRef.split(".")) {
    if (current == null) return null;
    current = current[part];
  }
  return typeof current === "function" ? current : null;
}

/** Delegate target resolution (feature 08 §5). */
function resolveDelegateTarget(el, selector, doc) {
  if (!selector) return null;
  if (selector === "parent") return el.parentElement ?? el.parent ?? null;
  if (selector === "body") return doc?.body ?? null;
  return doc?.querySelector ? doc.querySelector(selector) : null;
}

/**
 * Run the initial scan + observer once the document is ready (feature 08 §10).
 * Both paths are idempotent — rewiring replaces listeners (G2).
 */
function initEventRuntime(dispatcher, doc = typeof document === "undefined" ? null : document) {
  if (!doc) return;
  const boot = () => {
    dispatcher.scanAndWire(doc.body);
    dispatcher.observe(doc);
  };
  doc.addEventListener?.("DOMContentLoaded", boot);
  if (doc.readyState !== "loading") boot();
}

/**
 * Default `deliver` for REGISTRY callbacks (`callback-N` refs): serialise
 * EventData and ship via a `CallbackRegistry` (foundation-wasm.js), which
 * writes an arena slot and calls the `invoke_callback` export.
 *
 * @param {{invoke:(id:number, bytes:Uint8Array)=>void}} callbackRegistry
 * @param {(eventData:object)=>Uint8Array} [encode]
 */
function callbackDeliver(callbackRegistry, encode = jsonEncodeEventData) {
  return (callbackId, eventData) => callbackRegistry.invoke(callbackId, encode(eventData));
}

/**
 * `deliverSignal` for SIGNAL setters (`primal:setter` ids — the
 * foundation_signals registry, a separate namespace): write the JSON
 * EventData into a fresh global-arena slot and call the dedicated
 * `invoke_signal_callback(setterId, memoryId)` export. Rust reads, dispatches
 * to the setter, runs `stabilize()`, and frees the slot — no JS-side dispose.
 *
 * @param {{exports:object, memory:()=>WebAssembly.Memory}} bridge
 * @param {(eventData:object)=>Uint8Array} [encode]
 */
function signalDeliver(rt, encode = jsonEncodeEventData) {
  return (setterId, eventData) => {
    const bytes = encode(eventData);
    const memId = rt.memory.create(bytes.length);
    rt.memory.write(memId, bytes);
    rt.bridge.exports.invoke_signal_callback(BigInt(setterId), memId);
  };
}

function jsonEncodeEventData(eventData) {
  return new TextEncoder().encode(JSON.stringify(eventData));
}

// ─── DomOps over the batch protocol (Custom Binary, byte 0) ────────────────────────

/**
 * The registered batch opcode carrying one DomOp row:
 * `[BATCH_OP_APPLY_DOM][ArgStart (operation, nodeId, attribute, value, textVal)
 * ArgStop][Operations.End]` — params quantized (V2), strings via the texts pool.
 * Mirrors `foundation_wasm_ui::BATCH_OP_APPLY_DOM` on the Rust side.
 */
const BATCH_OP_APPLY_DOM = 10;

/**
 * Register the DomOp batch operation on a core runtime: byte-0 batch messages
 * carrying [`BATCH_OP_APPLY_DOM`] ops apply straight to the DOM through the same
 * applicator Arrow uses. This is the selective-opt-in pattern of the batch system
 * (`BatchInstructions.registerOperation`) — other processes register their own
 * opcodes the same way.
 *
 * @param {{batches:{registerOperation:Function, params:object}}} rt  core runtime
 * @param {DomOpApplicator} applicator
 */
function registerDomBatchOperation(rt, applicator) {
  rt.batches.registerOperation(BATCH_OP_APPLY_DOM, (batch, _opId, i, view, texts) => {
    // Markers are the shared cross-language contract: ArgumentOperations.Start = 1,
    // Operations.End = 254 (foundation_wasm base.rs).
    if (view.getUint8(i) !== 1) {
      throw new Error(`dom batch op: expected ArgumentOperations.Start, got ${view.getUint8(i)}`);
    }
    i += 1;
    let args;
    [i, args] = batch.params.parseParams(view, i, texts);
    if (view.getUint8(i) !== 254) {
      throw new Error(`dom batch op: expected Operations.End, got ${view.getUint8(i)}`);
    }
    i += 1;
    const [operation, nodeId, attribute, value, textVal] = args;
    const thunk = () => {
      applicator.applyOne(operation, nodeId, attribute, value, textVal);
      return null;
    };
    return [i, thunk];
  });
  return rt;
}

// ─── DomHeap (DOM external-pointer arena, = megatron DOMArena) ────────────────────

/**
 * Generation-arena heap for DOM nodes referenced across the ABI by `ExternalPointer`
 * ids (uid = `(index << 32) | generation`, bigint — same scheme as the core runtime's
 * ExternalHeap; implemented here so this file stays import-free for the browser).
 *
 * Slots 0–4 are RESERVED at construction (megatron DOMArena parity):
 * 0 = self (or this heap when no `self`), 1 = the heap itself, 2 = window,
 * 3 = document, 4 = document.body. Reserved slots refuse `destroy`.
 */
class DomHeap {
  static RESERVED_SLOTS = 5;

  /** @param {{window?:object, document?:object}} [host] overrides for tests/SSR */
  constructor(host = globalThis) {
    this.items = []; // { item, generation, active }
    this.free = [];
    const doc = host.document ?? null;
    this.create(typeof self !== "undefined" ? self : this);
    this.create(this);
    this.create(host.window ?? null);
    this.create(doc);
    this.create(doc && doc.body ? doc.body : null);
  }

  #unpack(uid) {
    const v = BigInt(uid);
    // The well-known DOM handles (Rust DOM_SELF..DOM_BODY) are the RAW values 0–4,
    // not packed uids. Reserved slots are never destroyed, so their generation stays
    // 0 and small raw values stay unambiguous (a packed index-n uid is n<<32).
    if (v < BigInt(DomHeap.RESERVED_SLOTS)) return { index: Number(v), generation: 0n };
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

  /** Resolve a uid → node (undefined when stale/missing). */
  get(uid) {
    const { index, generation } = this.#unpack(uid);
    const slot = this.items[index];
    if (!slot || !slot.active || slot.generation !== generation) return undefined;
    return slot.item;
  }

  /** Fill a pre-allocated uid. False when stale. */
  update(uid, item) {
    const { index, generation } = this.#unpack(uid);
    const slot = this.items[index];
    if (!slot || slot.generation !== generation) return false;
    slot.item = item;
    slot.active = true;
    return true;
  }

  /** Retire a uid (reserved slots 0–4 refuse, megatron parity). */
  destroy(uid) {
    const { index, generation } = this.#unpack(uid);
    if (index < DomHeap.RESERVED_SLOTS) return false;
    const slot = this.items[index];
    if (!slot || !slot.active || slot.generation !== generation) return false;
    slot.item = null;
    slot.active = false;
    this.free.push(index);
    return true;
  }
}

// ─── DOM ABI extension ─────────────────────────────────────────────────────────────

/**
 * Wire the DOM layer onto a core `FoundationWasm` runtime: creates the DomHeap,
 * registers it as the ReplyEncoder's DOM heap (DOMObject returns intern here), and
 * returns the DOM-specific import fragment to spread into the import object:
 *
 *   const rt = new FoundationWasm();
 *   const dom = new DomHeap();
 *   const abi = { ...rt.web_abi, ...domAbi(rt, dom) };
 *   const instance = new WebAssembly.Instance(module, { abi });
 *
 * @param {{functions:{reply:{dom:object}}}} rt  the core runtime
 * @param {DomHeap} dom
 * @returns {object} import fragment (`dom_allocate_external_pointer`, …)
 */
function domAbi(rt, dom) {
  rt.functions.reply.dom = dom;
  return {
    // Pre-allocate an external-pointer slot earmarked for a DOM node
    // (foundation_wasm_ui's allocate_dom_reference).
    dom_allocate_external_pointer: () => dom.create(null),
    // Retire a DOM handle (reserved slots 0–4 refuse; stale ids no-op).
    host_dom_drop_external_pointer: (handle) => {
      dom.destroy(BigInt(handle));
    },
    // DOM fast-path: the returned node interns into the DOM heap; its handle
    // crosses naked (29 = ReturnTypeId.DOMObject, the shared contract value).
    host_invoke_function_as_dom: (h, p, l) => {
      const v = rt.functions.invokeNakedAs(h, p, l, 29);
      return typeof v === "bigint" ? v : BigInt(v);
    },
  };
}

// ─── Global registration ──────────────────────────────────────────────────────────
//
// The ESM exports above are canonical (`<script type="module">` / import). This
// ─── Web Components (feature 06 — decisions 021/022/023/024) ─────────────────
//
// Three thin custom elements over four shared layers: Transport (how bytes
// move), ProtocolHandler (what the bytes are, by content type), Patcher (how
// parsed results land in the DOM), Hydrator (post-insertion styles/scripts —
// events stay with the F08 EventDispatcher). Browser-only APIs (EventSource,
// WebSocket, customElements, CSSStyleSheet) are guarded so the logic runs and
// tests under node.

/** Exponential reconnect backoff: 1s/2s/4s… capped at 30s (spec §12). */
function reconnectDelay(attempt) {
  return Math.min(1000 * 2 ** attempt, 30_000);
}

/** Probe `HEAD /primal/messages` (feature 11 §2): 200 = server batches. */
async function probeBatching(fetchFn = globalThis.fetch) {
  try {
    const res = await fetchFn("/primal/messages", { method: "HEAD" });
    return !!res && res.ok === true;
  } catch {
    return false;
  }
}

/**
 * HTTP request bundling (feature 11 / decision 026): when the server supports
 * `/primal/messages`, queued requests flush as ONE id'd batch per microtask
 * tick (`[{id, url, method, headers, body}]`); a lone request skips the
 * wrapper and goes out as itself.
 */
class RequestQueue {
  constructor(transport) {
    this.queue = [];
    this.scheduled = false;
    this.transport = transport;
    this.bundlingEnabled = false; // set after the probe
    this.nextId = 1;
  }

  /** Probe once; enable bundling on 200 OK. */
  async probe(fetchFn = globalThis.fetch) {
    this.bundlingEnabled = await probeBatching(fetchFn);
    return this.bundlingEnabled;
  }

  enqueue(request) {
    if (!this.bundlingEnabled) {
      return this.transport.send(request.url, request.method, request.data);
    }
    this.queue.push({
      id: this.nextId++,
      url: request.url,
      method: request.method ?? "GET",
      headers: request.headers,
      body: request.data,
    });
    if (!this.scheduled) {
      this.scheduled = true;
      queueMicrotask(() => this.flush());
    }
    return null;
  }

  flush() {
    this.scheduled = false;
    if (this.queue.length === 0) return; // no network call (test 7)
    const batch = this.queue.splice(0);
    if (batch.length === 1) {
      // Single request: as-is, no batch wrapper (test 5).
      const only = batch[0];
      this.transport.send(only.url, only.method, only.body);
      return;
    }
    this.transport.send("/primal/messages", "POST", batch);
  }
}

/**
 * Parse one batched-response entry (`{id, status, headers, body}`) per its
 * declared content type and produce the same result objects the protocol
 * handlers emit — so `Patcher.route` works on batch members too (§5).
 */
function parseBatchEntry(entry) {
  const ct = entry.headers?.["content-type"] ?? entry.headers?.["Content-Type"] ?? "";
  if (ct.includes("primal-json")) return { id: entry.id, ...routeJson(entry.body) };
  if (ct.includes("primal-html") || ct.includes("text/html")) {
    return { id: entry.id, ...routeHtml(String(entry.body)) };
  }
  if (ct.includes("primal-arrow")) {
    return { id: entry.id, type: "arrow", columns: ColumnarParser.parse(new Uint8Array(entry.body)) };
  }
  return { id: entry.id, type: "raw", text: String(entry.body ?? "") };
}

/**
 * WebSocket frame coalescing (feature 11 §6): one frame per microtask; a
 * single message ships unwrapped, several wrap as `{ batch: [...] }`.
 */
class WSBatchQueue {
  constructor(ws) {
    this.ws = ws;
    this.queue = [];
    this.scheduled = false;
  }

  send(data) {
    this.queue.push(data);
    if (!this.scheduled) {
      this.scheduled = true;
      queueMicrotask(() => this.flush());
    }
  }

  flush() {
    this.scheduled = false;
    if (this.queue.length === 1) {
      this.ws.send(JSON.stringify(this.queue.splice(0, 1)[0]));
    } else if (this.queue.length > 1) {
      this.ws.send(JSON.stringify({ batch: this.queue.splice(0) }));
    }
  }
}

/**
 * Worker postMessage coalescing (feature 11 §7): one structured-clone
 * transfer per microtask, transferables forwarded rather than copied.
 */
class WorkerBatchQueue {
  constructor(worker) {
    this.worker = worker;
    this.queue = [];
    this.scheduled = false;
  }

  postMessage(data) {
    this.queue.push(data);
    if (!this.scheduled) {
      this.scheduled = true;
      queueMicrotask(() => this.flush());
    }
  }

  flush() {
    this.scheduled = false;
    if (this.queue.length === 0) return;
    const batch = this.queue.splice(0);
    const transfers = batch.map((d) => d?.transferable).filter(Boolean);
    this.worker.postMessage({ batch }, transfers);
  }
}

// ─── Transport layer (G28: send for request-response, connect for streams) ────

class FetchTransport {
  constructor(config = {}) {
    this.config = config;
    this.fetchFn = config.fetchFn ?? ((...args) => globalThis.fetch(...args));
  }

  async send(url, method = "POST", data = undefined) {
    const init = { method, headers: this.config.headers };
    if (data !== undefined && method !== "GET" && method !== "HEAD") {
      init.body = typeof data === "string" ? data : JSON.stringify(data);
    }
    const response = await this.fetchFn(url, init);
    if (!response.ok) throw new Error(`FetchTransport: ${method} ${url} -> ${response.status}`);
    const handler =
      ProtocolHandler.named(this.config.protocol) ?? ProtocolHandler.fromContentType(response);
    return handler.process(response);
  }

  disconnect() {}
}

// ─── Owned SSE: parser + fetch-based EventSource (feature 21) ─────────────────
//
// The browser EventSource API is GET-only — no bodies, no custom verbs, no
// headers. Datastar's answer (and ours): fetch() the stream with ANY method
// and parse the SSE protocol from the response ReadableStream. The parser
// MIRRORS foundation_netio's Rust SseParser semantics exactly (id rejects
// NUL, one leading value space stripped, multi-line data joined with \n,
// comments surfaced, empty line dispatches only with data, EOF flushes,
// no-colon lines ignored), plus the byte-stream concerns the Rust reader
// doesn't face here: UTF-8 sequences and CRLF pairs split across chunks, and
// partial trailing lines buffered until their terminator arrives.

class SseParser {
  constructor() {
    this.decoder = new TextDecoder("utf-8"); // {stream:true} handles split runes
    this.textBuffer = "";
    this.sawCarriageReturn = false; // CRLF split across chunks
    this.lastEventId = null;
    this.#resetBuilder();
  }

  #resetBuilder() {
    this.id = null;
    this.eventType = null;
    this.data = [];
    this.retry = null;
  }

  /** Feed one byte chunk; returns the COMPLETE events it finished. */
  push(chunk) {
    let text = this.decoder.decode(chunk, { stream: true });
    // A \r at a previous chunk's end: swallow a leading \n (split CRLF).
    if (this.sawCarriageReturn) {
      this.sawCarriageReturn = false;
      if (text.startsWith("\n")) text = text.slice(1);
    }
    if (text.endsWith("\r")) {
      this.sawCarriageReturn = true;
    }
    this.textBuffer += text;

    const events = [];
    for (;;) {
      const cut = this.#nextLineEnd();
      if (cut === null) break;
      const [line, rest] = cut;
      this.textBuffer = rest;
      const event = this.#processLine(line);
      if (event) events.push(event);
    }
    return events;
  }

  /** EOF: flush any accumulated data as a final event (Rust parity). */
  end() {
    const tail = this.decoder.decode(); // flush a dangling partial rune
    if (tail) this.textBuffer += tail;
    const events = [];
    if (this.textBuffer.length > 0) {
      const event = this.#processLine(this.textBuffer.replace(/\r$/, ""));
      this.textBuffer = "";
      if (event) events.push(event);
    }
    const flushed = this.#dispatch();
    if (flushed) events.push(flushed);
    return events;
  }

  /** Find the next complete line, honoring \r\n, \n, and lone \r. */
  #nextLineEnd() {
    const buf = this.textBuffer;
    for (let i = 0; i < buf.length; i++) {
      const ch = buf[i];
      if (ch === "\n") return [buf.slice(0, i), buf.slice(i + 1)];
      if (ch === "\r") {
        if (i + 1 < buf.length) {
          const skip = buf[i + 1] === "\n" ? 2 : 1;
          return [buf.slice(0, i), buf.slice(i + skip)];
        }
        return null; // lone \r at buffer end — wait for the next chunk
      }
    }
    return null;
  }

  #processLine(line) {
    if (line.length === 0) {
      return this.#dispatch(); // empty line — dispatch IF data accumulated
    }
    if (line.startsWith(":")) {
      // Comments surface immediately (Rust parity).
      return { type: "comment", comment: line.slice(1).replace(/^ /, "").trimStart(), lastEventId: this.lastEventId };
    }
    const colon = line.indexOf(":");
    if (colon === -1) return null; // no-colon lines ignored (Rust parity)
    const field = line.slice(0, colon);
    let value = line.slice(colon + 1);
    if (value.startsWith(" ")) value = value.slice(1); // exactly ONE space

    switch (field) {
      case "id":
        if (!value.includes("\0")) this.id = value;
        break;
      case "event":
        this.eventType = value;
        break;
      case "data":
        this.data.push(value);
        break;
      case "retry": {
        const ms = Number(value);
        if (Number.isInteger(ms) && ms >= 0 && /^\d+$/.test(value)) this.retry = ms;
        break;
      }
      default: // unknown fields ignored
    }
    return null;
  }

  #dispatch() {
    if (this.data.length === 0) {
      this.#resetBuilder(); // reset even without an event (Rust parity)
      return null;
    }
    if (this.id !== null) this.lastEventId = this.id;
    const event = {
      type: "message",
      event: this.eventType,
      data: this.data.join("\n"),
      id: this.id,
      lastEventId: this.lastEventId,
      retry: this.retry,
    };
    this.#resetBuilder();
    return event;
  }
}

/**
 * EventSource over fetch (feature 21): ANY method, headers, body; streams the
 * response through {@link SseParser}; reconnects (F06 backoff, server
 * `retry:` override, `Last-Event-ID` carried) until {@link FetchEventSource#close}.
 */
class FetchEventSource {
  /**
   * @param {string} url
   * @param {{ method?:string, headers?:object, body?:any, fetchFn?:Function,
   *           onOpen?:Function, onEvent?:Function, onComment?:Function,
   *           onError?:Function }} [options]
   */
  constructor(url, options = {}) {
    this.url = url;
    this.options = options;
    this.fetchFn = options.fetchFn ?? ((...args) => globalThis.fetch(...args));
    this.closed = false;
    this.attempt = 0;
    this.retryOverride = null; // server `retry:` field
    this.lastEventId = null;
    this.abort = null;
  }

  /** Open the stream (returns when the FIRST connection attempt settles). */
  async connect() {
    this.closed = false;
    await this.#attemptOnce();
  }

  async #attemptOnce() {
    if (this.closed) return;
    this.abort = typeof AbortController === "undefined" ? null : new AbortController();
    const headers = {
      accept: "text/event-stream",
      ...(this.options.headers ?? {}),
    };
    if (this.lastEventId !== null) headers["last-event-id"] = this.lastEventId;

    let response;
    try {
      response = await this.fetchFn(this.url, {
        method: this.options.method ?? "GET",
        headers,
        body: this.options.body,
        signal: this.abort?.signal,
      });
      if (!response.ok) throw new Error(`FetchEventSource: ${response.status}`);
    } catch (error) {
      this.options.onError?.(error);
      this.#scheduleReconnect();
      return;
    }

    this.options.onOpen?.(response);
    this.attempt = 0; // a successful open resets the backoff
    const parser = new SseParser();
    const reader = response.body.getReader();
    try {
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        for (const event of parser.push(value)) this.#deliver(event);
      }
      for (const event of parser.end()) this.#deliver(event);
    } catch (error) {
      if (!this.closed) this.options.onError?.(error);
    }
    this.lastEventId = parser.lastEventId ?? this.lastEventId;
    // Stream ended — SSE semantics: reconnect unless closed.
    this.#scheduleReconnect();
  }

  #deliver(event) {
    if (event.type === "comment") {
      this.options.onComment?.(event);
      return;
    }
    if (event.retry !== null && event.retry !== undefined) {
      this.retryOverride = event.retry;
    }
    if (event.id !== null) this.lastEventId = event.id;
    this.options.onEvent?.(event);
  }

  #scheduleReconnect() {
    if (this.closed) return;
    const delay = this.retryOverride ?? reconnectDelay(this.attempt);
    this.attempt += 1;
    this.reconnectTimer = setTimeout(() => {
      if (!this.closed) this.#attemptOnce();
    }, delay);
  }

  /** Stop: abort the in-flight fetch and cancel reconnection. */
  close() {
    this.closed = true;
    clearTimeout(this.reconnectTimer);
    this.abort?.abort?.();
  }
}

class SSETransport {
  constructor(config = {}) {
    this.config = config;
    this.source = null;
  }

  /**
   * Open an SSE stream with ANY method (feature 21 — the browser EventSource
   * is GET-only; this rides {@link FetchEventSource}). `config.method`
   * defaults to POST when `data` is given, GET otherwise.
   */
  connect(url, data, onResult) {
    const method = this.config.method ?? (data === undefined ? "GET" : "POST");
    this.source = new FetchEventSource(url, {
      method,
      headers: {
        ...(data === undefined ? {} : { "content-type": "application/json" }),
        ...(this.config.headers ?? {}),
      },
      body: data === undefined ? undefined : JSON.stringify(data),
      fetchFn: this.config.fetchFn,
      // G46: the `event:` field names the payload kind; unnamed = html.
      // The mount `protocol` attribute overrides per feature-04 precedence.
      onEvent: (event) =>
        onResult(streamEventResult(this.config.protocol ?? event.event ?? "html", event.data)),
      onError: this.config.onError,
    });
    return this.source.connect();
  }

  disconnect() {
    this.source?.close?.();
    this.source = null;
  }
}

class WebSocketTransport {
  constructor(config = {}) {
    this.config = config;
    this.ws = null;
    this.attempt = 0;
    this.closed = false;
  }

  connect(url, _data, onResult) {
    const factory =
      this.config.wsFactory ??
      (typeof WebSocket === "undefined" ? null : (target) => new WebSocket(target));
    if (!factory) {
      throw new Error("WebSocketTransport: WebSocket unavailable in this environment");
    }
    this.closed = false;
    this.url = url;
    this.onResult = onResult;
    this.ws = factory(url);
    // Binary frames carry the ENVELOPE — its header tells the protocol
    // (feature 04); text frames fall back to the declared protocol, else
    // json (the pre-feature-04 behavior, preserved as the text default).
    if ("binaryType" in this.ws) this.ws.binaryType = "arraybuffer";
    this.ws.onmessage = (event) => {
      const { data } = event;
      if (data instanceof ArrayBuffer || ArrayBuffer.isView(data)) {
        const result = decodeEnvelopeFrame(
          data instanceof ArrayBuffer ? new Uint8Array(data) : new Uint8Array(data.buffer, data.byteOffset, data.byteLength),
        );
        if (result) onResult(result);
        return; // malformed frames already surfaced a typed error
      }
      onResult(streamEventResult(this.config.protocol ?? "json", data));
    };
    this.ws.onclose = () => {
      if (this.closed) return;
      const delay = reconnectDelay(this.attempt);
      this.attempt += 1;
      setTimeout(() => {
        if (!this.closed) this.connect(this.url, undefined, this.onResult);
      }, delay);
    };
  }

  send(data) {
    this.ws?.send(typeof data === "string" ? data : JSON.stringify(data));
  }

  disconnect() {
    this.closed = true;
    this.ws?.close?.();
    this.ws = null;
  }
}

class ChunkedTransport {
  constructor(config = {}) {
    this.config = config;
    this.abort = null;
    this.fetchFn = config.fetchFn ?? ((...args) => globalThis.fetch(...args));
  }

  async connect(url, data, onChunk) {
    this.abort = new AbortController();
    const response = await this.fetchFn(url, {
      method: data === undefined ? "GET" : "POST",
      body: data === undefined ? undefined : JSON.stringify(data),
      signal: this.abort.signal,
    });
    const handler =
      ProtocolHandler.named(this.config.protocol) ?? ProtocolHandler.fromContentType(response);
    const reader = response.body.getReader();
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      onChunk(await handler.processChunk(value));
    }
  }

  disconnect() {
    this.abort?.abort?.();
    this.abort = null;
  }
}

class WorkerTransport {
  constructor(config = {}) {
    this.worker = config.worker;
  }

  send(_url, _method, data) {
    this.worker.postMessage(data);
  }

  onMessage(callback) {
    this.worker.onmessage = (event) => callback(event.data);
  }

  disconnect() {
    this.worker?.terminate?.();
  }
}

class Transport {
  /** Factory (spec §2): default is fetch. */
  static create(config = {}) {
    switch (config.transport) {
      case "sse":
        return new SSETransport(config);
      case "ws":
        return new WebSocketTransport(config);
      case "chunked":
        return new ChunkedTransport(config);
      case "worker":
        return new WorkerTransport(config);
      default:
        return new FetchTransport(config);
    }
  }
}

// ─── Protocol handlers (decision 022 content types) ───────────────────────────

/**
 * Decode one envelope-framed wire message (feature 04): the 6-byte header
 * `[protocol][version][length:4 LE]` is AUTHORITATIVE — this is how
 * header-less channels (WebSocket binary frames, base64 SSE `arrow`
 * events) self-describe. Returns a routed result, or null after surfacing
 * a typed error (NEVER a silent json attempt).
 */
function decodeEnvelopeFrame(bytes) {
  if (!(bytes instanceof Uint8Array)) bytes = new Uint8Array(bytes);
  if (bytes.byteLength < 6) {
    console.error("decodeEnvelopeFrame: truncated header", bytes.byteLength);
    return null;
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const protocol = view.getUint8(0);
  const version = view.getUint8(1);
  const length = view.getUint32(2, true);
  const payload = bytes.subarray(6);
  if (payload.byteLength !== length) {
    console.error(`decodeEnvelopeFrame: length mismatch (header ${length}, payload ${payload.byteLength})`);
    return null;
  }
  if (protocol === 1 && version === 1) {
    return { type: "arrow", columns: ColumnarParser.parse(payload) };
  }
  if (protocol === 1 && version === 2) {
    // Real Arrow IPC needs the apache-arrow reader (embedded asset); the
    // core runtime stays lean — consumers register a reader.
    if (typeof ProtocolHandler.arrowIpcReader === "function") {
      return { type: "arrow-ipc", table: ProtocolHandler.arrowIpcReader(payload) };
    }
    console.error("decodeEnvelopeFrame: wire v2 (Arrow IPC) frame received but no reader is registered — set ProtocolHandler.arrowIpcReader (e.g. apache-arrow tableFromIPC)");
    return null;
  }
  if (protocol === 2) {
    try {
      return routeJson(JSON.parse(new TextDecoder().decode(payload)));
    } catch (error) {
      console.error("decodeEnvelopeFrame: json payload did not parse", error);
      return null;
    }
  }
  console.error(`decodeEnvelopeFrame: unknown protocol ${protocol} v${version}`);
  return null;
}

function base64ToBytes(text) {
  const bin = atob(String(text).trim());
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i += 1) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

function streamEventResult(kind, raw) {
  if (kind === "json") {
    try {
      return routeJson(JSON.parse(raw));
    } catch {
      return { type: "raw", text: String(raw) };
    }
  }
  if (kind === "arrow") {
    // SSE is text — arrow frames arrive base64'd, envelope included
    // (feature 04; completes the F11 follow-up note that used to live here).
    try {
      return decodeEnvelopeFrame(base64ToBytes(raw)) ?? { type: "raw", text: String(raw) };
    } catch (error) {
      console.error("streamEventResult: arrow event was not valid base64", error);
      return { type: "raw", text: String(raw) };
    }
  }
  return routeHtml(String(raw));
}

/**
 * Convert a JSON `DomOp` batch (the {@link JsonEncoder} flat-row form:
 * `[{op_id, node_id, operation, attribute, value, text_val}, …]`) into the same
 * batch shape {@link DomOpApplicator#apply} consumes, so the JSON and columnar
 * wires share ONE apply path. `null` columns map to `""` (what `applyOne` /
 * `resolveWireName` expect); `operation`/`node_id` are numeric.
 */
function jsonRowsToBatch(rows) {
  return {
    count: rows.length,
    nodeIds: rows.map((r) => r.node_id >>> 0),
    operations: rows.map((r) => r.operation),
    attribute: rows.map((r) => r.attribute ?? ""),
    value: rows.map((r) => r.value ?? ""),
    textVal: rows.map((r) => r.text_val ?? ""),
  };
}

/**
 * Convert a decoded Apache Arrow IPC table (wire VERSION 2, read via
 * `ProtocolHandler.arrowIpcReader`, e.g. apache-arrow `tableFromIPC`) into the
 * batch shape {@link DomOpApplicator#apply} consumes — the SAME shape the
 * columnar and JSON wires produce. Reads the canonical DomOp columns
 * (`operation`, `node_id`, `attribute`, `value`, `text_val`); null Utf8 cells
 * become `""`. Works with any table exposing `numRows` + `getChild(name).get(i)`.
 */
function arrowTableToBatch(table) {
  const count = table.numRows;
  const col = (name) => table.getChild(name);
  const op = col("operation");
  const nid = col("node_id");
  const attr = col("attribute");
  const val = col("value");
  const txt = col("text_val");
  const nodeIds = new Array(count);
  const operations = new Array(count);
  const attribute = new Array(count);
  const value = new Array(count);
  const textVal = new Array(count);
  for (let i = 0; i < count; i += 1) {
    operations[i] = Number(op.get(i));
    nodeIds[i] = Number(nid.get(i)) >>> 0;
    attribute[i] = attr.get(i) ?? "";
    value[i] = val.get(i) ?? "";
    textVal[i] = txt.get(i) ?? "";
  }
  return { count, nodeIds, operations, attribute, value, textVal };
}

/** A JSON `DomOp` batch is a non-empty array whose rows carry `operation`. */
function isJsonDomOpBatch(value) {
  return (
    Array.isArray(value) &&
    value.length > 0 &&
    value[0] != null &&
    typeof value[0] === "object" &&
    "operation" in value[0]
  );
}

/** JSON morph-wrapper / DomOp-batch detection (spec §3). */
function routeJson(value) {
  if (value && typeof value === "object" && value.morph) {
    return { type: "json-morph", morph: value.morph };
  }
  // A JSON-encoded DomOp batch applies through the SAME applicator the columnar
  // ("arrow") wire uses — the envelope's protocol byte is the only difference, so
  // a mount renders identically whichever wire it streamed over.
  if (isJsonDomOpBatch(value)) {
    return { type: "arrow", columns: jsonRowsToBatch(value) };
  }
  return { type: "json", patches: value };
}

/** HTML island-wrapper detection (spec §3). */
function routeHtml(html) {
  const match = /^\s*<island\b[^>]*data-target="([^"]+)"[^>]*data-action="([^"]+)"[^>]*>([\s\S]*)<\/island>\s*$/.exec(html);
  if (match) {
    return { type: "html-morph", target: match[1], action: match[2], content: match[3] };
  }
  return { type: "html", html };
}

class ArrowHandler {
  async process(response) {
    const buffer = await response.arrayBuffer();
    return { type: "arrow", columns: ColumnarParser.parse(new Uint8Array(buffer)) };
  }
}

class JsonHandler {
  async process(response) {
    return routeJson(await response.json());
  }
}

class HtmlHandler {
  async process(response) {
    return routeHtml(await response.text());
  }
}

class RawHandler {
  async process(response) {
    return { type: "raw", text: await response.text() };
  }
}

/**
 * NEGOTIATION TABLE (feature 04 — the contract, finally written down):
 *
 * | channel      | decided by                                                  |
 * |--------------|--------------------------------------------------------------|
 * | HTTP fetch   | response `content-type` (`fromContentType` below)            |
 * | SSE          | per-message `event:` name (html/json/arrow; unnamed = html)  |
 * | WS binary    | the ENVELOPE header (protocol byte + version — authoritative)|
 * | WS text      | declared `protocol` attr, else json                          |
 * | `protocol=`  | mount attribute OVERRIDES headers/event names entirely       |
 *
 * Precedence: attribute > headers/event name > channel default.
 */
class ProtocolHandler {
  /** Explicit handler by name — the mount `protocol` attribute override. */
  static named(name) {
    switch (name) {
      case "arrow":
        return new ArrowHandler();
      case "json":
        return new JsonHandler();
      case "html":
        return new HtmlHandler();
      default:
        return null;
    }
  }

  /** Content-type table (spec §3, G46). */
  static fromContentType(response) {
    const ct = response.headers?.get?.("content-type") || "";
    if (ct.includes("primal-arrow") || ct.includes("event-stream-arrow")) return new ArrowHandler();
    if (ct.includes("primal-json") || ct.includes("event-stream-json")) return new JsonHandler();
    if (ct.includes("primal-html") || ct.includes("event-stream-html") || ct.includes("text/html")) {
      return new HtmlHandler();
    }
    return new RawHandler();
  }
}

// ─── Hydrator (styles + scripts ONLY — events are F08's, G2) ──────────────────

/**
 * Prefix every rule selector with `prefix` — `.title{...}` inside `#island-1`
 * becomes `#island-1 .title{...}`; compound selectors each get the prefix.
 * Pure function (the CSSStyleSheet adoption around it is browser-only).
 */
function scopeCss(cssText, prefix) {
  return cssText.replace(/(^|\})\s*([^@{}][^{}]*)\{/g, (_, brace, selectors) => {
    const scoped = selectors
      .split(",")
      .map((sel) => `${prefix} ${sel.trim()}`)
      .join(", ");
    return `${brace}\n${scoped} {`;
  });
}

class Hydrator {
  /** Scoped-script execution scope (spec §5). */
  static createScope(targetElement) {
    const eventCleanups = [];
    return {
      targets: () => targetElement.querySelectorAll("[primal-id]"),
      parent: () => targetElement,
      querySelector: (sel) => targetElement.querySelector(sel),
      querySelectorAll: (sel) => targetElement.querySelectorAll(sel),
      addEvent(sel, event, handler) {
        const el = targetElement.querySelector(sel);
        if (el) {
          el.addEventListener(event, handler);
          eventCleanups.push({ el, event, handler });
        }
      },
      cleanup() {
        for (const { el, event, handler } of eventCleanups) {
          el.removeEventListener(event, handler);
        }
        eventCleanups.length = 0;
      },
    };
  }

  /** Styles + scripts. NOT events (F08 owns primal:on*). Browser-leaning. */
  static hydrate(root, doc = root.ownerDocument ?? globalThis.document) {
    if (typeof root.querySelectorAll !== "function") return;
    // Styles: scope + adopt.
    for (const style of root.querySelectorAll("style[primal\\:style]")) {
      const prefix = root.id ? `#${root.id}` : `[primal-id="${root.getAttribute?.("primal-id") ?? ""}"]`;
      const scoped = scopeCss(style.textContent, prefix);
      if (typeof CSSStyleSheet === "function" && doc?.adoptedStyleSheets) {
        const sheet = new CSSStyleSheet();
        sheet.replaceSync(scoped);
        doc.adoptedStyleSheets = [...doc.adoptedStyleSheets, sheet];
        (root._sheets ??= []).push(sheet);
      }
      style.remove?.();
    }
    // Scripts: run with the scope object; per-script error isolation. Scope to
    // the script's OWN parent (spec §5.2), so hydrating a subtree containing
    // many components scopes each script to its component root, not the subtree.
    for (const script of root.querySelectorAll("script[primal\\:script]")) {
      const owner = script.parentElement ?? root;
      try {
        const fn = new Function("scope", script.textContent);
        fn(Hydrator.createScope(owner));
      } catch (error) {
        console.error("Hydrator: scoped script failed", error);
      }
      script.remove?.();
    }
  }
}

// ─── Patcher ───────────────────────────────────────────────────────────────────

class Patcher {
  /** Injectable seams: the F08 dispatcher, the DomOp applicator, signals. */
  static runtime = { dispatcher: null, applicator: null, signalBridge: null };
  /** Monotonic count of applied results (the frame instrument; see `route`). */
  static frameSeq = 0;

  static materialize(html, target, doc = target.ownerDocument ?? globalThis.document) {
    if (typeof doc?.createRange === "function") {
      const fragment = doc.createRange().createContextualFragment(html);
      target.appendChild(fragment);
    } else {
      target.innerHTML = html;
    }
    Hydrator.hydrate(target, doc);
    Patcher.runtime.dispatcher?.scanAndWire?.(target); // F08 (idempotent)
  }

  static applyDomOps(columns) {
    Patcher.runtime.applicator?.apply?.(columns);
  }

  /**
   * Lazily install the Arrow DOM-op applicator the streaming path needs. A
   * native App (Mode 1) ships absolute primal-ids and appends its root onto a
   * reserved ambient node, so the applicator's NodeRegistry MUST be seeded with
   * the live document (head=0, body=1, html=2). Idempotent; a host that wants a
   * custom registry/onEvent hook can pre-set `Patcher.runtime.applicator`.
   */
  static ensureApplicator(doc) {
    if (!Patcher.runtime.applicator) {
      Patcher.runtime.applicator = new DomOpApplicator(new NodeRegistry().seedDocument(doc), doc);
    }
    return Patcher.runtime.applicator;
  }

  static applySignalPatches(patches) {
    const bridge = Patcher.runtime.signalBridge;
    if (bridge?.applyPatches) bridge.applyPatches(patches);
    else console.warn("Patcher: no signalBridge installed; JSON patches dropped", patches);
  }

  /** Route one ProtocolHandler result to the DOM (shared by both mounts). */
  static route(result, targetEl, doc) {
    Patcher.applyRoute(result, targetEl, doc);
    // Frame instrument: every applied result bumps a sequence + timestamp. Cheap,
    // always-on, broadly useful — a server-driven client (or a test driver) can
    // poll `globalThis.__primalFrames` to know the DOM has caught up.
    Patcher.frameSeq += 1;
    const stamp = (typeof performance !== "undefined" ? performance.now() : Date.now());
    if (typeof globalThis !== "undefined") {
      globalThis.__primalFrames = { seq: Patcher.frameSeq, at: stamp };
    }
  }

  /** Apply one routed result (the actual DOM work; instrumented by `route`). */
  static applyRoute(result, targetEl, doc) {
    switch (result.type) {
      case "html":
        Patcher.materialize(result.html, targetEl, doc);
        break;
      case "html-morph": {
        const morphTarget = doc.querySelector(result.target);
        if (morphTarget && typeof doc.createRange === "function") {
          MorphDom.morph(morphTarget, doc.createRange().createContextualFragment(result.content), doc);
        }
        break;
      }
      case "json-morph": {
        const morphTarget = doc.querySelector(result.morph.target);
        if (morphTarget && typeof doc.createRange === "function") {
          MorphDom.morph(morphTarget, doc.createRange().createContextualFragment(result.morph.content), doc);
        }
        break;
      }
      case "json":
        Patcher.applySignalPatches(result.patches);
        break;
      case "arrow":
        Patcher.ensureApplicator(doc);
        Patcher.applyDomOps(result.columns);
        break;
      case "arrow-ipc":
        // Apache Arrow IPC (wire v2): the decoded table carries the same DomOp
        // columns — read them into a batch and apply through the SAME applicator.
        Patcher.ensureApplicator(doc);
        Patcher.applyDomOps(arrowTableToBatch(result.table));
        break;
      default:
        targetEl.textContent = result.text ?? "";
    }
  }
}

// ─── Response placement (spec §9, shared by both mounts) ──────────────────────

function resolveMountTarget(element, target, doc) {
  if (!target) {
    // Self-replacement: the mount is a placeholder that disappears (G29).
    const container = doc.createElement("div");
    element.parentNode?.replaceChild?.(container, element);
    return container;
  }
  if (target === "parent") {
    const parent = element.parentElement ?? element.parent ?? null;
    if (!parent) throw new Error("mount target not found: parent");
    clearChildren(parent);
    return parent;
  }
  const found = doc.querySelector(target);
  if (!found) throw new Error(`mount target not found: ${target}`);
  clearChildren(found);
  return found;
}

function clearChildren(el) {
  while (el.firstChild) el.removeChild(el.firstChild);
}

// ─── Custom elements (browser base class guarded for node) ─────────────────────

const BaseElement = typeof HTMLElement === "undefined" ? class {} : HTMLElement;

/** `<primal-island>` — no network; styles + scripts + F08 event wiring. */
class IslandComponent extends BaseElement {
  connectedCallback() {
    Hydrator.hydrate(this);
    Patcher.runtime.dispatcher?.scanAndWire?.(this);
  }

  disconnectedCallback() {
    const doc = this.ownerDocument ?? globalThis.document;
    if (this._sheets?.length && doc?.adoptedStyleSheets) {
      doc.adoptedStyleSheets = doc.adoptedStyleSheets.filter((s) => !this._sheets.includes(s));
    }
    this._sheets = [];
    Patcher.runtime.dispatcher?.removeListeners?.(this);
  }
}

/** Shared mount bootstrap: read attrs, build transport, resolve target. */
function mountSetup(element, defaults = {}) {
  const api = element.getAttribute("api");
  if (!api) throw new Error(`${element.tagName?.toLowerCase() ?? "mount"}: missing api attribute`);
  let data = {};
  try {
    data = JSON.parse(element.getAttribute("data") || "{}");
  } catch (error) {
    console.error("mount: invalid JSON in data attribute", error);
  }
  const transport =
    element._transportOverride ??
    Transport.create({
      transport: element.getAttribute("transport") ?? defaults.transport,
      protocol: element.getAttribute("protocol") ?? undefined,
      url: api,
    });
  return { api, data, transport, target: element.getAttribute("target") };
}

/** `<mount-data>` — one request, one response (spec §7). */
class MountDataComponent extends BaseElement {
  async connectedCallback() {
    const doc = this.ownerDocument ?? globalThis.document;
    try {
      const { api, data, transport, target } = mountSetup(this);
      this._transport = transport;
      const method = this.getAttribute("method") || "POST";
      const result = await transport.send(api, method, data);
      const targetEl = resolveMountTarget(this, target, doc);
      Patcher.route(result, targetEl, doc);
    } catch (error) {
      console.error("mount-data:", error);
    }
  }

  disconnectedCallback() {
    this._transport?.disconnect?.();
  }
}

/** `<mount-stream>` — continuous results until disconnect (spec §8). */
class MountStreamComponent extends BaseElement {
  connectedCallback() {
    const doc = this.ownerDocument ?? globalThis.document;
    try {
      const { api, data, transport, target } = mountSetup(this, { transport: "sse" });
      this._transport = transport;
      const targetEl = resolveMountTarget(this, target, doc);
      transport.connect(api, data, (result) => Patcher.route(result, targetEl, doc));
    } catch (error) {
      console.error("mount-stream:", error);
    }
  }

  disconnectedCallback() {
    this._transport?.disconnect?.();
    this._transport = null;
  }
}

/** Register the custom elements (browser only; idempotent). */
function registerWebComponents() {
  if (typeof customElements === "undefined") return;
  if (!customElements.get("primal-island")) customElements.define("primal-island", IslandComponent);
  if (!customElements.get("mount-data")) customElements.define("mount-data", MountDataComponent);
  if (!customElements.get("mount-stream")) customElements.define("mount-stream", MountStreamComponent);
}

/**
 * Wire a `FoundationWasm` runtime for direct WASM→DOM UI (mode 1 / client-side).
 *
 * The `FoundationWasm` core only registers protocol byte 0 (batch instructions).
 * `App::new()` (the compact columnar wire, protocol byte 1) needs an explicit
 * `columnarHandler` on the dispatcher — otherwise `host_apply` throws
 * `"unknown protocol: 1"` and every `stabilize()` drops its DomOps.
 *
 * This ONE call sets up the full DOM path:
 *   NodeRegistry(seed document) → DomOpApplicator → EventDispatcher → rAF scan
 * and registers `columnarHandler(applicator)` on protocol byte 1.
 *
 * ```js
 * import { FoundationWasm } from './foundation-wasm.js';
 * import { registerWasmApp } from './foundation-wasm-ui.js';
 *
 * const rt = new FoundationWasm();
 * const imports = { abi: rt.web_abi };
 * const { instance } = await WebAssembly.instantiate(wasmBytes, imports);
 * rt.init(instance);
 * registerWasmApp(rt);          // ← wires protocol 1 → DOM
 * instance.exports.myapp();     // ← Rust code can now stabilize() into the DOM
 * ```
 *
 * @param {{dispatcher: ProtocolDispatcher}} runtime  a FoundationWasm instance
 *   (anything with `.dispatcher.setHandler(protocol, handler)`)
 * @param {{document?: Document}} [opts]  override for tests/SSR
 */
function registerWasmApp(runtime, opts = {}) {
  const doc = opts.document ?? globalThis.document;
  if (!doc) {
    console.error("registerWasmApp: no document available (non-browser environment)");
    return;
  }
  const registry = new NodeRegistry().seedDocument(doc);
  const dispatcher = new EventDispatcher(
    signalDeliver(runtime, jsonEncodeEventData), // callback ids from html! macro
    { deliverSignal: signalDeliver(runtime, jsonEncodeEventData) },
  );
  const applicator = new DomOpApplicator(registry, doc, (eventName, nodeId, event, el) => {
    dispatcher.deliver(
      parseCallbackId(el.getAttribute?.("primal:setter") ?? ""),
      buildEventData(eventName, event, el),
    );
  });
  runtime.dispatcher.setHandler(1, columnarHandler(applicator));
  // Initial scan + observer: wires primal:on* attrs that exist on page load.
  initEventRuntime(dispatcher, doc);

  // F43: Wire IPC bridge — host_ipc_invoke_async routes to invokeIpc
  // (Tauri/Deno/browser), which resolves via ipc_resolve(token, allocId).
  if (typeof globalThis !== 'undefined' && globalThis.registerIpcTriggers) {
    globalThis.registerIpcTriggers(runtime);
  }
}

/**
 * Register the Apache Arrow IPC (wire v2) reader so streamed `arrow-ipc` frames
 * decode through {@link decodeEnvelopeFrame}. `arrow` defaults to the global the
 * bundled `apache-arrow.js` UMD sets (`globalThis.Arrow`); pass a module if you
 * load it another way. This is the ONE framework call a consumer makes for Arrow
 * IPC — the `tableFromIPC` wiring stays in the runtime, not in each page.
 */
function registerArrowIpc(arrow = globalThis.Arrow) {
  if (!arrow || typeof arrow.tableFromIPC !== "function") {
    console.error(
      "registerArrowIpc: no apache-arrow `tableFromIPC` available — load apache-arrow.js first",
    );
    return;
  }
  ProtocolHandler.arrowIpcReader = (bytes) => arrow.tableFromIPC(bytes);
}

/**
 * The frame instrument: `{ seq, at }` — `seq` is the count of applied routed
 * results, `at` the timestamp of the last apply. A server-driven client (or a
 * test driver) polls this to know the DOM has caught up with the stream. Also
 * mirrored on `globalThis.__primalFrames`.
 */
function frameStats() {
  return globalThis.__primalFrames ?? { seq: Patcher.frameSeq, at: null };
}

/** Build the `window.primal` namespace (G30) over injected runtime seams. */
function createPrimal({ dispatcher, doc = globalThis.document } = {}) {
  return {
    mountData(api, data, targetNode, opts = {}) {
      const transport = Transport.create({ transport: opts.transport, url: api });
      return transport
        .send(api, opts.method || "POST", data)
        .then((result) => Patcher.route(result, targetNode, doc));
    },
    mountStream(api, data, targetNode, opts = {}) {
      const transport = Transport.create({ transport: opts.transport ?? "sse", url: api });
      transport.connect(api, data, (result) => Patcher.route(result, targetNode, doc));
      return transport;
    },
    unmount(element) {
      element.disconnectedCallback?.();
      element.remove?.();
    },
    on: (selector, event, handler) => doc.querySelector(selector)?.addEventListener(event, handler),
    onclick(selector, handler) {
      this.on(selector, "click", handler);
    },
    onchange(selector, handler) {
      this.on(selector, "change", handler);
    },
    off: (selector, event, handler) => doc.querySelector(selector)?.removeEventListener(event, handler),
    scope: (element) => Hydrator.createScope(element),
    dispatcher,
    Transport,
    ProtocolHandler,
    Patcher,
    Hydrator,
  };
}

if (typeof window !== "undefined") {
  registerWebComponents();
  window.primal ??= createPrimal({});
}

// mirror lets classic (non-module) scripts on the same page reach the DOM runtime as
// `globalThis.FoundationWasmUiRuntime` once the module has loaded.
globalThis.FoundationWasmUiRuntime = Object.freeze({
  Op,
  RESERVED,
  ColumnarParser,
  NodeRegistry,
  DomOpApplicator,
  columnarHandler,
  MorphDom,
  moveBefore,
  buildEventData,
  parseCallbackId,
  EventDispatcher,
  CONVENIENCE_EVENTS,
  callbackDeliver,
  signalDeliver,
  resolveFunctionRef,
  resolveDelegateTarget,
  initEventRuntime,
  Transport,
  ProtocolHandler,
  Patcher,
  Hydrator,
  scopeCss,
  RequestQueue,
  SseParser,
  FetchEventSource,
  WSBatchQueue,
  WorkerBatchQueue,
  probeBatching,
  parseBatchEntry,
  reconnectDelay,
  resolveMountTarget,
  IslandComponent,
  MountDataComponent,
  MountStreamComponent,
  registerWebComponents,
  registerWasmApp,
  createPrimal,
  DomHeap,
  domAbi,
  BATCH_OP_APPLY_DOM,
  registerDomBatchOperation,
});

/* ═════════ platform-scheme-interceptor.js ═════════ */
// Platform scheme interceptor — foundation_wasm_ui (spec-52).
//
// Android WebView has no API for custom URI scheme handlers. wry works around
// this by converting custom-scheme URLs to HTTP URLs during initial load and
// intercepting them in shouldInterceptRequest. But link clicks and
// programmatic navigations to ewe:// URLs are NOT converted — Android's
// shouldOverrideUrlLoading lets them fall through to the OS, which has no
// handler for them.
//
// This script intercepts clicks and navigations to platform schemes
// (ewe://, foundation://, platform://) and rewrites them to
// http://{scheme}.localhost/... — the format wry's shouldInterceptRequest
// recognizes and routes to the registered protocol handler.
//
// Desktop engines (WebKit, WebView2) support custom schemes natively through
// register_uri_scheme_protocol — this script is a no-op on those platforms.
//
// Usage: include this script BEFORE any other application code.
// foundation_platform's builder injects it automatically; foundation_wasm_ui
// embeds it in the core JS runtime for standalone WASM apps.

;(function () {
  'use strict';

  var PLATFORM_SCHEMES = ['ewe', 'foundation', 'platform'];
  var PREFIX = 'http://';

  function isPlatformScheme(href) {
    if (!href) return false;
    for (var i = 0; i < PLATFORM_SCHEMES.length; i++) {
      var prefix = PLATFORM_SCHEMES[i] + '://';
      if (href.indexOf(prefix) === 0) return PLATFORM_SCHEMES[i];
    }
    return null;
  }

  function toWorkaroundUrl(href) {
    var scheme = isPlatformScheme(href);
    if (!scheme) return null;
    return href.replace(scheme + '://', PREFIX + scheme + '.');
  }

  function isAndroid() {
    return /android/i.test(navigator.userAgent);
  }

  // Only activate on Android — desktop engines handle custom schemes natively.
  if (!isAndroid()) return;

  // ── Intercept <a> clicks ──────────────────────────────────────────
  document.addEventListener('click', function (e) {
    var a = e.target.closest('a');
    if (!a) return;
    var href = a.getAttribute('href') || a.href;
    var newUrl = toWorkaroundUrl(href);
    if (!newUrl) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    location.href = newUrl;
  }, true); // capture phase — beats user-land listeners

  // ── Intercept programmatic navigations ────────────────────────────
  var _origAssign = location.assign;
  var _origReplace = location.replace;
  var _origSetHref = Object.getOwnPropertyDescriptor(Location.prototype, 'href');

  function wrap(fn) {
    return function (url) {
      var rewritten = toWorkaroundUrl(url);
      if (rewritten) {
        url = rewritten;
      }
      return fn.call(this, url);
    };
  }

  // Android WebView may make location.assign / location.replace read-only.
  // Try direct assignment first; fall back to defineProperty if that throws.
  try {
    location.assign = wrap(_origAssign);
  } catch (_) {
    try {
      Object.defineProperty(location, 'assign', {
        value: wrap(_origAssign),
        writable: true,
        configurable: true,
      });
    } catch (__) { /* best-effort */ }
  }
  try {
    location.replace = wrap(_origReplace);
  } catch (_) {
    try {
      Object.defineProperty(location, 'replace', {
        value: wrap(_origReplace),
        writable: true,
        configurable: true,
      });
    } catch (__) { /* best-effort */ }
  }

  if (_origSetHref && _origSetHref.set) {
    var _set = _origSetHref.set;
    Object.defineProperty(location, 'href', {
      get: _origSetHref.get,
      set: function (url) {
        var rewritten = toWorkaroundUrl(url);
        _set.call(this, rewritten || url);
      },
      configurable: true,
      enumerable: true,
    });
  }

  // ── Intercept window.open ─────────────────────────────────────────
  var _origOpen = window.open;
  window.open = function (url) {
    var rewritten = toWorkaroundUrl(url);
    if (rewritten) {
      arguments[0] = rewritten;
    }
    return _origOpen.apply(this, arguments);
  };
})();
