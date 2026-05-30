//! Wasm modules (request dispatch, memory-backed streams).

pub mod stream;
pub mod serve_writer;
pub mod response;

// Re-export WebServe/WebConn/WebConnectionResult from shared (moved from wasm)
pub use crate::shared::serve_web::{WebServe, WebServeFactory};
pub use crate::shared::web_conn::{WebConn, WebConnectionResult};

// CfConn and its serve trait require web_sys for into_response()
#[cfg(feature = "wasm-bindgen-http")]
pub mod cf_conn;
#[cfg(feature = "wasm-bindgen-http")]
pub mod serve_cf;

// Bridge modules require wasm-bindgen — only available on wasm32 target
#[cfg(target_arch = "wasm32")]
pub mod bridge;

// Dispatch logic for wasm-specific handler types — requires web_sys
#[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-http"))]
pub mod dispatch;
