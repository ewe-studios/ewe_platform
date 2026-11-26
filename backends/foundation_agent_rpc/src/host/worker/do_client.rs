//! The Worker's client for one instance Durable Object (spec 83 feature 05,
//! section 5.5).
//!
//! WHY: a Worker does not run a turn. It forwards the call to the Durable
//! Object (DO) that holds the instance, over the DO stub, with the same
//! Connect transport as a remote caller.
//!
//! WHAT: [`instance_client`] (a generated `AgentServiceClient` over the stub)
//! and [`forwarded_principal_header`].
//!
//! HOW: `StubHttpClient` implements `NetClient` over `worker::Stub`, so
//! `H1Transport` carries the call with no new fetch pump.

use std::sync::Arc;

use foundation_connectrpc::{ClientOptions, H1Transport};
use foundation_errstacks::ErrorTrace;
use foundation_netio::cloudflare::StubHttpClient;
use foundation_netio::shared::client::config::ClientConfig;

use crate::error::{AgentRpcError, AgentRpcResult};
use crate::host::worker::config::ServeLimits;
use crate::proto::v1::agent_service::AgentServiceClient;

/// The Durable Object binding name.
pub const INSTANCE_BINDING: &str = "AGENT_INSTANCES";

/// The base URL of the instance client. The stub ignores the host; the value
/// gives the Connect client a well-formed URL.
pub const INSTANCE_BASE_URL: &str = "https://agent-instance.internal";

/// The header that carries the checked principal to the Durable Object.
pub const FORWARDED_PRINCIPAL_HEADER: &str = "ewe-forwarded-principal";

/// The generated `AgentServiceClient` for the instance named `name`.
///
/// # Errors
/// [`AgentRpcError::Config`] when the binding or the name is bad.
pub fn instance_client(
    env: &worker::Env,
    name: &str,
    limits: &ServeLimits,
) -> AgentRpcResult<AgentServiceClient> {
    let config = ClientConfig::default()
        .with_body_queue_chunks(limits.response_queue)
        .with_max_header_block(limits.max_header_bytes);
    let stub =
        StubHttpClient::new_with_config(env, INSTANCE_BINDING, name, &config).map_err(|error| {
            ErrorTrace::new(AgentRpcError::Config(format!(
                "the instance {name:?} is not reachable: {error:?}"
            )))
        })?;
    let transport = H1Transport::new(Arc::new(stub));
    AgentServiceClient::new(Arc::new(transport), INSTANCE_BASE_URL, ClientOptions::new()).map_err(
        |error| {
            ErrorTrace::new(AgentRpcError::Config(format!(
                "the instance client cannot be built: {error}"
            )))
        },
    )
}

/// The forwarded principal header pair.
///
/// The Durable Object trusts this header only because the Worker removes every
/// incoming `ewe-forwarded-*` header before it adds its own (invariant I4).
#[must_use]
pub fn forwarded_principal_header(subject: &str) -> (String, String) {
    (FORWARDED_PRINCIPAL_HEADER.to_string(), subject.to_string())
}
