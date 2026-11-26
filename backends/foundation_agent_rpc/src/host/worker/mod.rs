//! The Cloudflare Worker host (spec 83 feature 05, T21).
//!
//! WHY: a Worker serves unrelated clients and forwards each call to the
//! Durable Object that holds the instance.
//!
//! WHAT: the configuration, the secrets, the Durable Object client and, in
//! later modules, the entry and the instance object.
//!
//! HOW: every layer is built from the Worker bindings and the `Ctx`. Nothing
//! here opens a socket; the platform-neutral client sends each call.

pub mod config;
pub mod do_client;
pub mod entry;
pub mod object;
pub mod registry;
pub mod secrets;
pub mod serve;

/// The Connect path of every `AgentService` procedure.
///
/// The generated code names each one; a route with a `:method` parameter does
/// not match a Durable Object dispatch, so each procedure has its own route.
pub const SERVICE_PATHS: &[&str] = &[
    crate::proto::v1::agent_service::procedure::HEALTH,
    crate::proto::v1::agent_service::procedure::GET_CAPABILITIES,
    crate::proto::v1::agent_service::procedure::CREATE_INSTANCE,
    crate::proto::v1::agent_service::procedure::LIST_INSTANCES,
    crate::proto::v1::agent_service::procedure::DESTROY_INSTANCE,
    crate::proto::v1::agent_service::procedure::CHAT,
    crate::proto::v1::agent_service::procedure::CHAT_STREAM,
];
