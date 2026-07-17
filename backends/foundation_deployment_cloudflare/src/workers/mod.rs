//! Cloudflare Workers runtime support — Durable Objects, WebSocket, Env bindings.
//!
//! WHY: Workers provides a unique runtime model (Durable Objects for stateful
//! singleton compute, WebSocketPair for connections, Env for resource bindings)
//! that other crates need when targeting the Workers platform.
//!
//! WHAT: Wrappers for DO lifecycle, WebSocket accept/send/recv, and typed Env
//! binding access that other crates can consume without importing `worker::` directly.

pub mod durable_object;
pub mod websocket;
pub mod env;
pub mod context;
