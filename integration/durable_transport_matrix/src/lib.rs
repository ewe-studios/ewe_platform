//! The Durable Streams transport conformance matrix (spec 71, handoff section
//! 4.6).
//!
//! One Given/When/Then scenario list drives every transport: Hypertext Transfer
//! Protocol (HTTP), WebSocket and `ConnectRPC`. The upstream reference suites
//! stay the HTTP gate. This crate proves that the other transports reach the
//! same protocol outcome.
//!
//! The suite runs each scenario on each driver. It checks every driver against
//! the model oracle first, then against the other drivers. Agreement alone is
//! not proof, because three drivers can agree and all be wrong.
//!
//! A new transport adds one [`Driver`] implementation and runs
//! [`Scenario::all`] unchanged.
//!
//! The native HTTP server and the platform WebSocket client are Unix only, so
//! the crate is Unix only.

#![cfg(unix)]

pub mod driver;
pub mod http;
pub mod model;
pub mod rpc;
pub mod scenario;
pub mod server;
pub mod ws;

pub use driver::{Driver, ErrorKind, Observation, Outcome, Producer, StepResult};
pub use http::HttpDriver;
pub use model::model_scenario;
pub use rpc::RpcDriver;
pub use scenario::{run_scenario, Scenario};
pub use server::Environment;
pub use ws::WsDriver;
