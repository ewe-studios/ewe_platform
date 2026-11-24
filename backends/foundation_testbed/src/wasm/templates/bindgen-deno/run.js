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

// ── the Connect endpoint (spec 88 feature 03, issue #752) ───────────────────
// This host starts no Rust server, so it answers the real Connect request here.
// The three procedures mirror the Rust `/connect` route in
// `foundation_testbed::src::wasm::server.rs` byte for byte: a unary echo, an
// `invalid_argument` error, and a three-frame server stream. This is no stub:
// the body, the status and the Connect envelope bytes are the routed answer.
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

async function echo(request) {
    const url = new URL(request.url);

    // A Connect call answers before the echo catch-all.
    if (url.pathname.startsWith("/connect/")) {
        return connect(request);
    }

    // `/stream?chunks=N&size=M&status=S` serves a streaming body of N chunks of
    // M bytes each. The wasm fetch client tests use it to prove that a response
    // body streams, that a slow consumer bounds the read-ahead, and that
    // `max_body_size` stops the transfer.
    if (url.pathname === "/stream") {
        const chunks = Number(url.searchParams.get("chunks") ?? "1");
        const size = Number(url.searchParams.get("size") ?? "8");
        const status = Number(url.searchParams.get("status") ?? "200");
        let sent = 0;
        const body = new ReadableStream({
            pull(controller) {
                if (sent >= chunks) {
                    controller.close();
                    return;
                }
                sent += 1;
                controller.enqueue(new Uint8Array(size).fill(120));
            },
        });
        return new Response(body, {
            status,
            headers: { "content-type": "application/octet-stream" },
        });
    }

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
        path: url.pathname,
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
// The same origin answers the Connect endpoint, so the transport cases call it
// here (spec 88 feature 03 T5).
globalThis.__FWT_RPC_BASE_URL__ = globalThis.__FWT_BASE_URL__;

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
