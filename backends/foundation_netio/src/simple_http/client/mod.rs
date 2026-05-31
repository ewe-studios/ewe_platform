// HTTP 1.1 Client Module

// Shared types — always compiled, including on wasm32
pub mod shared;

// Native types — gated behind not(target_arch = "wasm32")
#[cfg(all(feature = "multi", not(target_arch = "wasm32")))]
pub mod native;

#[cfg(all(feature = "multi", not(target_arch = "wasm32")))]
pub use native::*;
