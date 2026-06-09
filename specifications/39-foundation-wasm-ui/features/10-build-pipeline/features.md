# Feature 10: Build Pipeline CLI

**CLI binary:** `ewe-wasm build` (part of `foundation_codegentools`)
**Generation logic:** `WasmBundleGenerator` in `foundation_codegen`
**Proc macros:** `#[wasm_bin]`, `#[wasm_worker]`, `#[wasm_service]` in `foundation_macros`
**Decisions:** 014 (execution modes), 016 (build pipeline), 017 (service worker uses WebServe)

## Architecture

Three layers:

```
┌─────────────────────────────────────────────────┐
│ User writes (convenience):                       │
│   #[wasm_bin(js = single-file, encoded = b64)]   │
│   fn auth_app() { ... }                          │
└──────────────────────┬──────────────────────────┘
                       │ proc macro expansion
┌──────────────────────▼──────────────────────────┐
│ Expands to (foundation_codegen marker):          │
│   #[wasm_entrypoint(                             │
│     name = "auth_app",                           │
│     desc = "...",                                │
│     metadata = {                                 │
│       target = "wasm_bin",                       │
│       js = "single-file", encoded = "b64"        │
│     }                                            │
│   )]                                             │
└──────────────────────┬──────────────────────────┘
                       │ scanned at build time
┌──────────────────────▼──────────────────────────┐
│ WasmBundleGenerator (foundation_codegen):        │
│   1. Wraps WasmBinGenerator → discovers          │
│      entrypoints, generates WASM binaries        │
│   2. For each WASM binary, generates JS wrappers │
│      + single-file bundles                       │
│   3. Copies JS runtime assets                    │
└──────────────────────┬──────────────────────────┘
                       │ invoked by CLI
┌──────────────────────▼──────────────────────────┐
│ ewe-wasm build CLI (foundation_codegentools):    │
│   Orchestrates: cargo build → WASM gen → JS gen  │
│   → output to build/                             │
└──────────────────────────────────────────────────┘
```

## 1. Proc Macros (convenience layer)

Three convenience attributes that expand to `#[wasm_entrypoint]`:

```rust
// User writes:
#[wasm_bin(js = single-file, encoded = b64)]
fn auth_app() { ... }

#[wasm_worker(js = separate)]
fn data_worker() { ... }

#[wasm_service(routes = ["/api/auth"], js = single-file)]
fn api_service() { ... }

// Each expands to:
#[wasm_entrypoint(
    name = "auth_app",
    desc = "Auth application",
    metadata = { target = "wasm_bin", js = "single-file", encoded = "b64" }
)]
fn auth_app() { ... }
```

The proc macros validate the syntax and pass through to `#[wasm_entrypoint]`. All real work is done by `foundation_codegen`.

## 2. WasmBundleGenerator (in foundation_codegen)

Wraps the existing `WasmBinGenerator` and adds JS bundling:

```rust
use system_operations::WasmBinGenerator;

pub struct WasmBundleGenerator {
    inner: WasmBinGenerator,        // foundation_codegen - WASM binary discovery + generation
    js_assets: JsAssetCollection,   // JS runtime files to bundle
    output_dir: PathBuf,
}

impl WasmBundleGenerator {
    pub fn new(crate_dir: &Path, js_dir: &Path, output_dir: &Path) -> Result<Self, Error>;
    pub fn plan(&self) -> Result<WasmBundlePlan, Error>;   // dry-run
    pub fn execute(&self) -> Result<WasmBundleOutput, Error>;  // generate everything
}
```

**execute() flow:**
1. Call `inner.generate()` — discovers entrypoints, generates `bin/*/main.rs`, updates `Cargo.toml`
2. Build WASM: `cargo build --target wasm32-unknown-unknown`
3. For each entrypoint, generate JS wrapper per metadata (bin/worker/service)
4. If `js = single-file`, bundle WASM bytes into JS (b64 or uint8array)
5. Copy `foundation-wasm.js` and `foundation-wasm-ui.js` to output

## 3. CLI Interface

```
ewe-wasm build [OPTIONS]

OPTIONS:
  --release              Compile WASM with optimizations
  --dev                  Compile WASM in debug mode (default)
  --output <DIR>         Output directory (default: build/)
  --target <CRATE>       Cargo package to build
  --skip-runtimes        Do not copy JS runtime assets
  --verbose              Print each build step
```

