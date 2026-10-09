//! Wasm-only modules — compiled only on wasm32.

pub mod netcap;

#[cfg(feature = "wasm-fetch")]
pub mod client;
