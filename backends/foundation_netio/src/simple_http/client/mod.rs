// HTTP 1.1 Client Module

// Shared types — always compiled, including on wasm32
pub mod shared;

// Re-export Extensions from simple_http::shared
pub use crate::simple_http::shared::Extensions;

// Re-export ExecutionAction from valtron
pub use crate::valtron::ExecutionAction;

// Native types — gated behind not(target_arch = "wasm32")
#[cfg(feature = "multi")]
pub mod native;

#[cfg(feature = "multi")]
pub use native::*;
