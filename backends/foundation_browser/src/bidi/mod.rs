//! # WebDriver BiDi support (spec-43 phase-2 — Firefox)
//!
//! WHY: BiDi is the cross-browser convergence protocol; Firefox speaks it
//! directly. It is JSON-RPC over a WebSocket — the same engine as CDP — so only
//! the bootstrap (`session.new`) and the operation mapping differ.
//!
//! WHAT: session/context bootstrap helpers + [`backend::BiDiBackend`].
//!
//! HOW: After connecting to Firefox's BiDi endpoint (`ws://host:port/session`),
//! `session.new` opens a session (connection-scoped — no per-message session id),
//! and `browsingContext.getTree` yields the default context every command targets.

pub(crate) mod backend;

use serde_json::{json, Value};

use crate::error::{BrowserError, Result};
use crate::jsonrpc::{RpcEngine, WireProtocol};

/// Open a BiDi session over the connected engine (`session.new`). BiDi is
/// connection-scoped, so the returned id is informational; subsequent commands
/// on the same socket run in this session.
///
/// # Errors
/// Protocol error from `session.new`.
pub(crate) fn new_session(engine: &RpcEngine) -> Result<String> {
    let res = engine.call("session.new", json!({ "capabilities": {} }), None)?;
    Ok(res.get("sessionId").and_then(Value::as_str).unwrap_or_default().to_string())
}

/// The first (default) top-level browsing context — Firefox opens one on start.
///
/// # Errors
/// Protocol error, or no context present.
pub(crate) fn default_context(engine: &RpcEngine) -> Result<String> {
    let res = engine.call("browsingContext.getTree", json!({}), None)?;
    res.get("contexts")
        .and_then(Value::as_array)
        .and_then(|c| c.first())
        .and_then(|c| c.get("context"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| BrowserError::Protocol { code: 0, message: "BiDi: no browsing context".into() })
}
