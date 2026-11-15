import { WasmBindgenTestContext, wasm } from "./PACKAGE_NAME.js";

let cx = null;
let consoleWrapped = false;

// wasm-bindgen-test prints its own `test result:` summary, and `Context::run`
// resolves the global object on this toolchain, not a boolean. The summary is
// the verdict, so capture every line that carries it.
let lastSummary = null;
const note = (args) => {
  const text = args.map((arg) => String(arg)).join(" ");
  if (text.includes("test result:")) {
    lastSummary = text;
  }
};

function wrapConsoleOnce() {
  if (consoleWrapped) return;
  consoleWrapped = true;
  const originalLog = console.log;
  globalThis.__wbgtest_og_console_log = function (...args) {
    note(args);
    originalLog.apply(console, args);
  };
  console.log = function (...args) {
    note(args);
    originalLog.apply(console, args);
  };
}

export default {
  async fetch(request, env, ctx) {
    // Readiness probe: answer before the tests run.
    if (new URL(request.url).pathname === "/health") {
      return new Response("ok");
    }
    // Hand the real D1/R2/KV bindings to the wasm tests. A test reads them
    // from `globalThis.__CF_TEST_ENV__` and passes `env.DB`, `env.BUCKET` or
    // `env.KV` to the store it builds.
    globalThis.__CF_TEST_ENV__ = env;

    if (!cx) {
      // wasm-bindgen-test's worker formatter and poll loop call these host
      // functions. The official runner injects them; the testbed defines them
      // here (output goes to workerd's console log).
      wrapConsoleOnce();
      globalThis.__wbg_test_output_writeln = (line) => console.log(line);
      globalThis.__wbg_test_invoke = (callback) => callback();
      cx = new WasmBindgenTestContext();
    }
    // The module target instantiates the wasm at import time; the runner
    // patches its JS to export that instance as `wasm`, and the test names
    // are raw wasm exports on it.
    const names = [/* TEST_NAMES */];
    lastSummary = null;
    await cx.run(names.map((name) => wasm[name]));
    const passed = lastSummary !== null && lastSummary.includes("test result: ok");
    // Always answer 200: a failing test is a result, not a worker error. (A
    // 500 goes through wrangler's error middleware, whose re-encoded response
    // is not what the runner reads.)
    return new Response(JSON.stringify({ passed }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  },
};
