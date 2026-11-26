//! Native-only modules (TCP server, HTTP reader, protocol upgrades).

#[cfg(unix)]
pub mod app;
#[cfg(unix)]
pub mod handlers;
pub mod reader;
pub mod serve;
// The server parks on the native reactor, which is Unix only. Every other
// operating system builds the client half of this crate.
#[cfg(unix)]
pub mod server;
#[cfg(unix)]
pub mod upgrade;
#[cfg(unix)]
pub mod writer;

pub mod routes;
