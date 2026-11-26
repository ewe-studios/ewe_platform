//! The routing rule of the Cloudflare Worker entry.
//!
//! The Worker entry reads the request path and a small part of the request
//! body, then gives the [`RouteTarget`]. This module is library code: it holds
//! no `worker` type and no platform call, so the native tests run the same code
//! as the Worker.
//!
//! The path shape is `/ewe.sandbox.control.v1.SandboxService/<Method>`.

use serde::Deserialize;

use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::ids::SandboxId;

/// The service prefix of every `SandboxService` call.
pub const SANDBOX_SERVICE_PREFIX: &str = "/ewe.sandbox.control.v1.SandboxService/";

/// The methods that carry their sandbox identifier in the request body.
///
/// `Probe`, `List` and `Create` do not name one sandbox, so they have their own
/// targets.
pub const SCOPED_METHODS: &[&str] = &[
    "Get",
    "Stop",
    "Kill",
    "Exec",
    "ExecInput",
    "ExecSignal",
    "FileWriteBegin",
    "FileWriteChunk",
    "FileWriteCommit",
    "FileWriteAbort",
    "FileRead",
];

/// The Durable Object (DO) that one request reaches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteTarget {
    /// The Worker answers the probe itself.
    Probe,
    /// The index Durable Object answers the list.
    List,
    /// `Create` makes a new sandbox identifier, then the call reaches that new
    /// Durable Object.
    NewSandbox,
    /// The Durable Object with this name.
    Sandbox(SandboxId),
}

/// Finds the target of one request.
///
/// `path` is the request path. `body` is the JavaScript Object Notation (JSON)
/// request message. A path that is not a method of `SandboxService` gives
/// [`SandboxError::NotFound`] with `what` set to `method`. A missing or invalid
/// `sandbox_id` gives [`SandboxError::InvalidSpec`] with `field` set to
/// `sandbox_id`.
///
/// # Errors
/// [`SandboxError::NotFound`] for an unknown method, and
/// [`SandboxError::InvalidSpec`] for a body with no valid `sandbox_id`.
pub fn route(path: &str, body: &[u8]) -> SandboxResult<RouteTarget> {
    let Some(method) = path.strip_prefix(SANDBOX_SERVICE_PREFIX) else {
        return unknown_method();
    };
    match method {
        "Probe" => Ok(RouteTarget::Probe),
        "List" => Ok(RouteTarget::List),
        "Create" => Ok(RouteTarget::NewSandbox),
        known if SCOPED_METHODS.contains(&known) => {
            Ok(RouteTarget::Sandbox(sandbox_id_from_body(body)?))
        }
        _ => unknown_method(),
    }
}

/// The error of a path that is not a method of `SandboxService`.
fn unknown_method() -> crate::shared::error::SandboxResult<RouteTarget> {
    Err(SandboxError::NotFound {
        what: "method".to_string(),
    }
    .into())
}

/// The one field that the routing rule reads from a scoped request body.
#[derive(Deserialize)]
struct ScopedBody {
    sandbox_id: String,
}

/// The sandbox identifier of one scoped request body.
fn sandbox_id_from_body(body: &[u8]) -> SandboxResult<SandboxId> {
    let scoped =
        serde_json::from_slice::<ScopedBody>(body).map_err(|error| SandboxError::InvalidSpec {
            field: "sandbox_id".to_string(),
            reason: format!("the body is not JSON with a sandbox_id: {error}"),
        })?;
    scoped.sandbox_id.parse::<SandboxId>().map_err(|error| {
        SandboxError::InvalidSpec {
            field: "sandbox_id".to_string(),
            reason: error.to_string(),
        }
        .into()
    })
}
