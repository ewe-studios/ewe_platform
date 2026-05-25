// HTTP 1.1 Client Module

// Shared types — always compiled, including on wasm32
pub mod shared;

// Native types — gated behind not(target_arch = "wasm32")
#[cfg(feature = "multi")]
pub mod native;

#[cfg(feature = "multi")]
pub use native::*;
