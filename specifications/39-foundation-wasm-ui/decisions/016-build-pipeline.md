# 016 — Build Pipeline: CLI bundler with foundation_codegen

**Date:** 2026-06-08  
**Status:** Resolved

### Decision

Proc macros can't generate JS files (TokenStream is Rust-only). Instead, a **CLI bundler** binary in `foundation_wasm_ui` handles the full build pipeline. It uses `foundation_codegen` to produce the bundled output.

### How it works

```rust
#[wasm_bin]                    // regular WASM
#[wasm_bin(js=single-file)]    // bundled JS output
#[wasm_bin(js=single-file, encoded=b64)]

#[wasm_worker]                 // web worker WASM
#[wasm_worker(js=single-file, encoded=uint8array)]

#[wasm_service]                // service worker WASM
#[wasm_service(js=single-file, encoded=b64)]
```

The CLI orchestrates the build:

```bash
# CLI entrypoint in foundation_wasm_ui
ewe-wasm build --release
```

### What the CLI does

1. **`cargo build --target wasm32-unknown-unknown`** — compiles the WASM binary
2. **Collects JS runtimes** — `foundation-wasm.js` + `foundation-wasm-ui.js`
3. **Generates output** — in `build/` directory:

```
build/
├── my-app.wasm              # raw WASM binary
├── my-app.js                # standalone JS wrapper (loads .wasm separately)
├── my-app.bundle.js         # single-file: WASM bundled as Uint8Array or b64
├── foundation-wasm.js       # runtime (standalone, reusable)
└── foundation-wasm-ui.js    # runtime (standalone, reusable)
```

### Single-file bundling

When `js=single-file` is specified, the WASM binary is embedded into the JS file:

```javascript
// my-app.bundle.js
const WASM_BYTES = new Uint8Array([0x00, 0x61, 0x73, 0x6d, ...]);  // or b64
const instance = await WebAssembly.instantiate(WASM_BYTES, imports);
```

This allows `<script src="my-app.bundle.js"></script>` — no separate `.wasm` file needed.

### `application/wasm` script tag support

Like web-rs, the runtime can auto-discover and instantiate WASM:

```html
<script type="application/wasm" src="my-app.wasm"></script>
<script src="foundation-wasm.js"></script>
<script>
  // Runtime scans for <script type="application/wasm">,
  // instantiates them, and wires up the import objects
  FoundationWasm.autoInstantiate();
</script>
```
