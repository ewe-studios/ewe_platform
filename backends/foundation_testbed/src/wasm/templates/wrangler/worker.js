import { instantiate } from "./loader.js";
let wasm = null;

export default {
  async fetch(request, env, ctx) {
    if (!wasm) {
      wasm = await instantiate("PACKAGE_NAME", ctx);
    }
    wasm.main();
    return new Response("Tests complete", { status: 200 });
  },
};
