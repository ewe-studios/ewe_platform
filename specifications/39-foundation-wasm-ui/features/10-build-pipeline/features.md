# Feature 10: Build Pipeline CLI

**Crate path:** `crates/foundation_codegen/` (CLI binary `ewe-wasm` + bundler logic)
**Decisions:** 014 (execution modes), 016 (build pipeline), 017 (service worker uses WebServe)
**Constraint:** Proc macros cannot generate JS files (TokenStream is Rust-only). The CLI handles all JS wrapper generation, WASM embedding, and runtime collection.

`ewe-wasm build` CLI binary that orchestrates the full build pipeline: compiles WASM, collects JS runtimes, reads proc macro annotations from compiled WASM exports, generates JS wrappers per execution mode (`#[wasm_bin]`, `#[wasm_worker]`, `#[wasm_service]`), and optionally bundles WASM bytes into single-file JS outputs.

---

## 1. CLI Interface

```
ewe-wasm build [OPTIONS]

OPTIONS:
  --release              Compile WASM with optimizations (--release to cargo)
  --dev                  Compile WASM in debug mode (default)
  --output <DIR>         Output directory (default: build/)
  --target <CRATE>       Cargo package to build (default: current workspace member)
  --skip-runtimes        Do not copy foundation-wasm.js / foundation-wasm-ui.js
  --verbose              Print each build step as it executes

ENVIRONMENT VARIABLES:
  EWE_WASM_JS_DIR        Override path to JS runtime assets directory
  CARGO_TARGET_DIR       Override cargo target directory (default: target/)
```

`--release` and `--dev` are mutually exclusive. If neither is specified, `--dev` is the default. `--output` creates the directory if it does not exist. `--target` accepts a crate name, not a path.

---

## 2. Build Steps Algorithm

The CLI executes these steps sequentially. Each step receives inputs from the previous step.

### Step 1: Compile WASM

**Command:** `cargo build --target wasm32-unknown-unknown [--release]`
**Input:** Cargo workspace, `--target` crate name, `--release`/`--dev` flag.
**Output:** `target/wasm32-unknown-unknown/{release|debug}/{crate_name}.wasm`
**Failure:** If cargo returns non-zero exit code, CLI prints the full compiler output to stderr and exits with code 1. No subsequent steps run.

### Step 2: Collect JS Runtimes

**Input:** `EWE_WASM_JS_DIR` or default `assets/` directory relative to workspace root.
**Action:** Locate `foundation-wasm.js` and `foundation-wasm-ui.js`. If `--skip-runtimes` is set, skip this step entirely.
**Output:** Paths to both JS files held in memory for copying in Step 5.
**Failure:** If either file is missing and `--skip-runtimes` is not set, CLI exits with `error: runtime file not found: {path}`.

### Step 3: Discover Proc Macro Annotations

**Input:** Compiled `.wasm` binary from Step 1.
**Action:** Parse the WASM binary's export section to discover annotated entry points. The proc macros (`#[wasm_bin]`, `#[wasm_worker]`, `#[wasm_service]`) generate exports with known prefixes:

| Prefix | Macro | Meaning |
|--------|-------|---------|
| `__ewe_bin_invoke` | `#[wasm_bin]` | Main-thread entry point |
| `__ewe_worker_invoke` | `#[wasm_worker]` | Web Worker entry point |
| `__ewe_service_invoke` | `#[wasm_service]` | Service Worker entry point |
| `__ewe_extract_routes` | `#[wasm_service]` | Route list export for service workers |
| `__ewe_meta_{name}` | any | JSON-encoded annotation options (js, encoded) |

The CLI reads the WASM binary's export table using `wasmparser`, iterates exports, and collects all `__ewe_*` prefixed function names. For each `__ewe_meta_{name}` export, the CLI instantiates the WASM module in a minimal runtime (wasmtime) and calls the export to retrieve the JSON-encoded options string.

