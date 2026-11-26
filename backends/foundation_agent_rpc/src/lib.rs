//! `foundation_agent_rpc` — the per-machine agent RPC model server.
//!
//! WHY: launching one binary on a host should start an RPC server that serves
//! many agent sessions against a model loaded **once** for the whole machine.
//!
//! WHAT: the crate root. The transport-agnostic core is on every build. The
//! native host and the legacy feature-01 modules need the `multi` feature: the
//! thread pool, the TLS and SQLite dependencies, and the serve threads.
//!
//! | Module | Role | Build |
//! |---|---|---|
//! | [`core`] | the transport-agnostic core: models, registry, service, enrollment | all |
//! | [`error`] | [`AgentRpcError`] — the single error type + wire status mapping | all |
//! | [`proto`] | generated wire types + service stubs (`proto/`) | all |
//! | [`auth`] | CA, mTLS config, `BootstrapService` (enrollment) | `multi` |
//! | [`bootstrap`] | the base64 / QR bootstrap payload | `multi` |
//! | [`client`] | enroll with a signed secret, then a typed client over mutual TLS | `multi` |
//! | `cloudflare` | the Cloudflare container and the agent-rpc deployment plan | `sandbox` / `deployment` |
//! | [`host`] | the native host: config, listeners, catalog, client | `multi` |
//! | [`model`] | [`ModelHandle`] — load-once, `Arc`-shared model handle | `multi` |
//! | [`secret`] | signing key + signed, expiring bootstrap secrets | `multi` |
//! | [`trust`] | trusted-client store (`foundation_db` SQL; SQLite by default) | `multi` |
//! | [`server`] | [`AgentRpcServer`] — builder, listener, graceful shutdown | `multi` |
//! | [`service`] | [`AgentRpcService`] — the legacy generated `AgentService` over the registry | `multi` |
//! | [`instance`] | the legacy concrete instance registry over shared models | `multi` |
//!
//! HOW: the RPC edge is async (`foundation_connectrpc` handlers); inference runs
//! on valtron behind `Send + 'static` boundaries; the model is shared through an
//! `Arc`-backed handle. A deterministic simulation test depends on this crate
//! with `default-features = false`, so it gets `core` only.

/// The `agent-rpc` process command line (spec 83 feature 08).
#[cfg(not(target_family = "wasm"))]
pub mod cli;
pub mod core;
pub mod error;
/// The `agent-rpc keys` commands and the native key encryption key (KEK)
/// selection (spec 83 feature 08).
#[cfg(not(target_family = "wasm"))]
pub mod keys;
pub mod proto;

#[cfg(feature = "multi")]
pub mod auth;
#[cfg(feature = "multi")]
pub mod bootstrap;
pub mod client;
#[cfg(any(feature = "sandbox", feature = "deployment"))]
pub mod cloudflare;
#[cfg(any(feature = "multi", feature = "worker"))]
pub mod host;
#[cfg(feature = "multi")]
pub mod instance;
#[cfg(feature = "multi")]
pub mod model;
#[cfg(feature = "multi")]
pub mod secret;
/// The listener needs the native HTTP server, which is Unix only. Every other
/// platform gets the client, the model and the registry.
#[cfg(all(feature = "multi", unix))]
pub mod server;
#[cfg(feature = "multi")]
pub mod service;
#[cfg(feature = "multi")]
pub mod trust;

pub use core::model::{Device, ModelKind};
pub use error::{AgentRpcError, AgentRpcResult};
#[cfg(feature = "multi")]
pub use instance::{
    InstanceConfig, InstanceHandle, InstanceMeta, InstanceRegistry, RegistryConfig, TurnGuard,
};
#[cfg(all(feature = "multi", feature = "gguf"))]
pub use model::GgufDevice;
#[cfg(feature = "multi")]
pub use model::{ModelHandle, ModelSource};
#[cfg(all(feature = "multi", unix))]
pub use server::{AgentRpcServer, AgentRpcServerBuilder, RunningServer};
#[cfg(feature = "multi")]
pub use service::{agent_router, AgentRpcService, TurnEventStream};
