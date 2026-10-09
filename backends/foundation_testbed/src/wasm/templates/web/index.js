import { instantiate } from "./loader.js";

const wasm = await instantiate("PACKAGE_NAME");
// Add your test assertions here
// wasm.main(); or call exported functions directly

document.getElementById("output").textContent = "Tests complete";