**Output:** A list of `AnnotatedEntry { name, mode, options }` structs.
**Failure:** If zero `__ewe_*` exports are found, CLI prints `warning: no proc macro annotations found, generating default wasm_bin output` and proceeds with a default `wasm_bin` entry using the crate name.

### Step 4: Generate JS Wrappers Per Mode

**Input:** Annotation list from Step 3, compiled `.wasm` path from Step 1.
**Action:** For each annotated entry, generate the appropriate JS wrapper file. Details in Section 5.
**Output:** Generated `.js` files written to `--output` directory.

### Step 5: Copy Runtimes and WASM Binary

**Input:** JS runtime paths from Step 2, compiled `.wasm` from Step 1.
**Action:** Copy raw `.wasm` binary and JS runtime files to `--output` directory.
**Output:** Final output directory populated.

### Step 6: Single-File Bundling (conditional)

**Input:** Entries with `js=single-file` option, `.wasm` binary, encoding preference.
**Action:** Read `.wasm` bytes, encode per `encoded` option, embed into JS wrapper. Details in Section 6.
**Output:** `{name}.bundle.js` files in `--output` directory.

---

## 3. Proc Macro Annotation Discovery

The proc macros do not directly communicate with the CLI. Instead, they generate WASM exports that the CLI discovers post-compilation:

```rust
// User writes:
#[wasm_bin(js = single-file, encoded = b64)]
fn my_app() { /* ... */ }

// Proc macro generates:
#[no_mangle]
pub extern "C" fn __ewe_bin_invoke(params_ptr: u32, params_len: u32, returns_ptr: u32) {
    my_app_inner(params_ptr, params_len, returns_ptr)
}

#[no_mangle]
pub extern "C" fn __ewe_meta_my_app() -> u32 {
    // Returns pointer to JSON: {"mode":"bin","js":"single-file","encoded":"b64"}
    static META: &str = r#"{"mode":"bin","js":"single-file","encoded":"b64"}"#;
    META.as_ptr() as u32
}
```

The CLI reads metadata by:
1. Parsing WASM export section with `wasmparser` (no instantiation needed for export names).
2. For each `__ewe_meta_*` export, instantiating the WASM in wasmtime with stub imports.
3. Calling the meta export, reading the returned string pointer from WASM linear memory.
4. Deserializing the JSON into `AnnotationOptions { mode, js, encoded }`.

---

## 4. Output Generation Per Mode

### `#[wasm_bin]` -- Main Thread JS Wrapper

Generated `{name}.js`:

```javascript
// {name}.js — generated by ewe-wasm build
import { FoundationWasm } from './foundation-wasm.js';

const WASM_URL = './{name}.wasm';

export async function init(importOverrides = {}) {
    const response = await fetch(WASM_URL);
    const wasmBytes = await response.arrayBuffer();
    const runtime = new FoundationWasm();
    const imports = runtime.buildImportObject(importOverrides);
    const { instance } = await WebAssembly.instantiate(wasmBytes, imports);
    runtime.bind(instance.exports);
    instance.exports.__ewe_bin_invoke(0, 0, 0);
    return { runtime, instance };
}
```

### `#[wasm_worker]` -- Web Worker JS Wrapper

Generated `{name}-worker.js`:

```javascript
// {name}-worker.js — generated by ewe-wasm build
importScripts('./foundation-wasm.js');

let runtime, instance;

async function initWorker() {
    const response = await fetch('./{name}.wasm');
    const wasmBytes = await response.arrayBuffer();
    runtime = new FoundationWasm();
    const imports = runtime.buildImportObject({});
    const result = await WebAssembly.instantiate(wasmBytes, imports);
    instance = result.instance;
    runtime.bind(instance.exports);
}

const ready = initWorker();

self.onmessage = async (e) => {
    await ready;
    if (e.data.type === 'invoke') {
        const buffer = e.data.data;
        const ptr = instance.exports.create_allocation(buffer.byteLength);
        const startPtr = instance.exports.allocation_start_pointer(ptr);
        new Uint8Array(instance.exports.memory.buffer, startPtr, buffer.byteLength)
            .set(new Uint8Array(buffer));
        instance.exports.__ewe_worker_invoke(startPtr, buffer.byteLength, 0);
        const resultPtr = instance.exports.allocation_start_pointer(ptr);
        const resultLen = instance.exports.allocation_length(ptr);
        const result = new Uint8Array(instance.exports.memory.buffer, resultPtr, resultLen).slice();
        self.postMessage({ type: 'result', data: result.buffer }, [result.buffer]);
    }
};
```

