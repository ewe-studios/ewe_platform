//! Native-only glue: UDP transport, the valtron tunnel driver, and the TLS-PSK bootstrap.

pub mod bootstrap;
pub mod dataplane;
pub mod driver;
pub mod mtls;
pub mod node;
pub mod overlay_acceptor;
pub mod overlay_connection;
pub mod overlay_connector;
pub mod relay;
pub mod relay_ws;
pub mod transport;
pub mod webrtc;

pub use bootstrap::{
    BootstrapClient, BootstrapConnection, BootstrapHandler, BootstrapServer,
};
pub use driver::{TunnelDriver, TunnelDriverTask};
pub use node::{MeshTask, PeerInfo, WgConfig, WgHandle, WgNode};
