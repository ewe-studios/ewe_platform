---
feature: "Foundation WASM Testbed"
description: "CLI-driven test harness for wasm32-unknown-unknown, supporting browser (Playwright), Deno, and Cloudflare Workers (wrangler) execution, with bindgen and custom modes"
status: "planned"
priority: "high"
depends_on: ["01-core-wasm-compat", "08-wasm-oauth-manager"]
estimated_effort: "large"
created: 2026-05-18
last_updated: 2026-06-01
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---

# Feature: Foundation WASM Testbed

## Overview

A CLI crate `foundation_wasm_testbed` that provides a unified testing interface for wasm32-unknown-unknown projects. It replaces the complexity of `wasm-pack test` and `wasm-bindgen-test-runner` with a simple, predictable model.

## Cargo.toml

```toml
[package]
name = "foundation_wasm_testbed"
version = "0.0.1"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
description = "CLI-driven test harness for wasm32-unknown-unknown"

[dependencies]
clap = { version = "4", features = ["derive"] }
anyhow = "1"
walrus = "0.23"
serde = { workspace = true }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
foundation_http = { workspace = true }
foundation_macros = { workspace = true }
foundation_nostd = { workspace = true }
tempfile = "3"
which = "7"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
indicatif = "0.18"
portpicker = "0.1"
reqwest = { version = "0.12", features = ["blocking", "json"] }
fs_extra = "1.3"

[[bin]]
name = "wasm-testbed"
path = "src/main.rs"

[lints]
workspace = true
```

### Dependency rationale

| Dependency | Why |
|---|---|
| `walrus` | wasm binary parsing — discovers `__wbgt_` exports by name in the wasm module's export section |
| `portpicker` | Find a random available port for the HTTP server / wrangler dev |
| `reqwest` (blocking) | curl-equivalent for wrangler mode — sends HTTP request to wrangler dev and reads response |
| `indicatif` | Progress spinner UX during build / test steps |
| `fs_extra` | Recursive directory copy for staging templates during `init` |
| `tempfile` | Temp directories for Playwright scripts |
| `which` | Locate `wasm-bindgen`, `deno`, `wrangler`, `npx` on PATH |

### NEW workspace dependencies to add to root Cargo.toml

These are not currently in the workspace:
- `walrus = "0.23"` — wasm binary parsing (test discovery)
- `portpicker = "0.1"` — random available port allocation
- `reqwest = { version = "0.12", features = ["blocking", "json"] }` — HTTP client for wrangler mode
- `fs_extra = "1.3"` — recursive file copying
- `indicatif = "0.18"` — progress spinners
- `tracing-subscriber = { version = "0.3", features = ["env-filter"] }` — CLI log output

---

## Crate Structure

```
backends/foundation_wasm_testbed/
├── Cargo.toml
├── src/
│   ├── main.rs           # CLI entry point (clap), tokio runtime bootstrap, tracing init
│   ├── lib.rs            # Re-exports for testing
│   ├── cli.rs            # Clap subcommand definitions (InitArgs, TestArgs, Mode, InitType)
│   ├── init.rs           # init command — scaffold integration directories from templates
│   ├── build.rs          # cargo build --target wasm32-unknown-unknown orchestrator
│   ├── wasm.rs           # locate wasm binary in target/, run wasm-bindgen CLI
│   ├── wasm_test.rs      # discover __wbgt_ exports via walrus wasm parsing
│   ├── deno.rs           # deno run --allow-read --allow-net command construction + execution
│   ├── wrangler.rs       # wrangler dev subprocess management + HTTP curl
│   ├── browser.rs        # Playwright subprocess management for web tests
│   ├── server.rs         # foundation_http server setup (uses StaticFileHandler)
│   └── templates/        # Template source files (NOT compiled in, read at build time by macro)
│       ├── web/
│       │   ├── index.html
│       │   ├── index.js
│       │   └── loader.js
│       ├── deno/
│       │   ├── index.js
│       │   └── loader.js
│       ├── wrangler/
│       │   ├── worker.js
│       │   ├── loader.js
│       │   └── wrangler.toml
│       ├── bindgen-web/
│       │   ├── index.html
│       │   └── run.js
│       ├── bindgen-deno/
│       │   └── run.js
│       └── bindgen-wrangler/
│           ├── worker.js
│           └── wrangler.toml
```

---

## Module Specifications

### `main.rs` — CLI Entry Point

**Responsibility:** Parse CLI args, initialize tracing, bootstrap tokio runtime, dispatch to command handler.

**Flow:**
1. Parse args via `Cli::parse()` (clap derive)
2. Initialize `tracing_subscriber` with `env-filter` (default level `info`)
3. Create `tokio::runtime::Runtime`
4. Match on `cli.command`:
   - `Command::Init(args)` → `init::run(args).await`
   - `Command::Test(args)` → `test::run(args).await`
5. Exit with code 0 on success, 1 on error (anyhow prints the chain)

**Structure:**
```rust
#[derive(Parser)]
#[command(name = "wasm-testbed", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scaffold integration test directories for a wasm crate
    Init(InitArgs),
    /// Build, stage, and run tests for a wasm crate
    Test(TestArgs),
}
```

---

### `cli.rs` — Clap Argument Definitions

**Responsibility:** Define all CLI argument types shared across modules.

#### `InitArgs`

