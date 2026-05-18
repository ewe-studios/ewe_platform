//! Wasm-bindgen bridge entry points for web and Cloudflare Workers.

#[cfg(feature = "wasm-bindgen-http")]
pub mod web;

#[cfg(feature = "wasm-bindgen-http")]
pub mod cf;
