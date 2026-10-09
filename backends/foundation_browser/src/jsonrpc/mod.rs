//! # JSON-RPC engine (spec-43 phase-1 §2)
//!
//! WHY: CDP and WebDriver BiDi are both JSON-RPC over a WebSocket. A single
//! protocol-agnostic engine — request/result/error correlation by `id`, plus
//! `method` events — serves both; the CDP client is a thin typed layer on top.
//!
//! WHAT: [`RpcEngine`] (the engine) + [`WireProtocol`] (the trait the driver
//! programs against).
//!
//! HOW: [`RpcEngine::connect`] dials the browser's debugging socket with the
//! `foundation_netio` WebSocket client (driven by the valtron multi-threaded
//! pool), spawns ONE reader thread that drains incoming frames — resolving
//! pending requests and fanning events to subscribers — and sends requests over
//! the `MessageDelivery` handle, blocking each on a per-request channel.

mod engine;

pub use engine::{EventSub, RpcEngine, WireProtocol};