```rust
struct InitArgs {
    /// Which integration type to scaffold: web, deno, wrangler
    #[arg(value_enum)]
    r#type: Option<InitType>,
    /// Path to the Cargo crate to scaffold
    crate_path: PathBuf,
}
```

- If `type` is `None`, scaffold ALL three types (web, deno, wrangler)
- `crate_path` is the directory containing the user's `Cargo.toml`

#### `InitType` (clap `ValueEnum`)

| Variant | CLI value | Integration dir created |
|---------|-----------|------------------------|
| `Web` | `web` | `integrations/web/` |
| `Deno` | `deno` | `integrations/deno/` |
| `Wrangler` | `wrangler` | `integrations/wrangler/` |

#### `TestArgs`

```rust
struct TestArgs {
    /// Test mode: web, deno, wrangler, bindgen-web, bindgen-deno, bindgen-wrangler
    #[arg(value_enum)]
    mode: Mode,
    /// Path to the Cargo crate to test
    crate_path: PathBuf,
    /// Build in release mode (default: debug)
    #[arg(long)]
    release: bool,
    /// Cargo features to enable during build
    #[arg(long)]
    features: Option<String>,
    /// Browser to use (web/bindgen-web modes only)
    #[arg(long, default_value = "chrome")]
    browser: Browser,
    /// Run browser in headless mode (web/bindgen-web modes only)
    #[arg(long)]
    headless: bool,
}
```

#### `Mode` (clap `ValueEnum`)

| Variant | CLI value | Category |
|---------|-----------|----------|
| `Web` | `web` | Custom harness |
| `Deno` | `deno` | Custom harness |
| `Wrangler` | `wrangler` | Custom harness |
| `BindgenWeb` | `bindgen-web` | Auto-generated |
| `BindgenDeno` | `bindgen-deno` | Auto-generated |
| `BindgenWrangler` | `bindgen-wrangler` | Auto-generated |

#### `Browser` (clap `ValueEnum`)

| Variant | CLI value |
|---------|-----------|
| `Chrome` | `chrome` |
| `Firefox` | `firefox` |
| `Safari` | `safari` |

---

### `templates/` — Template Files

**Why embedded:** The `init` command reads embedded templates and writes them to the target crate's `integrations/` directory. Templates use `EmbedDirectoryAs` for dual-mode access: disk reads in debug (fast iteration), embedded bytes in release.

**How embedded:** A struct in `init.rs` derives `EmbedDirectoryAs`:

```rust
#[derive(foundation_macros::EmbedDirectoryAs)]
#[source = "$CURRENT_CRATE/src/templates"]
struct TemplateDirectory;
```

This generates `TemplateDirectory` implementing `EmbeddableDirectory` and `DirectoryData`. In debug mode, `read_utf8_for("web/index.html")` reads from `src/templates/web/index.html` on disk. In release, it reads from compiled-in bytes.

**Placeholder replacement:** All template file content has `PACKAGE_NAME` replaced with the actual cargo package name during `init` (for user-facing templates) or during test (for auto-generated bindgen templates).

#### `templates/web/index.html`

Basic HTML harness page. Loads `index.js` as ES module. Provides `#output` `<pre>` element for test result display.

```html
<!DOCTYPE html>
<html>
  <head><meta charset="utf-8"><title>WASM Tests</title></head>
  <body>
    <pre id="output"></pre>
    <script type="module" src="index.js"></script>
  </body>
</html>
```

#### `templates/web/index.js`

Stub user entry point. Imports `instantiate()` from `loader.js`. User replaces the comment with actual test assertions.

```js
import { instantiate } from "./loader.js";

const wasm = await instantiate("PACKAGE_NAME");
// Add your test assertions here
// wasm.main(); or call exported functions directly

document.getElementById("output").textContent = "Tests complete";
```

#### `templates/web/loader.js`

Wasm instantiation helper for browser. Fetches `./PACKAGE_NAME.wasm` via HTTP, instantiates with `WebAssembly.instantiate`. Provides `env.memory` as a host import.

```js
export async function instantiate(name) {
  const response = await fetch(`./${name}.wasm`);
  const bytes = await response.arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {
    env: {
      memory: new WebAssembly.Memory({ initial: 256, maximum: 512 }),
    },
  });
  return instance.exports;
}
```

#### `templates/deno/index.js`

Stub Deno entry point. Imports from `loader.js` (Deno-specific, uses `Deno.readFile`).

```js
import { instantiate } from "./loader.js";

const wasm = await instantiate("PACKAGE_NAME");
// Add your test assertions here

console.log("Tests complete");
```

#### `templates/deno/loader.js`

Deno-specific wasm loader. Uses `Deno.readFile` to load wasm from filesystem (not HTTP).

```js
export async function instantiate(name) {
  const wasmPath = new URL(`./${name}.wasm`, import.meta.url);
  const bytes = await Deno.readFile(wasmPath);
  const { instance } = await WebAssembly.instantiate(bytes, {
    env: {
      memory: new WebAssembly.Memory({ initial: 256, maximum: 512 }),
    },
  });
  return instance.exports;
}
```

#### `templates/wrangler/worker.js`

Stub Cloudflare Workers entry point. Exports a `fetch` handler. Lazily instantiates wasm on first request. Calls `wasm.main()` and returns response.

```js
import { instantiate } from "./loader.js";
let wasm = null;

export default {
  async fetch(request, env, ctx) {
    if (!wasm) {
      wasm = await instantiate("PACKAGE_NAME", ctx);
    }
    wasm.main();
    return new Response("Tests complete", { status: 200 });
  },
};
```

