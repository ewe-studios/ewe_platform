//! Wasm modules (request dispatch, memory-backed streams).

pub mod stream;
pub mod serve_writer;
pub mod response;

// CfConn/WebConn and their serve traits require web_sys for into_response()
#[cfg(feature = "wasm-bindgen-http")]
pub mod cf_conn;
#[cfg(feature = "wasm-bindgen-http")]
pub mod web_conn;
#[cfg(feature = "wasm-bindgen-http")]
pub mod serve_cf;
#[cfg(feature = "wasm-bindgen-http")]
pub mod serve_web;

// Bridge modules require wasm-bindgen — only available on wasm32 target
#[cfg(target_arch = "wasm32")]
pub mod bridge;

// Dispatch logic for wasm-specific handler types — requires web_sys
#[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-http"))]
pub mod dispatch;
