//! Wasm-only modules.

// The response sink writes to netio's fetch response, so it needs the fetch
// client. A module without `wasm-fetch` leaves it out (spec 88 feature 03, T7).
#[cfg(feature = "wasm-fetch")]
pub mod body;
#[cfg(feature = "wasm-fetch")]
pub mod sink;
