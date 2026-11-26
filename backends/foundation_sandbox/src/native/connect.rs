//! The client of a remote control plane (feature 06).
//!
//! [`SandboxRpcClient::connect`] builds the transport that an address names and
//! returns the same client that a test builds over an in-memory transport. A
//! Unix domain socket and a resolved TCP address ignore the authority; a base
//! `http://` or `https://` address uses it, and an `https://` address needs a
//! build with a TLS provider.

use std::net::ToSocketAddrs as _;
use std::path::PathBuf;

use foundation_connectrpc::H1Transport;
use foundation_context::Ctx;
use foundation_netio::HttpClientBuilder;

use crate::shared::control::SandboxRpcClient;
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::ids::check_socket_path;
use crate::shared::limits::SandboxLimits;

/// Where a remote control plane is (the `--connect` argument).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectAddr {
    /// A Unix domain socket path.
    Unix(PathBuf),
    /// A Transmission Control Protocol (TCP) host and port.
    Tcp {
        /// The host name or address.
        host: String,
        /// The port.
        port: u16,
    },
    /// A base `http://` or `https://` address (the Cloudflare Worker of
    /// feature 08).
    Url(String),
}

impl SandboxRpcClient {
    /// Builds the transport for an address. Native only.
    ///
    /// # Errors
    /// [`SandboxError::SocketPathTooLong`] for a long Unix path,
    /// [`SandboxError::Io`] when the host does not resolve, and
    /// [`SandboxError::InvalidSpec`] for an address with no scheme.
    pub fn connect(ctx: Ctx, addr: ConnectAddr, limits: SandboxLimits) -> SandboxResult<Self> {
        let timeout = limits.rpc_timeout;
        let (client, authority): (foundation_netio::DynNetClient, String) = match addr {
            ConnectAddr::Unix(path) => {
                check_socket_path(&path)?;
                #[cfg(unix)]
                {
                    let client = HttpClientBuilder::new()
                        .unix_socket(path)
                        .unix_read_timeout(timeout)
                        .read_timeout(timeout)
                        .expect_continue(false)
                        .build();
                    (client, "http://localhost".to_string())
                }
                #[cfg(not(unix))]
                {
                    let _ = path;
                    return Err(SandboxError::InvalidSpec {
                        field: "connect".to_string(),
                        reason: "a Unix domain socket needs a Unix host".to_string(),
                    }
                    .into());
                }
            }
            ConnectAddr::Tcp { host, port } => {
                let address = resolve(&host, port)?;
                let client = HttpClientBuilder::new()
                    .resolve_to(address)
                    .read_timeout(timeout)
                    .expect_continue(false)
                    .build();
                (client, format!("http://{host}:{port}"))
            }
            ConnectAddr::Url(url) => {
                let client = HttpClientBuilder::new()
                    .read_timeout(timeout)
                    .expect_continue(false)
                    .build();
                (client, url)
            }
        };
        let transport = std::sync::Arc::new(H1Transport::new(client));
        Ok(Self::with_authority(ctx, transport, limits, &authority))
    }
}

/// Resolves `host:port` to one socket address.
fn resolve(host: &str, port: u16) -> SandboxResult<std::net::SocketAddr> {
    let mut addresses = (host, port)
        .to_socket_addrs()
        .map_err(|error| SandboxError::Io {
            op: "resolve control-plane address".to_string(),
            message: error.to_string(),
        })?;
    addresses.next().ok_or_else(|| {
        SandboxError::Io {
            op: "resolve control-plane address".to_string(),
            message: format!("{host}:{port} resolved to no address"),
        }
        .into()
    })
}
