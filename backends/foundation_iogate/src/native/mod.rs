//! Native I/O-gate wiring: the completion socket and the accept path.
//!
//! WHY: this is the crate's payload — the code that both `foundation_netio`
//! (the `Connection`/`CompletionReadWrite` seam) and `foundation_nativeapis`
//! (the reactor and registered fd) meet in. It is native-only because the
//! reactor is native-only.
//!
//! WHAT: [`CompletionSocket`] (a completion-backed byte source), its
//! `CompletionReadWrite` impl so a boxed `Connection::Completion` can delegate
//! through it, and [`accept_connection`] — the one-line accept-path helper that
//! turns an accepted `TcpStream` into the netio `Connection` a given
//! [`crate::ServerIo`] mode asks for.
//!
//! HOW: `CompletionSocket` wraps a `RegisteredFd`. On Linux with io_uring it
//! registers for multishot `RECV` and reads by popping the inbox; elsewhere it
//! registers plainly and reads with `read(2)`.

mod accept;
mod completion_socket;

pub use accept::{accept_connection, init_reactor_for};
pub use completion_socket::CompletionSocket;
