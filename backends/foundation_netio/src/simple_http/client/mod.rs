// HTTP 1.1 Client Module

// Shared types — always compiled, including on wasm32
pub mod shared;

#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub use crate::simple_http::client::native::default_http_client;

#[cfg(all(target_family = "wasm", feature = "wasm-fetch"))]
pub use crate::simple_http::client::wasm::default_http_client;

// Native types — gated behind not(target_family = "wasm")
#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub mod native;

#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub use native::*;

// Wasm fetch client — gated behind wasm32 + wasm-fetch feature
#[cfg(all(target_family = "wasm", feature = "wasm-fetch"))]
pub mod wasm;

#[cfg(all(target_family = "wasm", feature = "wasm-fetch"))]
pub use wasm::*;
