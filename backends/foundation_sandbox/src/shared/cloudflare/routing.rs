//! The routing rule of the Cloudflare Worker entry.
//!
//! The Worker entry reads the request path and a small part of the request
//! body, then gives the [`RouteTarget`]. This module is library code: it holds
//! no `worker` type and no platform call, so the native tests run the same code
//! as the Worker.
//!
//! The path shapes are `/ewe.sandbox.control.v1.SandboxService/<Method>` and
//! `/ewe.sandbox.container.v1.ContainerService/<Method>`.

use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::ids::SandboxId;

/// The service prefix of every `SandboxService` call.
pub const SANDBOX_SERVICE_PREFIX: &str = "/ewe.sandbox.control.v1.SandboxService/";

/// The service prefix of every `ContainerService` call.
pub const CONTAINER_SERVICE_PREFIX: &str = "/ewe.sandbox.container.v1.ContainerService/";

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

/// The `ContainerService` methods that carry their container identifier in the
/// request body.
///
/// On Cloudflare the container identifier is the Durable Object name, so it is
/// also the sandbox identifier. `Start` and `List` name no container: a Start
/// belongs to one Durable Object, and a List is the list of that object.
pub const CONTAINER_SCOPED_METHODS: &[&str] = &[
    "Get",
    "Fetch",
    "Exec",
    "ExecInput",
    "ExecSignal",
    "Signal",
    "Wait",
    "Stop",
    "Destroy",
    "LogTail",
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
    /// The `ContainerService` call of the Durable Object with this name. It is
    /// the same Durable Object as [`RouteTarget::Sandbox`].
    Container(SandboxId),
}

/// Finds the target of one request.
///
/// `path` is the request path. `body` is the JavaScript Object Notation (JSON)
/// request message. A path that is not a method of the two services gives
/// [`SandboxError::NotFound`] with `what` set to `method`. A missing or invalid
/// `sandbox_id` or `container_id` gives [`SandboxError::InvalidSpec`] with
/// `field` set to that name.
///
/// # Errors
/// [`SandboxError::NotFound`] for an unknown method, and
/// [`SandboxError::InvalidSpec`] for a body with no valid identifier.
pub fn route(path: &str, body: &[u8]) -> SandboxResult<RouteTarget> {
    if let Some(method) = path.strip_prefix(SANDBOX_SERVICE_PREFIX) {
        return match method {
            "Probe" => Ok(RouteTarget::Probe),
            "List" => Ok(RouteTarget::List),
            "Create" => Ok(RouteTarget::NewSandbox),
            known if SCOPED_METHODS.contains(&known) => {
                Ok(RouteTarget::Sandbox(scoped_id(body, "sandbox_id")?))
            }
            _ => unknown_method(),
        };
    }
    if let Some(method) = path.strip_prefix(CONTAINER_SERVICE_PREFIX) {
        if CONTAINER_SCOPED_METHODS.contains(&method) {
            return Ok(RouteTarget::Container(scoped_id(body, "container_id")?));
        }
        if method == "Start" || method == "List" {
            return Err(SandboxError::InvalidSpec {
                field: "container_id".to_string(),
                reason: "the request names no container".to_string(),
            }
            .into());
        }
    }
    unknown_method()
}

/// The error of a path that is not a method of the two services.
fn unknown_method() -> crate::shared::error::SandboxResult<RouteTarget> {
    Err(SandboxError::NotFound {
        what: "method".to_string(),
    }
    .into())
}

/// The identifier of one scoped request body.
///
/// The entry reads only this field; every other field of the message is
/// ignored.
fn scoped_id(body: &[u8], field: &str) -> SandboxResult<SandboxId> {
    let value = serde_json::from_slice::<serde_json::Value>(message_of(body)).map_err(|error| {
        SandboxError::InvalidSpec {
            field: field.to_string(),
            reason: format!("the body is not JSON: {error}"),
        }
    })?;
    let text = value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| SandboxError::InvalidSpec {
            field: field.to_string(),
            reason: format!("the body has no {field}"),
        })?;
    text.parse::<SandboxId>().map_err(|error| {
        SandboxError::InvalidSpec {
            field: field.to_string(),
            reason: error.to_string(),
        }
        .into()
    })
}

/// The message of one request body.
///
/// A unary call sends the bare JSON message. A server-streaming scoped call
/// sends a Connect envelope: a one-byte flag and a four-byte big-endian length,
/// then the message. The route reads only the first message, so it skips a
/// whole uncompressed envelope (issue #1140).
fn message_of(body: &[u8]) -> &[u8] {
    const ENVELOPE: usize = 5;
    if body.len() >= ENVELOPE && body[0] == 0 {
        let length = u32::from_be_bytes([body[1], body[2], body[3], body[4]]);
        if usize::try_from(length).ok() == Some(body.len() - ENVELOPE) {
            return &body[ENVELOPE..];
        }
    }
    body
}
