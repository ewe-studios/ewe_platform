// foundation-wasm.js — core WASM↔JS ABI runtime (clean rewrite of megatron.js).
//
// WHY: megatron.js is one 7000-line IIFE that tangles the pure ABI (memory,
// messaging, callbacks, timers) with DOM concerns. This file is the ABI half only —
// no DOM, no window — matching the `foundation_wasm` crate split (decision 015).
//
// WHAT: the core classes the spec's Part D lists for foundation-wasm.js:
//   - WasmEnvelope      : the 14-byte [protocol][version][memory_id][length] header
//   - ProtocolDispatcher: reads the envelope, routes to a protocol handler by byte
//   - MemoryAllocations : JS view over the WASM arena (create/get/write/dispose/clear)
//   - TimerRegistry     : schedule_timeout/interval host imports ↔ run_*_callback exports
//   - CallbackRegistry  : async JS→WASM responses via invoke_callback
//   - FoundationWasm    : owns the bridge, exposes `web_abi` (import object) + `init()`
//
// HOW: bootstrap mirrors the proven integration pattern —
//   const rt = new FoundationWasm();
//   const mod = await WebAssembly.instantiate(bytes, { abi: rt.web_abi });
//   rt.init(mod);                  // memory comes from instance.exports.memory
// The WASM module EXPORTS its own linear memory; the host imports close over a
// lazily-populated `bridge`, so they work even though the instance doesn't exist at
// the time the import object is built.
//
// Protocol bytes (decision 014/022): 0 = Custom Binary, 1 = Arrow, 2 = JSON.

import { FunctionRegistry } from "./function-registry.js";

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
    this.functions = new FunctionRegistry(this.bridge, this.memory, this.strings);
    this.dispatcher = new ProtocolDispatcher();
  }

  /**
   * The `{ ...abi imports }` object — pass as `{ abi: rt.web_abi }` to instantiate.
   * The DOM-specific imports (rAF, function invocation, dom_* refs) are layered on by
   * foundation-wasm-ui.js, which can extend this object.
   */
  get web_abi() {
    const { memory, dispatcher, timers, animation, strings, functions } = this;
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

      // Function registry: register a JS fn (source string) → handle; invoke it.
      host_register_function(start, len, utf) {
        return functions.register(start, len, utf);
      },
      host_invoke_function(handle, pPtr, pLen, rPtr, rLen) {
        return functions.invoke(handle, pPtr, pLen, rPtr, rLen);
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
      host_invoke_function_as_i64: (h, p, l) => functions.invokeAsBigInt(h, p, l),
      host_unregister_function(handle) {
        functions.heap.delete(BigInt(handle));
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
    return this;
  }

  /** Register a protocol handler `{ apply(memoryId, payload) }` for a protocol byte. */
  setProtocolHandler(protocol, handler) {
    this.dispatcher.setHandler(protocol, handler);
    return this;
  }
}
