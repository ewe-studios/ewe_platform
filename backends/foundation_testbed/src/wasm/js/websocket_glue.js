// F51: publish the `WebSocket` global for the embedded test runtime.
//
// deno_websocket ships `01_websocket.js` as `lazy_loaded_esm` (an ES module), not
// `lazy_loaded_js`, so it cannot be pulled in via `Deno.core.loadExtScript` the way
// the deno_web/deno_fetch globals are. Instead this ES module is registered as an
// `esm_entry_point`, so it is evaluated at runtime init: it imports the real
// WebSocket class from the extension module and assigns it to `globalThis`.
//
// NOTE: deno_core requires extension code to be 7-bit ASCII (no smart punctuation).
import { WebSocket } from "ext:deno_websocket/01_websocket.js";

Object.defineProperty(globalThis, "WebSocket", {
  value: WebSocket,
  writable: true,
  enumerable: false,
  configurable: true,
});
