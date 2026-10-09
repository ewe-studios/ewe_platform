//! Connection connector abstraction (spec-55, F11).
//!
//! WHY: `HttpConnectionPool` hardcodes DNS resolution + `TcpStream::connect()`.
//! This trait lets different transports plug in — kernel TCP, WireGuard overlay,
//! Unix sockets — without changing the HTTP stack.
//!
//! WHAT: Re-exported from [`crate::native::connection::Connector`]. The trait is
//! defined there so it is reachable without the `multi` feature (it only needs
//! `Connection` and `io::Result`). This module stays as a thin re-export so
//! callers that already import from `http::connector` continue to compile.
//!
//! HOW: The real trait lives in `native::connection` alongside `Acceptor`.

pub use crate::native::connection::Connector;
