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

/// The path pattern of every `AgentService` procedure.
///
/// The Connect path is `/ewe.agent.v1.AgentService/<Method>`; the router
/// matches the last segment as `:method`.
pub const SERVICE_PATH: &str = "/ewe.agent.v1.AgentService/:method";