Generated `{name}-worker-host.js` (main thread side):

```javascript
// {name}-worker-host.js — generated by ewe-wasm build
const worker = new Worker(new URL('./{name}-worker.js', import.meta.url));

export function callWorker(protocol, payload) {
    return new Promise((resolve) => {
        worker.onmessage = (e) => {
            if (e.data.type === 'result') resolve(e.data.data);
        };
        worker.postMessage({ type: 'invoke', data: payload }, [payload.buffer]);
    });
}
```

### `#[wasm_service]` -- Service Worker JS Wrapper

Generated `{name}-sw.js`:

```javascript
// {name}-sw.js — generated by ewe-wasm build
importScripts('./foundation-wasm.js');

const WASM_ROUTES = ${JSON.stringify(routes)};

let runtime, instance;
const ready = (async () => {
    const response = await fetch('./{name}.wasm');
    const wasmBytes = await response.arrayBuffer();
    runtime = new FoundationWasm();
    const imports = runtime.buildImportObject({});
    const result = await WebAssembly.instantiate(wasmBytes, imports);
    instance = result.instance;
    runtime.bind(instance.exports);
})();

self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (e) => e.waitUntil(self.clients.claim()));

self.addEventListener('fetch', (event) => {
    const url = new URL(event.request.url);
    const matches = WASM_ROUTES.some(route => url.pathname.startsWith(route));
    if (!matches) return;
    event.respondWith((async () => {
        await ready;
        const reqBytes = await serializeRequest(event.request);
        const ptr = instance.exports.create_allocation(reqBytes.byteLength);
        const startPtr = instance.exports.allocation_start_pointer(ptr);
        new Uint8Array(instance.exports.memory.buffer, startPtr, reqBytes.byteLength)
            .set(new Uint8Array(reqBytes));
        const respPtr = instance.exports.__ewe_service_invoke(startPtr, reqBytes.byteLength);
        return deserializeResponse(instance, respPtr);
    })());
});
```

The `WASM_ROUTES` array is populated at build time via route extraction (Section 8).

---

## 5. Single-File Bundling

When an annotation specifies `js=single-file`, the CLI reads the compiled `.wasm` binary and embeds it into the JS wrapper.

### Algorithm

1. Read `.wasm` file into `Vec<u8>`.
2. Check `encoded` option: `b64` or `uint8array` (default: `uint8array`).
3. If `b64`: encode bytes as base64 string, generate decoder + instantiation JS.
4. If `uint8array`: format bytes as comma-separated decimal values in a `Uint8Array` literal.
5. Replace the `fetch()` call in the wrapper template with inline instantiation.
6. Write to `{name}.bundle.js`.

### Base64 Encoding Output

```javascript
// {name}.bundle.js (encoded=b64)
const WASM_B64 = "AGFzbQEAAAA...";
function _b64decode(str) {
    const bin = atob(str);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    return bytes;
}
const WASM_BYTES = _b64decode(WASM_B64);
const { instance } = await WebAssembly.instantiate(WASM_BYTES, imports);
```

### Uint8Array Literal Output

```javascript
// {name}.bundle.js (encoded=uint8array)
const WASM_BYTES = new Uint8Array([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, ...]);
const { instance } = await WebAssembly.instantiate(WASM_BYTES, imports);
```

