//! The Durable Streams transport conformance matrix (spec 71, handoff section
//! 4.6).
//!
//! One Given/When/Then scenario list drives every transport: an embedded
//! service, Hypertext Transfer Protocol (HTTP), WebSocket, `ConnectRPC` and a
//! Durable Object (DO) worker. The upstream reference suites stay the HTTP
//! gate. This crate proves that the other transports reach the same protocol
//! outcome.
//!
//! The suite runs each scenario on each driver. It checks every driver against
//! the model oracle first, then against the other drivers. Agreement alone is
//! not proof, because five drivers can agree and all be wrong.
//!
//! A new transport adds one [`Driver`] implementation and runs
//! [`Scenario::all`] unchanged.
//!
//! # Targets
//!
//! [`driver`], [`model`] and [`scenario`] build for every target, including
//! wasm32. The native drivers sit behind the `native-drivers` feature. A wasm
//! fixture depends on this crate with `default-features = false` and supplies
//! its own driver.

pub mod driver;
pub mod model;
pub mod scenario;

#[cfg(all(unix, feature = "native-drivers"))]
pub mod embedded;
#[cfg(all(unix, feature = "native-drivers"))]
pub mod http;
#[cfg(all(unix, feature = "native-drivers"))]
pub mod rpc;
#[cfg(all(unix, feature = "native-drivers"))]
pub mod server;
#[cfg(all(unix, feature = "native-drivers"))]
pub mod ws;

pub use driver::{Driver, ErrorKind, Observation, Outcome, Producer, StepResult};
pub use model::model_scenario;
pub use scenario::{run_scenario, Scenario};

#[cfg(all(unix, feature = "native-drivers"))]
pub use embedded::EmbeddedDriver;
#[cfg(all(unix, feature = "native-drivers"))]
pub use http::HttpDriver;
#[cfg(all(unix, feature = "native-drivers"))]
pub use rpc::RpcDriver;
#[cfg(all(unix, feature = "native-drivers"))]
pub use server::Environment;
#[cfg(all(unix, feature = "native-drivers"))]
pub use ws::WsDriver;
