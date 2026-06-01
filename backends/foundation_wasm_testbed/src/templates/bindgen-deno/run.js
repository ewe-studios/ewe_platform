import init, { WasmBindgenTestContext } from "./PACKAGE_NAME.js";

const wasm = await init();
const cx = new WasmBindgenTestContext();

const tests = [/* TEST_NAMES */];
await cx.run(tests.map(name => wasm[name]));

console.log("test result: ok");
