//! WebSocket protocol implementation (RFC 6455).
//!
//! WHY: Provides synchronous WebSocket support for the `ewe_platform` project.
//! WHAT: Frame encoding/decoding, message types, and error handling per RFC 6455.

mod assembler;
mod batch_writer;
mod connection;
mod error;
mod frame;
mod handshake;
mod message;
mod reconnecting_task;
mod server;
mod task;

pub use assembler::*;
pub use batch_writer::*;
pub use connection::*;
pub use error::*;
pub use frame::*;
pub use handshake::*;
pub use message::*;
pub use reconnecting_task::*;
pub use server::*;
pub use task::*;
