# Feature 12: Build Pipeline CLI

## Description

`ewe-wasm build` CLI binary in `foundation_wasm_ui` that orchestrates the full build pipeline: compiles WASM, collects JS runtimes, invokes `foundation_codegen` for bundling. Proc macro annotations (`#[wasm_bin]`, `#[wasm_worker]`, `#[wasm_service]`) mark entry points — the CLI reads them and generates output.

**Decision:** 016

## Crate

`crates/foundation_codegen/` (CLI binary + bundler logic)

## CLI

```bash
ewe-wasm build --release
ewe-wasm build --dev
ewe-wasm build --target wasm32-unknown-unknown
```

## What the CLI does

1. **`cargo build --target wasm32-unknown-unknown`** — compiles WASM binary
2. **Collects JS runtimes** — `foundation-wasm.js` + `foundation-wasm-ui.js`
3. **Reads proc macro annotations** — which macros want which output format
4. **Generates output** in `build/`:

```
build/
├── my-app.wasm              # raw WASM binary
├── my-app.js                # standalone JS wrapper
├── my-app.bundle.js         # single-file: WASM as Uint8Array or b64
├── foundation-wasm.js       # runtime
└── foundation-wasm-ui.js    # runtime
```

## Proc macro annotations

```rust
#[wasm_bin]                    // regular WASM
#[wasm_bin(js=single-file)]    // bundled JS output
#[wasm_bin(js=single-file, encoded=b64)]

#[wasm_worker]                 // web worker WASM
#[wasm_worker(js=single-file, encoded=uint8array)]

#[wasm_service]                // service worker WASM
#[wasm_service(js=single-file, encoded=b64)]
```

## Single-file bundling

```javascript
// my-app.bundle.js
const WASM_BYTES = new Uint8Array([0x00, 0x61, 0x73, 0x6d, ...]);  // or b64
const instance = await WebAssembly.instantiate(WASM_BYTES, imports);
```

`<script src="my-app.bundle.js"></script>` — no separate `.wasm` file.

## `application/wasm` script tag

```html
<script type="application/wasm" src="my-app.wasm"></script>
<script src="foundation-wasm.js"></script>
<script>
  FoundationWasm.autoInstantiate();
</script>
```

Runtime scans for `<script type="application/wasm">`, instantiates, wires imports.

## Dependencies

- Feature 06 (JS runtime assets to bundle)
- Feature 07 (JS runtime assets to bundle)

## Testing

- CLI: `ewe-wasm build` → all output files generated
- Single-file: bundle.js loads and instantiates WASM
- Standalone: .js + .wasm load correctly
- application/wasm: auto-discover and instantiate
