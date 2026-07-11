// HTTP 1.1 Client Module — re-exports from canonical paths (F51 Stage 4).
//
// Shared → foundation_netio::shared::client
// Native → foundation_netio::http
// Wasm   → foundation_netio::wasm::client

pub mod shared {
    pub use crate::shared::client::*;
}
#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub mod native {
    pub use crate::http::*;
}
#[cfg(all(target_family = "wasm", feature = "wasm-fetch"))]
pub mod wasm {
    pub use crate::wasm::client::*;
}

#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub use crate::http::default_http_client;
#[cfg(all(target_family = "wasm", feature = "wasm-fetch"))]
pub use crate::wasm::client::default_http_client;

#[cfg(all(feature = "multi", not(target_family = "wasm")))]
pub use native::*;
pub use shared::*;
#[cfg(all(target_family = "wasm", feature = "wasm-fetch"))]
pub use wasm::*;
