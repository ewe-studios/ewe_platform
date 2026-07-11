# Running JS and WASM Tests

The platform has three testing paths for JS/WASM/browser code:

1. **Owned `#[wasm_test]` / `#[valtron_wasm_test]`** — foundation_wasm ABI,
   no wasm-bindgen. Runs in embedded Deno (in-process V8) or Chromium via
   CDP. Used by `foundation_core`, `foundation_wasm`.
2. **Wasm-bindgen `#[wasm_bindgen_test]` / `#[valtron_bindgen]`** — for
   crates that use `web_sys` / `js_sys` (browser APIs like WebSocket, fetch,
   DOM). Runs in Chromium via CDP. Used by `foundation_netio`'s wasm client
   tests.
3. **Native browser tests via `foundation_browser::test::Harness`** —
   plain `#[test]` functions, no wasm32. Launches real Chromium/Firefox,
   serves HTML via `foundation_http`, drives the page over CDP/BiDi. Used
   by `foundation_browser`'s own smoke tests and `foundation_wasm_ui`'s
   cross-browser UI tests.

### Valtron-pool macros (F52)

Two convenience macros auto-init the valtron pool so `valtron::execute()`
and `valtron::spawn()` work without manual setup:

| Macro | Path | Use with |
|---|---|---|
| `#[valtron_wasm_test]` | `foundation_macros` | Owned `#[wasm_test]` tests |
| `#[valtron_bindgen]` | `foundation_macros` | Wasm-bindgen `#[wasm_bindgen_test]` tests |

Both initialise a single-threaded pool before the test body and tear it
down afterwards. For the owned path, replace `#[wasm_test]` with
`#[valtron_wasm_test]`. For bindgen, use `#[valtron_bindgen]` instead of
`#[wasm_bindgen_test]` — it also emits the browser-mode link-section marker
(`wasm_bindgen_test_configure!(run_in_browser)` equivalent) so no separate
`configure!()` call is needed.

## Quick Reference

| Command | Test System | Runtime | Wasm32? |
|---|---|---|---|
| `wasm-testbed deno <crate>` | `#[wasm_test]` (owned) | Embedded Deno (V8) | Yes |
| `wasm-testbed browser <crate>` | `#[wasm_test]` (owned) | Chromium (CDP) | Yes |
| `wasm-testbed bindgen <crate>` | `#[wasm_bindgen_test]` | Chromium (CDP) | Yes |
| `cargo test --features browser-tests` | `#[test]` (native) | Chromium / Firefox | No |

The `wasm-testbed` commands build the crate to `wasm32-unknown-unknown`
automatically. Native browser tests run directly on the host.

---

## Owned `#[wasm_test]` (Deno / Browser)

The **default** test path for foundation crates. Uses `foundation_wasm`'s ABI
and `foundation_macros::wasm_test` — no wasm-bindgen.

### Setup

Add to your crate's `Cargo.toml`:

```toml
[dependencies]
foundation_wasm = { path = "../foundation_wasm", features = ["web"] }

[dev-dependencies]
foundation_macros = { path = "../foundation_macros" }
```

Add to `lib.rs`:

```rust
#[cfg(target_arch = "wasm32")]
mod wasm_tests;
```

Write tests in `src/wasm_tests.rs`:

```rust
use foundation_macros::wasm_test;

#[wasm_test]
fn it_works() {
    assert_eq!(2 + 2, 4);
}

#[wasm_test]
async fn async_works() {
    assert!(true);
}

#[wasm_test(should_panic)]
fn panics_are_expected() {
    panic!("this is fine");
}
```

### Running

```bash
# In-process (no browser, no node — embeds V8 via deno_core)
cargo run -p foundation_testbed --features wasm,wasm-embedded-js --bin wasm-testbed -- \
  deno <crate-path>

# In real Chromium via CDP
cargo run -p foundation_testbed --features wasm --bin wasm-testbed -- \
  browser <crate-path> --headless
```

**Prerequisites for `deno` mode:** none. The embedded Deno runtime links V8
in-process (`wasm-embedded-js` feature). No external `node` or `deno` binary.

**Prerequisites for `browser` mode:** Chromium on PATH.

### How it works

1. `cargo build --target wasm32-unknown-unknown` (lib only, LLVM backend)
2. Discovers `__fwt_*` function exports + `__fwt_manifest` custom section
3. Stages a self-contained harness (`runner.mjs` + `foundation-wasm.js`) in a temp dir
4. Runs: embedded Deno (in-process) or Chromium (CDP)

