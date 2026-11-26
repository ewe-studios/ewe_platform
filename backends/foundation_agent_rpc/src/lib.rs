//! `foundation_agent_rpc` — the per-machine agent RPC model server.
//!
//! WHY: launching one binary on a host should start an RPC server that serves
//! many agent sessions against a model loaded **once** for the whole machine.
//!
//! WHAT: the crate root. Module map (spec 83 features 01 and 03):
//!
//! | Module | Role |
//! |---|---|
//! | [`auth`] | CA, mTLS config, `BootstrapService` (enrollment), client-cert verification |
//! | [`bootstrap`] | the base64 / QR bootstrap payload |
//! | [`client`] | enroll with a signed secret, then a typed client over mutual TLS |
//! | [`core`] | the transport-agnostic core: models, registry, service, enrollment |
//! | [`error`] | [`AgentRpcError`] — the single error type + wire status mapping |
//! | [`model`] | [`ModelHandle`] — load-once, `Arc`-shared model handle |
//! | [`proto`] | generated wire types + service stubs (`proto/`) |
//! | [`secret`] | signing key + signed, expiring bootstrap secrets |
//! | [`trust`] | trusted-client store (`foundation_db` SQL; SQLite by default) |
//! | [`server`] | [`AgentRpcServer`] — builder, listener, graceful shutdown |
//! | [`service`] | [`AgentRpcService`] — the legacy generated `AgentService` over the registry |
//! | [`instance`] | the legacy concrete instance registry over shared models |
//!
//! HOW: the RPC edge is async (`foundation_connectrpc` handlers); inference runs
//! on valtron behind `Send + 'static` boundaries; the model is shared through an
//! `Arc`-backed handle. Modules are added as their task lands (see
//! `specifications/83-agent-rpc-serving-system/features/01-agent-rpc`).

pub mod auth;
pub mod bootstrap;
pub mod client;
pub mod core;
pub mod error;
pub mod instance;
/// The `agent-rpc keys` commands and the native key encryption key (KEK)
/// selection (spec 83 feature 08).
#[cfg(not(target_family = "wasm"))]
pub mod keys;
pub mod model;
pub mod proto;
pub mod secret;
/// The listener needs the native HTTP server, which is Unix only. Every other
/// platform gets the client, the model and the registry.
#[cfg(unix)]
pub mod server;
pub mod service;
pub mod trust;

pub use core::model::{Device, ModelKind};
pub use error::{AgentRpcError, AgentRpcResult};
pub use instance::{
    InstanceConfig, InstanceHandle, InstanceMeta, InstanceRegistry, RegistryConfig, TurnGuard,
};
#[cfg(feature = "gguf")]
pub use model::GgufDevice;
pub use model::{ModelHandle, ModelSource};
#[cfg(unix)]
pub use server::{AgentRpcServer, AgentRpcServerBuilder, RunningServer};
pub use service::{agent_router, AgentRpcService, TurnEventStream};
