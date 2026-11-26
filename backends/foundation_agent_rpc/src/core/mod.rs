//! The transport-agnostic core of agent-rpc.
//!
//! WHY: one set of handlers, selection rules and registry logic serves the
//! native host, the Worker host and the deterministic simulation. No module
//! here opens a socket, reads a file or reads a global clock.
//!
//! WHAT: the model kinds and devices ([`model`]), the instance registry
//! ([`registry`]), the configuration limits ([`config`]), the checked
//! bootstrap payload ([`bootstrap`]), enrollment ([`enroll`]) and the
//! generated `AgentService` handlers ([`service`]).
//!
//! HOW: every side effect comes from [`foundation_context::Ctx`] or from a
//! trait that a host implements. The handlers read the checked caller from
//! the request extension that the auth interceptor sets.

pub mod archive;
pub mod bootstrap;
pub mod config;
pub mod enroll;
pub mod model;
pub mod registry;
pub mod service;
pub mod turn;
