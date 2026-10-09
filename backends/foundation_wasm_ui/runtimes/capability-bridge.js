// capability-bridge.js — portable invokeCapability() for WASM apps (F23).
//
// WHY: Capabilities (clipboard, camera, filesystem) need a single JS API that works
// on every host — Tauri, browser, Deno, testbed. The host determines the transport;
// the app code never knows the difference.
//
// WHAT: a global `invokeCapability(name, action, payload)` function.
//   - Tauri:    routes through __TAURI_INTERNALS__.invoke('__ewe_capabilities', ...)
//   - Browser:  direct WASM bridge (host_apply transport)
//   - Deno:     Deno.core.opAsync
//   - None:     throws a descriptive error
//
// HOW: exported as both ESM and globalThis.invokeCapability for classic <script>.

;(function () {
  'use strict';

  /**
   * Check if we're running inside a Tauri webview.
   * Tauri injects __TAURI_INTERNALS__ during webview creation.
   */
  function isTauri() {
    return typeof window !== 'undefined'
      && window.__TAURI_INTERNALS__
      && typeof window.__TAURI_INTERNALS__.invoke === 'function';
  }

  /**
   * Check if we're running inside Deno (worker or main).
   */
  function isDeno() {
    return typeof Deno !== 'undefined' && Deno.core && typeof Deno.core.opAsync === 'function';
  }

  /**
   * Tauri transport: invoke via __TAURI_INTERNALS__.invoke('__ewe_capabilities', ...).
   */
  // ── invokeIpc ────────────────────────────────────────────────────────
  // Same transport logic as invokeCapability, but for IPC invocations
  // through __ewe_ipc instead of __ewe_capabilities.

  async function tauriInvokeIpc(name, action, payload) {
    var jsonPayload = JSON.stringify(payload !== undefined ? payload : {});
    var result = await window.__TAURI_INTERNALS__.invoke('__ewe_ipc', {
      ipc: name,
      action: action,
      payload: Array.from(new TextEncoder().encode(jsonPayload)),
      content_type: 'application/json',
    });
    try { return JSON.parse(new TextDecoder().decode(new Uint8Array(result))); }
    catch (_) { return result; }
  }

  async function browserInvokeIpc(name, action, payload) {
    if (typeof FoundationWasm === 'undefined') {
      throw new Error('invokeIpc: no WASM runtime found.');
    }
    return FoundationWasm.host_apply(2, 0, new TextEncoder().encode(JSON.stringify({
      ipc: name, action: action,
      payload: payload !== undefined ? payload : {},
    })));
  }

  async function invokeIpc(name, action, payload) {
    if (!name || typeof name !== 'string') {
      throw new Error('invokeIpc: name must be a non-empty string');
    }
    if (!action || typeof action !== 'string') {
      throw new Error('invokeIpc: action must be a non-empty string');
    }
    if (isTauri()) return await tauriInvokeIpc(name, action, payload);
    if (isDeno()) return await denoInvokeIpc(name, action, payload);
    return await browserInvokeIpc(name, action, payload);
  }

  // ── invokeCapability ──────────────────────────────────────────────────

  async function tauriInvokeCapability(name, action, payload) {
    var jsonPayload = JSON.stringify(payload !== undefined ? payload : {});

    var result = await window.__TAURI_INTERNALS__.invoke('__ewe_capabilities', {
      capability: name,
      action: action,
      payload: jsonPayload,
    });

    try {
      return JSON.parse(result);
    } catch (_) {
      return result;
    }
  }

  /**
   * Deno transport: invoke via Deno.core.opAsync.
   */
  async function denoInvokeCapability(name, action, payload) {
    return await Deno.core.opAsync('ewe_capability', {
      capability: name,
      action: action,
      payload: payload !== undefined ? JSON.stringify(payload) : '{}',
    });
  }

  async function denoInvokeIpc(name, action, payload) {
    return await Deno.core.opAsync('ewe_ipc', {
      ipc: name,
      action: action,
      payload: payload !== undefined ? JSON.stringify(payload) : '{}',
    });
  }

  /**
   * Browser transport: invoke via the WASM bridge directly.
   * Falls back to throwing if the WASM runtime is not initialized.
   */
  async function browserInvokeCapability(name, action, payload) {
    if (typeof FoundationWasm === 'undefined') {
      throw new Error(
        'invokeCapability: no WASM runtime found. ' +
        'Capabilities require a WASM host (Tauri, Deno, or browser with FoundationWasm).'
      );
    }
    // Use the FoundationWasm bridge's host_apply transport.
    // Protocol byte 2 = JSON, memory_id 0, envelope wrapping.
    var jsonPayload = JSON.stringify({
      capability: name,
      action: action,
      payload: payload !== undefined ? payload : {},
    });
    // Delegate to FoundationWasm.host_apply(codec_id, memory_id, bytes)
    return FoundationWasm.host_apply(2, 0, new TextEncoder().encode(jsonPayload));
  }

  /**
   * Portable capability invocation.
   *
   * @param {string} name     - The registered capability name (e.g. "clipboard").
   * @param {string} action   - The action to perform (e.g. "read", "write").
   * @param {*}      [payload] - Optional action parameters.
   * @returns {Promise<*>}    - The capability response (deserialized).
   */
  async function invokeCapability(name, action, payload) {
    if (!name || typeof name !== 'string') {
      throw new Error('invokeCapability: name must be a non-empty string');
    }
    if (!action || typeof action !== 'string') {
      throw new Error('invokeCapability: action must be a non-empty string');
    }

    if (isTauri()) {
      return await tauriInvokeCapability(name, action, payload);
    }

    if (isDeno()) {
      return await denoInvokeCapability(name, action, payload);
    }

    // Browser or other host
    return await browserInvokeCapability(name, action, payload);
  }

  // F27: Register capability and IPC trigger handlers with the FoundationWasm
  // runtime. Called by WASM app startup code after setting up TriggerRegistry.
  function registerTriggerHandlers(onCapability, onIpc) {
    if (typeof FoundationWasm === 'undefined' || !FoundationWasm.prototype) {
      console.warn('registerTriggerHandlers: FoundationWasm runtime not loaded');
      return;
    }
    // FoundationWasm.triggerCapability / triggerIpc are called by the host.
    // Register callbacks that forward to the WASM-side TriggerRegistry.
    if (onCapability) {
      FoundationWasm.prototype._capTriggerHandler = onCapability;
    }
    if (onIpc) {
      FoundationWasm.prototype._ipcTriggerHandler = onIpc;
    }
  }

  // ── Export ──────────────────────────────────────────────────────────────

  // ESM binding
  if (typeof globalThis !== 'undefined') {
    globalThis.invokeCapability = invokeCapability;
    globalThis.invokeIpc = invokeIpc;
  }

  // Classic global
  if (typeof window !== 'undefined') {
    window.invokeCapability = invokeCapability;
    window.invokeIpc = invokeIpc;
    window.registerTriggerHandlers = registerTriggerHandlers;
  }

  // For module consumers
  if (typeof exports !== 'undefined') {
    exports.invokeCapability = invokeCapability;
    exports.invokeIpc = invokeIpc;
    exports.registerTriggerHandlers = registerTriggerHandlers;
  }
})();
