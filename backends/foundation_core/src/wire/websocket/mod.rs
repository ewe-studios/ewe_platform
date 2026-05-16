//! WebSocket protocol implementation (RFC 6455).
//!
//! WHY: Provides synchronous WebSocket support for the `ewe_platform` project.
//! WHAT: Frame encoding/decoding, message types, and error handling per RFC 6455.

mod assembler;
mod batch_writer;
mod error;
mod frame;
mod handshake;
mod message;

pub use assembler::*;
pub use batch_writer::*;
pub use error::*;
pub use frame::*;
pub use handshake::*;
pub use message::*;

// Connection/task modules — depend on netcap::RawStream, HttpConnectionPool
#[cfg(not(target_arch = "wasm32"))]
mod connection;
#[cfg(not(target_arch = "wasm32"))]
mod reconnecting_task;
#[cfg(not(target_arch = "wasm32"))]
mod server;
#[cfg(not(target_arch = "wasm32"))]
mod task;

#[cfg(not(target_arch = "wasm32"))]
pub use connection::*;
#[cfg(not(target_arch = "wasm32"))]
pub use reconnecting_task::*;
#[cfg(not(target_arch = "wasm32"))]
pub use server::*;
#[cfg(not(target_arch = "wasm32"))]
pub use task::*;
