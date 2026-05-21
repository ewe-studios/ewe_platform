//! Core backends — shared between native and wasm.

pub mod memory;
pub mod memory_json;
pub mod async_utils;

// Re-exports
pub use memory::*;
pub use memory_json::*;
pub use async_utils::*;