### Base64 vs Uint8Array: When Each Is Used

| Aspect | `encoded=b64` | `encoded=uint8array` (default) |
|--------|---------------|-------------------------------|
| JS file size | ~33% larger than raw WASM | ~3-4x larger (decimal literals) |
| After gzip/brotli | Comparable to uint8array | Comparable to b64 |
| Parse time | Slower (atob + charCodeAt loop) | Faster (engine parses array literal directly) |
| AST node count | 1 string token | 1 node per byte value (can crash V8 for >1MB WASM) |
| Recommended for | Large WASM (>500KB), GTM embeds, CSP-restricted envs | Small-to-medium WASM (<500KB), performance-critical |

Default is `uint8array` because it avoids the atob decode overhead and the bytes are passed directly to `WebAssembly.instantiate`. For WASM binaries exceeding 500KB, `b64` is recommended to avoid JS parser pressure from large array literals.

---

## 6. `application/wasm` Script Tag and autoInstantiate

The runtime supports declarative WASM loading via `<script type="application/wasm">`:

```html
<script type="application/wasm" src="my-app.wasm"></script>
<script src="foundation-wasm.js"></script>
<script>FoundationWasm.autoInstantiate();</script>
```

### autoInstantiate Algorithm

```javascript
FoundationWasm.autoInstantiate = async function(importOverrides = {}) {
    const scripts = document.querySelectorAll('script[type="application/wasm"]');
    for (const script of scripts) {
        const src = script.getAttribute('src');
        if (!src) continue;
        const response = await fetch(src);
        const wasmBytes = await response.arrayBuffer();
        const runtime = new FoundationWasm();
        const imports = runtime.buildImportObject(importOverrides);
        const { instance } = await WebAssembly.instantiate(wasmBytes, imports);
        runtime.bind(instance.exports);
        const name = src.replace(/\.wasm$/, '').split('/').pop();
        window[`__ewe_${name}`] = { runtime, instance };
        script.dispatchEvent(new CustomEvent('wasm-ready', { detail: { runtime, instance } }));
    }
};
```

The import object is constructed by `FoundationWasm.buildImportObject()`, which provides the standard host functions: `host_batch_apply`, `host_arrow_apply`, `host_json_apply`, `schedule_timeout`, `cancel_timeout`, `schedule_interval`, `cancel_interval`, `hook_up_animation_frames`, `host_cache_string`, `host_register_function`, `host_unregister_function`, `host_invoke_function`. Each auto-instantiated module gets its own `FoundationWasm` runtime instance and is registered on `window.__ewe_{name}`.

---

## 7. Service Worker Route Extraction

For `#[wasm_service]` entries, the CLI must extract the route list at build time so the generated service worker JS can filter fetch events without invoking WASM for every request.

### Algorithm

1. After compiling WASM (Step 1), check if any `__ewe_service_invoke` export exists.
2. If so, look for `__ewe_extract_routes` export in the WASM binary.
3. Instantiate the WASM in wasmtime with stub imports (all host functions are no-ops).
4. Call `__ewe_extract_routes()` which returns a `u32` pointer into WASM linear memory.
5. Read from that pointer: a `u32` length prefix followed by a JSON-encoded `string[]` of route prefixes.
6. Deserialize into `Vec<String>`.
7. Embed as `const WASM_ROUTES = ["/api/users", "/api/auth"];` in the generated service worker JS.

**Failure:** If `__ewe_extract_routes` is missing, CLI exits with `error: #[wasm_service] requires route extraction but __ewe_extract_routes export not found`. If the export returns invalid JSON, CLI exits with `error: route extraction returned invalid data: {details}`.

---

## 8. `embedded-js` Feature Flag

When `foundation_wasm_ui` is compiled with `features = ["embedded-js"]`, the JS runtime files are embedded into the Rust binary via `include_str!`:

