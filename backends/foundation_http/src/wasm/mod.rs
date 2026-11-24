//! Wasm modules (request dispatch, memory-backed streams).

pub mod response;
pub mod serve_writer;
pub mod stream;

// Re-export WebServe/WebConn/WebConnectionResult from shared (moved from wasm)
pub use crate::shared::serve_web::{WebServe, WebServeFactory};
pub use crate::shared::web_conn::{WebConn, WebConnectionResult};

// CfConn and its serve trait require web_sys for into_response()
#[cfg(feature = "wasm-bindgen-http")]
pub mod cf_conn;
#[cfg(feature = "wasm-bindgen-http")]
pub mod serve_cf;

// Bridge modules require wasm-bindgen — only available on wasm32 target
#[cfg(target_family = "wasm")]
pub mod bridge;

// Dispatch logic for wasm-specific handler types — requires web_sys
#[cfg(all(target_family = "wasm", feature = "wasm-bindgen-http"))]
pub mod dispatch;

// The panic boundary of the host entries — requires wasm-bindgen (issue #742)
#[cfg(all(target_family = "wasm", feature = "wasm-bindgen-http"))]
pub mod panic_guard;

// Re-export singleton for easy access when wasm-bindgen-http is enabled
#[cfg(all(target_family = "wasm", feature = "wasm-bindgen-http"))]
pub use bridge::{CfHttpAppGuard, CfHttpAppSingleton, WasmHttpAppGuard, WasmHttpAppSingleton};
