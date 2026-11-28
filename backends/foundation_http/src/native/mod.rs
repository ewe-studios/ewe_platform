//! Native-only modules (TCP server, HTTP reader, protocol upgrades).

#[cfg(any(unix, windows))]
pub mod app;
#[cfg(any(unix, windows))]
pub mod durable;
#[cfg(any(unix, windows))]
pub mod durable_ws;
#[cfg(any(unix, windows))]
pub mod handlers;
pub mod reader;
pub mod serve;
#[cfg(any(unix, windows))]
pub mod server;
#[cfg(any(unix, windows))]
pub mod upgrade;
#[cfg(any(unix, windows))]
pub mod writer;

pub mod routes;
