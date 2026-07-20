//! WHY: Servers and bundler-less deployments need both runtime files without an
//! asset pipeline — embedding keeps the served JS in lockstep with the crates that
//! speak its ABI (feature 00, Part B).
//!
//! WHAT: The DOM-layer runtime (`foundation-wasm-ui.js`) as a compile-time string,
//! plus a re-export of the core runtime from `foundation_wasm`, behind the
//! `embedded-js` feature.
//!
//! HOW: `include_str!` from `runtimes/` at build time; the feature forwards to
//! `foundation_wasm/embedded-js` so one flag embeds the pair.

/// The single-file DOM runtime (`runtimes/foundation-wasm-ui.js`): Arrow DOM
/// applicator, event dispatcher, `DomHeap` + the `domAbi` import fragment.
/// Self-contained — serve alongside [`FOUNDATION_WASM_JS`].
pub const FOUNDATION_WASM_UI_JS: &str = include_str!("../runtimes/foundation-wasm-ui.js");

/// The bundled Apache Arrow JS library (`runtimes/apache-arrow.js`, G3/G25 —
/// feature 20): the reader for wire VERSION 2 (real Arrow IPC) payloads on
/// server content types (`application/primal-arrow` v2, SSE). The wasm loop's
/// compact columnar v1 needs only `ColumnarParser` and never loads this.
pub const APACHE_ARROW_JS: &str = include_str!("../runtimes/apache-arrow.js");

/// The injected test helper (`runtimes/primal-test.js`, spec-43 §6): defines
/// `window.__primalTest` with `waitForReactive` + `batchLayout` over the runtime's
/// frame instrument. Self-contained (no imports) so a driver can inject it on any
/// page. Used by the `foundation_browser` test harness.
pub const PRIMAL_TEST_JS: &str = include_str!("../runtimes/primal-test.js");

/// Platform scheme workaround for Android: catches `ewe://`, `foundation://`,
/// `platform://` link clicks and redirects to `http://{scheme}.localhost/...`.
/// wry's `shouldInterceptRequest` hooks these HTTP URLs and routes them to
/// the registered custom protocol handler. Desktop engines natively support
/// custom schemes — this is a no-op there.
pub const PLATFORM_SCHEME_INTERCEPTOR_JS: &str =
    include_str!("../runtimes/platform-scheme-interceptor.js");

/// The core ABI runtime, re-exported so consumers embed the set from one place.
pub use foundation_wasm::embedded::FOUNDATION_WASM_JS;

/// Portable capability invocation bridge (`runtimes/capability-bridge.js`, F23):
/// provides `window.invokeCapability(name, action, payload)` that routes through
/// Tauri (`__ewe_capabilities`), browser (direct WASM bridge), or Deno.
/// Injected by F24 ScriptInjector alongside the other platform runtimes.
pub const CAPABILITY_BRIDGE_JS: &str = include_str!("../runtimes/capability-bridge.js");
