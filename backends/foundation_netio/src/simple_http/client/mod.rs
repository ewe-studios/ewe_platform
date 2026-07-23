pub mod shared { pub use crate::shared::client::*; }
#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub mod native { pub use crate::http::*; }
#[cfg(target_family = "wasm")]
pub mod wasm { pub use crate::wasm::client::*; }
#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub use crate::http::default_http_client;
#[cfg(target_family = "wasm")]
pub use crate::wasm::client::default_http_client;
pub use shared::*;
#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub use native::*;
#[cfg(target_family = "wasm")]
pub use wasm::*;
