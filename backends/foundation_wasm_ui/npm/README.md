# Ewe browser runtime

Install `@ewe-studios/foundation-wasm-ui` at the version of your Rust `foundation_wasm_ui` crate.

```js
import { WasmLoader, createApp, registerFrameApp } from "@ewe-studios/foundation-wasm-ui";
```

Use the module entry in a browser bundler. Supply your application's WebAssembly module and generated bindings.

For a classic script, copy `bundle.js` to your public directory and load it before your application.
Use `bundle.min.js` for the smaller deployment file. Both install `FoundationUiBootstrap` and `FoundationWasmRuntime` on the browser global object.
