//! `foundation_agent_rpc` — the per-machine agent RPC model server.
//!
//! WHY: launching one binary on a host should start an RPC server that serves
//! many agent sessions against a model loaded **once** for the whole machine.
//!
//! WHAT: the crate root. Module map (spec 65 feature 01):
//!
//! | Module | Role |
//! |---|---|
//! | [`auth`] | bootstrap token, CA, mTLS config, `BootstrapService` (enrollment) |
//! | [`bootstrap`] | the base64 / QR bootstrap payload |
//! | [`client`] | enroll with a token, then a typed client over mutual TLS |
//! | [`error`] | [`AgentRpcError`] — the single error type + wire status mapping |
//! | [`model`] | [`ModelHandle`] — load-once, `Arc`-shared model handle |
//! | [`proto`] | generated wire types + service stubs (`proto/`) |
//! | [`server`] | [`AgentRpcServer`] — builder, listener, graceful shutdown |
//! | [`service`] | [`AgentRpcService`] — the generated `AgentService` over the registry |
//! | [`instance`] | [`InstanceRegistry`] — named instances over the shared models |
//!
//! HOW: the RPC edge is async (`foundation_connectrpc` handlers); inference runs
//! on valtron behind `Send + 'static` boundaries; the model is shared through an
//! `Arc`-backed handle. Modules are added as their task lands (see
//! `specifications/65-agent-rpc-serving-system/features/01-agent-rpc`).

pub mod auth;
pub mod bootstrap;
pub mod client;
pub mod error;
pub mod instance;
pub mod model;
pub mod proto;
pub mod server;
pub mod service;

pub use error::{AgentRpcError, AgentRpcResult};
pub use instance::{
    InstanceConfig, InstanceHandle, InstanceMeta, InstanceRegistry, RegistryConfig, TurnGuard,
};
pub use model::{ModelHandle, ModelSource};
pub use server::{AgentRpcServer, AgentRpcServerBuilder, RunningServer};
pub use service::{agent_router, AgentRpcService, TurnEventStream};
