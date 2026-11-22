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

// ── the Connect endpoint (spec 88 feature 03, issue #752) ───────────────────
// This host starts no Rust server, so the worker answers the real Connect
// request itself. A subrequest to the worker's own origin re-enters `fetch`,
// so `/connect` answers before the test run. The three procedures mirror the
// Rust `/connect` route in `foundation_testbed::src::wasm::server.rs` byte for
// byte: a unary echo, an `invalid_argument` error, and a three-frame server
// stream. This is no stub: the body, the status and the Connect envelope bytes
// are the routed answer.
const encoder = new TextEncoder();

/** One Connect envelope: a flags byte, a big-endian u32 length, then the payload. */
function connectEnvelope(flags, payload) {
  const out = new Uint8Array(5 + payload.length);
  out[0] = flags;
  new DataView(out.buffer).setUint32(1, payload.length);
  out.set(payload, 5);
  return out;
}

/** Concatenate byte arrays into one `Uint8Array`. */
function concatBytes(parts) {
  const total = parts.reduce((sum, part) => sum + part.length, 0);
  const out = new Uint8Array(total);
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}

async function connect(request) {
  const url = new URL(request.url);
  const procedure = url.pathname.slice("/connect/".length);
  if (procedure === "case.CaseService/Unary") {
    const body = new Uint8Array(await request.arrayBuffer());
    const out = concatBytes([encoder.encode("echo:"), body]);
    return new Response(out, {
      status: 200,
      headers: { "content-type": "application/proto" },
    });
  }
  if (procedure === "case.CaseService/Fail") {
    await request.arrayBuffer();
    return new Response(
      JSON.stringify({ code: "invalid_argument", message: "the handler rejected the call" }),
      { status: 400, headers: { "content-type": "application/json" } },
    );
  }
  if (procedure === "case.CaseService/Stream") {
    // The request holds one Connect envelope; the procedure ignores its message.
    await request.arrayBuffer();
    const body = concatBytes([
      connectEnvelope(0, encoder.encode("frame-0")),
      connectEnvelope(0, encoder.encode("frame-1")),
      connectEnvelope(0, encoder.encode("frame-2")),
      // The Connect EndStream frame: an empty EndStreamResponse.
      connectEnvelope(2, encoder.encode("{}")),
    ]);
    return new Response(body, {
      status: 200,
      headers: { "content-type": "application/connect+proto" },
    });
  }
  return new Response("not found", { status: 404 });
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

// A Durable Object for the netio `StubHttpClient` test (spec 85 T5). It
// echoes the request body, so the test proves the stub round trip.
export class TestDurableObject {
  constructor(state, env) {
    this.state = state;
    this.env = env;
  }

  async fetch(request) {
    const body = await request.text();
    return new Response(body, {
      status: 200,
      headers: { "content-type": "text/plain" },
    });
  }
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
    // A Connect subrequest re-enters this worker. Answer before the run starts,
    // so the probe never runs the tests a second time.
    if (url.pathname.startsWith("/connect/")) {
      return connect(request);
    }
    // The runner passes the origin in a header: workerd hands the worker a URL
    // without the port. Fall back to the request origin when it is absent.
    globalThis.__FWT_BASE_URL__ = request.headers.get("x-fwt-base-url") ?? url.origin;
    // The same origin answers the Connect endpoint, so the transport cases call
    // it here (spec 88 feature 03 T5).
    globalThis.__FWT_RPC_BASE_URL__ = globalThis.__FWT_BASE_URL__;
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