```rust
// crates/foundation_wasm_ui/src/embedded.rs
#[cfg(feature = "embedded-js")]
pub const FOUNDATION_WASM_JS: &str = include_str!("../../assets/foundation-wasm.js");
#[cfg(feature = "embedded-js")]
pub const FOUNDATION_WASM_UI_JS: &str = include_str!("../../assets/foundation-wasm-ui.js");
```

The CLI binary links against `foundation_wasm_ui` with this feature enabled, so it can write the JS runtimes from embedded strings without requiring the assets directory at runtime. This makes the `ewe-wasm` binary fully self-contained.

When `embedded-js` is active and `EWE_WASM_JS_DIR` is also set, the environment variable takes precedence (allows overriding embedded assets during development).

---

## 9. Output Directory Structure

### Default (`#[wasm_bin]`, no single-file)

```
build/
├── my-app.wasm              # raw WASM binary (copied from cargo output)
├── my-app.js                # main-thread JS wrapper (fetch + instantiate)
├── foundation-wasm.js       # core ABI runtime
└── foundation-wasm-ui.js    # DOM runtime
```

### `#[wasm_bin(js=single-file, encoded=uint8array)]`

```
build/
├── my-app.wasm              # raw WASM binary (always included)
├── my-app.js                # standalone wrapper (fetch-based)
├── my-app.bundle.js         # single-file: WASM as Uint8Array + runtime
├── foundation-wasm.js
└── foundation-wasm-ui.js
```

### `#[wasm_worker]`

```
build/
├── my-app.wasm
├── my-app-worker.js         # worker-side: importScripts + onmessage
├── my-app-worker-host.js    # main-thread: Worker() + postMessage
├── foundation-wasm.js
└── foundation-wasm-ui.js
```

### `#[wasm_worker(js=single-file, encoded=b64)]`

```
build/
├── my-app.wasm
├── my-app-worker.js
├── my-app-worker-host.js
├── my-app-worker.bundle.js  # single-file worker: WASM as b64 + runtime
├── foundation-wasm.js
└── foundation-wasm-ui.js
```

### `#[wasm_service]`

```
build/
├── my-app.wasm
├── my-app-sw.js             # service worker: fetch listener + route table
├── foundation-wasm.js
└── foundation-wasm-ui.js
```

### Multiple annotations in one crate

All outputs are generated. A crate with both `#[wasm_bin]` and `#[wasm_worker]` produces both wrapper sets. The `.wasm` binary is shared.

---

## 10. Error Cases

| Scenario | Behavior |
|----------|----------|
| WASM compilation fails (cargo non-zero exit) | CLI prints cargo stderr, exits code 1. No output generated. |
| No `__ewe_*` exports found | Warning printed. Default `wasm_bin` wrapper generated using crate name. |
| `__ewe_meta_*` returns invalid JSON | CLI exits with `error: invalid annotation metadata for {name}: {parse_error}`. |
| `__ewe_extract_routes` missing for service worker | CLI exits with `error: #[wasm_service] requires __ewe_extract_routes export`. |
| `__ewe_extract_routes` returns invalid data | CLI exits with `error: route extraction returned invalid data: {details}`. |
| JS runtime files missing (no `--skip-runtimes`) | CLI exits with `error: runtime file not found: {path}`. |
| Output directory not writable | CLI exits with `error: cannot write to output directory: {io_error}`. |
| WASM binary exceeds 1MB with `encoded=uint8array` | Warning: `warning: WASM binary is {size}MB; consider encoded=b64 to avoid JS parser limits`. |
| wasmtime instantiation failure during meta/route extraction | CLI exits with `error: failed to instantiate WASM for metadata extraction: {details}`. |
| Duplicate annotation names | CLI exits with `error: duplicate entry name: {name}`. |

---

## 11. Integration Points

