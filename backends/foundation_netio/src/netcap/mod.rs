#![allow(dead_code)]

pub mod errors;
pub use errors::*;

// #[cfg(not(target_family = "wasm"))]
// mod tls_verification;
//
// #[cfg(not(target_family = "wasm"))]
// pub use tls_verification::*;

mod core;
pub use core::*;

#[cfg(not(target_family = "wasm"))]
pub mod connection;

#[cfg(not(target_family = "wasm"))]
pub use connection::*;

#[cfg(not(target_family = "wasm"))]
pub mod ssl;

#[cfg(not(target_family = "wasm"))]
mod no_wasm;

#[cfg(not(target_family = "wasm"))]
pub use no_wasm::*;

#[cfg(target_family = "wasm")]
mod wasm;

#[cfg(target_family = "wasm")]
pub use wasm::*;
