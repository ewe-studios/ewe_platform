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

/// The core ABI runtime, re-exported so consumers embed the pair from one place.
pub use foundation_wasm::embedded::FOUNDATION_WASM_JS;
