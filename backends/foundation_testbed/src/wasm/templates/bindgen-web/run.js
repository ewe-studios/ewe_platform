import init, { WasmBindgenTestContext, __wbgtest_console_log } from "./PACKAGE_NAME.js";

const wasm = await init();
const cx = new WasmBindgenTestContext();
window.on_console_log = __wbgtest_console_log;

const tests = [/* TEST_NAMES */];
await cx.run(tests.map(name => wasm[name]));

document.getElementById("output").textContent = "test result: ok";
