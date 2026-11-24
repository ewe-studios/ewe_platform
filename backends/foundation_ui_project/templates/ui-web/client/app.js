import { moduleUrl } from "./ewe-app-config.js";
import { WasmLoader } from "./foundation-wasm.js";
import { createBindings, importName } from "./bindings/ewe_browser_app_bg.js";

async function start() {
  const bindings = createBindings();
  const loader = new WasmLoader({environment: {[importName]: bindings}});
  await loader.loadURL(moduleUrl);
  bindings.__wbg_set_wasm(loader.module.instance.exports);
  const button = document.querySelector("button");
  const count = document.querySelector("output");
  button.addEventListener("click", () => { count.value = String(bindings.increment(Number(count.value))); });
  button.disabled = false;
}
start().catch(error => { document.querySelector("output").value = error.message; });
