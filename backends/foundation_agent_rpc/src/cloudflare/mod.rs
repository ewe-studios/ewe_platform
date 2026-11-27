//! The Cloudflare host of agent-rpc (spec 83 feature 06).
//!
//! WHY: agent-rpc has two Cloudflare shapes. The thin orchestrator runs on a
//! Worker. The full native model server runs in a Cloudflare container that a
//! Durable Object controls through the spec 77 handle.
//!
//! WHAT: [`container`] is the native agent-rpc container over the spec 77
//! `ContainerControl` and `ContainerHandle`. [`plan`] is the `DeploymentPlan`
//! of the agent-rpc Worker. The plan needs the native spec 77 deployers, so it
//! is native only.
//!
//! HOW: this module owns no container API and no deployer. It calls the spec
//! 77 types. The Worker entry itself is in `host::worker::orchestrator`.

#[cfg(feature = "sandbox")]
pub mod container;

#[cfg(all(not(target_family = "wasm"), feature = "deployment"))]
pub mod plan;
