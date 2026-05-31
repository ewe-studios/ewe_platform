pub mod shared;
pub use shared::*;

#[cfg(all(feature = "multi", not(target_arch = "wasm32")))]
pub mod native;

#[cfg(all(feature = "multi", not(target_arch = "wasm32")))]
pub use native::*;
