// batch-instructions.js — the V2 quantized batch codec (ported from megatron's
// ParameterParserV2 + BatchInstructions + BatchOperation, per feature 17 research docs).
//
// This is the backbone of the custom binary protocol: WASM builds an instruction batch
// (Rust `Instructions`, ops.rs) in TWO arena slots — an OPS buffer of opcodes/markers
// and a TEXTS buffer of raw UTF-8 — and ships both via `host_batch_apply` (no results)
// or `host_batch_returning_apply` (group-return slot id). DISTINCT from the flat invoke
// codec (V1) in function-registry.js: batch params are marker-framed AND quantized.
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

import {
  ParamType, ReturnType, ReturnIds, ThreeStateId,
  ExternalPointer, InternalPointer, ErrorCodeValue, TypedArraySliceValue, TypedSliceArray,
  ReturnHintParser,
} from "./function-registry.js";

const B1 = 1, B2 = 2, B4 = 4, B8 = 8;

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
  /** @param {{memory:WebAssembly.Memory}} bridge @param {import('./foundation-wasm.js').StringCache} strings */
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
   * @param {import('./foundation-wasm.js').MemoryAllocations} memory
   * @param {import('./foundation-wasm.js').StringCache} strings
   * @param {import('./function-registry.js').FunctionRegistry} functions
   * @param {import('./foundation-wasm.js').CallbackRegistry} callbacks
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
      Promise.resolve(result).then(
        (value) => b.reply.callbackSuccess(b.callbacks, callbackId, hint, value),
        (error) => b.reply.callbackFailure(b.callbacks, callbackId, error),
      );
      return null; // delivery happens via invoke_callback, never inline
    };
    return [i, thunk];
  }
}
