pub mod shared;
pub use shared::*;

#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub mod native;

#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub use native::*;

// Wasm browser WebSocket bridge + FetchHttpClient connector (F51 Stage 6).
#[cfg(target_family = "wasm")]
pub mod wasm;

#[cfg(target_family = "wasm")]
pub use wasm::*;