| Feature | Relationship |
|---------|-------------|
| F00 (foundation_wasm refactor) | CLI generates wrappers that import `foundation-wasm.js` host functions. |
| F01 (foundation_ui_traits) | Protocol encoders used by generated WASM; CLI does not interact directly. |
| F04 (InstructionReceiver) | Runtime wired by generated wrappers; CLI ensures correct import object. |
| F05 (Arrow encoding) | Arrow protocol messages flow through generated wrappers unchanged. |
| F06 (Web Components) | Service worker wrappers may serve component mount data. |
| F08 (Event Runtime) | Event callbacks flow through the same ABI wired by generated wrappers. |

---

## 12. File Ownership

```
crates/foundation_codegen/
├── Cargo.toml                    # binary: ewe-wasm, deps: wasmparser, wasmtime, clap, base64
├── src/
│   ├── main.rs                   # CLI entrypoint, argument parsing (clap)
│   ├── build.rs                  # Step 1: cargo build orchestration
│   ├── discovery.rs              # Step 3: WASM export scanning, annotation parsing
│   ├── generator/
│   │   ├── mod.rs                # Generator trait + dispatch
│   │   ├── wasm_bin.rs           # wasm_bin JS wrapper template
│   │   ├── wasm_worker.rs        # wasm_worker JS wrapper templates (worker + host)
│   │   └── wasm_service.rs       # wasm_service JS wrapper template + route embedding
│   ├── bundler.rs                # Step 6: single-file bundling (b64 + uint8array)
│   └── runtime_collector.rs      # Step 2: JS runtime file collection
```

---

## 13. Refactoring Strategy

**Phase 1 -- CLI Skeleton:** Implement `main.rs` with clap argument parsing. Implement `build.rs` to invoke `cargo build --target wasm32-unknown-unknown`. Verify it compiles a test crate.

**Phase 2 -- Discovery:** Implement `discovery.rs` with `wasmparser` to scan WASM exports. Test against a `.wasm` file with known `__ewe_*` exports. Verify annotation metadata extraction via wasmtime.

**Phase 3 -- Generators:** Implement `wasm_bin.rs` generator. Write the JS wrapper template. Test: generated JS loads a real `.wasm` and calls the entry point. Then `wasm_worker.rs`, then `wasm_service.rs`.

**Phase 4 -- Bundler:** Implement `bundler.rs`. Test both b64 and uint8array encoding paths. Verify generated `.bundle.js` self-instantiates without network fetch.

**Phase 5 -- Service Worker Routes:** Implement route extraction via wasmtime. Test against a crate with `#[wasm_service]` and known routes. Verify extracted routes match.

**Phase 6 -- End-to-End:** Full pipeline: compile crate with all three annotations, verify all output files generated, each JS file loads and functions correctly.

---

## 14. Testing

### CLI Arguments (tests 1-5)

| # | Scenario | Verify |
|---|----------|--------|
| 1 | `ewe-wasm build --release` | Cargo invoked with `--release` flag. Output in `build/`. |
| 2 | `ewe-wasm build --dev --output dist/` | Cargo invoked without `--release`. Output in `dist/`. |
| 3 | `ewe-wasm build --release --dev` | CLI exits with error: conflicting flags. |
| 4 | `ewe-wasm build --skip-runtimes` | No `foundation-wasm.js` or `foundation-wasm-ui.js` in output. |
| 5 | `ewe-wasm build --target my-crate` | Cargo invoked with `-p my-crate`. |

### Annotation Discovery (tests 6-11)

| # | Scenario | Verify |
|---|----------|--------|
| 6 | WASM with `__ewe_bin_invoke` export | Discovered as `wasm_bin` entry. |
| 7 | WASM with `__ewe_worker_invoke` export | Discovered as `wasm_worker` entry. |
| 8 | WASM with `__ewe_service_invoke` + `__ewe_extract_routes` | Both exports found. |
| 9 | WASM with `__ewe_meta_app` returning JSON | Metadata deserialized: mode, js, encoded fields correct. |
| 10 | WASM with no `__ewe_*` exports | Warning printed, default `wasm_bin` created. |
| 11 | WASM with `__ewe_meta_app` returning invalid JSON | CLI exits with parse error message. |

