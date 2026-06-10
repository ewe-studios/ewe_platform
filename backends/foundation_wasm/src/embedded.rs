//! WHY: Servers and bundler-less deployments need to serve the JS runtime without a
//! separate asset pipeline — embedding it in the binary keeps the runtime version in
//! lockstep with the crate that speaks its ABI (feature 00, Part B).
//!
//! WHAT: The core ABI runtime (`foundation-wasm.js`) as a compile-time string,
//! behind the `embedded-js` feature.
//!
//! HOW: `include_str!` from `runtime/` at build time. The DOM-layer asset is embedded
//! by `foundation_wasm_ui` (its crate owns that file).

/// The single-file core ABI runtime (`runtime/foundation-wasm.js`): memory arena,
/// envelope dispatch, timers, callbacks, function-call codec (V1), and the V2
/// quantized batch codec. Self-contained — serve as an ES module or inline it.
pub const FOUNDATION_WASM_JS: &str = include_str!("../runtime/foundation-wasm.js");