Each test case runs in a **fresh WebAssembly instance** (panic aborts the
instance, so isolation matters).

---

## Wasm-bindgen `#[wasm_bindgen_test]` (Bindgen)

For crates that use `web_sys`/`js_sys` (browser APIs like `WebSocket`, `fetch`,
DOM) — the `bindgen` command runs them in Chromium via CDP.

### Setup

Add to your crate's `Cargo.toml`:

```toml
[dev-dependencies]
foundation_testbed = { path = "../foundation_testbed", default-features = false, features = ["wasm-bindgen-test"] }
# wasm-bindgen-test must be a direct dev-dep (proc-macro attributes cannot be
# re-exported across crate boundaries).
wasm-bindgen-test = "=0.3.76"
# For #[valtron_bindgen] — the one-step macro with pool init.
foundation_macros = { path = "../foundation_macros" }

[[test]]
name = "wasm"
path = "tests/wasm/mod.rs"
required-features = ["wasm-fetch"]
```

Move native-only dev-deps to target-gated:

```toml
[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]
tokio = { ... }
# ... other native-only dev-deps
```

Create `tests/wasm/mod.rs`:

```rust
#![cfg(target_arch = "wasm32")]

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

mod my_module;
```

Write tests in `tests/wasm/`:

```rust
use wasm_bindgen_test::wasm_bindgen_test;
use foundation_testbed::bindgen::{js_sys, web_sys};

#[wasm_bindgen_test]
fn my_browser_test() {
    let window = web_sys::window().expect("window in browser");
}
```

### Valtron-pool bindgen macro

For tests that need `valtron::execute()` / `valtron::spawn()`, use
`#[valtron_bindgen]` (from `foundation_macros`) — it combines browser-mode
marker + single-threaded pool init + wasm-bindgen test discovery:

```rust
use foundation_macros::valtron_bindgen;
use foundation_core::valtron::{self, Stream};
use foundation_testbed::bindgen::{js_sys, web_sys};

#[valtron_bindgen]
fn my_test_with_valtron() {
    let (task, delivery) = client.open_websocket_task("ws://...", config).unwrap();
    let mut stream = valtron::execute(task, None).unwrap();
    // pool is live, use execute()/spawn() freely
}
```

No separate `wasm_bindgen_test_configure!(run_in_browser)` needed.

### `open_websocket_task` — boxed TaskIterator for the transport path

`WebSocketConnector` has two open methods:

| Method | Returns | Use case |
|---|---|---|
| `open_websocket()` | `WebSocketClient` | Simple iteration (`client.messages()`) |
| `open_websocket_task()` | `(WsExchangeTask, MessageDelivery)` | Transport pump — the task is a `Box<dyn TaskIterator>` you spawn on your own pool |

`WsExchangeTask` mirrors `HttpExchangeClientTask` — same `TaskIterator` shape
with shared `Ready`/`Pending`/`Spawner` types. Native spawns the task via
`valtron::execute()`; wasm wraps the browser bridge in a `Stream → TaskStatus`
adapter. The caller never sees platform-specific types.

```rust
use foundation_netio::websocket::shared::connector::WebSocketConnector;

let (task, delivery) = client
    .open_websocket_task("ws://...", WebSocketConnectConfig::new())
    .expect("open_websocket_task");
// task: Box<dyn TaskIterator<Ready=Result<WebSocketMessage,WebSocketError>, Pending=WsProgress, Spawner=BoxedSendExecutionAction> + Send>
let stream = valtron::execute(task, None).unwrap();
```

### The unified client handle

`NetClient` (supertrait of `HttpClient + WebSocketConnector`) and
`DynNetClient` (`Arc<dyn NetClient>`) let transports hold one handle for both
HTTP and WebSocket:

```rust
use foundation_netio::network_client::{DynNetClient, NetClient};
use foundation_netio::HttpClientBuilder;

let client: DynNetClient = HttpClientBuilder::new().build().into();
// client can do both: client.open_exchange(...) and client.open_websocket_task(...)
```

### Running

```bash
cargo run -p foundation_testbed --no-default-features --features wasm,wasm-bindgen-test --bin wasm-testbed -- \
  bindgen backends/foundation_netio --headless --features wasm-fetch
```