### JS Wrapper Generation (tests 12-17)

| # | Scenario | Verify |
|---|----------|--------|
| 12 | `wasm_bin` wrapper generated | Contains `WebAssembly.instantiate`, `fetch()`, `__ewe_bin_invoke` call. |
| 13 | `wasm_worker` wrapper generated | Worker file has `importScripts`, `self.onmessage`. Host file has `new Worker()`. |
| 14 | `wasm_service` wrapper generated | Contains `self.addEventListener('fetch')`, `WASM_ROUTES` array. |
| 15 | `wasm_bin` wrapper loads real WASM | Integration test: generated JS fetches `.wasm`, instantiates, entry point runs. |
| 16 | `wasm_worker` host+worker communicate | postMessage round-trip: host sends, worker processes, host receives result. |
| 17 | `wasm_service` intercepts matching fetch | Service worker intercepts `/api/users`, passes through `/static/style.css`. |

### Single-File Bundling (tests 18-23)

| # | Scenario | Verify |
|---|----------|--------|
| 18 | `encoded=uint8array` output | Bundle contains `new Uint8Array([0x00, 0x61, ...])`; no `fetch()` call. |
| 19 | `encoded=b64` output | Bundle contains base64 string + `atob` decoder; no `fetch()` call. |
| 20 | Bundle self-instantiates | Load `{name}.bundle.js` in browser; WASM runs without network. |
| 21 | Bundle bytes match raw WASM | Decode embedded bytes, compare SHA-256 with original `.wasm` file. |
| 22 | Large WASM (>1MB) with uint8array | Warning printed about JS parser limits. |
| 23 | Large WASM (>1MB) with b64 | No warning. Bundle loads correctly. |

### Service Worker Route Extraction (tests 24-27)

| # | Scenario | Verify |
|---|----------|--------|
| 24 | Extract routes from test WASM | `__ewe_extract_routes` returns `["/api/users", "/api/auth"]`. |
| 25 | Routes baked into generated SW JS | Generated file contains `const WASM_ROUTES = ["/api/users","/api/auth"]`. |
| 26 | Missing `__ewe_extract_routes` for service mode | CLI exits with descriptive error. |
| 27 | Route extraction returns malformed data | CLI exits with descriptive error. |

### autoInstantiate (tests 28-30)

| # | Scenario | Verify |
|---|----------|--------|
| 28 | Single `<script type="application/wasm">` | `autoInstantiate()` fetches, instantiates, registers on `window.__ewe_{name}`. |
| 29 | Multiple WASM script tags | Each gets own runtime instance. All registered on `window`. |
| 30 | Script tag with no `src` attribute | Skipped silently. No error thrown. |

### Output Structure (tests 31-35)

| # | Scenario | Verify |
|---|----------|--------|
| 31 | Default wasm_bin build | Output contains: `.wasm`, `.js`, `foundation-wasm.js`, `foundation-wasm-ui.js`. |
| 32 | wasm_bin + single-file build | Output also contains `.bundle.js`. |
| 33 | wasm_worker build | Output contains: `.wasm`, `-worker.js`, `-worker-host.js`, runtimes. |
| 34 | wasm_service build | Output contains: `.wasm`, `-sw.js`, runtimes. |
| 35 | All three modes in one crate | All wrapper files generated. Single `.wasm` binary shared. |

### Error Handling (tests 36-40)

| # | Scenario | Verify |
|---|----------|--------|
| 36 | Cargo compilation failure | CLI prints error, exits code 1, no output files created. |
| 37 | Missing JS runtime files | CLI prints path in error message, exits code 1. |
| 38 | Non-writable output directory | CLI prints IO error, exits code 1. |
| 39 | Duplicate annotation names | CLI prints duplicate name, exits code 1. |
| 40 | wasmtime instantiation failure | CLI prints details, exits code 1. |
