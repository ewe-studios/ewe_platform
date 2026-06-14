# Spec 43 — Rust-native browser testing (`#[wasm_ui_server]` + an owned CDP/BiDi driver)

Status: **COMPLETE** (2026-06-14). Both phases ship and are green: the pure-Rust
driver (CDP **and** WebDriver BiDi behind one `Backend` seam), the
`#[wasm_ui_server]` macro, the `foundation_http`-backed `TestServer`, live
server-driven-UI testing across all four protocols against **real Chromium AND
Firefox**, the HTTPS dev-cert mode, the `primal-test` helper, and node/Playwright
retirement. Only WebKit/Safari remains explicitly out of scope. See
[§14 Status](#14-status--what-shipped). Original design pass over the existing
stack + the Playwright references
(`@formulas/src.UIFrameworks/src.playwright/{playwright,playwright-python}`).

## 1. Why we are unusually well-positioned

We already own every layer this needs, in pure Rust, no async runtime:

- **An HTTP/1.1 server we can boot in a thread** — `foundation_http`
  (`Server::serve(&shutdown)` / `serve_with_listener(listener, &shutdown)`,
  bind to `:0` for an ephemeral port), with an App/router/middleware builder.
- **WebSocket, server AND client, surfaced through `foundation_http`** — built on
  `foundation_netio::websocket`: a full frame codec (`shared/{frame,assembler,
  batch_writer}.rs`), a native server (`native/{server,connection,task}.rs`), and
  a ready **client**: `native::connection::WebSocketClient::connect(...)` →
  `WebSocketConnection { send, recv, messages(), close }` plus a queue-backed
  `MessageDelivery` for concurrent sends. **This is the CDP/BiDi dial-out path with
  nothing new to build** — the driver's JSON-RPC engine is `WebSocketClient::connect`
  + a `recv` reader thread.
- **SSE streaming, server AND client** — server: `foundation_http::native::upgrade::
  {SseStream, SseEvent}`; client: `foundation_netio::event_source` (consumer/parser/
  reconnecting task). Both directions are owned, so channel A (our app stream) and a
  Rust SSE consumer (if a browser/endpoint speaks SSE) are both in hand.
- **A DOM-op wire protocol + hydration** — `foundation_wasm_ui`
  (`protocol/{json,columnar}.rs`, `instruction/receiver.rs`) and the scoped-script
  hydrator. This is the channel that already streams UI changes to the browser.
- **TLS** — `foundation_netio::netcap::ssl` + `foundation_testing::http::tls_server`.
- **Test-server helpers** — `foundation_testing::http::{server,sse_server,websocket_server,tls_server}`.

The crucial realization: **CDP (Chrome DevTools Protocol) and WebDriver BiDi are
just JSON-RPC over a WebSocket.** There is no magic in Playwright's browser link —
its Node/Python clients are JSON-RPC peers; the driver process speaks CDP/Juggler/
WebKit-inspector to the browser. We can speak those wire protocols **directly from
Rust** and delete the Node + Playwright dependency the current testbed shells out to
(`foundation_wasm_testbed/src/browser.rs` generates a Playwright JS script and runs
it under `node`).

## 2. Goals / non-goals

**Goals**
- Write component/integration tests in pure Rust that drive a REAL browser
  (Chromium first; Firefox/WebKit on a roadmap), with commands originating from
  Rust and delivered directly to the browser over its native JSON-RPC protocol.
- A `#[wasm_ui_server]` attribute macro that, for a test, boots our HTTP server in
  a background thread (configurable bind/port/https), serves the app under test +
  a thin in-page helper, launches a browser, and hands the test a driver handle.
- Inspect what the browser actually rendered: element presence, geometry
  (box model), text/attributes/computed style, **screenshots**, and dispatch
  **trusted input** — all from Rust.
- Fail fast and clearly when a browser (or its deps) is not installed, with
  `mise` tasks that install everything per-distro (Debian/Ubuntu/Arch) and are
  wired into the project templates.
- A thin injected JS wrapper for the few things genuinely suited to in-page
  execution (await reactive state, one-round-trip batched layout reads,
  app-specific event hooks) — riding our existing SSE/WS transport, not a
  separately-maintained agent framework.

**Non-goals (v1)**
- Re-implementing all of CDP/BiDi. We implement the slice the driver API needs.
- WebKit/Safari on day one (it's the hardest link — §6).
- Replacing `wasm-bindgen-test`/headless unit runners; this is for real-DOM,
  real-paint, real-input behavior (exactly the M1–M8 machinery we just built).

## 3. Architecture — two JSON channels, one Rust API

```
   ┌─────────────────────────── Rust test process ───────────────────────────┐
   │                                                                          │
   │  #[wasm_ui_server] test fn(server: &TestServer, page: &Page)             │
   │            │                                   │                         │
   │            │ (A) app/stream channel            │ (B) control channel     │
   │            ▼                                   ▼                         │
   │  foundation_http server (thread)        BrowserDriver (pure Rust)        │
   │   • serves wasm UI app + helper          • JSON-RPC over WS client       │
   │   • streams DOM ops (SSE/WS)             • CdpClient / BiDiClient         │
   │   • TestServer handle: push streams      • Page / Locator / assertions   │
   └────────────┬───────────────────────────────────┬─────────────────────────┘
                │ HTTP + SSE/WS (our protocol)        │ ws://…/devtools (CDP) or
                ▼                                      ▼ ws://…/session (BiDi)
        ┌───────────────── Browser (chromium/firefox) ─────────────────┐
        │  page: our wasm UI runtime  ◀── DOM ops ── (A)               │
        │        + thin primal-test helper (injected, optional)        │
        │  devtools endpoint  ◀──── JSON-RPC commands ──── (B) ─────▶   │
        └──────────────────────────────────────────────────────────────┘
```

**Channel A — app/stream (our protocol).** The `#[wasm_ui_server]` server serves
the app under test and streams DOM ops over SSE/WS exactly as production does. The
test's `TestServer` handle can *push* state/streams into the page (the user's "a
handle the function receives to send streams of changes to the browser"). This is
the system-under-test's own transport — we test the streaming path for real.

**Channel B — control (native JSON-RPC, the driver).** A pure-Rust client dials the
browser's debugging WebSocket and speaks CDP (Chromium) or WebDriver BiDi
(Firefox/cross-browser). This is the test's eyes and hands: navigate, query the
DOM, read the box model, screenshot, dispatch trusted input, capture console/network.
Commands originate in Rust; nothing external translates them.

**Thin injected JS helper (rides A, complements B).** A small script
(`primal-test`) hydrated like our other scoped scripts exposes in-page-suited
verbs: `waitForReactive(predicate)`, `batchLayout([selectors]) -> rects`,
`dispatchAppEvent(name, detail)`. The driver invokes these either via CDP
`Runtime.evaluate` (one round trip) or over channel A. It is a convenience layer,
not the primary mechanism — anything it does, the driver can also do via native
protocol domains.

### Why both channels (and not JS-only)

In-page JS can do presence, geometry (`getBoundingClientRect`), text, attributes,
computed style, scrolling, and synthetic events. It **cannot** take a pixel-accurate
viewport screenshot (no in-page API; `html2canvas` re-renders and lies) and cannot
produce **trusted** input events (`isTrusted=false` changes focus/security-gated
behavior). Those require the native protocol (`Page.captureScreenshot`,
`Input.dispatchMouseEvent`). So the native driver is primary; the JS helper is an
ergonomic complement for reactive-wait and batched reads.

## 4. The `#[wasm_ui_server]` macro (lives in `foundation_macros`)

Signature forms:

```rust
#[wasm_ui_server]                                  // ephemeral 127.0.0.1:0, http
#[wasm_ui_server(port = 8080)]                     // fixed port
#[wasm_ui_server("0.0.0.0", 8443, https)]          // bind, port, TLS (pre-gen cert)
#[wasm_ui_server(browser = "firefox", headless = false)]
fn dialog_opens_and_traps_focus(server: &TestServer, page: &Page) -> TestResult {
    // 1. mount the component under test (server already serving it)
    server.mount(|ctx, rcv| dialog(ctx, rcv, DialogConfig::default(), &open, set_open, slots));
    // 2. drive + assert via the pure-Rust driver
    page.goto(server.url())?;
    page.locator("[data-dialog-close]").click()?;        // trusted input via CDP
    page.locator("dialog").expect().to_be_hidden()?;     // box model / computed style
    page.screenshot("dialog-closed.png")?;               // Page.captureScreenshot
    Ok(())
}
```

What the macro expands to (sketch):
1. Resolve config (bind/port/scheme/browser/headless) with env overrides
   (`PRIMAL_TEST_BROWSER`, `PRIMAL_TEST_HEADLESS`, `PRIMAL_TEST_PORT`).
2. Build a `TestServer`: a `foundation_http` App serving (a) the app shell +
   wasm bundle, (b) the SSE/WS DOM-op stream, (c) the injected `primal-test`
   helper, (d) a `/__test__/health` route. Bind, spawn `Server::serve` on a
   thread with an `OnSignal` shutdown; read back the bound `SocketAddr`.
3. Launch the browser via `BrowserSupervisor` (§7), get a `BrowserDriver`,
   open a `Page`. **Fail with an actionable error** (`run: mise run test:browsers`)
   if the binary/deps are missing.
4. Run the user's body with `(&server, &page)`; on panic/Err, capture a
   screenshot + console log to `target/primal-test/<name>/` before tearing down.
5. Teardown: close page, kill browser, signal server shutdown, join thread.

The macro is sync (our whole stack is). Each test gets an isolated server +
fresh browser context, so tests parallelize under `cargo nextest`.

## 5. The pure-Rust driver

A protocol-agnostic Rust API over a `WireProtocol` trait so CDP and BiDi share one
surface.

```rust
pub trait WireProtocol {                       // the JSON-RPC engine
    fn send(&self, method: &str, params: Json) -> Result<Json>;   // request → result
    fn on_event(&self, method: &str, cb: EventCb);                // subscribe
}

pub struct BrowserDriver { proto: Box<dyn WireProtocol>, /* process handle */ }
pub struct Page<'d> { /* targetId/sessionId or BiDi context */ }
pub struct Locator<'p> { selector: String, /* page ref */ }

impl Page<'_> {
    pub fn goto(&self, url: &str) -> Result<()>;
    pub fn locator(&self, sel: &str) -> Locator<'_>;
    pub fn screenshot(&self, path: impl AsRef<Path>) -> Result<()>;
    pub fn eval(&self, js: &str) -> Result<Json>;          // escape hatch (Runtime.evaluate)
    pub fn wait_for(&self, sel: &str, opts: WaitOpts) -> Result<Locator<'_>>;
}

impl Locator<'_> {
    pub fn click(&self) -> Result<()>;                     // box model centre → Input.dispatchMouseEvent
    pub fn hover(&self) -> Result<()>;
    pub fn fill(&self, text: &str) -> Result<()>;          // focus + Input.insertText / dispatchKeyEvent
    pub fn press(&self, key: &str) -> Result<()>;
    pub fn text(&self) -> Result<String>;
    pub fn attribute(&self, name: &str) -> Result<Option<String>>;
    pub fn bounding_box(&self) -> Result<Rect>;            // DOM.getBoxModel
    pub fn computed_style(&self, prop: &str) -> Result<String>;
    pub fn expect(&self) -> LocatorAssertions;             // to_be_visible/hidden/have_text/…
}
```

**Command→domain mapping (CDP, Chromium):**

| Driver op | CDP |
|-----------|-----|
| `goto` | `Page.navigate` + `Page.loadEventFired` |
| `locator(sel)` resolve | `DOM.getDocument` → `DOM.querySelector` (or `Runtime.evaluate` returning a `RemoteObject`) |
| `bounding_box` | `DOM.getBoxModel` |
| `text` / `attribute` / `computed_style` | `DOM.getOuterHTML` / `DOM.getAttributes` / `CSS.getComputedStyleForNode` |
| `click` / `hover` | `DOM.getBoxModel` (centre) → `Input.dispatchMouseEvent` (trusted) |
| `fill` / `press` | `DOM.focus` → `Input.insertText` / `Input.dispatchKeyEvent` |
| `screenshot` | `Page.captureScreenshot` (+ `Page.getLayoutMetrics` for full-page) |
| `wait_for` | poll `DOM.querySelector`, or subscribe to mutation via injected helper |
| console / errors | `Runtime.consoleAPICalled`, `Runtime.exceptionThrown` |

This is exactly the surface our M1–M8 machinery needs to verify in-browser:
- **M1 positioning**: `bounding_box` of trigger vs popup; assert `data-side`,
  `--anchor-*` vars via `computed_style`.
- **M3 dismiss / hover safe-polygon**: trusted `Input.dispatchMouseEvent` along a
  path; assert open/closed.
- **M4 focus-trap**: `Input.dispatchKeyEvent` Tab; read `document.activeElement`.
- **M5 roving/typeahead**: key events; assert `data-highlighted`/`tabindex`.
- **M7 transitions**: `data-starting-style` presence over frames; screenshots.
- **M8 gestures**: trusted pointer drag sequences; assert value/transform vars.

## 6. Protocol choice & browser roadmap

- **Phase 1 — Chromium / CDP.** Most mature; has everything (screenshots, box
  model, trusted input). Launch with `--remote-debugging-port=0` (ephemeral) or
  `--remote-debugging-pipe` (fd 3/4, no port — Playwright's default), `--headless=new`,
  `--no-sandbox` (CI), plus the safe subset of Playwright's `chromiumSwitches`.
  Discover the WS endpoint from the `DevToolsActivePort` file or stdout, or
  `GET /json/version` → `webSocketDebuggerUrl`.
- **Phase 2 — Firefox / WebDriver BiDi.** Firefox dropped CDP for **WebDriver
  BiDi** (a W3C JSON-RPC standard). Launch `firefox --remote-debugging-port`/
  `--websocket`, speak BiDi (`session.new`, `browsingContext.navigate`,
  `script.evaluate`, `input.performActions`, `browsingContext.captureScreenshot`).
  BiDi is the strategic cross-browser target (Chromium also speaks BiDi now), so
  the `WireProtocol` trait is designed to host both; we ship CDP first for depth.
- **Phase 3 — WebKit.** Hardest: vanilla WebKit uses the WebKit Remote Inspector
  protocol; Safari exposes classic WebDriver via `safaridriver`; BiDi-for-WebKit
  is nascent. Options: `safaridriver`/WebDriver-classic adapter, or a BiDi bridge
  when ready. Deferred but the trait keeps the door open.

## 7. Browser launch, discovery, supervision (`BrowserSupervisor`)

- A `Browser` enum (`Chromium`/`Firefox`/`Webkit`) with per-OS binary discovery:
  `$PRIMAL_TEST_<B>_BIN`, then `mise`-installed path, then PATH
  (`chromium`/`google-chrome`/`chromium-browser`, `firefox`).
- `which`-style probe up front; **missing → typed error** naming the `mise` task.
- Spawn with a fresh `--user-data-dir` temp profile; capture stdout/stderr; on
  drop, kill the process group and remove the temp dir.
- Endpoint discovery: pipe mode (preferred, hermetic) or port-file poll with a
  timeout. Reuse `foundation_netio::websocket` client handshake to connect.

## 8. TLS / pre-generated cert

`#[wasm_ui_server(.., https)]` serves over TLS via `foundation_netio` TLS + a
**pre-generated localhost cert** shipped in the templates (and a `cert` helper to
regenerate). The browser is launched trusting it
(`--ignore-certificate-errors`/profile policy) so HTTPS-only features (secure
cookies, service workers, clipboard) can be tested. Decision: ship a fixed
`localhost`/`127.0.0.1` cert+key under `templates/.../certs/` rather than add an
`rcgen` runtime dep (none exists today); offer `mise run test:cert:regen`.

## 9. `mise` tasks + per-distro setup + templates

New tasks (root `mise.toml`, mirrored into templates), replacing the
`setup:playwright` (node) task:

```toml
[tasks."test:browsers"]            # install all test browsers for this OS
depends = ["test:browsers:chromium"]   # + firefox later
[tasks."test:browsers:chromium"]
run = """
case "$(. /etc/os-release; echo $ID)" in
  arch)          sudo pacman -S --needed --noconfirm chromium ;;
  debian|ubuntu) sudo apt-get install -y chromium || sudo apt-get install -y chromium-browser ;;
  *) echo "Install a Chromium build and set PRIMAL_TEST_CHROMIUM_BIN" >&2; exit 1 ;;
esac
"""
[tasks."test:cert:regen"]          # regenerate the localhost dev cert
```

We are on **Arch** (`pacman -S chromium`). Debian/Ubuntu use
`chromium`/`chromium-browser`. CI installs add `--no-sandbox`. The driver's
"not installed" error points straight at `mise run test:browsers`.

## 10. Crate layout

- `backends/foundation_browser/` (new): the `WireProtocol` engine, `CdpClient`
  (phase 1), `BiDiClient` (phase 2), `BrowserSupervisor`, `Browser`/`Page`/
  `Locator`/assertions. Depends on `foundation_netio` (WS client), `foundation_core`.
- `foundation_macros`: the `#[wasm_ui_server]` attribute macro (per the
  all-macros-live-here rule).
- `foundation_testing` or `foundation_wasm_testbed`: `TestServer` (the
  `foundation_http`-backed harness the macro boots) + the `primal-test` injected
  helper asset. Retire `foundation_wasm_testbed/src/browser.rs`'s node path.
- Templates: `mise` tasks, the dev cert, and an example `#[wasm_ui_server]` test.

## 11. Build order

1. ✅ **JSON-RPC engine** over `WebSocketClient::connect` (request/result/error +
   event pump): one reader thread draining `WebSocketConnection::messages()`,
   correlating `id`→pending oneshot, dispatching `method` events to subscribers;
   sends via `MessageDelivery`. Sync, protocol-agnostic. (`src/jsonrpc/engine.rs`.)
2. ✅ **`CdpClient`** + `BrowserSupervisor` (Chromium launch/discover/kill) +
   `Page::goto`/`eval`/`screenshot` → green: launch, navigate, shoot.
3. ✅ **`Locator`** + DOM/box-model/computed-style/`Input` ops + `LocatorAssertions`.
4. ✅ **`#[wasm_ui_server]` macro** + `TestServer` (serve app + stream); wired real
   end-to-end tests — Mode 1 native-App streaming across all four protocols.
5. ✅ **`mise` tasks** (`test:browsers[:chromium]`, `test:browser`,
   `test:cert:regen`) + **HTTPS dev cert** (`https = true` → TLS TestServer) +
   **node-path retirement** (`foundation_wasm_testbed/src/browser.rs` now drives
   Chromium via `foundation_browser`, no node/Playwright).
6. ✅ **`primal-test` injected helper** — the runtime frame instrument
   (`globalThis.__primalFrames`) + `primal-test.js` (`window.__primalTest`:
   `waitForReactive`/`batchLayout`) + `Page::wait_for_reactive`/`bounding_boxes`.
7. ✅ **Phase 2 — WebDriver BiDi / Firefox** behind a `Backend` seam: `Page`/
   `Locator` program against an operation-level [`Backend`] (CDP or BiDi); reads
   run through `eval` so they're identical on both. Firefox parity is green
   (locator ops + all four protocols + the helper).

## 12. Open decisions

- **CDP pipe vs port**: pipe is hermetic (no port, no race) but needs fd plumbing;
  port-file is simpler. Lean pipe, fall back to port.
- **One `Page` per test vs context reuse**: default fresh context per test
  (isolation) — confirm against `nextest` parallelism cost.
- **Screenshot baselines**: ship raw capture v1; visual-diff (perceptual) later.
- **Trusted vs synthetic input default**: default trusted (CDP `Input`); expose
  `page.eval`-dispatched synthetic as an explicit opt-in.
- **BiDi-first vs CDP-first**: CDP-first for Chromium depth; BiDi as the
  cross-browser convergence. Both behind `WireProtocol`.
- **Where the macro-booted `TestServer` lives**: `foundation_testing` (test-only)
  vs `foundation_wasm_testbed` (owns the wasm build). Likely the latter (it already
  builds the bundle).

## 13. References (our guides)

- `@formulas/src.UIFrameworks/src.playwright/playwright/packages/playwright-core/src/server/chromium/`
  — `chromiumSwitches.ts` (launch flags), `chromium.ts` (`--remote-debugging-port=0`
  / `--remote-debugging-pipe`, headless, sandbox).
- `playwright-python/playwright/_impl/{_connection,_transport,_cdp_session}.py`
  — JSON-RPC connection + CDP passthrough shape.
- Our `foundation_netio::websocket` (frame codec + client handshake),
  `foundation_http` (server/SSE/WS), `foundation_wasm_ui` (DOM-op protocol +
  hydration), `foundation_wasm_testbed/src/browser.rs` (the node path we replace).

## 14. Status — what shipped

**Phase 1 is delivered and green.** A native `foundation_wasm_ui` App streams DOM
frames over SSE to a real Chromium, driven by Rust signals, asserted against real
paint/layout — with **zero node/Playwright**.

### Crate layout (as built)

- **`backends/foundation_browser/`** — the pure-Rust driver:
  - `src/jsonrpc/engine.rs` — `RpcEngine` + `WireProtocol` trait (CDP today, BiDi
    behind the same seam).
  - `src/cdp/`, `src/supervisor.rs` — `CdpClient`, `BrowserProcess` (launch flags,
    DevToolsActivePort discovery, RAII kill + profile cleanup).
  - `src/browser.rs`, `src/locator.rs`, `src/geometry.rs` — `BrowserDriver`/`Page`
    (goto/eval/screenshot) + `Locator`/`LocatorAssertions` (retrying).
  - `src/test/` — `TestServer` (on `foundation_http`), the SSE `StreamHandler` +
    `SseTransport` glue, `Harness` (RAII + panic trap), `TestConfig`
    (incl. `window_size`, `headless`). Gated behind the `browser-tests` feature.
- **`foundation_wasm_ui::server`** — the transport-agnostic server-driven-UI
  machinery (`Broadcaster`/`BroadcastTx`/`BroadcastSink`/`FrameTransport`): a real
  capability, not test-only. foundation_http gained `StaticAssetHandler` +
  `SseStream::binary` (the binary-over-SSE half).
- **`foundation_macros`** — `#[wasm_ui_server]` (args: `port/host/headless/headful/
  html/file/static_dir/static_mount/encoding/headers/window_size`).

### Verified

- `foundation_browser` smoke suite (12 tests) green against real Chromium, incl.
  Mode-1 native-App streaming for **all four protocols** (columnar, Apache Arrow
  IPC, JSON, HTML) each rendering identically; runnable headful (`PRIMAL_TEST_HEADFUL=1`).
- The runtime gained the missing apply paths: `Patcher.ensureApplicator`, JSON
  DomOp-batch routing, the `arrow-ipc` route + `registerArrowIpc`, and the frame
  instrument (`globalThis.__primalFrames`).
- **HTTPS mode** (`https = true`): TLS `TestServer` via a bundled localhost dev
  cert + `HttpServer::serve_tls_with_listener`; browser trusts it
  (`--ignore-certificate-errors`). `https_serves_a_secure_context` green.
- **`primal-test` helper**: `primal-test.js` (`window.__primalTest`) +
  `Page::wait_for_reactive`/`bounding_boxes`. `primal_test_helper_and_reactive_settle` green.
- **node/Playwright retired**: `foundation_wasm_testbed/src/browser.rs` now drives
  Chromium via `foundation_browser` (no node, npm, or Playwright).
- `mise`: `test:browsers[:chromium]`, `test:browser`, `test:cert:regen`.

### Phase 2 — Firefox over WebDriver BiDi (delivered)

`Page`/`Locator` were lifted onto an operation-level `Backend` trait (navigate,
evaluate, screenshot, trusted input). Element READS run through `eval`, so they're
identical across backends; only navigation/evaluate/input differ. Two backends:
`cdp::backend::CdpBackend` (Chromium) and `bidi::backend::BiDiBackend` (Firefox,
`browsingContext.*`/`script.*`/`input.performActions`, with `RemoteValue → JSON`).
`Browser::Firefox` launches Firefox's Remote Agent (BiDi-direct at
`ws://host:port/session`). Firefox parity is green (`firefox_*` smoke tests):
locator geometry/attributes/style/input + all four protocols (columnar, Arrow IPC,
JSON, HTML) + the `primal-test` helper.

Three foundation_netio WS-client interop bugs (surfaced by Firefox's strict
server, masked by Chromium's lenience) were fixed along the way:
- the client now **masks** all frames (RFC 6455 §5.3);
- the upgrade `Host` header carries the **port**;
- a bodyless request no longer emits **`Content-Length`** (the request-builder
  twin of the earlier response-builder fix) — strict servers reject a GET with a
  body.

### Out of scope

- **WebKit/Safari** — no BiDi-direct story on Linux; revisit if needed.

Design narrative for both test modes:
[`features/01-test-modes.md`](./features/01-test-modes.md).