**Prerequisites:**
- **Chromium** on PATH
- **wasm-bindgen CLI** installed: `cargo install wasm-bindgen-cli --version 0.2.126`
- Version checked at startup (CLI `>=` crate version required)

### How it works

1. `cargo build --target wasm32-unknown-unknown --tests` (LLVM backend, `--tests` so `#[cfg(test)]` expansions compile)
2. Finds the test wasm by scanning `deps/` for `__wbgt_` byte sequence
3. Runs `wasm-bindgen --target web` → generates JS glue in a temp dir
4. Writes `index.html` + `run.js` (embedded templates) into the temp dir
5. Serves via `foundation_http` static file handler
6. Launches Chromium via `foundation_browser::BrowserDriver` (CDP)
7. Polls `<pre id="output">` for `test result:` sentinel
8. Prints verdict, exits 0 (pass) or 1 (fail)

The `run.js` template at `templates/bindgen-web/run.js` imports the bindgen
glue, inits the module, creates a `WasmBindgenTestContext`, and runs all
discovered `__wbgt_*` tests. `WasmBindgenTestContext` needs `<pre id="output">`
in the DOM — the template provides it.

---

## Version Pinning

`foundation_testbed` pins the wasm-bindgen ecosystem:

| Crate | Version |
|---|---|
| `wasm-bindgen` | `=0.2.126` |
| `wasm-bindgen-test` | `=0.3.76` |
| `wasm-bindgen-futures` | `=0.4.76` |
| `js-sys` | `=0.3.103` |
| `web-sys` | `=0.3.103` |

The `wasm-bindgen-cli` on PATH must be `>= 0.2.126`. To update, change the pins
in `Cargo.toml` + the constant in `src/bindgen.rs` + reinstall the CLI.

---

## Native Browser Tests (foundation_browser::test::Harness)

For testing browser functionality directly — DOM interaction, CDP/BiDi
protocols, screenshots, reactive UI, SSE streaming — **without** wasm32
compilation. These are plain `#[test]` functions that launch a real
browser, serve HTML over `foundation_http`, and drive the page.

### When to use this

- Testing `foundation_browser` itself (CDP/BiDi engine, Locator, Page API)
- Testing `foundation_wasm_ui` components in a real browser
- Any test that serves HTML to a browser and inspects the result
- Cross-browser parity tests (Chromium + Firefox over BiDi)

### Setup

The crate under test already depends on `foundation_browser`:

```toml
[dependencies]
foundation_browser = { path = "../foundation_browser" }

[features]
browser-tests = []  # gate so CI without a browser skips them
```

### API: `BrowserDriver` (low-level)

Direct CDP/BiDi access — launch, navigate, eval, screenshot:

```rust
use foundation_browser::{BrowserDriver, LaunchConfig};

#[test]
fn low_level_example() {
    let driver = BrowserDriver::launch(LaunchConfig::chromium().headless(true))
        .expect("launch chromium");
    let page = driver.new_page().expect("attach a page");

    // Navigate to `data:` URL (no server needed)
    page.goto("data:text/html,<h1 id='hi'>hello</h1>").expect("navigate");

    // Read DOM over CDP
    let text = page.eval("document.getElementById('hi').textContent")
        .expect("evaluate");
    assert_eq!(text.as_str(), Some("hello"));

    // Screenshot
    page.screenshot("target/smoke.png").expect("screenshot");
}
```

`BrowserDriver` speaks CDP natively over `foundation_netio`'s WebSocket
client — no Node.js, no Playwright, no external driver binary.

### API: `Harness` + `TestConfig` (high-level)

For tests that need a server + browser together, with deterministic
setup/teardown. The server serves HTML, static assets, and SSE streams:

```rust
use foundation_browser::test::{Harness, TestConfig};

#[test]
fn harness_example() {
    let harness = Harness::setup(TestConfig {
        html: "<button id='go'>Go</button><div id='out'></div>\
               <script>document.getElementById('go').onclick=\
                 function(){document.getElementById('out').textContent='done'}</script>"
            .into(),
        ..TestConfig::default()
    }).expect("setup server + browser");

    harness.run("my_test", |_server, page| {
        page.locator("#go").click()?;
        page.locator("#out").expect().to_have_text("done")?;
        Ok(())
    });
    // run() consumed the harness → server stopped, browser closed.
}
```

**TestConfig fields:**

