extern crate url;

#[cfg(all(feature = "native-tls", not(target_arch = "wasm32")))]
extern crate native_tls_crate as native_tls;

mod error;
pub use error::*;

#[cfg(not(target_arch = "wasm32"))]
mod no_wasm;

#[cfg(not(target_arch = "wasm32"))]
pub use no_wasm::*;

#[cfg(not(target_arch = "wasm32"))]
mod server;

#[cfg(not(target_arch = "wasm32"))]
pub use server::*;

#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(target_arch = "wasm32")]
pub use wasm::*;

mod types;
pub use types::*;

mod core;
pub use core::*;