**Build steps:**
1. `WasmBundleGenerator::new()` — scan crate, discover entrypoints
2. `generator.execute()` — generates WASM + JS wrappers + bundles
3. Copy output to `--output` directory

## 4. JS Wrapper Generation Per Mode

### `wasm_bin` — Main Thread
```javascript
// {name}.js — generated
import { FoundationWasm } from './foundation-wasm.js';
import { FoundationWasmUi } from './foundation-wasm-ui.js';

const WASM_URL = './{name}.wasm';

export async function init(importOverrides = {}) {
    const response = await fetch(WASM_URL);
    const wasmBytes = await response.arrayBuffer();
    const runtime = new FoundationWasm();
    const ui = new FoundationWasmUi(runtime);
    const imports = runtime.buildImportObject(importOverrides);
    const { instance } = await WebAssembly.instantiate(wasmBytes, imports);
    runtime.bind(instance.exports);
    ui.bind(instance.exports);
    instance.exports.__ewe_bin_invoke(0, 0, 0);
    return { runtime, ui, instance };
}
```

### `wasm_worker` — Web Worker
Generates `{name}-worker.js` (worker side) and `{name}-worker-host.js` (main thread side).

### `wasm_service` — Service Worker
Generates `{name}-sw.js` with fetch interception + route table from metadata.

## 5. Single-File Bundling

When `js = single-file`, WASM bytes are embedded into JS:
- `encoded = b64` — base64 string + atob decoder
- `encoded = uint8array` (default) — Uint8Array literal

## 6. File Ownership

```
foundation_codegen/
├── src/
│   ├── wasm_bundle/
│   │   ├── mod.rs              // WasmBundleGenerator
│   │   ├── generator.rs        // WASM + JS generation orchestration
│   │   ├── js_wrapper.rs       // JS wrapper templates per mode
│   │   └── bundler.rs          // single-file bundling (b64 + uint8array)

foundation_macros/
├── src/
│   ├── wasm_bin.rs             // #[wasm_bin] → #[wasm_entrypoint]
│   ├── wasm_worker.rs          // #[wasm_worker] → #[wasm_entrypoint]
│   └── wasm_service.rs         // #[wasm_service] → #[wasm_entrypoint]

foundation_codegentools/
├── src/
│   ├── ewe_wasm.rs             // ewe-wasm build CLI
```

## 7. Testing

### CLI Arguments (tests 1-5)
| # | Scenario | Verify |
|---|----------|--------|
| 1 | `ewe-wasm build --release` | Cargo invoked with `--release`. Output in `build/`. |
| 2 | `ewe-wasm build --dev --output dist/` | Cargo without `--release`. Output in `dist/`. |
| 3 | `ewe-wasm build --release --dev` | CLI exits with error: conflicting flags. |
| 4 | `ewe-wasm build --skip-runtimes` | No JS runtime files in output. |
| 5 | `ewe-wasm build --target my-crate` | Cargo invoked with `-p my-crate`. |

### Proc Macro Expansion (tests 6-8)
| # | Scenario | Verify |
|---|----------|--------|
| 6 | `#[wasm_bin(js = single-file, encoded = b64)]` | Expands to correct `#[wasm_entrypoint(...)]` |
| 7 | `#[wasm_worker]` | Expands with `target = "wasm_worker"` |
| 8 | `#[wasm_service(routes = [...])]` | Expands with routes in metadata |

### JS Wrapper Generation (tests 9-14)
| # | Scenario | Verify |
|---|----------|--------|
| 9 | `wasm_bin` wrapper generated | Contains `WebAssembly.instantiate`, `fetch()`, init function |
| 10 | `wasm_worker` wrapper | Worker file + host file generated |
| 11 | `wasm_service` wrapper | Contains fetch listener + route table |
| 12 | `encoded = uint8array` bundle | Contains `new Uint8Array([...])`, no fetch |
| 13 | `encoded = b64` bundle | Contains base64 + atob decoder, no fetch |
| 14 | Bundle self-instantiates | Loads without network fetch |

### Integration (tests 15-18)
| # | Scenario | Verify |
|---|----------|--------|
| 15 | Full pipeline: annotate → build → generate | WASM + JS files in output dir |
| 16 | Worker host+worker communicate | postMessage round-trip works |
| 17 | Service worker intercepts matching fetch | Intercepts `/api/*`, passes through `/static/*` |
| 18 | Multiple annotations in one crate | All wrapper sets generated, single WASM shared |
