import init, { WasmBindgenTestContext } from "./PACKAGE_NAME.js";
import wasmModule from "./PACKAGE_NAME_bg.wasm";

let cx = null;

export default {
  async fetch(request, env, ctx) {
    if (!cx) {
      const wasm = await init(wasmModule);
      cx = new WasmBindgenTestContext();
    }
    const results = await cx.run(/* TEST_NAMES */);
    return new Response(JSON.stringify(results), {
      headers: { "Content-Type": "application/json" },
    });
  },
};
