//! The Durable Streams WebSocket transport.
//!
//! The transport lives above the protocol core. It upgrades a connection and
//! maps each JSON text frame to [`DurableService`](foundation_core::durable::DurableService).
//! The core keeps no WebSocket type.
//!
//! This module has three parts:
//!
//! - The `message` module holds the JSON request and reply schema.
//! - [`DurableWsConfig`] holds the limits, including the frame size cap.
//! - [`DurableWsHandler`] upgrades a GET request and serves the protocol.

mod handler;
mod message;

pub use handler::{DurableWsConfig, DurableWsHandler};
pub use message::{
    decode, encode, error_kind, offset_text, WsError, WsErrorBody, WsOp, WsReply, WsRequest,
};
