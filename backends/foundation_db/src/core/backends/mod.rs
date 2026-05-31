//! Core backends — shared between native and wasm.

pub mod memory;
pub mod memory_json;

// Re-exports
pub use memory::*;
pub use memory_json::*;
