#![allow(dead_code)]

pub mod errors;
pub use errors::*;

// #[cfg(not(target_arch = "wasm32"))]
// mod tls_verification;
//
// #[cfg(not(target_arch = "wasm32"))]
// pub use tls_verification::*;

mod core;
pub use core::*;

#[cfg(not(target_arch = "wasm32"))]
pub mod connection;

#[cfg(not(target_arch = "wasm32"))]
pub use connection::*;

#[cfg(not(target_arch = "wasm32"))]
pub mod ssl;

#[cfg(not(target_arch = "wasm32"))]
mod no_wasm;

#[cfg(not(target_arch = "wasm32"))]
pub use no_wasm::*;

#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(target_arch = "wasm32")]
pub use wasm::*;
