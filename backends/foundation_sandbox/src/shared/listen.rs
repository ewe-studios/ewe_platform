//! A listen address of a program.
//!
//! The `--listen` argument of `agent-sandbox serve` and `agent-slave` names one
//! of these forms. The type is shared code: it holds no operating-system call,
//! so the command-line parser and both servers read it on every native target.

use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Where a program listens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListenAddress {
    /// A Unix domain socket path.
    Unix {
        /// The path of the socket file.
        path: PathBuf,
    },
    /// A Transmission Control Protocol (TCP) socket address.
    Tcp {
        /// The address, with the port already resolved.
        addr: SocketAddr,
    },
    /// An `AF_VSOCK` port of the guest.
    Vsock {
        /// The port.
        port: u32,
    },
}

impl fmt::Display for ListenAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unix { path } => write!(formatter, "unix:{}", path.display()),
            Self::Tcp { addr } => write!(formatter, "tcp:{addr}"),
            Self::Vsock { port } => write!(formatter, "vsock:{port}"),
        }
    }
}
