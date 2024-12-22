extern crate url;

mod error;

mod core;

#[cfg(not(target_arch = "wasm32"))]
mod no_wasm;


#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(target_arch = "wasm32")]
pub use wasm::*;
