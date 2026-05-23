//! Wasm-bindgen auth modules (gated behind `wasm-bindgen-oauth` feature).

#[cfg(feature = "wasm-bindgen-oauth")]
pub mod oauth;

/// Async session manager for wasm32 (gated behind `wasm-bindgen-session` feature).
#[cfg(feature = "wasm-bindgen-session")]
pub mod session;

/// D1-backed credential store using valtron-sync iterators (gated behind `wasm-bindgen-session` feature).
#[cfg(feature = "wasm-bindgen-session")]
pub mod d1_credential_store;

/// AsyncCredentialStore impl for D1WasmStorage (gated behind `wasm-bindgen-session` feature).
#[cfg(feature = "wasm-bindgen-session")]
pub mod async_credential_store;
