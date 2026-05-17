
mod core;
mod error;
mod parser;
mod response;
mod writer;

pub use core::*;
pub use error::*;
pub use parser::*;
pub use response::*;
pub use writer::*;

// Consumer/task modules — depend on simple_http::client (HttpConnectionPool, RawStream)
#[cfg(not(target_arch = "wasm32"))]
mod consumer;
#[cfg(not(target_arch = "wasm32"))]
mod reconnecting_task;
#[cfg(not(target_arch = "wasm32"))]
mod task;

#[cfg(not(target_arch = "wasm32"))]
pub use consumer::*;
#[cfg(not(target_arch = "wasm32"))]
pub use reconnecting_task::*;
#[cfg(not(target_arch = "wasm32"))]
pub use task::*;
