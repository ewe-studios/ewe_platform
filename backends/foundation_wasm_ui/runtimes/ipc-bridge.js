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
          // Tauri: encode as JSON, send via __ewe_ipc
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

  export { invokeIpc, registerIpcTriggers };
})();