| Field | Default | Description |
|---|---|---|
| `html` | `"<!doctype html>..."` | Inline HTML served on `/` |
| `file` | `None` | HTML file to serve on `/` (takes precedence over `html`) |
| `static_dir` | `None` | Directory of static assets (wasm, JS, CSS) |
| `static_mount` | `"/assets"` | URL prefix for `static_dir` |
| `host` | `"127.0.0.1"` | Bind host |
| `port` | `0` | Bind port (0 = ephemeral) |
| `headless` | `true` | Run browser headless |
| `browser` | `Chromium` | `Chromium` or `Firefox` (BiDi) |
| `encoding` | `Json` | Wire encoding for SSE channel |
| `https` | `false` | Serve over TLS (bundled dev cert) |
| `window_size` | `None` | Browser window `(width, height)` |
| `headers` | `[]` | Extra response headers |

### API: `#[wasm_ui_server]` macro

The ergonomic form — owns setup + teardown via attribute macro:

```rust
use foundation_browser::wasm_ui_server;

#[wasm_ui_server(html = "<button id='go'>Go</button><div id='out'></div>\
    <script>document.getElementById('go').onclick=\
      function(){document.getElementById('out').textContent='ok'}</script>")]
fn macro_drives_a_served_page(
    _server: &foundation_browser::test::TestServer,
    page: &foundation_browser::Page,
) -> foundation_browser::Result<()> {
    page.locator("#go").click()?;
    page.locator("#out").expect().to_have_text("ok")?;
    Ok(())
}
```

The macro expands into `Harness::setup()` + `Harness::run()` —
server boot, browser launch, page navigation, body execution, and
deterministic teardown (browser closes before server stops, even on
panic). Creates one `#[test]` function per annotated function.

### Locator API

Both `Page` and `Harness` give access to the **Locator** API for
selecting and asserting on DOM elements:

```rust
// Count, geometry, visibility
page.locator(".item").expect().to_have_count(2)?;
let rect = page.locator("#b").bounding_box()?;
page.locator("#b").expect().to_be_visible()?;
page.locator("#hidden").expect().to_be_hidden()?;

// Attributes + computed style
page.locator("#b").expect().to_have_attribute("data-state", "off")?;
page.locator("#b").expect().to_have_css("display", "block")?;

// Trusted interactions (CDP Input domain)
page.locator("#b").click()?;
page.locator("#in").fill("hello")?;
page.locator("#b").hover()?;

// Text content
page.locator("#b").expect().to_have_text("Clicked")?;
```

### SSE streaming + reactive UI

The `TestServer` has a built-in SSE broadcaster for streaming DOM updates
from Rust to the browser — used to test `foundation_wasm_ui`'s live
rendering:

```rust
use foundation_browser::test::{BroadcastSink, Encoding};
use foundation_ui_traits::JsonEncoder;
use foundation_wasm_ui::{html, App};

let harness = Harness::setup(TestConfig {
    html: "<mount-stream api='/__primal/stream' transport='sse'></mount-stream>\
           <script type='module'>import {registerWebComponents}\
             from'/__primal/foundation-wasm-ui.js';registerWebComponents()</script>"
        .into(),
    encoding: Encoding::Json,
    ..TestConfig::default()
}).expect("setup");

harness.run("sse_example", |server, page| {
    let sink = BroadcastSink::with_encoder(JsonEncoder, server.broadcaster());
    let app = App::with_protocol(sink);
    let (ctx, rcv) = app.context();
    let (msg, set_msg) = ctx.signal("hello".to_string());
    let m = msg.clone();
    app.mount(html! { ctx, rcv, <div id="msg">{m.get()}</div> });
    app.stabilize();

    page.locator("#msg").expect().to_have_text("hello")?;

    set_msg.set("updated".to_string());
    app.stabilize();
    page.locator("#msg").expect().to_have_text("updated")?;
    Ok(())
});
```

### Running

```bash
# Chromium smoke tests (gated behind browser-tests feature)
cargo test -p foundation_browser --features browser-tests

# Headful debugging
PRIMAL_TEST_HEADFUL=1 cargo test -p foundation_browser --features browser-tests -- \
  --nocapture harness_serves_drives_and_tears_down

# Firefox tests (skipped gracefully if not installed)
PRIMAL_TEST_HEADFUL=1 cargo test -p foundation_browser --features browser-tests -- \
  firefox
```

**Prerequisites:**
- **Chromium** on PATH (for CDP tests)
- **Firefox** on PATH (optional, for BiDi parity tests)
- No wasm32 target, no wasm-bindgen, no Node.js

