
mod consumer;
mod core;
mod error;
mod parser;
mod response;
mod writer;

pub use consumer::*;
pub use core::*;
pub use error::*;
pub use parser::*;
pub use response::*;
pub use writer::*;

// Task modules — depend on simple_http::client (RawStream, HttpConnectionPool)
#[cfg(not(target_arch = "wasm32"))]
mod reconnecting_task;
#[cfg(not(target_arch = "wasm32"))]
mod task;

#[cfg(not(target_arch = "wasm32"))]
pub use reconnecting_task::*;
#[cfg(not(target_arch = "wasm32"))]
pub use task::*;