#### `templates/wrangler/loader.js`

Cloudflare Workers wasm loader. Uses dynamic `import()` to load the `.wasm` file (Workers' way of loading wasm files). Accepts `ctx` (the ExecutionContext from the fetch handler) for waitUntil.

```js
export async function instantiate(name, ctx) {
  const { default: wasm } = await import(`./${name}.wasm`);
  const { instance } = await WebAssembly.instantiate(wasm, {
    // Workers provide ServiceWorkerGlobalScope APIs; add host imports here
  });
  return instance.exports;
}
```

#### `templates/wrangler/wrangler.toml`

Minimal wrangler config. Specifies `worker.js` as the entry point.

```toml
name = "wasm-test"
main = "worker.js"
compatibility_date = "2026-05-18"
```

#### `templates/bindgen-web/index.html`

Generated browser harness for bindgen mode. Has three `<pre>` elements: `#output` for final result, `#console_log` for wasm console.log output, `#console_error` for errors. Loads `run.js` as ES module.

```html
<!DOCTYPE html>
<html>
  <head><meta charset="utf-8"><title>WASM Bindgen Tests</title></head>
  <body>
    <pre id="output"></pre>
    <pre id="console_log"></pre>
    <pre id="console_error"></pre>
    <script type="module" src="run.js"></script>
  </body>
</html>
```

#### `templates/bindgen-web/run.js`

Auto-generated test runner for browser. Imports wasm-bindgen JS glue code, creates `WasmBindgenTestContext`, runs discovered tests.

Placeholders replaced at generation time:
- `PACKAGE_NAME` → actual cargo package name
- `TEST_NAMES` → comma-separated list of `__wbgt_` test names (e.g. `"__wbgt_test_add", "__wbgt_test_sub"`)

```js
import init, { WasmBindgenTestContext, __wbgtest_console_log } from "./PACKAGE_NAME.js";

const wasm = await init();
const cx = new WasmBindgenTestContext();
window.on_console_log = __wbgtest_console_log;

const tests = [/* TEST_NAMES */];
await cx.run(tests.map(name => wasm[name]));

document.getElementById("output").textContent = "test result: ok";
```

#### `templates/bindgen-deno/run.js`

Auto-generated test runner for Deno. Same as bindgen-web but without browser DOM references.

```js
import init, { WasmBindgenTestContext } from "./PACKAGE_NAME.js";

const wasm = await init();
const cx = new WasmBindgenTestContext();

const tests = [/* TEST_NAMES */];
await cx.run(tests.map(name => wasm[name]));

console.log("test result: ok");
```

#### `templates/bindgen-wrangler/worker.js`

Auto-generated Cloudflare Workers test harness. Imports wasm-bindgen ES module output and wasm binary. Creates a persistent `cx` across requests (cached after first init). Runs tests on each request and returns JSON result.

```js
import init, { WasmBindgenTestContext } from "./PACKAGE_NAME.js";
import wasmModule from "./PACKAGE_NAME_bg.wasm";

let cx = null;

export default {
  async fetch(request, env, ctx) {
    if (!cx) {
      const wasm = await init(wasmModule);
      cx = new WasmBindgenTestContext();
    }
    const results = await cx.run(/* TEST_NAMES */);
    return new Response(JSON.stringify(results), {
      headers: { "Content-Type": "application/json" },
    });
  },
};
```

#### `templates/bindgen-wrangler/wrangler.toml`

Auto-generated minimal wrangler config for bindgen mode.

```toml
name = "wasm-test-bindgen"
main = "worker.js"
compatibility_date = "2026-05-18"
```

---

### `init.rs` — Init Command

**Responsibility:** Scaffold integration directories from embedded templates into the user's crate.

**Entry point:** `pub async fn run(args: InitArgs) -> anyhow::Result<()>`

**Flow:**

1. Resolve `args.crate_path` to absolute path. Validate it exists and contains `Cargo.toml`.
2. Read `Cargo.toml` to extract `[package].name` → `package_name`.
3. Determine which types to scaffold:
   - If `args.type` is `Some(t)` → scaffold only that type
   - If `None` → scaffold all three: web, deno, wrangler
4. For each type:
   a. Create `integrations/{type}/` directory under crate path
   b. For each template file in the type's subdirectory:
      - Call `TemplateDirectory.read_utf8_for("{type}/filename")` to get content bytes
      - Decode bytes to UTF-8 string
      - Replace all occurrences of `PACKAGE_NAME` with `package_name`
      - Write the resulting string to `crate_path/integrations/{type}/filename`
   c. Print `Scaffolded integrations/{type}/`
5. Print summary of all directories created.

**What gets created per type:**

| Type | Files created |
|------|--------------|
| `web` | `integrations/web/index.html`, `index.js`, `loader.js` |
| `deno` | `integrations/deno/index.js`, `loader.js` |
| `wrangler` | `integrations/wrangler/worker.js`, `loader.js`, `wrangler.toml` |

**Template access:** The `TemplateDirectory` struct is defined in `init.rs` itself (not in a separate file), derives `EmbedDirectoryAs`, and points to `$CURRENT_CRATE/src/templates`.

**Error cases:**
- Crate path doesn't exist → anyhow::anyhow!("Crate path does not exist: {path}")
- No `Cargo.toml` in crate path → anyhow::anyhow!("Not a Cargo crate: {path}")
- `[package].name` missing → anyhow::anyhow!("Cargo.toml missing [package].name")
- File write failure → propagate via `?`

---

### `build.rs` — Cargo Build Orchestrator

**Responsibility:** Run `cargo build --target wasm32-unknown-unknown` and return the path to the resulting `.wasm` file.

**Entry points:**
- `pub fn run(crate_path: &Path, release: bool, features: Option<&str>) -> anyhow::Result<BuildOutput>`

**`BuildOutput` struct:**
```rust
pub struct BuildOutput {
    pub wasm_path: PathBuf,       // Absolute path to the built .wasm file
    pub package_name: String,     // Cargo package name
    pub profile: String,          // "debug" or "release"
}
```

**Flow:**

1. Read `Cargo.toml` at `crate_path/Cargo.toml` to extract `[package].name`
2. Determine profile: if `release` → "release", else "debug"
3. Build the `cargo` command:
   - Base: `cargo build --target wasm32-unknown-unknown`
   - Add `--release` if release mode
   - Add `--features <features>` if features provided
   - Set working directory to `crate_path`
   - Set stdout/stderr to inherit (show build output to user)
4. Execute the command via `std::process::Command`
5. If exit status is failure → return error with cargo's stderr output
6. Construct wasm path: `crate_path/target/wasm32-unknown-unknown/{profile}/{package_name}.wasm`
7. Verify the wasm file exists → error if not
8. Return `BuildOutput`

**Why `std::process::Command` not tokio:** This runs synchronously — the user watches cargo output in real time. The tokio runtime is only for the HTTP server / Playwright subprocess later in the flow.

**Error cases:**
- `cargo` not on PATH → anyhow::anyhow!("cargo not found on PATH")
- `wasm32-unknown-unknown` target not installed → cargo's error message is shown, return anyhow::anyhow!("wasm32-unknown-unknown target not installed. Run: rustup target add wasm32-unknown-unknown")
- Build fails → anyhow::anyhow!("cargo build failed")
- Wasm file not found after build → anyhow::anyhow!("wasm binary not found at {path}")

---

### `wasm.rs` — Wasm Binary Location & wasm-bindgen CLI

**Responsibility:** Locate wasm binaries in `target/`, invoke `wasm-bindgen` CLI, manage the bindgen output directory.

**Entry points:**

- `pub fn locate_wasm(build: &BuildOutput) -> anyhow::Result<PathBuf>`
  - Returns the path from `build.wasm_path` (already computed by build.rs). Exists for API consistency.

- `pub fn run_wasm_bindgen(
      wasm_path: &Path,
      output_dir: &Path,
      target: BindgenTarget,
  ) -> anyhow::Result<()>`

**`BindgenTarget` enum:**

| Variant | CLI flag |
|---------|----------|
| `Web` | `--target web` (omitted, web is wasm-bindgen default) |
| `Deno` | `--target deno` |
| `EsModules` | `--target esmodules` (used for bindgen-wrangler) |

**Flow for `run_wasm_bindgen`:**

1. Ensure `output_dir` exists (`fs::create_dir_all`)
2. Clean any existing files in `output_dir` (remove all files, since bindgen may leave stale artifacts)
3. Locate `wasm-bindgen` binary on PATH via `which::which("wasm-bindgen")`
4. Build command: `wasm-bindgen {wasm_path} --out-dir {output_dir} {target_flag}`
5. Execute synchronously, inherit stdout/stderr
6. If exit failure → error with wasm-bindgen's stderr
7. Verify output: check that `{package_name}.js` and `{package_name}_bg.wasm` exist in `output_dir`

**Why subprocess:** wasm-bindgen CLI is the canonical way to generate JS glue. The version must match the user's `wasm-bindgen` dependency (which is in their Cargo.toml). Using the CLI binary ensures version alignment.

**Error cases:**
- `wasm-bindgen` not on PATH → anyhow::anyhow!("wasm-bindgen not found on PATH. Install: cargo install wasm-bindgen-cli")
- wasm-bindgen CLI fails → error with its stderr
- Expected output files not found → error (mismatched wasm-bindgen version or unexpected output format)

---

### `wasm_test.rs` — wasm Binary Test Discovery

**Responsibility:** Parse a wasm binary to discover `#[wasm_bindgen_test]` test functions. These are exported with names starting with `__wbgt_`.

**Entry point:**
- `pub fn discover_tests(wasm_path: &Path) -> anyhow::Result<Vec<String>>`

**Flow:**

1. Read wasm binary bytes: `std::fs::read(wasm_path)`
2. Parse wasm module via walrus: `walrus::ModuleConfig::new().parse(&bytes)`
3. Iterate the module's exports:
   - For each export, check if `export.name.starts_with("__wbgt_")`
   - If yes, add the export name to the list
4. Return the list of test names (sorted for deterministic ordering)

**Why walrus:** walrus is the same library wasm-bindgen-test-runner uses. It parses the wasm binary format, gives access to the export section, and handles name decoding. The `__wbgt_` prefix is the wasm-bindgen convention for test function exports.

**Important:** The wasm binary passed to this function must be the wasm-bindgen output (e.g., `integrations/bindgen-web/{name}_bg.wasm`), NOT the raw cargo build output. wasm-bindgen adds the `__wbgt_` exports during processing.

**Error cases:**
- wasm file doesn't exist → anyhow::anyhow!("wasm binary not found: {path}")
- wasm parse fails → anyhow::anyhow!("Failed to parse wasm binary: {error}")
- No tests discovered → returns empty vec (caller decides whether this is an error)

---

### `server.rs` — HTTP Server for Web Tests

**Responsibility:** Start an HTTP server serving the integration directory, report the bound port, and provide shutdown capability.

**Entry point:**
- `pub fn start_serving(integration_dir: &Path) -> anyhow::Result<TestServer>`

**`TestServer` struct:**
```rust
pub struct TestServer {
    port: u16,
    shutdown_signal: Arc<foundation_core::synca::OnSignal>,
    server_thread: Option<std::thread::JoinHandle<()>>,
}
```

**`TestServer` methods:**
- `pub fn port(&self) -> u16` — returns the bound port
- `pub fn url(&self, path: &str) -> String` — returns `http://localhost:{port}/{path}`
- `pub fn shutdown(mut self)` — signals server to stop and joins the thread

**Flow for `start_serving`:**

1. Use `portpicker::pick_unused_port()` to get a random available port → `port`
2. Create `OnSignal` for shutdown: `let shutdown = Arc::new(OnSignal::new())`
3. Create a `StaticFileHandler` from foundation_http: `StaticFileHandler::new(integration_dir)`
4. Create an `HttpApp`:
   ```rust
   let mut app = foundation_http::shared::app::HttpApp::new_serve();
   app.route_any::<foundation_http::native::handlers::static_file::StaticFileHandler>("/*");
   ```
   But wait — `StaticFileHandler::new(root)` requires the root directory, and `ServeFactory::create()` panics. So we need a different approach.

   **Alternative approach:** The server uses a custom handler. Looking at how `StaticFileHandler` works: it implements `Serve` and `ServeFactory`. The `ServeFactory::create` panics because it needs a root directory. We need to create the handler manually and register it.

   The actual approach: Create a wrapper type that holds the `StaticFileHandler` and implements `ServeFactory`:

   ```rust
   struct ServeRoot(PathBuf);
   impl ServeFactory for ServeRoot {
       fn create(_bag: &ContextBag) -> Self {
           // This won't work — ServeFactory::create has no way to pass the root
       }
   }
   ```

   **Better approach:** Don't use `HttpApp::route_any::<H>` at all. Instead, use a simpler server setup. Since `foundation_http` requires the full valtron/foundation_core machinery which is heavy for a test server, consider using a lightweight alternative:

   **Use `tiny_http` or similar:** For the testbed's test server, a minimal static file server is sufficient. No need for valtron, connection pooling, etc. Use a simple HTTP server that serves files from a directory.

   Actually, looking more carefully at the spec: "The `foundation_http` server serves via the `EmbedDirectoryAs` structs". But the existing `StaticFileHandler` serves from a filesystem path, not from an `EmbedDirectoryAs`. The spec says to use `foundation_http` but the integration is complex.

   **Decision:** Use a minimal static file server via `axum` or `tiny_http` for simplicity. The test server only needs to serve static files with correct Content-Type headers. It doesn't need valtron, middleware, or connection pooling.

   Revised dependency: add `tiny_http = "0.12"` to Cargo.toml, remove `foundation_http` and `foundation_core` from testbed dependencies. The test server is a simple `tiny_http::Server` with a handler that:
   - Maps request path to file path under the integration directory
   - Prevents path traversal (`..` check)
   - Returns 404 for missing files
   - Sets `Content-Type: application/wasm` for `.wasm` files
   - Serves from disk (not embedded) — this is a test server, not a deployment artifact

   This is simpler and has fewer moving parts. The spec mentions `foundation_http` but that's overkill for a local test server that only runs during testing.

   **Revised `TestServer` implementation:**

   1. Pick unused port via `portpicker`
   2. Spawn a thread running a `tiny_http::Server` on `127.0.0.1:{port}`
   3. The server handler:
      - Parse request path (strip query string)
      - Strip leading `/`
      - Check for `..` → 403
      - Resolve path against `integration_dir`
      - Verify resolved path starts with `integration_dir` → 403 if not
      - If directory, try `index.html`
      - Read file bytes → 404 if missing
      - Detect MIME type from extension (`.wasm` → `application/wasm`, `.js` → `application/javascript`, `.html` → `text/html`, etc.)
      - Respond with 200, correct Content-Type, file bytes as body
   4. Return `TestServer` with port, shutdown signal, and thread handle
   5. Shutdown: set signal, wait for thread to finish

---

### `deno.rs` — Deno Test Runner

**Responsibility:** Run a JS file via `deno run --allow-read --allow-net`.

**Entry point:**
- `pub fn run(integration_dir: &Path, entry_file: &str) -> anyhow::Result<DenoOutput>`

**`DenoOutput` struct:**
```rust
pub struct DenoOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}
```

**Flow:**

1. Verify `deno` is on PATH via `which::which("deno")`
2. Build command:
   ```
   deno run --allow-read --allow-net --no-check integration_dir/entry_file
   ```
   - `--no-check` skips TypeScript type checking (these are JS files)
   - Working directory set to `integration_dir` so relative imports resolve
3. Execute synchronously, capture stdout and stderr
4. Return `DenoOutput` with captured output and exit code

**Why `--no-check`:** The generated JS files and user loaders are plain JavaScript, not TypeScript. Skipping type checking speeds up execution.

**Error cases:**
- `deno` not on PATH → anyhow::anyhow!("deno not found on PATH. Install from https://deno.land")
- Deno exits non-zero → print stdout/stderr, return anyhow::anyhow!("deno test failed (exit code {code})")

---

### `wrangler.rs` — Wrangler Dev + HTTP Test Runner

**Responsibility:** Start `wrangler dev` on a random port, send HTTP request, read response, kill wrangler.

**Entry point:**
- `pub fn run(integration_dir: &Path, wrangler_config_path: Option<&Path>) -> anyhow::Result<WranglerOutput>`

**`WranglerOutput` struct:**
```rust
pub struct WranglerOutput {
    pub response_body: String,
    pub status_code: u16,
    pub wrangler_stderr: String,
}
```

**Flow:**

1. Verify `wrangler` is on PATH via `which::which("wrangler")`
2. Pick unused port via `portpicker::pick_unused_port()`
3. Build wrangler command:
   ```
   wrangler dev --port {port} --ip 127.0.0.1
   ```
   - Working directory set to `integration_dir`
   - Capture stderr (wrangler logs), stdout (wrangler may output nothing)
4. Start wrangler as `std::process::Command::spawn()` (not `output()` — we need it running while we curl)
5. Wait for wrangler to be ready:
   - Poll `http://127.0.0.1:{port}/` with a short timeout (reqwest blocking, 100ms timeout)
   - Retry up to 50 times (5 seconds total) with 100ms sleep between
   - On connection refused → wrangler not ready yet, sleep and retry
   - On success → wrangler is ready
6. Send the actual test request:
   ```
   GET http://127.0.0.1:{port}/
   ```
   - Read response body as text
   - Record status code
7. Kill wrangler process (and any child processes)
8. Return `WranglerOutput`

**Why poll:** wrangler dev takes a few seconds to compile the worker and start listening. The poll loop waits for the HTTP server to be responsive before sending the test request.

**Error cases:**
- `wrangler` not on PATH → anyhow::anyhow!("wrangler not found on PATH. Install: npm install -g wrangler")
- wrangler fails to start within timeout → anyhow::anyhow!("wrangler dev did not become ready within 5 seconds")
- HTTP request fails after wrangler is ready → anyhow::anyhow!("HTTP request to wrangler failed: {error}")
- wrangler exits non-zero → print stderr, return error

---

### `browser.rs` — Playwright Browser Test Runner

**Responsibility:** Launch a browser via Playwright, navigate to the test page, poll for results, return outcome.

**Entry point:**
- `pub fn run(url: &str, browser: Browser, headless: bool, integration_dir: &Path) -> anyhow::Result<BrowserOutput>`

**`BrowserOutput` struct:**
```rust
pub struct BrowserOutput {
    pub test_result: String,    // Content of #output element
    pub console_logs: Vec<String>,
    pub exit_code: i32,
}
```

**Flow:**

1. Verify `npx` is on PATH via `which::which("npx")`
2. Create a temporary directory for the Playwright script
3. Generate a Playwright Node.js script (written to a temp `.js` file):

   The script does the following:
   a. Import `chromium`/`firefox`/`webkit` from `playwright`
   b. Launch browser with `{ headless: true/false }`
   c. Create a new page
   d. Navigate to `url`
   e. Collect console messages via `page.on('console')`
   f. Poll `#output` element for text containing "test result: " or "Tests complete" (up to 30 seconds, 500ms interval)
   g. On timeout → print "TIMEOUT: test did not produce results", exit 1
   h. On success → read `#output.textContent`, print it, exit 0
   i. Collect all console messages, print them as JSON to stdout
   j. Close browser

4. Run the script via `npx playwright test` or more simply: `node {temp_script}`
   - Actually, for Playwright we need `npx playwright install {browser}` first if not installed
   - The script is run via `node` directly (not `npx playwright test` which expects a test framework)

   Revised: Run via `node {temp_script.js}`. Playwright is a Node.js package, so the script imports it directly.

   But `node` can't resolve `import { chromium } from 'playwright'` without a package.json. Options:
   a. Create a temporary package.json in the temp dir with `playwright` dependency, `npm install`, then `node script.js`
   b. Use `npx playwright {browser} {url}` — but this doesn't exist
   c. Use `npx -y playwright@latest` — npx can run playwright directly

   **Decision:** Write the script to use `playwright` and run it via `npx`:
   ```
   cd {temp_dir}
   npm init -y
   npm install playwright
   node script.js
   ```

   This ensures playwright is available. On subsequent runs, the temp dir can be reused if `node_modules` already exists.

   **Alternative (simpler):** Use `npx playwright install` to ensure browsers are installed, then run a script that uses the locally installed playwright. The testbed assumes the user has playwright installed or installs it on first run.

   **Simplest approach:** Generate the script, run it via:
   ```
   node --experimental-vm-modules {script}
   ```
   But this requires playwright to be available.

   **Practical approach:** The testbed uses `npx` to run playwright scripts:
   ```
   npx playwright install {browser-name}
   ```
   Then generates a CommonJS script (not ESM) to avoid module issues:
   ```js
   const { chromium } = require('playwright');
   ```

   Actually, `npx` with a specific package works:
   ```
   npx -y playwright@latest install chrome
   ```

   But running a script via npx is tricky. The cleanest approach:

   Create a temporary directory, write a `package.json` with `playwright` as a dependency, write the script, `npm install`, `node script.js`. On subsequent runs, if `node_modules` exists, skip `npm install`.

   **Implementation detail:** The temp dir is created with a stable name (e.g., `{crate_path}/.wasm-testbed-playwright/`) so that `node_modules` persists between runs. This avoids re-installing playwright every test.

   Flow:
   a. Create persistent cache dir: `{crate_path}/.wasm-testbed-playwright/`
   b. If `node_modules` doesn't exist there, run `npm init -y && npm install playwright` in that dir
   c. Write the test script to `{cache_dir}/run-test.js`
   d. Run `node {cache_dir}/run-test.js`
   e. Parse stdout for JSON-formatted console logs
   f. Read exit code

5. Parse the output: the script prints the `#output` text to stdout and console messages to stderr as JSON
6. Return `BrowserOutput`

**Playwright script (generated at runtime):**

```js
const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ headless: HEADLESS });
  const page = await browser.newPage();
  const logs = [];

  page.on('console', msg => logs.push({ type: msg.type(), text: msg.text() }));

  try {
    await page.goto('URL', { waitUntil: 'domcontentloaded', timeout: 30000 });

    // Poll for results
    let output = null;
    for (let i = 0; i < 60; i++) {
      output = await page.$eval('#output', el => el.textContent).catch(() => null);
      if (output && (output.includes('test result:') || output.includes('Tests complete'))) break;
      await page.waitForTimeout(500);
    }

    if (!output) {
      console.error('TIMEOUT: test did not produce results within 30 seconds');
      process.exit(1);
    }

    console.log(output);
    console.error(JSON.stringify(logs));
  } finally {
    await browser.close();
  }
})();
```

The `URL` and `HEADLESS` placeholders are replaced before writing the script.

**Browser mapping:**

| Browser arg | Playwright import | Launch call |
|-------------|------------------|-------------|
| chrome | `{ chromium }` | `chromium.launch()` |
| firefox | `{ firefox }` | `firefox.launch()` |
| safari | `{ webkit }` | `webkit.launch()` |

**Error cases:**
- `npx` / `node` not on PATH → anyhow::anyhow!("node/npx not found on PATH")
- Playwright installation fails → anyhow::anyhow!("Failed to install playwright: {error}")
- Browser not installed → anyhow::anyhow!("Browser not available. Run: npx playwright install {browser}")
- Page navigation timeout → anyhow::anyhow!("Browser test timed out after 30 seconds")
- Playwright script exits non-zero → print captured output, return error

---

## Test Mode Detailed Flows

### `test web <crate_path>`

1. Parse `TestArgs`: mode=web, crate_path, release, features, browser, headless
2. **Build:** `build::run(&crate_path, release, features)` → `BuildOutput`
3. **Copy wasm:** `fs::copy(build.wasm_path, crate_path/integrations/web/{package_name}.wasm)`
4. **Start HTTP server:** `server::start_serving(&crate_path/integrations/web)` → `TestServer`
5. **Run browser:** `browser::run(&server.url("index.html"), browser, headless, &integration_dir)` → `BrowserOutput`
6. **Shutdown server:** `server.shutdown()`
7. **Print results:** output from browser test
8. Exit code: 0 if tests passed, 1 otherwise

### `test deno <crate_path>`

1. Parse `TestArgs`
2. **Build:** `build::run()` → `BuildOutput`
3. **Copy wasm:** `fs::copy(build.wasm_path, crate_path/integrations/deno/{package_name}.wasm)`
4. **Run deno:** `deno::run(&crate_path/integrations/deno, "index.js")` → `DenoOutput`
5. **Print results:** deno stdout/stderr
6. Exit code: deno's exit code

### `test wrangler <crate_path>`

1. Parse `TestArgs`
2. **Build:** `build::run()` → `BuildOutput`
3. **Copy wasm:** `fs::copy(build.wasm_path, crate_path/integrations/wrangler/{package_name}.wasm)`
4. **Run wrangler:** `wrangler::run(&crate_path/integrations/wrangler, None)` → `WranglerOutput`
5. **Print results:** response body + stderr
6. Exit code: 0 if response status 2xx, 1 otherwise

### `test bindgen-web <crate_path>`

1. Parse `TestArgs`
2. **Build:** `build::run()` → `BuildOutput`
3. **wasm-bindgen:** `wasm::run_wasm_bindgen(&build.wasm_path, &crate_path/integrations/bindgen-web, BindgenTarget::Web)`
4. **Copy wasm:** (wasm-bindgen already outputs to the dir) Also need to copy `index.html` template — read from `TemplateDirectory`, replace `PACKAGE_NAME`, write to `integrations/bindgen-web/index.html`
5. **Discover tests:** `wasm_test::discover_tests(&crate_path/integrations/bindgen-web/{package_name}_bg.wasm)` → `Vec<String>`
6. **Generate run.js:**
   - Read `templates/bindgen-web/run.js` from `TemplateDirectory`
   - Replace `PACKAGE_NAME` with package name
   - Replace `/* TEST_NAMES */` with comma-separated quoted test names: `"__wbgt_foo", "__wbgt_bar"`
   - Write to `integrations/bindgen-web/run.js`
7. **Start HTTP server:** `server::start_serving(&crate_path/integrations/bindgen-web)` → `TestServer`
8. **Run browser:** `browser::run(&server.url("index.html"), browser, headless)`
9. **Shutdown server**
10. **Print results**

### `test bindgen-deno <crate_path>`

1. Parse `TestArgs`
2. **Build:** `build::run()` → `BuildOutput`
3. **wasm-bindgen:** `wasm::run_wasm_bindgen(&build.wasm_path, &crate_path/integrations/bindgen-deno, BindgenTarget::Deno)`
4. **Discover tests:** `wasm_test::discover_tests(&crate_path/integrations/bindgen-deno/{package_name}_bg.wasm)`
5. **Generate run.js:**
   - Read `templates/bindgen-deno/run.js`
   - Replace `PACKAGE_NAME` and `TEST_NAMES`
   - Write to `integrations/bindgen-deno/run.js`
6. **Run deno:** `deno::run(&crate_path/integrations/bindgen-deno, "run.js")`
7. **Print results**

### `test bindgen-wrangler <crate_path>`

1. Parse `TestArgs`
2. **Build:** `build::run()` → `BuildOutput`
3. **wasm-bindgen:** `wasm::run_wasm_bindgen(&build.wasm_path, &crate_path/integrations/bindgen-wrangler, BindgenTarget::EsModules)`
4. **Discover tests:** `wasm_test::discover_tests(&crate_path/integrations/bindgen-wrangler/{package_name}_bg.wasm)`
5. **Generate worker.js:**
   - Read `templates/bindgen-wrangler/worker.js`
   - Replace `PACKAGE_NAME` and `TEST_NAMES`
   - Write to `integrations/bindgen-wrangler/worker.js`
6. **Generate wrangler.toml:**
   - Read `templates/bindgen-wrangler/wrangler.toml`
   - Write to `integrations/bindgen-wrangler/wrangler.toml` (no placeholders to replace)
7. **Run wrangler:** `wrangler::run(&crate_path/integrations/bindgen-wrangler, None)`
8. **Print results**

---

## Error Handling Strategy

All functions return `anyhow::Result<T>`. Error chains use `anyhow::Context` for context:

```rust
let wasm_path = build_output.wasm_path
    .clone()
    .context("wasm binary path from build output")?;
```

User-facing errors should be actionable — tell the user what to install, what command to run, what went wrong.

**Error categories:**

| Category | Example | Actionable message |
|----------|---------|-------------------|
| Missing tool | wasm-bindgen not found | "Install: cargo install wasm-bindgen-cli" |
| Missing target | wasm32-unknown-unknown not installed | "Run: rustup target add wasm32-unknown-unknown" |
| Build failure | cargo build errors | Show cargo output, "Fix the compilation errors above" |
| Missing integration | integrations/web/ doesn't exist | "Run: wasm-testbed init web ./crate" |
| Timeout | wrangler didn't start | "Check wrangler logs below" |
| Test failure | wasm tests failed | Show test output, "Tests failed" |

---

## Integration with Existing Crates

### `foundation_macros::EmbedDirectoryAs`

Used in `init.rs` for template embedding:

```rust
#[derive(foundation_macros::EmbedDirectoryAs)]
#[source = "$CURRENT_CRATE/src/templates"]
struct TemplateDirectory;
```

This gives:
- `TemplateDirectory.read_utf8_for("web/index.html")` → `Option<Vec<u8>>`
- In debug: reads from `src/templates/web/index.html` on disk every call
- In release: reads from compiled-in bytes

### `foundation_nostd::embeddable::EmbeddableDirectory`

The trait implemented by the derive above. Used via `read_utf8_for()`.

### Not used: `foundation_http`

The test HTTP server uses `tiny_http` instead of `foundation_http` because:
1. `foundation_http` requires `foundation_core` with valtron executor
2. `StaticFileHandler`'s `ServeFactory::create()` panics without a context bag setup
3. The test server is a simple static file server — doesn't need valtron, connection pooling, or keep-alive
4. `tiny_http` is a well-known, minimal HTTP server library perfect for this use case

If the team prefers `foundation_http`, a wrapper `ServeFactory` implementation would be needed to pass the root directory. The spec recommends `tiny_http` for simplicity.

---

## Testing Strategy

### Unit Tests (in-crate `#[cfg(test)]` modules)

- `wasm_test::discover_tests` — test with a known wasm binary that has `__wbgt_` exports
- `build::run` — test with a minimal Cargo crate (create temp crate, build, verify wasm)
- Template generation — test that `PACKAGE_NAME` replacement works correctly

### Integration Tests (`tests/` directory)

- `tests/init_test.rs` — create temp crate, run `init web`, verify files created
- `tests/deno_test.rs` — create temp wasm crate with a simple exported function, run `test deno`, verify output
- `tests/bindgen_deno_test.rs` — create temp wasm crate with `#[wasm_bindgen_test]`, run `test bindgen-deno`, verify test discovery and execution

---

## Success Criteria

- [ ] `cargo check -p foundation_wasm_testbed` passes with no warnings
- [ ] `cargo clippy -p foundation_wasm_testbed` passes
- [ ] `wasm-testbed init web ./my_crate` creates `integrations/web/` with 3 files
- [ ] `wasm-testbed test deno ./my_crate` builds wasm and runs deno test
- [ ] `wasm-testbed test bindgen-web ./my_crate` discovers `__wbgt_` tests and runs in browser
- [ ] `wasm-testbed test wrangler ./my_crate` runs wrangler dev and curls results
- [ ] `cargo test -p foundation_wasm_testbed` passes unit tests
- [ ] `cargo test -p foundation_wasm_testbed --test init_test` passes init integration test

---

_Created: 2026-05-18_
_Last updated: 2026-06-01 — Full implementation spec with every module, type, flow, and error case_
