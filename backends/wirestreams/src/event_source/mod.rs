extern crate url;

mod error;
pub use error::*;

mod core;
pub use core::*;

#[cfg(not(target_arch = "wasm32"))]
mod no_wasm;

#[cfg(not(target_arch = "wasm32"))]
pub use no_wasm::*;

#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(target_arch = "wasm32")]
pub use wasm::*;
