//! Native-only glue: UDP transport and the valtron tunnel driver.

pub mod driver;

pub use driver::{TunnelDriver, TunnelDriverTask};
