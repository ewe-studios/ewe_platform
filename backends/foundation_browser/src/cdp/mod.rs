//! # CDP client (spec-43 phase-1 §3)
//!
//! WHY: A thin typed layer over the JSON-RPC engine that speaks the slice of CDP
//! the driver needs — attach a page session, navigate, screenshot, and (added in
//! `locator`) query/inspect/input.
//!
//! WHAT: [`launch`] (flags) + the session attach helpers used by [`crate::browser`].
//!
//! HOW: Connect the engine to the browser endpoint, `Target.createTarget` +
//! `Target.attachToTarget {flatten:true}` to get a `sessionId`, then enable the
//! `Page`/`Runtime`/`DOM`/`CSS` domains on that session. All page commands carry
//! the `sessionId` (flatten mode multiplexes every session over the one socket).

pub mod launch;

use serde_json::{json, Value};

use crate::error::{BrowserError, Result};
use crate::jsonrpc::WireProtocol;

/// Create a fresh page target and attach a flattened session to it.
///
/// Returns the `sessionId` to thread through subsequent page commands.
///
/// # Errors
/// Protocol errors from `Target.createTarget`/`attachToTarget`, or a malformed
/// response.
pub(crate) fn attach_page<P: WireProtocol>(proto: &P) -> Result<String> {
    let created = proto.call(
        "Target.createTarget",
        json!({ "url": "about:blank" }),
        None,
    )?;
    let target_id = created
        .get("targetId")
        .and_then(Value::as_str)
        .ok_or_else(|| BrowserError::Protocol {
            code: 0,
            message: "Target.createTarget returned no targetId".into(),
        })?;

    let attached = proto.call(
        "Target.attachToTarget",
        json!({ "targetId": target_id, "flatten": true }),
        None,
    )?;
    let session_id = attached
        .get("sessionId")
        .and_then(Value::as_str)
        .ok_or_else(|| BrowserError::Protocol {
            code: 0,
            message: "Target.attachToTarget returned no sessionId".into(),
        })?
        .to_string();

    for domain in ["Page", "Runtime", "DOM", "CSS"] {
        proto.call(&format!("{domain}.enable"), json!({}), Some(&session_id))?;
    }
    Ok(session_id)
}
