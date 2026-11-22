// Connect an instantiated WebAssembly app to the browser frame reader.
import { createApp } from "./foundation-ui-actions.js";
export { createApp };
import * as ui from "./foundation-wasm-ui.js";
import { installFrameRuntime } from "./foundation-ui-frames.js";

export function registerFrameApp(runtime, options = {}) {
  const frames = installFrameRuntime(ui, options);
  ui.registerWasmApp(runtime, options);
  ui.registerWebComponents();
  return frames;
}
globalThis.FoundationUiBootstrap = { createApp, registerFrameApp };
