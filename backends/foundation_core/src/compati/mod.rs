//! Sub module to provide compatibility layer for specific types to use in specific platforms.

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub use foundation_nostd::primtivies::Mutex;

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
pub use std::sync::Mutex;

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub use foundation_nostd::primtivies::RwLock;

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
pub use std::sync::RwLock;
