import { instantiate } from "./loader.js";

const wasm = await instantiate("PACKAGE_NAME");
// Add your test assertions here

console.log("Tests complete");
