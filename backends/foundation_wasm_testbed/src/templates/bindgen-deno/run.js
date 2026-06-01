import { WasmBindgenTestContext, wasm } from "./PACKAGE_NAME.js";

// --- Shared setup (mirrors wasm-bindgen-test-runner deno.rs) ---

const nocapture = true;
const handlers = {};

const wrap = method => {
    const og = console[method];
    const on_method = `on_console_${method}`;
    console[method] = function (...args) {
        if (nocapture) {
            og.apply(this, args);
        }
        if (handlers[on_method]) {
            handlers[on_method](args);
        }
    };
};

globalThis.__wbgtest_og_console_log = console.log;
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

console.log("test result: ok");
