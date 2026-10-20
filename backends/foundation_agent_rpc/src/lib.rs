//! `foundation_agent_rpc` — the per-machine agent RPC model server.
//!
//! WHY: launching one binary on a host should start an RPC server that serves
//! many agent sessions against a model loaded **once** for the whole machine.
//!
//! WHAT: the crate root. Module map (spec 65 feature 01):
//!
//! | Module | Role |
//! |---|---|
//! | [`error`] | [`AgentRpcError`] — the single error type + wire status mapping |
//! | [`model`] | [`ModelHandle`] — load-once, `Arc`-shared model handle |
//! | [`instance`] | [`InstanceRegistry`] — named instances over the shared models |
//!
//! HOW: the RPC edge is async (`foundation_connectrpc` handlers); inference runs
//! on valtron behind `Send + 'static` boundaries; the model is shared through an
//! `Arc`-backed handle. Modules are added as their task lands (see
//! `specifications/65-agent-rpc-serving-system/features/01-agent-rpc`).

pub mod error;
pub mod instance;
pub mod model;

pub use error::{AgentRpcError, AgentRpcResult};
pub use instance::{
    InstanceConfig, InstanceHandle, InstanceMeta, InstanceRegistry, RegistryConfig, TurnGuard,
};
pub use model::{ModelHandle, ModelSource};
