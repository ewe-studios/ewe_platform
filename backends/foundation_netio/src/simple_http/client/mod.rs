// HTTP 1.1 Client Module

// Shared types — always compiled, including on wasm32
pub mod shared;

#[cfg(all(feature = "multi", not(target_arch = "wasm32")))]
pub use crate::simple_http::client::native::default_http_client;

#[cfg(all(target_arch = "wasm32", feature = "wasm-fetch"))]
pub use crate::simple_http::client::wasm::default_http_client;

// Native types — gated behind not(target_arch = "wasm32")
#[cfg(all(feature = "multi", not(target_arch = "wasm32")))]
pub mod native;

#[cfg(all(feature = "multi", not(target_arch = "wasm32")))]
pub use native::*;

// Wasm fetch client — gated behind wasm32 + wasm-fetch feature
#[cfg(all(target_arch = "wasm32", feature = "wasm-fetch"))]
pub mod wasm;

#[cfg(all(target_arch = "wasm32", feature = "wasm-fetch"))]
pub use wasm::*;
