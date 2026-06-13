//! # `foundation_browser` — a pure-Rust browser test driver (spec-43)
//!
//! WHY: CDP (Chrome DevTools Protocol) and WebDriver BiDi are just JSON-RPC over
//! a WebSocket. We already own a WebSocket client (`foundation_netio`), so we can
//! speak those protocols DIRECTLY from Rust — no Node, no Playwright binary, no
//! external driver. This lets us drive a real browser from Rust tests and verify
//! the spec-42 UI machinery against real paint, layout, and trusted input.
//!
//! WHAT (phase 1, Chromium/CDP only): [`jsonrpc`] (the protocol-agnostic engine)
//! + the CDP client, browser supervision, and a `Page`/`Locator` API (added
//! incrementally). Firefox/BiDi is a later feature behind the same engine.
//!
//! HOW: One reader thread drains the WebSocket, correlating `id → result` and
//! fanning `method → events`; commands block on a per-request channel. Browser
//! processes + sockets are RAII guards so a test's setup/teardown is
//! deterministic on success, error, or panic (spec-43 phase-1 §0).

#![allow(clippy::module_name_repetitions)]

pub mod browser;
pub mod cdp;
pub mod error;
pub mod jsonrpc;
pub mod runtime;
pub mod supervisor;

pub use browser::{BrowserDriver, Page};
pub use cdp::launch::{Browser, LaunchConfig};
pub use error::{BrowserError, Result};
pub use jsonrpc::{RpcEngine, WireProtocol};
