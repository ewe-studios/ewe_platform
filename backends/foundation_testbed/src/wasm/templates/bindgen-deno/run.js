import { WasmBindgenTestContext, wasm } from "./PACKAGE_NAME.js";

// --- Shared setup (mirrors wasm-bindgen-test-runner deno.rs) ---

const nocapture = true;
const handlers = {};

// wasm-bindgen-test prints its own `test result:` summary, and `Context::run`
// resolves the global object on this toolchain, not a boolean. The summary is
// therefore the verdict. Capture every line that carries it, whatever route
// prints it.
let lastSummary = null;
const note = (args) => {
    const text = args.map((arg) => String(arg)).join(" ");
    if (text.includes("test result:")) {
        lastSummary = text;
    }
};

const originalLog = console.log;

const wrap = method => {
    const og = console[method];
    const on_method = `on_console_${method}`;
    console[method] = function (...args) {
        note(args);
        if (nocapture) {
            og.apply(this, args);
        }
        if (handlers[on_method]) {
            handlers[on_method](args);
        }
    };
};

// The formatter reads this function at start and prints the final summary
// through it. Route it through `note`, so the summary is never missed.
globalThis.__wbgtest_og_console_log = function (...args) {
    note(args);
    originalLog.apply(console, args);
};

wrap("debug");
wrap("log");
wrap("info");
wrap("warn");
wrap("error");

const cx = new WasmBindgenTestContext();
handlers.on_console_debug = wasm.__wbgtest_console_debug;
handlers.on_console_log = wasm.__wbgtest_console_log;
handlers.on_console_info = wasm.__wbgtest_console_info;
handlers.on_console_warn = wasm.__wbgtest_console_warn;
handlers.on_console_error = wasm.__wbgtest_console_error;

globalThis.__wbg_test_invoke = f => f();
globalThis.__wbg_test_output_writeln = arg => {
    // Test output handler — just print to console
    console.log(arg);
};

// The suite calls its own endpoint (spec 88 invariant I5). Deno.serve picks a
// free port when the port is 0; `onListen` reports the address.
function base64(bytes) {
    let binary = "";
    const chunk = 0x8000;
    for (let index = 0; index < bytes.length; index += chunk) {
        binary += String.fromCharCode(...bytes.subarray(index, index + chunk));
    }
    return btoa(binary);
}

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

const listening = new Promise((resolve) => {
    globalThis.__fwt_echo_server = Deno.serve({
        port: 0,
        hostname: "127.0.0.1",
        onListen: ({ hostname, port }) => resolve(`http://${hostname}:${port}`),
    }, echo);
});
globalThis.__FWT_BASE_URL__ = await listening;

const tests = [/* TEST_NAMES */];
await cx.run(tests.map(name => wasm[name]));

await globalThis.__fwt_echo_server.shutdown();

// The runner reads the last `test result:` line. A suite that printed none is
// not a pass: print one red line, so the run can never look green.
if (lastSummary === null) {
    lastSummary = "test result: FAILED. runner error: the suite printed no summary";
    originalLog.call(console, lastSummary);
}
// The exit code repeats the summary verdict, so either one is trustworthy.
Deno.exit(lastSummary.includes("test result: ok") ? 0 : 1);
