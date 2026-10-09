//! Cloudflare Workers runtime support — Durable Objects, WebSocket transport, Env bindings.
//!
//! WHY: Workers provides a unique runtime model (Durable Objects for stateful
//! singleton compute, WebSocketPair for connections, Env for resource bindings)
//! that other crates need when targeting the Workers platform.
//!
//! WebSocket: this module only provides the Workers-specific transport layer
//! (WebSocketPair accept, send/recv bridge, handshake). All framing logic
//! (binary MessagePack, SignalR protocol) is reused from
//! `foundation_netio::websocket::shared`.

pub mod durable_object;
pub mod websocket;
pub mod env;
pub mod context;
