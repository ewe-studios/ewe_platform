//! Wasm-only modules (request dispatch, memory-backed streams).

pub mod stream;
pub mod server;
pub mod response;

// Bridge modules require wasm-bindgen — only available on wasm32 target
#[cfg(target_arch = "wasm32")]
pub mod bridge;
