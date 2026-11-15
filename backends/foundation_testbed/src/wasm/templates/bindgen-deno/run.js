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

const tests = [/* TEST_NAMES */];
await cx.run(tests.map(name => wasm[name]));

// The runner reads the last `test result:` line. A suite that printed none is
// not a pass: print one red line, so the run can never look green.
if (lastSummary === null) {
    lastSummary = "test result: FAILED. runner error: the suite printed no summary";
    originalLog.call(console, lastSummary);
}
// The exit code repeats the summary verdict, so either one is trustworthy.
Deno.exit(lastSummary.includes("test result: ok") ? 0 : 1);
