import { instantiate } from "./loader.js";
let wasm = null;

export default {
  async fetch(request, env, ctx) {
    if (new URL(request.url).pathname === "/health") {
      return new Response("ok");
    }
    // Hand the real D1/R2/KV bindings to the wasm code: read them from
    // `globalThis.__CF_TEST_ENV__`.
    globalThis.__CF_TEST_ENV__ = env;
    if (!wasm) {
      wasm = await instantiate("PACKAGE_NAME", ctx);
    }
    wasm.main();
    return new Response("Tests complete", { status: 200 });
  },
};
