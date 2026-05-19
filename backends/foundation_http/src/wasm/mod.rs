//! Wasm modules (request dispatch, memory-backed streams).

pub mod stream;
pub mod serve_writer;
pub mod response;
pub mod cf_conn;
pub mod web_conn;
pub mod serve_cf;
pub mod serve_web;

// Bridge modules require wasm-bindgen — only available on wasm32 target
#[cfg(target_arch = "wasm32")]
pub mod bridge;
