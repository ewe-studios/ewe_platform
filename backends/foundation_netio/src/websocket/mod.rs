pub mod shared;
pub use shared::*;

#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub mod native;

#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub use native::*;