### Architecture

```
┌─────────────────────────────────────────────┐
│  Rust test thread                           │
│  ┌───────────────────────┐                  │
│  │ Harness / BrowserDriver│                 │
│  │  ├─ TestServer (HTTP)  │──port:0────────│
│  │  │  serves HTML+assets │                 │
│  │  │  SSE stream (/__primal/stream)        │
│  │  ├─ BrowserDriver      │──CDP WS────────│
│  │  │  Page.locator()     │                 │
│  │  │  Page.eval()        │                 │
│  │  │  Page.screenshot()  │                 │
│  │  └─ teardown           │                 │
│  └───────────────────────┘                  │
└─────────────────────────────────────────────┘
         │ HTTP (foundation_http)    │ CDP/WebSocket (foundation_netio)
         ▼                           ▼
┌────────────────┐    ┌──────────────────────┐
│  HTTP Server    │    │  Chromium / Firefox   │
│  (background    │    │  (headless or visible) │
│   thread)       │    │                       │
└────────────────┘    └──────────────────────┘
```

The entire stack is our own: `foundation_http` for the server,
`foundation_netio`'s WebSocket client for CDP/BiDi, `foundation_browser`
for the driver. No external tools.

---

## FAQ

### Which test path should I use?

| Scenario | Use |
|---|---|
| Rust wasm logic (no browser APIs) | `#[wasm_test]` + `wasm-testbed deno` |
| Rust wasm logic + browser APIs (window, DOM) | `#[wasm_test]` + `wasm-testbed browser` |
| `web_sys` bridges (fetch, WebSocket, etc.) | `#[wasm_bindgen_test]` + `wasm-testbed bindgen` |
| Above + need `valtron::execute()`/`spawn()` | `#[valtron_bindgen]` or `#[valtron_wasm_test]` |
| Transport pump (boxed TaskIterator) | `open_websocket_task()` via `WebSocketConnector` |
| Unified HTTP+WS client handle | `NetClient` / `DynNetClient` from `network_client` |
| CDP/BiDi protocol, Locator, screenshots | `foundation_browser::test::Harness` (native `#[test]`) |
| `foundation_wasm_ui` components in real browser | `#[wasm_ui_server]` or `Harness` with SSE |
| Cross-browser parity (Chromium + Firefox) | `Harness` with `Browser::Chromium` / `Browser::Firefox` |

### Can `#[wasm_test]` and `#[wasm_bindgen_test]` coexist?

Yes. Different export prefixes (`__fwt_` vs `__wbgt_`) and different build modes
(lib vs `--tests`). They never interfere.

### Why can't foundation_testbed re-export `#[wasm_bindgen_test]`?

Rust does not support re-exporting proc-macro attributes across crate
boundaries. The consumer must have `wasm-bindgen-test` as a direct dev-dep.
`foundation_testbed` brings everything else: `js-sys`, `web-sys`,
`wasm-bindgen-futures`, version checking, the Chromium driver, and templates.

### How are native browser tests different from wasm-testbed browser tests?

Native browser tests (`foundation_browser::test::Harness`) run directly on
the host as plain `#[test]` functions — no wasm32 compilation, no
wasm-bindgen. They launch a browser, serve HTML via `foundation_http`, and
drive the page over CDP/BiDi. This is for testing the browser automation
machinery itself and server-driven UI components.

`wasm-testbed browser` compiles your crate to wasm32 and runs it in
Chromium — for testing `#[wasm_test]` cases that compile to wasm32.

`wasm-testbed bindgen` compiles with wasm-bindgen and runs in Chromium —
for testing `web_sys`-based browser bridges.

### Test times out in Chromium

`page.goto()` has a 30s timeout (in `browser.rs`). Large wasm modules may
exceed this on first compile. Try `--release` for smaller binaries.

### How do I debug a failing browser test?

The test page has `<pre id="console_log">` and `<pre id="console_error">`.
The browser driver reads `#output` for the verdict. To see more:

```bash
# Run Chromium manually against the staged files:
find /tmp -name ".tmp*" -type d -newer <crate> # find the staged dir
cd /tmp/.tmpXXXXX
python3 -m http.server 9999 &
chromium --headless --disable-gpu --no-sandbox --virtual-time-budget=15000 \
  --dump-dom http://localhost:9999/index.html
```
