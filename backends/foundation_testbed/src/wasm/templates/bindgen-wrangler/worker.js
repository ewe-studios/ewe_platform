import { WasmBindgenTestContext, wasm } from "./PACKAGE_NAME.js";

let cx = null;
let consoleWrapped = false;

// wasm-bindgen-test prints its own `test result:` summary, and `Context::run`
// resolves the global object on this toolchain, not a boolean. The summary is
// the verdict, so capture every line that carries it.
let lastSummary = null;
const outputLines = [];
const note = (args) => {
  const text = args.map((arg) => String(arg)).join(" ");
  outputLines.push(text);
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
  // The panic hook writes through console.error, so capture every level.
  for (const level of ["log", "info", "warn", "error", "debug"]) {
    const original = console[level].bind(console);
    console[level] = function (...args) {
      note(args);
      original(...args);
    };
  }
}

// Base64 of a byte array, in chunks so a large body never overflows the stack.
function base64(bytes) {
  let binary = "";
  const chunk = 0x8000;
  for (let index = 0; index < bytes.length; index += chunk) {
    binary += String.fromCharCode(...bytes.subarray(index, index + chunk));
  }
  return btoa(binary);
}

// The suite calls the worker's own origin (spec 88 invariant I5). workerd
// permits a subrequest to the dev server's own origin: measured 2026-11-16,
// the echo case answered 200 inside `wrangler dev --local`. The origin arrives
// in a header because workerd gives the worker a URL without the port.
async function echo(request) {
  const bytes = new Uint8Array(await request.arrayBuffer());
  if (bytes.length > 1024 * 1024) {
    return new Response("echo body too large", { status: 413 });
  }
  const headers = {};
  for (const [name, value] of request.headers) {
    headers[name] = value;
  }
  return Response.json({
    method: request.method,
    path: new URL(request.url).pathname,
    headers,
    body: base64(bytes),
    bytes: bytes.length,
  });
}

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    // Readiness probe and echo answer before the tests run.
    if (url.pathname === "/health") {
      return new Response("ok");
    }
    if (url.pathname === "/echo") {
      return echo(request);
    }
    // The runner passes the origin in a header: workerd hands the worker a URL
    // without the port. Fall back to the request origin when it is absent.
    globalThis.__FWT_BASE_URL__ = request.headers.get("x-fwt-base-url") ?? url.origin;
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
    outputLines.length = 0;
    await cx.run(names.map((name) => wasm[name]));
    const passed = lastSummary !== null && lastSummary.includes("test result: ok");
    // Always answer 200: a failing test is a result, not a worker error. (A
    // 500 goes through wrangler's error middleware, whose re-encoded response
    // is not what the runner reads.)
    return new Response(JSON.stringify({ passed, summary: lastSummary, output: outputLines.join("\n") }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  },
};
