/// Networking types built on top of the poll layer.
///
/// Provides async-compatible (via poll layer) networking types:
/// - `TcpStream` — TCP connection
/// - `TcpListener` — TCP server socket
/// - `UdpSocket` — UDP socket
/// - Unix domain sockets (Linux/macOS only)

pub mod tcp;
pub mod udp;
#[cfg(unix)]
pub mod unix;

pub use tcp::{TcpListener, TcpStream};
pub use udp::UdpSocket;
#[cfg(unix)]
pub use unix::{UnixDatagram, UnixListener, UnixStream};
