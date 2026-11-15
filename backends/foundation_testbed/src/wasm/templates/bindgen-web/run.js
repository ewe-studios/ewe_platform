import init, { WasmBindgenTestContext, __wbgtest_console_log } from "./PACKAGE_NAME.js";

const wasm = await init();
const cx = new WasmBindgenTestContext();
window.on_console_log = __wbgtest_console_log;

// wasm-bindgen-test's poll loop and output writer call these host functions.
// The official browser runner injects them; the testbed defines them here.
globalThis.__wbg_test_invoke = (callback) => callback();
globalThis.__wbg_test_output_writeln = (line) => console.log(line);

// The testbed serves /echo next to this page, so the page's own origin is the
// endpoint (spec 88 invariant I5).
globalThis.__FWT_BASE_URL__ = location.origin;

const tests = [/* TEST_NAMES */];
await cx.run(tests.map(name => wasm[name]));

// wasm-bindgen-test's browser formatter writes every result line, including the
// final `test result:` summary, into #output. The browser runner reads the last
// such line as the verdict. A suite that printed none is not a pass: append one
// red line, so the run can never look green.
const output = document.getElementById("output");
if (!output.textContent.includes("test result:")) {
    output.textContent += "\ntest result: FAILED. runner error: the suite printed no summary";
}
